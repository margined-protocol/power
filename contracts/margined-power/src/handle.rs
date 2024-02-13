use crate::{
    contract::{CLOSE_SHORT_REPLY_ID, FLASH_LIQUIDATE_SWAP_REPLY_ID},
    funding::apply_funding_rate,
    helpers::{
        create_apply_funding_event, get_collateral_from_vault_type, get_liquidation_results,
        get_min_amount_out_from_slippage, PoolParams,
    },
    operations::{burn, liquidate, mint},
    queries::{
        estimate_single_pool_swap_exact_amount_out, get_balance, get_denom_authority,
        get_total_supply,
    },
    state::{Config, State, TmpCacheValues, CONFIG, DEFAULT_LIMIT, OWNER, STATE, TMP_CACHE},
    vault::{
        add_collateral, check_can_burn, check_vault, get_vault_type, is_vault_safe, remove_vault,
        subtract_collateral, VAULTS,
    },
};

use cosmwasm_std::{
    coin, ensure, ensure_eq, BankMsg, CosmosMsg, Decimal, DepsMut, Env, Event, MessageInfo, Order,
    ReplyOn, Response, StdResult, SubMsg, Uint128,
};
use cw_storage_plus::Bound;
use cw_utils::{must_pay, nonpayable};
use margined_common::{
    common::{may_pay_two_denoms, WEEK_IN_SECONDS},
    errors::ContractError,
    messages::{
        create_send_message, create_swap_exact_amount_in_message,
        create_swap_exact_amount_out_message,
    },
};
use std::str::FromStr;

pub fn handle_open_contract(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    let state: State = STATE.load(deps.storage)?;
    ensure!(!state.is_open, ContractError::IsOpen {});

    let config: Config = CONFIG.load(deps.storage)?;

    // get power token denom authority
    let admin = get_denom_authority(deps.as_ref(), config.power_asset.denom).unwrap();
    ensure_eq!(admin, env.contract.address, ContractError::NotTokenAdmin {});

    // set the contract to open, reset the last funding and pause time
    STATE.update(deps.storage, |mut state| -> StdResult<_> {
        state.is_open = true;
        state.is_paused = false;
        state.last_funding_update = env.block.time;
        state.last_pause = env.block.time;
        Ok(state)
    })?;

    Ok(Response::new().add_event(Event::new("open_contract")))
}

pub fn handle_update_config(
    deps: DepsMut,
    info: MessageInfo,
    fee_rate: Option<String>,
    fee_pool: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    let mut event = Event::new("update_config");
    if let Some(fee_rate) = fee_rate {
        config.fee_rate = Decimal::from_str(&fee_rate)?;
        event = event.add_attribute("fee_rate", fee_rate);
    }

    if let Some(fee_pool) = fee_pool {
        config.fee_pool_contract = deps.api.addr_validate(&fee_pool)?;
        event = event.add_attribute("fee_pool", fee_pool);
    }

    config.validate()?;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default().add_event(event))
}

pub fn handle_pause(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    let mut state = STATE.load(deps.storage)?;

    state.is_open_and_unpaused()?;

    state.is_paused = true;
    state.last_pause = env.block.time;

    STATE.save(deps.storage, &state)?;

    let event = Event::new("pause");

    Ok(Response::default().add_event(
        event
            .add_attribute("is_paused", state.is_paused.to_string())
            .add_attribute("last_pause", state.last_pause.to_string()),
    ))
}

pub fn handle_unpause(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    if !state.is_open {
        return Err(ContractError::NotOpen {});
    }

    let unpause_time = if !OWNER.is_admin(deps.as_ref(), &info.sender)? {
        state.last_pause.seconds() + WEEK_IN_SECONDS
    } else {
        state.last_pause.seconds()
    };

    if env.block.time.seconds() < unpause_time {
        return Err(ContractError::NotExpired {});
    }

    state.is_paused = false;

    STATE.save(deps.storage, &state)?;

    let event = Event::new("unpause");

    Ok(Response::default().add_event(
        event
            .add_attribute("is_paused", state.is_paused.to_string())
            .add_attribute("last_pause", state.last_pause.to_string()),
    ))
}

pub fn handle_mint_power_perp(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mint_amount: Uint128,
    vault_id: Option<u64>,
    rebase: bool,
) -> Result<Response, ContractError> {
    mint(deps, env, info, mint_amount, vault_id, rebase, false)
}

pub fn handle_burn_power_perp(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount_to_withdraw: Option<Uint128>,
    vault_id: u64,
) -> Result<Response, ContractError> {
    burn(deps, env, info, amount_to_withdraw, vault_id)
}

pub fn handle_open_short(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mint_amount: Uint128,
    vault_id: Option<u64>,
    slippage: Option<Decimal>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let total_supply = get_total_supply(deps.as_ref(), config.power_asset.denom)?;

    TMP_CACHE.save(
        deps.storage,
        &TmpCacheValues {
            total_supply: Some(total_supply),
            sender: Some(info.sender.clone()),
            vault_id,
            slippage,
            ..Default::default()
        },
    )?;

    mint(deps, env, info, mint_amount, vault_id, false, true)
}

pub fn handle_close_short(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    exposure_to_burn: Uint128,
    amount_to_withdraw: Option<Uint128>,
    vault_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let power_balance = get_balance(
        deps.as_ref(),
        env.contract.address.to_string(),
        config.power_asset.denom.clone(),
    )?;

    let (base_to_swap, amount_extra_power) =
        may_pay_two_denoms(&info, &config.base_asset.denom, &config.power_asset.denom)
            .map_err(|_| ContractError::InvalidFunds {})?;

    if exposure_to_burn.is_zero() {
        return burn(deps, env, info, amount_to_withdraw, vault_id);
    }

    check_can_burn(
        deps.as_ref().storage,
        vault_id,
        info.sender.clone(),
        exposure_to_burn + amount_extra_power,
        amount_to_withdraw.unwrap_or(Uint128::zero()),
    )?;

    let swap_msg = create_swap_exact_amount_out_message(
        env.contract.address.to_string(),
        config.power_pool.id,
        config.base_asset.denom,
        config.power_asset.denom,
        exposure_to_burn.to_string(),
        base_to_swap.to_string(),
    );

    let swap_submsg: SubMsg = SubMsg {
        id: CLOSE_SHORT_REPLY_ID,
        msg: swap_msg.into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    TMP_CACHE.save(
        deps.storage,
        &TmpCacheValues {
            balance: Some(power_balance - amount_extra_power),
            amount_to_swap: Some(base_to_swap),
            amount_to_withdraw,
            sender: Some(info.sender),
            vault_id: Some(vault_id),
            ..Default::default()
        },
    )?;

    Ok(Response::new().add_submessage(swap_submsg))
}

pub fn handle_liquidation(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    max_debt_amount: Uint128,
    vault_id: u64,
) -> Result<Response, ContractError> {
    liquidate(
        deps,
        env,
        info,
        max_debt_amount,
        vault_id,
        "liquidation".to_string(),
        None,
    )
}

pub fn handle_flash_liquidation(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    vault_id: u64,
    slippage: Option<Decimal>,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config: Config = CONFIG.load(deps.storage)?;
    if !VAULTS.has(deps.storage, vault_id) {
        return Err(ContractError::VaultDoesNotExist(vault_id));
    };

    // flash liquidator must send collateral to be swapped for power perp
    let collateral_sent =
        must_pay(&info, &config.base_asset.denom).map_err(|_| ContractError::InvalidFunds {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    let is_safe = is_vault_safe(
        deps.as_ref(),
        config.clone(),
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(!is_safe, ContractError::SafeVault {});

    let vault = VAULTS.load(deps.storage, vault_id)?;

    let (max_amount_to_liquidate, _, _) =
        get_liquidation_results(deps.as_ref(), env.clone(), vault.short_amount, vault);

    let estimate_base_from_power = estimate_single_pool_swap_exact_amount_out(
        deps.as_ref(),
        config.power_pool.id,
        max_amount_to_liquidate,
        config.power_asset.denom.clone(),
        config.base_asset.denom.clone(),
    )?;

    let mut response = Response::default();

    let refund_amount = collateral_sent
        .checked_sub(estimate_base_from_power)
        .unwrap_or_default();

    if !refund_amount.is_zero() {
        let send_msg = create_send_message(
            env.contract.address.to_string(),
            info.sender.to_string(),
            refund_amount.to_string(),
            config.base_asset.denom.clone(),
        );

        response = response.add_message(send_msg);
    }

    let swap_amount = if collateral_sent > estimate_base_from_power {
        estimate_base_from_power
    } else {
        collateral_sent
    };

    let token_out_min_amount = get_min_amount_out_from_slippage(
        deps.as_ref(),
        swap_amount,
        slippage,
        PoolParams {
            id: config.power_pool.id,
            base_denom: config.base_asset.denom.clone(),
            quote_denom: config.power_asset.denom.clone(),
            base_decimal_places: config.base_asset.decimals,
            quote_decimal_places: config.power_asset.decimals,
        },
    )?;

    let swap_msg = create_swap_exact_amount_in_message(
        env.contract.address.to_string(),
        config.power_pool.id,
        config.base_asset.denom.clone(),
        config.power_asset.denom,
        swap_amount.to_string(),
        token_out_min_amount.to_string(),
    );

    let swap_submsg: SubMsg = SubMsg {
        id: FLASH_LIQUIDATE_SWAP_REPLY_ID,
        msg: swap_msg.into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    TMP_CACHE.save(
        deps.storage,
        &TmpCacheValues {
            sender: Some(info.sender),
            vault_id: Some(vault_id),
            amount_to_swap: Some(swap_amount),
            ..Default::default()
        },
    )?;

    Ok(response.add_submessage(swap_submsg))
}

pub fn handle_deposit(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    vault_id: u64,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config = CONFIG.load(deps.storage)?;

    let expected_vault_type = get_vault_type(deps.storage, vault_id)
        .map_err(|_| ContractError::VaultDoesNotExist(vault_id))?;

    let deposit_asset = get_collateral_from_vault_type(&config, expected_vault_type)?;
    let deposit_amount =
        must_pay(&info, &deposit_asset.denom).map_err(|_| ContractError::InvalidFunds {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    add_collateral(deps.storage, vault_id, info.sender.clone(), deposit_amount)?;

    let (is_safe, min_collateral, _) = check_vault(
        deps.as_ref(),
        config,
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(is_safe, ContractError::UnsafeVault {});
    ensure!(min_collateral, ContractError::BelowMinCollateralAmount {});

    let deposit_event = Event::new("deposit").add_attributes([
        ("user", &info.sender.to_string()),
        ("collateral_deposited", &deposit_amount.to_string()),
        ("vault_id", &vault_id.to_string()),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new().add_events([deposit_event, funding_event]))
}

pub fn handle_withdrawal(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    vault_id: u64,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config = CONFIG.load(deps.storage)?;

    let expected_vault_type = get_vault_type(deps.storage, vault_id)
        .map_err(|_| ContractError::VaultDoesNotExist(vault_id))?;

    let withdrawal_asset = get_collateral_from_vault_type(&config, expected_vault_type)?;

    nonpayable(&info).map_err(|_| ContractError::NonPayable {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    subtract_collateral(deps.storage, vault_id, info.sender.clone(), amount)?;

    let (is_safe, min_collateral, _) = check_vault(
        deps.as_ref(),
        config,
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(is_safe, ContractError::UnsafeVault {});
    ensure!(min_collateral, ContractError::BelowMinCollateralAmount {});

    // transfer base to sender
    let msg_transfer: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![coin(amount.u128(), withdrawal_asset.denom)],
    });

    let withdrawal_event = Event::new("withdraw").add_attributes([
        ("user", &info.sender.to_string()),
        ("collateral_withdrawn", &amount.to_string()),
        ("vault_id", &vault_id.to_string()),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new()
        .add_messages(vec![msg_transfer])
        .add_events([withdrawal_event, funding_event]))
}

pub fn handle_remove_empty_vault(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    start_after: Option<u64>,
    limit: Option<u64>,
) -> Result<Response, ContractError> {
    nonpayable(&info).map_err(|_| ContractError::NonPayable {})?;

    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env)?;

    let start = start_after.map(Bound::exclusive);
    let limit = limit.unwrap_or(DEFAULT_LIMIT) as usize;

    let mut vaults_to_remove = Vec::new();

    let vaults = VAULTS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .collect::<Result<Vec<_>, _>>()?;

    for (id, vault) in vaults.iter() {
        if vault.collateral.is_zero() && vault.short_amount.is_zero() {
            remove_vault(deps.storage, *id)?;

            vaults_to_remove.push(*id);
        }
    }

    if vaults_to_remove.is_empty() {
        return Err(ContractError::NoEmptyVaults {});
    }

    let vaults_to_remove: String = vaults_to_remove
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>()
        .join(", ");

    let remove_event =
        Event::new("remove_empty_vaults").add_attributes([("vaults_to_remove", &vaults_to_remove)]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new().add_events([remove_event, funding_event]))
}

pub fn handle_apply_funding(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let funding = apply_funding_rate(deps, env)?;

    let funding_event = create_apply_funding_event(&funding.to_string());

    Ok(Response::new().add_event(funding_event))
}

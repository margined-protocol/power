use crate::{
    contract::CLOSE_SHORT_REPLY_ID,
    funding::apply_funding_rate,
    helpers::{
        create_apply_funding_event, create_swap_exact_amount_out_message, get_liquidation_results,
    },
    operations::{burn, mint},
    queries::{get_balance, get_denom_authority, get_total_supply},
    state::{Config, State, TmpCacheValues, CONFIG, OWNER, STATE, TMP_CACHE, WEEK_IN_SECONDS},
    vault::{add_collateral, burn_vault, check_can_burn, check_vault, subtract_collateral, VAULTS},
};

use cosmwasm_std::{
    coin, ensure, ensure_eq, BankMsg, CosmosMsg, Decimal, DepsMut, Env, Event, MessageInfo,
    ReplyOn, Response, StdResult, SubMsg, Uint128,
};
use cw_utils::{must_pay, nonpayable};
use margined_common::errors::ContractError;
use osmosis_std::types::{cosmos::base::v1beta1::Coin, osmosis::tokenfactory::v1beta1::MsgBurn};
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
    let admin = get_denom_authority(deps.as_ref(), config.power_denom).unwrap();
    ensure_eq!(admin, env.contract.address, ContractError::NotTokenAdmin {});

    // set the contract to open
    STATE.update(deps.storage, |mut state| -> StdResult<_> {
        state.is_open = true;
        state.is_paused = false;
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
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let total_supply = get_total_supply(deps.as_ref(), config.power_denom)?;

    TMP_CACHE.save(
        deps.storage,
        &TmpCacheValues {
            total_supply: Some(total_supply),
            sender: Some(info.sender.clone()),
            vault_id,
            ..Default::default()
        },
    )?;

    mint(deps, env, info, mint_amount, vault_id, false, true)
}

pub fn handle_close_short(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount_to_burn: Uint128,
    amount_to_withdraw: Option<Uint128>,
    vault_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    check_can_burn(
        deps.as_ref().storage,
        vault_id,
        info.sender.clone(),
        amount_to_burn,
        amount_to_withdraw.unwrap_or(Uint128::zero()),
    )?;

    let power_balance = get_balance(
        deps.as_ref(),
        env.contract.address.to_string(),
        config.power_denom.clone(),
    )?;

    let amount_to_swap =
        must_pay(&info, &config.base_denom).map_err(|_| ContractError::InvalidFunds {})?;

    let swap_msg = create_swap_exact_amount_out_message(
        env.contract.address.to_string(),
        config.power_pool.id,
        config.base_denom,
        config.power_denom,
        amount_to_burn.to_string(),
        amount_to_swap.to_string(),
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
            balance: Some(power_balance),
            amount_to_swap: Some(amount_to_swap),
            amount_to_withdraw,
            sender: Some(info.sender),
            vault_id: Some(vault_id),
            ..Default::default()
        },
    )?;

    Ok(Response::new().add_submessage(swap_submsg))
}

pub fn handle_liquidation(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    max_debt_amount: Uint128,
    vault_id: u64,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config: Config = CONFIG.load(deps.storage)?;
    if !VAULTS.has(deps.storage, &vault_id) {
        return Err(ContractError::VaultDoesNotExist {});
    };

    // liquidator does not require to send funds, rather it is burnt directly
    nonpayable(&info).map_err(|_| ContractError::NonPayable {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    let (is_safe, _) = check_vault(
        deps.as_ref(),
        config.clone(),
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(!is_safe, ContractError::SafeVault {});

    let vault = VAULTS.load(deps.storage, &vault_id)?;

    let (liquidation_amount, collateral_to_pay) =
        get_liquidation_results(deps.as_ref(), env.clone(), max_debt_amount, vault.clone());

    if max_debt_amount < liquidation_amount {
        return Err(ContractError::InvalidLiquidation {});
    }

    burn_vault(
        deps.storage,
        vault_id,
        vault.operator,
        collateral_to_pay,
        liquidation_amount,
    )?;

    // burn power perp token
    let msg_burn: CosmosMsg = MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom: config.power_denom,
            amount: liquidation_amount.to_string(),
        }),
        burn_from_address: info.sender.to_string(),
    }
    .into();

    // transfer collateral to sender
    let msg_transfer: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![coin(collateral_to_pay.u128(), config.base_denom)],
    });

    let liquidation_event = Event::new("liquidation").add_attributes([
        ("liquidation_amount", &liquidation_amount.to_string()),
        ("collateral_to_pay", &collateral_to_pay.to_string()),
        ("vault_id", &vault_id.to_string()),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new()
        .add_messages(vec![msg_burn, msg_transfer])
        .add_events([liquidation_event, funding_event]))
}

pub fn handle_deposit(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    vault_id: u64,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config: Config = CONFIG.load(deps.storage)?;

    if !VAULTS.has(deps.storage, &vault_id) {
        return Err(ContractError::VaultDoesNotExist {});
    };

    let deposit_amount =
        must_pay(&info, &config.base_denom).map_err(|_| ContractError::InvalidFunds {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    add_collateral(deps.storage, vault_id, info.sender, deposit_amount)?;

    let (is_safe, min_collateral) = check_vault(
        deps.as_ref(),
        config,
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(is_safe, ContractError::UnsafeVault {});
    ensure!(min_collateral, ContractError::BelowMinCollateralAmount {});

    let deposit_event = Event::new("deposit").add_attributes([
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

    let config: Config = CONFIG.load(deps.storage)?;

    if !VAULTS.has(deps.storage, &vault_id) {
        return Err(ContractError::VaultDoesNotExist {});
    };

    nonpayable(&info).map_err(|_| ContractError::NonPayable {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    subtract_collateral(deps.storage, vault_id, info.sender.clone(), amount)?;

    let (is_safe, min_collateral) = check_vault(
        deps.as_ref(),
        config.clone(),
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(is_safe, ContractError::UnsafeVault {});
    ensure!(min_collateral, ContractError::BelowMinCollateralAmount {});

    // transfer base to sender
    let msg_transfer: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![coin(amount.u128(), config.base_denom)],
    });

    let withdrawal_event = Event::new("withdraw").add_attributes([
        ("collateral_withdrawn", &amount.to_string()),
        ("vault_id", &vault_id.to_string()),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new()
        .add_messages(vec![msg_transfer])
        .add_events([withdrawal_event, funding_event]))
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

use crate::{
    funding::apply_funding_rate,
    helpers::{
        calculate_fee, create_apply_funding_event, create_mint_message_and_modify_response,
        get_collateral_from_vault_type, get_liquidation_results, get_rebase_mint_amount,
        get_sent_collateral_and_vault_type, get_vault_collateral,
    },
    state::{Config, CONFIG, STATE},
    utils::decimal_to_fixed,
    vault::{
        burn_vault, check_vault, create_vault, get_vault_type, is_vault_safe, update_vault, VAULTS,
    },
};

use cosmwasm_std::{
    coin, ensure, ensure_eq, BankMsg, CosmosMsg, Decimal, DepsMut, Env, Event, MessageInfo,
    Response, Uint128,
};
use cw_utils::{must_pay, nonpayable};
use margined_common::errors::ContractError;
use osmosis_std::types::{cosmos::base::v1beta1::Coin, osmosis::tokenfactory::v1beta1::MsgBurn};

pub fn mint(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mint_amount: Uint128,
    vault_id: Option<u64>,
    rebase: bool,
    should_sell: bool,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config = CONFIG.load(deps.storage)?;

    ensure!(!mint_amount.is_zero(), ContractError::ZeroMint {});

    let (collateral_sent, collateral_vault_type) =
        get_sent_collateral_and_vault_type(&info, &config)
            .map_err(|_| ContractError::InvalidFunds {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    let rebase_mint_amount = get_rebase_mint_amount(
        mint_amount,
        cached_normalisation_factor,
        config.base_asset.decimals,
        rebase,
    )?;

    let (vault_id, vault_type) = match (vault_id, collateral_vault_type) {
        (Some(vault_id), Some(vault_type)) => {
            let expected_vault_type = get_vault_type(deps.storage, vault_id)
                .map_err(|_| ContractError::VaultDoesNotExist(vault_id))?;

            ensure_eq!(
                vault_type,
                expected_vault_type,
                ContractError::VaultTypeDoesNotMatch(
                    expected_vault_type.to_string(),
                    vault_type.to_string(),
                    vault_id
                )
            );

            (vault_id, expected_vault_type)
        }
        (Some(vault_id), None) => {
            let expected_vault_type = get_vault_type(deps.storage, vault_id)
                .map_err(|_| ContractError::VaultDoesNotExist(vault_id))?;

            (vault_id, expected_vault_type)
        }
        (None, Some(vault_type)) => {
            let vault_id = create_vault(deps.storage, info.sender.clone(), vault_type.clone())?;

            (vault_id, vault_type)
        }
        (None, None) => return Err(ContractError::MustSendCollateralToNewVault {}),
    };

    let (fee_amount, collateral_with_fee) = calculate_fee(
        deps.branch(),
        env.clone(),
        info.sender.clone(),
        vault_id,
        &vault_type,
        Decimal::from_atomics(rebase_mint_amount, config.power_asset.decimals).unwrap(),
        Decimal::from_atomics(collateral_sent, config.base_asset.decimals).unwrap(),
    )?;

    update_vault(
        deps.storage,
        vault_id,
        info.sender.clone(),
        &vault_type,
        decimal_to_fixed(collateral_with_fee, config.base_asset.decimals),
        rebase_mint_amount,
    )?;

    let (is_safe, min_collateral, _) = check_vault(
        deps.as_ref(),
        config.clone(),
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(is_safe, ContractError::UnsafeVault {});
    ensure!(min_collateral, ContractError::BelowMinCollateralAmount {});

    let mut response: Response = Response::new();
    if !rebase_mint_amount.is_zero() {
        response = create_mint_message_and_modify_response(
            response,
            env.contract.address.to_string(),
            config.power_asset.denom.clone(),
            rebase_mint_amount.to_string(),
            info.sender.to_string(),
            should_sell,
        );
    }

    if !fee_amount.is_zero() {
        let fixed_fee_amount = decimal_to_fixed(fee_amount, config.base_asset.decimals);

        let fee_asset = get_vault_collateral(deps.storage, &config, vault_id)?;

        let msg_fee_transfer = BankMsg::Send {
            to_address: config.fee_pool_contract.to_string(),
            amount: vec![coin(fixed_fee_amount.u128(), fee_asset)],
        };

        response = response.clone().add_message(msg_fee_transfer);
    }

    let mint_event = Event::new("mint").add_attributes([
        ("user", &info.sender.to_string()),
        ("collateral_deposited", &collateral_sent.to_string()),
        ("mint_amount", &rebase_mint_amount.to_string()),
        ("fee_amount", &fee_amount.to_string()),
        ("vault_id", &vault_id.to_string()),
        ("vault_type", &vault_type.to_string()),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(response.add_events([mint_event, funding_event]))
}

pub fn burn(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount_to_withdraw: Option<Uint128>,
    vault_id: u64,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config: Config = CONFIG.load(deps.storage)?;

    if !VAULTS.has(deps.storage, vault_id) {
        return Err(ContractError::VaultDoesNotExist(vault_id));
    };

    let amount_to_withdraw = amount_to_withdraw.unwrap_or(Uint128::zero());

    let amount_to_burn =
        must_pay(&info, &config.power_asset.denom).map_err(|_| ContractError::InvalidFunds {})?;

    if amount_to_burn.is_zero() {
        return Err(ContractError::InvalidFunds {});
    }

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    burn_vault(
        deps.storage,
        vault_id,
        info.sender.clone(),
        amount_to_withdraw,
        amount_to_burn,
    )?;

    let is_safe = is_vault_safe(
        deps.as_ref(),
        config.clone(),
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(is_safe, ContractError::UnsafeVault {});

    let mut messages = Vec::<CosmosMsg>::new();

    // burn power perp token
    let msg_burn: CosmosMsg = MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom: config.power_asset.denom.clone(),
            amount: amount_to_burn.to_string(),
        }),
        burn_from_address: env.contract.address.to_string(),
    }
    .into();

    messages.push(msg_burn);

    // transfer base to sender
    if !amount_to_withdraw.is_zero() {
        let asset_to_withdraw = get_vault_collateral(deps.storage, &config, vault_id)?;

        let msg_transfer = CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![coin(amount_to_withdraw.u128(), asset_to_withdraw)],
        });

        messages.push(msg_transfer);
    };

    let burn_event = Event::new("burn").add_attributes([
        ("user", &info.sender.to_string()),
        ("collateral_burnt", &amount_to_burn.to_string()),
        ("withdrawn", &amount_to_withdraw.to_string()),
        ("vault_id", &vault_id.to_string()),
        (
            "vault_type",
            &get_vault_type(deps.storage, vault_id)?.to_string(),
        ),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new()
        .add_messages(messages)
        .add_events([burn_event, funding_event]))
}

pub fn liquidate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    max_debt_amount: Uint128,
    vault_id: u64,
    event: String,
    min_amount_to_pay: Option<Uint128>,
) -> Result<Response, ContractError> {
    STATE.load(deps.storage)?.is_open_and_unpaused()?;

    let config: Config = CONFIG.load(deps.storage)?;

    let vault_type = get_vault_type(deps.storage, vault_id)
        .map_err(|_| ContractError::VaultDoesNotExist(vault_id))?;
    let liquidation_asset = get_collateral_from_vault_type(&config, vault_type)?;

    // liquidator does not require to send funds, rather it is burnt directly
    nonpayable(&info).map_err(|_| ContractError::NonPayable {})?;

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

    let (liquidation_amount, collateral_to_pay, _) =
        get_liquidation_results(deps.as_ref(), env.clone(), max_debt_amount, vault.clone());

    if max_debt_amount < liquidation_amount {
        return Err(ContractError::InvalidLiquidation {});
    }

    if min_amount_to_pay.is_some() && collateral_to_pay < min_amount_to_pay.unwrap() {
        return Err(ContractError::UnprofitableLiquidation {});
    }

    burn_vault(
        deps.storage,
        vault_id,
        vault.operator.clone(),
        collateral_to_pay,
        liquidation_amount,
    )?;

    // burn power perp token
    let msg_burn: CosmosMsg = MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom: config.power_asset.denom,
            amount: liquidation_amount.to_string(),
        }),
        burn_from_address: info.sender.to_string(),
    }
    .into();

    // transfer collateral to sender
    let msg_transfer: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![coin(collateral_to_pay.u128(), liquidation_asset.denom)],
    });

    let liquidation_event = Event::new(event).add_attributes([
        ("liquidator", &info.sender.to_string()),
        ("liquidatee", &vault.operator.to_string()),
        ("liquidation_amount", &liquidation_amount.to_string()),
        ("collateral_to_pay", &collateral_to_pay.to_string()),
        ("vault_id", &vault_id.to_string()),
        (
            "vault_type",
            &get_vault_type(deps.storage, vault_id)?.to_string(),
        ),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new()
        .add_messages(vec![msg_burn, msg_transfer])
        .add_events([liquidation_event, funding_event]))
}

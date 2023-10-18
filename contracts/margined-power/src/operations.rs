use crate::{
    funding::apply_funding_rate,
    helpers::{calculate_fee, create_apply_funding_event, create_mint_message, decimal_to_fixed},
    state::{Config, CONFIG, STATE},
    vault::{burn_vault, check_vault, create_vault, update_vault, VAULTS},
};

use cosmwasm_std::{
    coin, ensure, BankMsg, CosmosMsg, Decimal, DepsMut, Env, Event, MessageInfo, Response, Uint128,
};
use cw_utils::{may_pay, must_pay};
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

    let config: Config = CONFIG.load(deps.storage)?;

    ensure!(!mint_amount.is_zero(), ContractError::ZeroMint {});

    let collateral_sent =
        may_pay(&info, &config.base_denom).map_err(|_| ContractError::InvalidFunds {})?;

    let cached_normalisation_factor = apply_funding_rate(deps.branch(), env.clone())?;

    let mint_amount = match rebase {
        true => {
            let fixed_normalisation_factor =
                decimal_to_fixed(cached_normalisation_factor, config.base_decimals);

            mint_amount
                .checked_mul(Uint128::from(10u128.pow(config.base_decimals)))
                .unwrap()
                .checked_div(fixed_normalisation_factor)
                .unwrap()
        }
        false => mint_amount,
    };

    let vault_id = match vault_id {
        Some(vault_id) => {
            if !VAULTS.has(deps.storage, &vault_id) {
                return Err(ContractError::VaultDoesNotExist {});
            };

            vault_id
        }
        None => create_vault(deps.storage, info.sender.clone())?,
    };

    let (fee_amount, collateral_with_fee) = calculate_fee(
        deps.branch(),
        env.clone(),
        info.sender.clone(),
        vault_id,
        Decimal::from_atomics(mint_amount, config.power_decimals).unwrap(),
        Decimal::from_atomics(collateral_sent, config.base_decimals).unwrap(),
    )?;

    update_vault(
        deps.storage,
        vault_id,
        info.sender.clone(),
        decimal_to_fixed(collateral_with_fee, config.base_decimals),
        mint_amount,
    )?;

    let (is_safe, min_collateral) = check_vault(
        deps.as_ref(),
        config.clone(),
        vault_id,
        cached_normalisation_factor,
        env.block.time,
    )?;

    ensure!(is_safe, ContractError::UnsafeVault {});
    ensure!(min_collateral, ContractError::BelowMinCollateralAmount {});

    let mut response: Response = Response::new();
    if !mint_amount.is_zero() {
        response = create_mint_message(
            response,
            env.contract.address.to_string(),
            config.power_denom.clone(),
            mint_amount.to_string(),
            info.sender.to_string(),
            should_sell,
        );
    }

    if !fee_amount.is_zero() {
        let fixed_fee_amount = decimal_to_fixed(fee_amount, config.base_decimals);

        let msg_fee_transfer = BankMsg::Send {
            to_address: config.fee_pool_contract.to_string(),
            amount: vec![coin(fixed_fee_amount.u128(), config.base_denom)],
        };

        response = response.clone().add_message(msg_fee_transfer);
    }

    let mint_event = Event::new("mint").add_attributes([
        ("collateral_deposited", &collateral_sent.to_string()),
        ("mint_amount", &mint_amount.to_string()),
        ("fee_amount", &fee_amount.to_string()),
        ("vault_id", &vault_id.to_string()),
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

    if !VAULTS.has(deps.storage, &vault_id) {
        return Err(ContractError::VaultDoesNotExist {});
    };

    let amount_to_withdraw = amount_to_withdraw.unwrap_or(Uint128::zero());

    let amount_to_burn =
        must_pay(&info, &config.power_denom).map_err(|_| ContractError::InvalidFunds {})?;

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

    let (is_safe, _) = check_vault(
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
            denom: config.power_denom.clone(),
            amount: amount_to_burn.to_string(),
        }),
        burn_from_address: env.contract.address.to_string(),
    }
    .into();

    messages.push(msg_burn);

    // transfer base to sender
    if !amount_to_withdraw.is_zero() {
        let msg_transfer = CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![coin(amount_to_withdraw.u128(), config.base_denom)],
        });

        messages.push(msg_transfer);
    };

    let burn_event = Event::new("burn").add_attributes([
        ("collateral_burnt", &amount_to_burn.to_string()),
        ("withdrawn", &amount_to_withdraw.to_string()),
        ("vault_id", &vault_id.to_string()),
    ]);

    let funding_event = create_apply_funding_event(&cached_normalisation_factor.to_string());

    Ok(Response::new()
        .add_messages(messages)
        .add_events([burn_event, funding_event]))
}

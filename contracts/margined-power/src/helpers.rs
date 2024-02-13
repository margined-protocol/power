use crate::{
    contract::OPEN_SHORT_REPLY_ID,
    queries::{get_pool_twap, get_scaled_pool_twap, get_spot_price, get_stake_pool_twap},
    state::{Config, CONFIG, LIQUIDATION_BOUNTY, STAKED_ASSET_MULTIPLIER, STAKE_ASSETS},
    utils::decimal_to_fixed,
    vault::{get_vault_type, subtract_collateral, Vault},
};

use cosmwasm_std::{
    Addr, Decimal, Deps, DepsMut, Env, Event, MessageInfo, ReplyOn, Response, StdError, StdResult,
    Storage, SubMsg, Timestamp, Uint128,
};
use cw_utils::may_pay;
use margined_common::common::TWAP_PERIOD;
use margined_protocol::power::{Asset, VaultType};
use osmosis_std::types::{cosmos::base::v1beta1::Coin, osmosis::tokenfactory::v1beta1::MsgMint};

pub struct PoolParams {
    pub id: u64,
    pub base_denom: String,
    pub quote_denom: String,
    pub base_decimal_places: u32,
    pub quote_decimal_places: u32,
}

pub fn calculate_fee(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    vault_id: u64,
    vault_type: &VaultType,
    power_amount: Decimal,
    deposit_amount: Decimal,
) -> StdResult<(Decimal, Decimal)> {
    let config = CONFIG.load(deps.storage).unwrap();

    if config.fee_rate.is_zero() {
        return Ok((Decimal::zero(), deposit_amount));
    }

    let base_amount_value =
        calculate_debt_in_collateral(deps.as_ref(), env, power_amount, vault_type).unwrap();

    let fee_rate_multiplier = match vault_type {
        VaultType::Default => Decimal::one(),
        VaultType::Staked { .. } => STAKED_ASSET_MULTIPLIER,
    };

    let fee_amount = base_amount_value
        .checked_mul(config.fee_rate * fee_rate_multiplier)
        .unwrap();

    // if the deposit is unsufficient to cover the fee, use the collateral deposited
    let deposit_post_fees = if deposit_amount > fee_amount {
        deposit_amount.checked_sub(fee_amount).map_err(|_| {
            StdError::generic_err("Subtraction underflow in calculating deposit post fees")
        })?
    } else {
        subtract_collateral(
            deps.storage,
            vault_id,
            sender,
            decimal_to_fixed(fee_amount, config.base_asset.decimals),
        )?;

        deposit_amount
    };

    Ok((fee_amount, deposit_post_fees))
}

pub fn calculate_index(deps: Deps, start_time: Timestamp) -> StdResult<Decimal> {
    let config = CONFIG.load(deps.storage).unwrap();

    let quote_price = get_scaled_pool_twap(
        &deps,
        config.base_pool.id,
        config.base_asset.denom.clone(),
        config.base_pool.quote_denom,
        start_time,
    )
    .unwrap();

    let index = quote_price
        .checked_mul(quote_price)
        .unwrap()
        .checked_div(Decimal::one())
        .unwrap();

    Ok(index)
}

pub fn calculate_denormalized_mark(
    deps: Deps,
    start_time: Timestamp,
    normalisation_factor: Decimal,
) -> StdResult<Decimal> {
    let config = CONFIG.load(deps.storage).unwrap();

    let quote_price = get_scaled_pool_twap(
        &deps,
        config.base_pool.id,
        config.base_asset.denom.clone(),
        config.base_pool.quote_denom.clone(),
        start_time,
    )
    .unwrap();

    let power_price = get_pool_twap(
        &deps,
        config.power_pool.id,
        config.power_asset.denom,
        config.base_asset.denom,
        start_time,
    )
    .unwrap();

    let mark = quote_price
        .checked_mul(power_price)
        .unwrap()
        .checked_div(normalisation_factor)
        .unwrap();

    Ok(mark)
}

pub fn get_min_amount_out_from_slippage(
    deps: Deps,
    token_amount_in: Uint128,
    slippage: Option<Decimal>,
    pool: PoolParams,
) -> StdResult<Uint128> {
    let slippage = match slippage {
        Some(s) => s,
        None => return Ok(Uint128::from(1u128)),
    };

    if slippage >= Decimal::one() {
        return Err(StdError::generic_err("Slippage cannot be greater than 1"));
    }

    let spot_price = get_spot_price(&deps, pool.id, pool.base_denom, pool.quote_denom)?;

    let best_amount_out = spot_price.checked_mul(
        Decimal::from_atomics(token_amount_in, pool.quote_decimal_places)
            .map_err(|_| StdError::generic_err("Invalid token amount"))?,
    )?;

    let min_amount_out = best_amount_out
        .checked_mul(Decimal::one() - slippage)
        .map_err(|_| StdError::generic_err("Overflow in min_amount_out calculation"))?;

    Ok(decimal_to_fixed(min_amount_out, pool.base_decimal_places))
}

pub fn calculate_debt_in_collateral(
    deps: Deps,
    env: Env,
    debt_amount: Decimal,
    vault_type: &VaultType,
) -> StdResult<Decimal> {
    let start_time = env.block.time.minus_seconds(TWAP_PERIOD);
    let config = CONFIG.load(deps.storage).unwrap();

    let power_price = get_pool_twap(
        &deps,
        config.power_pool.id,
        config.power_asset.denom,
        config.base_asset.denom,
        start_time,
    )
    .unwrap();

    match vault_type {
        VaultType::Default => Ok(debt_amount.checked_mul(power_price).unwrap()),
        VaultType::Staked { denom } => {
            let stake_price = get_stake_pool_twap(&deps, denom.to_string(), start_time)?.unwrap();

            Ok(debt_amount
                .checked_mul(power_price)
                .unwrap()
                .checked_div(stake_price)
                .unwrap())
        }
    }
}

pub fn create_mint_message_and_modify_response(
    response: Response,
    contract_address: String,
    denom: String,
    amount: String,
    sender: String,
    should_sell: bool,
) -> Response {
    match should_sell {
        true => {
            let mint_submsg: SubMsg = SubMsg {
                id: OPEN_SHORT_REPLY_ID,
                msg: MsgMint {
                    sender: contract_address.clone(),
                    amount: Some(Coin { denom, amount }),
                    mint_to_address: contract_address,
                }
                .into(),
                gas_limit: None,
                reply_on: ReplyOn::Success,
            };

            response.add_submessage(mint_submsg)
        }
        false => response.add_message(MsgMint {
            sender: contract_address,
            amount: Some(Coin { denom, amount }),
            mint_to_address: sender,
        }),
    }
}

pub fn get_sent_collateral_and_vault_type(
    info: &MessageInfo,
    config: &Config,
) -> StdResult<(Uint128, Option<VaultType>)> {
    match info.funds.len() {
        0 => Ok((Uint128::zero(), None)),
        1 => {
            if config.is_stake_enabled() {
                for stake_asset in config.stake_assets.as_ref().unwrap() {
                    if let Ok(stake_collateral) = may_pay(info, &stake_asset.denom) {
                        if !stake_collateral.is_zero() {
                            return Ok((
                                stake_collateral,
                                Some(VaultType::Staked {
                                    denom: stake_asset.denom.clone(),
                                }),
                            ));
                        }
                    }
                }
            }

            match may_pay(info, &config.base_asset.denom) {
                Ok(base_collateral) => Ok((base_collateral, Some(VaultType::Default))),
                Err(_) => Err(StdError::generic_err("Failed to get base collateral")),
            }
        }
        _ => Err(StdError::generic_err("Invalid funds")),
    }
}

pub fn get_vault_collateral(
    storage: &dyn Storage,
    config: &Config,
    vault_id: u64,
) -> StdResult<String> {
    let vault_type = get_vault_type(storage, vault_id)?;

    match vault_type {
        VaultType::Default => Ok(config.base_asset.denom.clone()),
        VaultType::Staked { denom } => {
            if config.is_stake_enabled() {
                Ok(denom)
            } else {
                Err(StdError::generic_err(
                    "Vault type invalid as staked collateral is not enabled",
                ))
            }
        }
    }
}

pub fn get_staked_asset_decimals(denom: String, config: &Config) -> StdResult<u32> {
    for asset in config.stake_assets.as_ref().unwrap() {
        if asset.denom == denom {
            return Ok(asset.decimals);
        }
    }

    Err(StdError::generic_err("Staked asset not found"))
}

pub fn get_collateral_from_vault_type(config: &Config, vault_type: VaultType) -> StdResult<Asset> {
    match vault_type {
        VaultType::Default => Ok(config.clone().base_asset),
        VaultType::Staked { denom } => {
            for asset in config.stake_assets.as_ref().unwrap() {
                if asset.denom == denom {
                    return Ok(Asset {
                        denom: asset.denom.clone(),
                        decimals: asset.decimals,
                    });
                }
            }

            Err(StdError::generic_err("Staked asset not found"))
        }
    }
}

pub fn get_liquidation_results(
    deps: Deps,
    env: Env,
    max_repayment_amount: Uint128,
    vault: Vault,
) -> (Uint128, Uint128, Uint128) {
    let config = CONFIG.load(deps.storage).unwrap();

    // first try just to liquidate half
    let max_liquidatable_amount = vault.short_amount.checked_div(2u128.into()).unwrap();

    let (mut liquidation_amount, mut collateral_to_pay, mut debt_to_repay) = get_liquidation_amount(
        deps,
        env.clone(),
        max_repayment_amount,
        max_liquidatable_amount,
        &vault.vault_type,
    );

    let half_collateral_denom = match &vault.vault_type {
        VaultType::Default => Uint128::from(10u128.pow(config.base_asset.decimals))
            .checked_div(2u128.into())
            .unwrap(),
        VaultType::Staked { denom } => {
            let stake_asset = STAKE_ASSETS.load(deps.storage, denom.to_string()).unwrap();

            Uint128::from(10u128.pow(stake_asset.decimals))
                .checked_div(2u128.into())
                .unwrap()
        }
    };

    if vault.collateral > collateral_to_pay
        && vault.collateral.checked_sub(collateral_to_pay).unwrap() < half_collateral_denom
    {
        (liquidation_amount, collateral_to_pay, debt_to_repay) = get_liquidation_amount(
            deps,
            env,
            max_repayment_amount,
            vault.short_amount,
            &vault.vault_type,
        );
    }

    if collateral_to_pay > vault.collateral {
        liquidation_amount = vault.short_amount;
        collateral_to_pay = vault.collateral;
    };

    (liquidation_amount, collateral_to_pay, debt_to_repay)
}

pub fn get_liquidation_amount(
    deps: Deps,
    env: Env,
    input_amount: Uint128,
    max_liquidatable_amount: Uint128,
    vault_type: &VaultType,
) -> (Uint128, Uint128, Uint128) {
    let config = CONFIG.load(deps.storage).unwrap();

    let amount_to_liquidate = std::cmp::min(input_amount, max_liquidatable_amount);

    let decimals_amount_to_liquidate =
        Decimal::from_atomics(amount_to_liquidate, config.power_asset.decimals).unwrap();

    let debt_to_repay =
        calculate_debt_in_collateral(deps, env, decimals_amount_to_liquidate, vault_type).unwrap();

    // 10% liquidation bounty
    let collateral_to_repay = debt_to_repay.checked_mul(LIQUIDATION_BOUNTY).unwrap();

    let collateral_to_repay = decimal_to_fixed(collateral_to_repay, config.base_asset.decimals);

    let debt_to_repay = decimal_to_fixed(debt_to_repay, config.base_asset.decimals);

    (amount_to_liquidate, collateral_to_repay, debt_to_repay)
}

pub fn get_rebase_mint_amount(
    mint_amount: Uint128,
    normalisation_factor: Decimal,
    base_decimals: u32,
    is_rebase: bool,
) -> StdResult<Uint128> {
    if is_rebase {
        let fixed_normalisation_factor = decimal_to_fixed(normalisation_factor, base_decimals);

        let scaled_amount = mint_amount.checked_mul(Uint128::from(10u128.pow(base_decimals)))?;

        let rebase_amount = scaled_amount.checked_div(fixed_normalisation_factor)?;

        Ok(rebase_amount)
    } else {
        Ok(mint_amount)
    }
}

pub fn create_apply_funding_event(funding_rate: &str) -> Event {
    Event::new("apply_funding").add_attribute("funding_rate", funding_rate)
}

use crate::{
    contract::OPEN_SHORT_REPLY_ID,
    queries::{get_pool_twap, get_scaled_pool_twap},
    state::{CONFIG, LIQUIDATION_BOUNTY, TWAP_PERIOD},
    vault::{subtract_collateral, Vault},
};

use cosmwasm_std::{
    Addr, Binary, Decimal, Deps, DepsMut, Env, Event, ReplyOn, Response, StdResult, SubMsg,
    SubMsgResponse, SubMsgResult, Timestamp, Uint128,
};
use injective_math::FPDecimal;
use margined_common::errors::ContractError;
use num::pow::Pow;
use osmosis_std::types::{
    cosmos::base::v1beta1::Coin,
    osmosis::poolmanager::v1beta1::{
        MsgSwapExactAmountIn, MsgSwapExactAmountOut, SwapAmountInRoute, SwapAmountOutRoute,
    },
    osmosis::tokenfactory::v1beta1::MsgMint,
};
use std::str::FromStr;

pub fn wrapped_pow(base: Decimal, exponent: Decimal) -> StdResult<Decimal> {
    let fp_base = FPDecimal::from_str(&base.to_string()).unwrap();
    let fp_exponent = FPDecimal::from_str(&exponent.to_string()).unwrap();

    let result = fp_base.pow(fp_exponent);

    Ok(Decimal::from_str(&result.to_string()).unwrap())
}

pub fn decimal_to_fixed(value: Decimal, decimal_places: u32) -> Uint128 {
    value
        .atomics()
        .checked_div(Uint128::new(
            10u128.pow(Decimal::DECIMAL_PLACES - decimal_places),
        ))
        .unwrap()
}

pub fn calculate_fee(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    vault_id: u64,
    power_amount: Decimal,
    deposit_amount: Decimal,
) -> StdResult<(Decimal, Decimal)> {
    let config = CONFIG.load(deps.storage).unwrap();

    if config.fee_rate.is_zero() {
        return Ok((Decimal::zero(), deposit_amount));
    }

    let base_amount_value = calculate_debt_in_base(deps.as_ref(), env, power_amount).unwrap();

    let fee_amount = base_amount_value.checked_mul(config.fee_rate).unwrap();

    // if the deposit is unsufficient to cover the fee, use the collateral deposited
    let deposit_post_fees = if deposit_amount > fee_amount {
        deposit_amount.checked_sub(fee_amount).unwrap()
    } else {
        subtract_collateral(
            deps.storage,
            vault_id,
            sender,
            decimal_to_fixed(fee_amount, config.base_decimals),
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
        config.base_denom.clone(),
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
        config.base_denom.clone(),
        config.base_pool.quote_denom.clone(),
        start_time,
    )
    .unwrap();

    let power_price = get_pool_twap(
        &deps,
        config.power_pool.id,
        config.power_denom,
        config.base_denom,
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

pub fn calculate_debt_in_base(deps: Deps, env: Env, debt_amount: Decimal) -> StdResult<Decimal> {
    let start_time = env.block.time.minus_seconds(TWAP_PERIOD);
    let config = CONFIG.load(deps.storage).unwrap();

    let power_price = get_pool_twap(
        &deps,
        config.power_pool.id,
        config.power_denom,
        config.base_denom,
        start_time,
    )
    .unwrap();

    let debt_value = debt_amount.checked_mul(power_price).unwrap();

    Ok(debt_value)
}

pub fn create_mint_message(
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

pub fn create_swap_exact_amount_in_message(
    sender: String,
    pool_id: u64,
    token_in_denom: String,
    token_out_denom: String,
    amount: String,
) -> MsgSwapExactAmountIn {
    MsgSwapExactAmountIn {
        sender,
        routes: vec![SwapAmountInRoute {
            pool_id,
            token_out_denom,
        }],
        token_in: Some(Coin {
            denom: token_in_denom,
            amount,
        }),
        token_out_min_amount: "1".to_string(),
    }
}

pub fn create_swap_exact_amount_out_message(
    sender: String,
    pool_id: u64,
    token_in_denom: String,
    token_out_denom: String,
    amount_out: String,
    token_in_max_amount: String,
) -> MsgSwapExactAmountOut {
    MsgSwapExactAmountOut {
        sender,
        routes: vec![SwapAmountOutRoute {
            pool_id,
            token_in_denom,
        }],
        token_out: Some(Coin {
            denom: token_out_denom,
            amount: amount_out,
        }),
        token_in_max_amount,
    }
}

pub fn get_liquidation_results(
    deps: Deps,
    env: Env,
    max_repayment_amount: Uint128,
    vault: Vault,
) -> (Uint128, Uint128) {
    let config = CONFIG.load(deps.storage).unwrap();

    // first try just to liquidate half
    let max_liquidateable_amount = vault.short_amount.checked_div(2u128.into()).unwrap();

    let (mut liquidation_amount, mut collateral_to_pay) = get_liquidation_amount(
        deps,
        env.clone(),
        max_repayment_amount,
        max_liquidateable_amount,
    );

    let half_base_denom = Uint128::from(10u128.pow(config.base_decimals))
        .checked_div(2u128.into())
        .unwrap();

    if vault.collateral > collateral_to_pay
        && vault.collateral.checked_sub(collateral_to_pay).unwrap() < half_base_denom
    {
        (liquidation_amount, collateral_to_pay) =
            get_liquidation_amount(deps, env, max_repayment_amount, vault.short_amount);
    }

    if collateral_to_pay > vault.collateral {
        liquidation_amount = vault.short_amount;
        collateral_to_pay = vault.collateral;
    };

    (liquidation_amount, collateral_to_pay)
}

pub fn get_liquidation_amount(
    deps: Deps,
    env: Env,
    input_amount: Uint128,
    max_liquidatable_amount: Uint128,
) -> (Uint128, Uint128) {
    let config = CONFIG.load(deps.storage).unwrap();

    let amount_to_liquidate = if input_amount > max_liquidatable_amount {
        max_liquidatable_amount
    } else {
        input_amount
    };

    let decimals_amount_to_liquidate =
        Decimal::from_atomics(amount_to_liquidate, config.power_decimals).unwrap();
    let mut collateral_to_repay =
        calculate_debt_in_base(deps, env, decimals_amount_to_liquidate).unwrap();

    // 10% liquidation bounty
    collateral_to_repay = collateral_to_repay.checked_mul(LIQUIDATION_BOUNTY).unwrap();

    let collateral_to_repay = decimal_to_fixed(collateral_to_repay, config.base_decimals);

    (amount_to_liquidate, collateral_to_repay)
}

pub fn parse_response_result_data(result: SubMsgResult) -> Result<Binary, ContractError> {
    match result {
        SubMsgResult::Ok(SubMsgResponse { data: Some(b), .. }) => Ok(b),
        SubMsgResult::Ok(SubMsgResponse { data: None, .. }) => {
            Err(ContractError::SubMsgError("No data in reply".to_string()))
        }
        SubMsgResult::Err(err) => Err(ContractError::SubMsgError(err)),
    }
}

pub fn create_apply_funding_event(funding_rate: &str) -> Event {
    Event::new("apply_funding").add_attribute("funding_rate", funding_rate)
}

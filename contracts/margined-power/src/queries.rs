use crate::state::{CONFIG, STAKE_ASSETS};

use cosmwasm_std::{
    to_binary, Coin, Decimal, Deps, QueryRequest, StdError, StdResult, Timestamp, Uint128,
    WasmQuery,
};
use margined_protocol::query::QueryMsg;
use osmosis_std::types::{
    cosmos::bank::v1beta1::BankQuerier, osmosis::poolmanager::v1beta1::PoolmanagerQuerier,
};
use std::str::FromStr;

pub fn get_spot_price(
    deps: &Deps,
    pool_id: u64,
    base_asset: String,
    quote_asset: String,
) -> StdResult<Decimal> {
    let poolmanager = PoolmanagerQuerier::new(&deps.querier);

    let res = poolmanager
        .spot_price(pool_id, base_asset, quote_asset)
        .map_err(|_| {
            StdError::generic_err(format!("Cannot get spot price, pool id: {}", pool_id))
        })?;

    let price = Decimal::from_str(&res.spot_price).unwrap();

    Ok(price)
}

pub fn get_stake_pool_twap(
    deps: &Deps,
    denom: String,
    start_time: Timestamp,
) -> StdResult<Option<Decimal>> {
    let config = CONFIG.load(deps.storage).unwrap();

    if config.is_stake_enabled() {
        let stake_pool = STAKE_ASSETS
            .load(deps.storage, denom)
            .map_err(|_| StdError::generic_err("Stake pool is not configured"))?;

        let stake_price = get_pool_twap(
            deps,
            stake_pool.pool.id,
            stake_pool.pool.base_denom,
            stake_pool.pool.quote_denom,
            start_time,
        )
        .map_err(|err| StdError::generic_err(format!("Failed to get pool TWAP: {}", err)))?;

        Ok(Some(stake_price))
    } else {
        Ok(None)
    }
}

pub fn get_pool_twap(
    deps: &Deps,
    pool_id: u64,
    base_asset: String,
    quote_asset: String,
    start_time: Timestamp,
) -> StdResult<Decimal> {
    let config = CONFIG.load(deps.storage).unwrap();

    let price: Decimal = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.query_contract.to_string(),
            msg: to_binary(&QueryMsg::GetArithmeticTwapToNow {
                pool_id,
                base_asset,
                quote_asset,
                start_time,
            })?,
        }))
        .map_err(|_| {
            StdError::generic_err(format!("Cannot get pool twap, pool id: {}", pool_id))
        })?;

    Ok(price)
}

pub fn get_scaled_pool_twap(
    deps: &Deps,
    pool_id: u64,
    base_asset: String,
    quote_asset: String,
    start_time: Timestamp,
) -> StdResult<Decimal> {
    let config = CONFIG.load(deps.storage).unwrap();

    let price: Decimal = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.query_contract.to_string(),
            msg: to_binary(&QueryMsg::GetArithmeticTwapToNow {
                pool_id,
                base_asset,
                quote_asset,
                start_time,
            })?,
        }))
        .map_err(|_| {
            StdError::generic_err(format!("Cannot get scaled pool twap, pool id: {}", pool_id))
        })?;

    Ok(price / Decimal::from_atomics(config.index_scale, 0).unwrap())
}

pub fn get_denom_authority(deps: Deps, denom: String) -> StdResult<String> {
    let config = CONFIG.load(deps.storage).unwrap();

    let res: Option<String> = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.query_contract.to_string(),
            msg: to_binary(&QueryMsg::GetDenomAuthority { denom })?,
        }))
        .unwrap();

    if res.is_none() {
        return Err(StdError::generic_err("No pool authority found"));
    }

    Ok(res.unwrap())
}

pub fn get_total_supply(deps: Deps, denom: String) -> StdResult<Uint128> {
    let bank = BankQuerier::new(&deps.querier);

    let res = bank.supply_of(denom)?;

    let amount = match res.amount {
        Some(amount) => Uint128::from_str(&amount.amount)?,
        None => return Err(StdError::generic_err("No supply found")),
    };

    Ok(amount)
}

pub fn get_balance(deps: Deps, address: String, denom: String) -> StdResult<Uint128> {
    let bank = BankQuerier::new(&deps.querier);

    let res = bank.balance(address, denom)?;

    let amount = match res.balance {
        Some(amount) => Uint128::from_str(&amount.amount)?,
        None => return Err(StdError::generic_err("No balance found")),
    };

    Ok(amount)
}

pub fn estimate_single_pool_swap_exact_amount_out(
    deps: Deps,
    pool_id: u64,
    token_out_amount: Uint128,
    token_out_denom: String,
    token_in_denom: String,
) -> StdResult<Uint128> {
    let poolmanager = PoolmanagerQuerier::new(&deps.querier);

    let res = poolmanager.estimate_single_pool_swap_exact_amount_out(
        pool_id,
        token_in_denom,
        Coin {
            denom: token_out_denom,
            amount: token_out_amount,
        }
        .to_string(),
    )?;

    Uint128::from_str(&res.token_in_amount)
}

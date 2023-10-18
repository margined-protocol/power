use crate::state::{CONFIG, INDEX_SCALE};

use cosmwasm_std::{
    to_binary, Decimal, Deps, QueryRequest, StdError, StdResult, Timestamp, Uint128, WasmQuery,
};
use margined_protocol::query::QueryMsg;
use osmosis_std::types::cosmos::bank::v1beta1::BankQuerier;
use std::str::FromStr;

pub fn get_pool_twap(
    deps: &Deps,
    pool_id: u64,
    base_asset: String,
    quote_asset: String,
    start_time: Timestamp,
) -> StdResult<Decimal> {
    let config = CONFIG.load(deps.storage).unwrap();

    let price: Decimal = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.query_contract.to_string(),
        msg: to_binary(&QueryMsg::GetArithmeticTwapToNow {
            pool_id,
            base_asset,
            quote_asset,
            start_time,
        })?,
    }))?;

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
        .unwrap();

    Ok(price / Decimal::from_atomics(INDEX_SCALE, 0).unwrap())
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

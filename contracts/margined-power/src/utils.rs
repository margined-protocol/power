use crate::state::STAKE_ASSETS;

use cosmwasm_std::{
    Binary, Decimal, StdError, StdResult, Storage, SubMsgResponse, SubMsgResult, Uint128,
};
use injective_math::FPDecimal;
use margined_common::errors::ContractError;
use margined_protocol::power::StakeAsset;
use num::pow::Pow;
use std::{collections::HashSet, str::FromStr};

pub fn check_duplicates(assets: &Vec<StakeAsset>) -> StdResult<()> {
    let mut seen = HashSet::new();

    for asset in assets {
        if !seen.insert(&asset.denom) {
            return Err(StdError::generic_err(format!(
                "Duplicate asset denom: {}",
                asset.denom
            )));
        }
    }

    Ok(())
}

pub fn check_is_staked_asset(storage: &dyn Storage, denom: String) -> StdResult<()> {
    match STAKE_ASSETS.has(storage, denom.clone()) {
        true => Ok(()),
        false => Err(StdError::generic_err(format!(
            "Asset {} is not a staked asset",
            denom
        ))),
    }
}

pub fn decimal_to_fixed(value: Decimal, decimal_places: u32) -> Uint128 {
    value
        .atomics()
        .checked_div(Uint128::new(
            10u128.pow(Decimal::DECIMAL_PLACES - decimal_places),
        ))
        .map_err(|_| StdError::generic_err("Failed to convert decimal to fixed"))
        .unwrap()
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

pub fn wrapped_pow(base: Decimal, exponent: Decimal) -> StdResult<Decimal> {
    let fp_base = FPDecimal::from_str(&base.to_string()).unwrap();
    let fp_exponent = FPDecimal::from_str(&exponent.to_string()).unwrap();

    let result = fp_base.pow(fp_exponent);

    Ok(Decimal::from_str(&result.to_string()).unwrap())
}

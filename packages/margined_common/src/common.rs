use crate::errors::ContractError;
use cosmwasm_std::{
    Binary, Coin, Decimal, Deps, Event, MessageInfo, StdError, StdResult, SubMsgResponse,
    SubMsgResult, Uint128,
};
use osmosis_std::types::{
    cosmos::bank::v1beta1::BankQuerier, osmosis::poolmanager::v1beta1::PoolmanagerQuerier,
};

pub fn parse_funds(funds: Vec<Coin>, expected_denom: String) -> StdResult<Uint128> {
    if funds.is_empty() {
        return Ok(Uint128::zero());
    };

    if funds.len() != 1 || funds[0].denom != expected_denom {
        return Err(StdError::generic_err("Invalid Funds"));
    }

    Ok(funds[0].amount)
}

pub fn check_denom_metadata(deps: Deps, denom: &str) -> StdResult<()> {
    let querier = BankQuerier::new(&deps.querier);

    querier.denom_metadata(denom.to_string())?;

    Ok(())
}

pub fn check_denom_exists_in_pool(deps: Deps, pool_id: u64, denom: &str) -> StdResult<()> {
    let querier = PoolmanagerQuerier::new(&deps.querier);

    let res = querier.total_pool_liquidity(pool_id)?;

    if res.liquidity.is_empty() {
        return Err(StdError::generic_err(format!(
            "No liquidity in pool id: {}",
            pool_id
        )));
    }

    res.liquidity
        .iter()
        .find(|x| x.denom == denom)
        .ok_or_else(|| {
            StdError::generic_err(format!("Denom \"{}\" in pool id: {}", denom, pool_id))
        })?;

    Ok(())
}

pub fn decimal_to_fixed(value: Decimal, decimal_places: u32) -> Uint128 {
    value
        .atomics()
        .checked_div(Uint128::new(
            10u128.pow(Decimal::DECIMAL_PLACES - decimal_places),
        ))
        .unwrap()
}

pub fn parse_event_attribute(events: Vec<Event>, event: &str, key: &str) -> String {
    events
        .iter()
        .find(|e| e.ty == event)
        .unwrap()
        .attributes
        .iter()
        .find(|e| e.key == key)
        .unwrap()
        .value
        .clone()
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

pub fn must_pay_two_denoms(
    info: &MessageInfo,
    first_denom: &str,
    second_denom: &str,
) -> Result<(Uint128, Uint128), String> {
    if info.funds.is_empty() {
        Err("No funds sent".to_string())
    } else if info.funds.len() == 1 && info.funds[0].denom == first_denom {
        Err(format!("Missing denom: {}", second_denom))
    } else if info.funds.len() == 1 && info.funds[0].denom == second_denom {
        Err(format!("Missing denom: {}", first_denom))
    } else if info.funds.len() == 2 {
        let base = match info.funds.iter().find(|c| c.denom == first_denom) {
            Some(c) => c,
            None => return Err(format!("Missing denom: {}", first_denom)),
        };

        let quote = match info.funds.iter().find(|c| c.denom == second_denom) {
            Some(c) => c,
            None => return Err(format!("Missing denom: {}", second_denom)),
        };

        Ok((base.amount, quote.amount))
    } else {
        // find first mis-match
        let wrong = info
            .funds
            .iter()
            .find(|c| c.denom != first_denom && c.denom != second_denom)
            .unwrap();

        Err(format!("Extra incorrect denom: {}", wrong.denom))
    }
}

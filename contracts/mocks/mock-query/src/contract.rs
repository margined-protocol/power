#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use cw_storage_plus::Map;
use margined_protocol::query::QueryMsg;
use osmosis_std::types::osmosis::tokenfactory::v1beta1::{
    DenomAuthorityMetadata, TokenfactoryQuerier,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const KEY_PRICES: Map<u64, Decimal> = Map::new("prices");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AppendPrice { pool_id: u64, price: Decimal },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: Addr,
}

#[cfg(not(tarpaulin_include))]
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg(not(tarpaulin_include))]
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::AppendPrice { pool_id, price } => append_price(deps, info, pool_id, price),
    }
}

/// this is a mock function that enables storage of data
/// by the contract owner will be replaced by integration
/// with on-chain price oracles in the future.
#[cfg(not(tarpaulin_include))]
pub fn append_price(
    deps: DepsMut,
    _info: MessageInfo,
    pool_id: u64,
    price: Decimal,
) -> StdResult<Response> {
    KEY_PRICES.save(deps.storage, pool_id, &price)?;

    Ok(Response::default())
}

#[cfg(not(tarpaulin_include))]
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetArithmeticTwapToNow { pool_id, .. } => {
            to_binary(&get_arithmetic_twap_now(deps, pool_id)?)
        }
        QueryMsg::GetDenomAuthority { denom } => to_binary(&get_denom_authority(deps, denom)?),
    }
}

/// Queries latest price for pair stored with key
#[cfg(not(tarpaulin_include))]
pub fn get_arithmetic_twap_now(deps: Deps, pool_id: u64) -> StdResult<Decimal> {
    KEY_PRICES.load(deps.storage, pool_id)
}

#[cfg(not(tarpaulin_include))]
pub fn get_denom_authority(deps: Deps, denom: String) -> StdResult<Option<String>> {
    let querier = TokenfactoryQuerier::new(&deps.querier);

    let result = querier
        .denom_authority_metadata(denom)
        .unwrap()
        .authority_metadata;

    match result {
        Some(DenomAuthorityMetadata { admin }) => Ok(Some(admin)),
        None => Ok(None),
    }
}

use crate::query::{get_arithmetic_twap_now, get_denom_authority};

use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use margined_protocol::query::{ExecuteMsg, InstantiateMsg, QueryMsg};

// version info for migration info
pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(
        deps.storage,
        format!("crates.io:{CONTRACT_NAME}"),
        CONTRACT_VERSION,
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: ExecuteMsg,
) -> StdResult<Response> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetArithmeticTwapToNow {
            pool_id,
            base_asset,
            quote_asset,
            start_time,
        } => to_binary(&get_arithmetic_twap_now(
            deps,
            pool_id,
            base_asset,
            quote_asset,
            start_time,
        )?),
        QueryMsg::GetDenomAuthority { denom } => to_binary(&get_denom_authority(deps, denom)?),
    }
}

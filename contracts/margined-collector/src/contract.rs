use crate::{
    handle::{add_token, remove_token, send_token, update_whitelist},
    query::{
        query_all_token, query_is_token, query_owner, query_token_list_length, query_whitelist,
    },
    state::{OWNER, OWNERSHIP_PROPOSAL, WHITELIST_ADDRESS},
};

use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};
use cw2::set_contract_version;
use margined_common::{
    errors::ContractError,
    ownership::{
        get_ownership_proposal, handle_claim_ownership, handle_ownership_proposal,
        handle_ownership_proposal_rejection,
    },
};
use margined_protocol::collector::{ExecuteMsg, InstantiateMsg, QueryMsg};

pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(
        deps.storage,
        format!("crates.io:{CONTRACT_NAME}"),
        CONTRACT_VERSION,
    )?;

    WHITELIST_ADDRESS.save(deps.storage, &info.sender)?;

    OWNER.set(deps, Some(info.sender))?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddToken { token } => add_token(deps, info, token),
        ExecuteMsg::RemoveToken { token } => remove_token(deps, info, token),
        ExecuteMsg::UpdateWhitelist { address } => update_whitelist(deps, info, address),
        ExecuteMsg::SendToken {
            token,
            amount,
            recipient,
        } => send_token(deps.as_ref(), env, info, token, amount, recipient),
        ExecuteMsg::ProposeNewOwner {
            new_owner,
            duration,
        } => handle_ownership_proposal(
            deps,
            info,
            env,
            new_owner,
            duration,
            OWNER,
            OWNERSHIP_PROPOSAL,
        ),
        ExecuteMsg::RejectOwner {} => {
            handle_ownership_proposal_rejection(deps, info, OWNER, OWNERSHIP_PROPOSAL)
        }
        ExecuteMsg::ClaimOwnership {} => {
            handle_claim_ownership(deps, info, env, OWNER, OWNERSHIP_PROPOSAL)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Owner {} => {
            to_binary(&query_owner(deps).map_err(|err| StdError::generic_err(err.to_string()))?)
        }
        QueryMsg::GetWhitelist {} => to_binary(&query_whitelist(deps)?),
        QueryMsg::IsToken { token } => to_binary(&query_is_token(deps, token)?),
        QueryMsg::GetTokenList { limit } => to_binary(&query_all_token(deps, limit)?),
        QueryMsg::GetTokenLength {} => to_binary(&query_token_list_length(deps)?),
        QueryMsg::GetOwnershipProposal {} => {
            to_binary(&get_ownership_proposal(deps, OWNERSHIP_PROPOSAL)?)
        }
    }
}

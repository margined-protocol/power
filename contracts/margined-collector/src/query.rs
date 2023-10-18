use cosmwasm_std::{Addr, Deps, StdResult};
use margined_common::errors::ContractError;
use margined_protocol::collector::{
    AllTokenResponse, TokenLengthResponse, TokenResponse, WhitelistResponse,
};

use crate::state::{is_token, read_token_list, OWNER, TOKEN_LIMIT, WHITELIST_ADDRESS};

const DEFAULT_PAGINATION_LIMIT: u32 = 10u32;
const MAX_PAGINATION_LIMIT: u32 = TOKEN_LIMIT as u32;

pub fn query_owner(deps: Deps) -> Result<Addr, ContractError> {
    if let Some(owner) = OWNER.get(deps)? {
        Ok(owner)
    } else {
        Err(ContractError::NoOwner {})
    }
}

pub fn query_whitelist(deps: Deps) -> StdResult<WhitelistResponse> {
    let address = WHITELIST_ADDRESS.may_load(deps.storage)?;

    Ok(WhitelistResponse { address })
}

pub fn query_is_token(deps: Deps, token: String) -> StdResult<TokenResponse> {
    let token_bool = is_token(deps.storage, token);

    Ok(TokenResponse {
        is_token: token_bool,
    })
}

pub fn query_all_token(deps: Deps, limit: Option<u32>) -> StdResult<AllTokenResponse> {
    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT) as usize;

    let list = read_token_list(deps, limit)?;
    Ok(AllTokenResponse { token_list: list })
}

pub fn query_token_list_length(deps: Deps) -> StdResult<TokenLengthResponse> {
    let limit = TOKEN_LIMIT;

    let list_length = read_token_list(deps, limit)?.len();
    Ok(TokenLengthResponse {
        length: list_length,
    })
}

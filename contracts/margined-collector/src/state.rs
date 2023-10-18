use cosmwasm_std::{Addr, Deps, DepsMut, StdError::GenericErr, StdResult, Storage};
use cw_controllers::Admin;
use cw_storage_plus::Item;
use margined_common::ownership::OwnerProposal;

pub const OWNER: Admin = Admin::new("owner");
pub const OWNERSHIP_PROPOSAL: Item<OwnerProposal> = Item::new("ownership_proposals");

pub const WHITELIST_ADDRESS: Item<Addr> = Item::new("whitelist-address");
pub const TOKEN_LIST: Item<Vec<String>> = Item::new("token-list");
pub const TOKEN_LIMIT: usize = 3usize;

pub fn save_token(deps: DepsMut, denom: String) -> StdResult<()> {
    let mut token_list = match TOKEN_LIST.may_load(deps.storage)? {
        None => vec![],
        Some(list) => list,
    };

    if token_list.contains(&denom) {
        return Err(GenericErr {
            msg: "This token is already added".to_string(),
        });
    };

    if token_list.len() >= TOKEN_LIMIT {
        return Err(GenericErr {
            msg: "The token capacity is already reached".to_string(),
        });
    };

    token_list.push(denom);

    TOKEN_LIST.save(deps.storage, &token_list)
}

pub fn read_token_list(deps: Deps, limit: usize) -> StdResult<Vec<String>> {
    let list = match TOKEN_LIST.may_load(deps.storage)? {
        None => Vec::new(),
        Some(list) => {
            let take = limit.min(list.len());
            list[..take].to_vec()
        }
    };

    Ok(list)
}

pub fn is_token(storage: &dyn Storage, token: String) -> bool {
    match TOKEN_LIST.may_load(storage).unwrap() {
        None => false,
        Some(list) => list.contains(&token),
    }
}

pub fn remove_token(deps: DepsMut, denom: String) -> StdResult<()> {
    let mut token_list = match TOKEN_LIST.may_load(deps.storage)? {
        None => {
            return Err(GenericErr {
                msg: "No tokens are stored".to_string(),
            })
        }
        Some(value) => value,
    };

    if !token_list.contains(&denom) {
        return Err(GenericErr {
            msg: "This token has not been added".to_string(),
        });
    }

    let index = token_list
        .clone()
        .iter()
        .position(|x| x.eq(&denom))
        .unwrap();
    token_list.swap_remove(index);

    TOKEN_LIST.save(deps.storage, &token_list)
}

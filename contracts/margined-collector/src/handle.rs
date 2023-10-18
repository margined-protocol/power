use cosmwasm_std::{
    ensure, BankMsg, Coin, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Response, Uint128,
};
use margined_common::{common::check_denom_metadata, errors::ContractError};
use osmosis_std::types::cosmos::bank::v1beta1::BankQuerier;
use std::str::FromStr;

use crate::state::{
    is_token, remove_token as remove_token_from_list, save_token, OWNER, WHITELIST_ADDRESS,
};

pub fn add_token(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
) -> Result<Response, ContractError> {
    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    check_denom_metadata(deps.as_ref(), &token)
        .map_err(|_| ContractError::InvalidDenom(token.clone()))?;

    save_token(deps, token.clone())?;

    Ok(Response::default()
        .add_event(Event::new("add_token").add_attributes([("denom", token.as_str())])))
}

pub fn remove_token(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
) -> Result<Response, ContractError> {
    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    remove_token_from_list(deps, token.clone())?;

    Ok(Response::default()
        .add_event(Event::new("remove_token").add_attributes([("denom", token.as_str())])))
}

pub fn update_whitelist(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    let address = deps.api.addr_validate(&address)?;

    WHITELIST_ADDRESS.save(deps.storage, &address)?;

    Ok(Response::default()
        .add_event(Event::new("update_whitelist").add_attributes([("address", address.as_str())])))
}

pub fn send_token(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    token: String,
    amount: Uint128,
    recipient: String,
) -> Result<Response, ContractError> {
    if amount.is_zero() {
        return Err(ContractError::ZeroTransfer {});
    }

    if !OWNER.is_admin(deps, &info.sender)?
        && info.sender != WHITELIST_ADDRESS.load(deps.storage)?
    {
        return Err(ContractError::Unauthorized {});
    }

    let valid_recipient = deps.api.addr_validate(&recipient)?;

    if !is_token(deps.storage, token.clone()) {
        return Err(ContractError::TokenUnsupported(token));
    };

    let bank = BankQuerier::new(&deps.querier);

    let balance = match bank
        .balance(env.contract.address.to_string(), token.clone())
        .unwrap()
        .balance
    {
        Some(balance) => Uint128::from_str(balance.amount.as_str()).unwrap(),
        None => Uint128::zero(),
    };

    if balance < amount {
        return Err(ContractError::InsufficientBalance {});
    }

    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: valid_recipient.to_string(),
        amount: vec![Coin {
            denom: token.clone(),
            amount,
        }],
    });

    Ok(Response::default()
        .add_message(msg)
        .add_event(Event::new("send_token").add_attributes([
            ("amount", &amount.to_string()),
            ("denom", &token),
            ("recipient", &info.sender.to_string()),
        ])))
}

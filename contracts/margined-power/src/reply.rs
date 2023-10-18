use crate::{
    contract::OPEN_SHORT_SWAP_REPLY_ID,
    helpers::{create_swap_exact_amount_in_message, parse_response_result_data},
    operations::burn,
    queries::{get_balance, get_total_supply},
    state::{Config, CONFIG, TMP_CACHE},
};

use cosmwasm_std::{
    coin, coins, BankMsg, CosmosMsg, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, SubMsg,
    Uint128,
};
use margined_common::errors::ContractError;
use osmosis_std::types::{
    cosmos::bank::v1beta1::MsgSend,
    cosmos::base::v1beta1::Coin,
    osmosis::poolmanager::v1beta1::{MsgSwapExactAmountInResponse, MsgSwapExactAmountOutResponse},
};
use std::str::FromStr;

pub fn handle_open_short_reply(
    deps: DepsMut,
    env: Env,
    _msg: Reply,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let current_total_supply = get_total_supply(deps.as_ref(), config.power_denom.clone())?;
    let cache = TMP_CACHE.load(deps.storage)?;

    let prev_total_supply = cache.total_supply.unwrap_or(Uint128::zero());

    let mint_amount = current_total_supply.checked_sub(prev_total_supply).unwrap();

    let swap_msg = create_swap_exact_amount_in_message(
        env.contract.address.to_string(),
        config.power_pool.id,
        config.power_denom,
        config.base_denom,
        mint_amount.to_string(),
    );

    let swap_submsg: SubMsg = SubMsg {
        id: OPEN_SHORT_SWAP_REPLY_ID,
        msg: swap_msg.into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    Ok(Response::new().add_submessage(swap_submsg))
}

pub fn handle_open_short_swap_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    let data = parse_response_result_data(msg.result)?;

    let response: MsgSwapExactAmountInResponse = data.try_into().map_err(ContractError::Std)?;

    let config: Config = CONFIG.load(deps.storage)?;
    let cache = TMP_CACHE.load(deps.storage)?;

    let send_msg = MsgSend {
        from_address: env.contract.address.to_string(),
        to_address: cache.sender.unwrap().to_string(),
        amount: vec![Coin {
            amount: response.token_out_amount,
            denom: config.base_denom,
        }],
    };

    TMP_CACHE.remove(deps.storage);

    Ok(Response::new().add_message(send_msg))
}

pub fn handle_close_short_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    let data = parse_response_result_data(msg.result)?;

    let response: MsgSwapExactAmountOutResponse = data.try_into().map_err(ContractError::Std)?;

    let token_in_amount = Uint128::from_str(&response.token_in_amount).unwrap();

    let config: Config = CONFIG.load(deps.storage)?;
    let current_balance = get_balance(
        deps.as_ref(),
        env.contract.address.to_string(),
        config.power_denom.clone(),
    )?;

    let cache = TMP_CACHE.load(deps.storage)?;

    let sender = cache.sender.unwrap();
    let burn_amount = current_balance.checked_sub(cache.balance.unwrap()).unwrap();

    let info = MessageInfo {
        sender: sender.clone(),
        funds: coins(burn_amount.u128(), config.power_denom),
    };

    let vault_id = match cache.vault_id {
        Some(id) => id,
        None => {
            return Err(ContractError::VaultDoesNotExist {});
        }
    };

    TMP_CACHE.remove(deps.storage);

    let mut response = burn(deps, env, info, cache.amount_to_withdraw, vault_id)?;

    if cache.amount_to_swap.unwrap() > token_in_amount {
        let refund = cache
            .amount_to_swap
            .unwrap()
            .checked_sub(token_in_amount)
            .unwrap();

        let msg_transfer = CosmosMsg::Bank(BankMsg::Send {
            to_address: sender.to_string(),
            amount: vec![coin(refund.u128(), config.base_denom)],
        });

        response = response.add_message(msg_transfer);
    };

    Ok(response)
}

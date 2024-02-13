use crate::{
    contract::{FLASH_LIQUIDATE_REPLY_ID, OPEN_SHORT_SWAP_REPLY_ID},
    helpers::{get_collateral_from_vault_type, get_min_amount_out_from_slippage, PoolParams},
    operations::{burn, liquidate},
    queries::{get_balance, get_total_supply},
    state::{Config, TmpCacheValues, CONFIG, TMP_CACHE},
    utils::parse_response_result_data,
    vault::get_vault_type,
};

use cosmwasm_std::{
    coin, coins, BankMsg, CosmosMsg, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, SubMsg,
    Uint128,
};
use margined_common::{
    errors::ContractError,
    messages::{create_send_message, create_swap_exact_amount_in_message},
};
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

    let current_total_supply = get_total_supply(deps.as_ref(), config.power_asset.denom.clone())?;
    let cache = TMP_CACHE.load(deps.storage)?;

    let prev_total_supply = cache.total_supply.unwrap_or(Uint128::zero());

    let mint_amount = current_total_supply.checked_sub(prev_total_supply).unwrap();

    let token_out_min_amount = get_min_amount_out_from_slippage(
        deps.as_ref(),
        mint_amount,
        cache.slippage,
        PoolParams {
            id: config.power_pool.id,
            base_denom: config.power_asset.denom.clone(),
            quote_denom: config.base_asset.denom.clone(),
            base_decimal_places: config.power_asset.decimals,
            quote_decimal_places: config.base_asset.decimals,
        },
    )?;

    let swap_msg = create_swap_exact_amount_in_message(
        env.contract.address.to_string(),
        config.power_pool.id,
        config.power_asset.denom,
        config.base_asset.denom,
        mint_amount.to_string(),
        token_out_min_amount.to_string(),
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

    let send_msg = create_send_message(
        env.contract.address.to_string(),
        cache.sender.unwrap().to_string(),
        response.token_out_amount,
        config.base_asset.denom,
    );

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

    let config = CONFIG.load(deps.storage)?;

    let current_balance = get_balance(
        deps.as_ref(),
        env.contract.address.to_string(),
        config.power_asset.denom.clone(),
    )?;

    let cache = TMP_CACHE.load(deps.storage)?;

    let sender = cache.sender.unwrap();
    let burn_amount = current_balance.checked_sub(cache.balance.unwrap()).unwrap();

    let info = MessageInfo {
        sender: sender.clone(),
        funds: coins(burn_amount.u128(), config.clone().power_asset.denom),
    };

    let vault_id = cache
        .vault_id
        .ok_or(ContractError::VaultDoesNotExist(0u64))?;

    let expected_vault_type = get_vault_type(deps.storage, vault_id)
        .map_err(|_| ContractError::VaultDoesNotExist(vault_id))?;

    TMP_CACHE.remove(deps.storage);

    let mut response = burn(deps, env, info, cache.amount_to_withdraw, vault_id)?;

    if cache.amount_to_swap.unwrap() > token_in_amount {
        let withdrawal_asset = get_collateral_from_vault_type(&config, expected_vault_type)?;

        let refund = cache
            .amount_to_swap
            .unwrap()
            .checked_sub(token_in_amount)
            .unwrap();

        let msg_transfer = CosmosMsg::Bank(BankMsg::Send {
            to_address: sender.to_string(),
            amount: vec![coin(refund.u128(), withdrawal_asset.denom)],
        });

        response = response.add_message(msg_transfer);
    };

    Ok(response)
}

pub fn handle_flash_liquidate_swap_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let cache = TMP_CACHE.load(deps.storage)?;

    let data = parse_response_result_data(msg.result)?;
    let response: MsgSwapExactAmountInResponse = data.try_into().map_err(ContractError::Std)?;

    let send_msg = MsgSend {
        from_address: env.contract.address.to_string(),
        to_address: cache.sender.unwrap().to_string(),
        amount: vec![Coin {
            amount: response.token_out_amount.clone(),
            denom: config.power_asset.denom,
        }],
    };

    let send_submsg: SubMsg = SubMsg {
        id: FLASH_LIQUIDATE_REPLY_ID,
        msg: send_msg.into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    TMP_CACHE.update(
        deps.storage,
        |mut cache: TmpCacheValues| -> Result<_, ContractError> {
            cache.amount_to_burn = Some(Uint128::from_str(&response.token_out_amount)?);
            Ok(cache)
        },
    )?;

    Ok(Response::new().add_submessage(send_submsg))
}

pub fn handle_flash_liquidate_reply(
    deps: DepsMut,
    env: Env,
    _msg: Reply,
) -> Result<Response, ContractError> {
    let cache = TMP_CACHE.load(deps.storage)?;

    let sender = cache.sender.unwrap();
    let amount_to_burn = cache.amount_to_burn.unwrap();
    let vault_id = cache.vault_id.unwrap();

    TMP_CACHE.remove(deps.storage);

    liquidate(
        deps,
        env,
        MessageInfo {
            sender,
            funds: vec![],
        },
        amount_to_burn,
        vault_id,
        "flash-liquidation".to_string(),
        cache.amount_to_swap,
    )
}

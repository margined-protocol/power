use crate::{
    distributor::update_rewards,
    helper::create_distribute_message_and_update_response,
    state::{UserStake, CONFIG, OWNER, STATE, TOTAL_STAKED, USER_STAKE},
};

use cosmwasm_std::{ensure, DepsMut, Env, Event, MessageInfo, Response, StdResult, Uint128};
use cw_utils::{must_pay, nonpayable};
use margined_common::errors::ContractError;
use osmosis_std::types::{cosmos::bank::v1beta1::MsgSend, cosmos::base::v1beta1::Coin};

pub fn handle_update_config(
    deps: DepsMut,
    info: MessageInfo,
    tokens_per_interval: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    let event = Event::new("update_config");

    if let Some(tokens_per_interval) = tokens_per_interval {
        config.tokens_per_interval = tokens_per_interval;

        event
            .clone()
            .add_attribute("Tokens per interval", tokens_per_interval);
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default().add_event(event))
}

pub fn handle_update_rewards(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let (_, rewards) = update_rewards(deps, env.clone(), env.contract.address.clone())?;

    let response = create_distribute_message_and_update_response(
        Response::new(),
        config.fee_collector.to_string(),
        config.reward_denom,
        rewards,
        env.contract.address.to_string(),
    )
    .unwrap();

    Ok(response.add_event(Event::new("update_rewards")))
}

pub fn handle_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    if !state.is_open {
        return Err(ContractError::Paused {});
    }
    state.is_open = false;

    STATE.save(deps.storage, &state)?;

    Ok(Response::default().add_event(Event::new("paused")))
}

pub fn handle_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );

    if state.is_open {
        return Err(ContractError::NotPaused {});
    }

    state.is_open = true;

    STATE.save(deps.storage, &state)?;

    Ok(Response::default().add_event(Event::new("unpaused")))
}

pub fn handle_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    let sender = info.sender.clone();

    nonpayable(&info).map_err(|_| ContractError::InvalidFunds {})?;

    ensure!(state.is_open, ContractError::Paused {});

    let recipient = match recipient {
        Some(recipient) => {
            deps.api.addr_validate(recipient.as_str())?;
            recipient
        }
        None => sender.to_string(),
    };

    let (deps, rewards) = update_rewards(deps, env.clone(), sender.clone())?;

    let mut claimable_amount = Uint128::zero();
    USER_STAKE.update(deps.storage, sender.clone(), |res| -> StdResult<_> {
        let mut stake = match res {
            Some(stake) => stake,
            None => UserStake::default(),
        };

        claimable_amount = stake.claimable_rewards;
        stake.claimable_rewards = Uint128::zero();

        Ok(stake)
    })?;

    let mut response = create_distribute_message_and_update_response(
        Response::new(),
        config.fee_collector.to_string(),
        config.reward_denom.clone(),
        rewards,
        env.contract.address.to_string(),
    )
    .unwrap();

    if !claimable_amount.is_zero() {
        let msg_claim = MsgSend {
            from_address: env.contract.address.to_string(),
            to_address: recipient,
            amount: vec![Coin {
                denom: config.reward_denom,
                amount: claimable_amount.into(),
            }],
        };
        response = response.add_message(msg_claim);
    }

    Ok(response.add_event(Event::new("claim").add_attributes([
        ("amount", &claimable_amount.to_string()),
        ("user", &sender.to_string()),
    ])))
}

pub fn handle_stake(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    ensure!(state.is_open, ContractError::Paused {});

    let sent_funds: Uint128 =
        must_pay(&info, &config.deposit_denom).map_err(|_| ContractError::InvalidFunds {})?;

    let sender = info.sender;

    let (deps, rewards) = update_rewards(deps, env.clone(), sender.clone())?;

    USER_STAKE.update(deps.storage, sender.clone(), |res| -> StdResult<_> {
        let mut stake = match res {
            Some(stake) => stake,
            None => UserStake::default(),
        };

        stake.staked_amounts = stake.staked_amounts.checked_add(sent_funds).unwrap();

        Ok(stake)
    })?;

    TOTAL_STAKED
        .update(deps.storage, |balance| -> StdResult<Uint128> {
            Ok(balance.checked_add(sent_funds).unwrap())
        })
        .unwrap();

    let response = create_distribute_message_and_update_response(
        Response::new(),
        config.fee_collector.to_string(),
        config.reward_denom,
        rewards,
        env.contract.address.to_string(),
    )
    .unwrap();

    Ok(response.add_event(Event::new("stake").add_attributes([
        ("amount", &sent_funds.to_string()),
        ("user", &sender.to_string()),
    ])))
}

pub fn handle_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    ensure!(state.is_open, ContractError::Paused {});

    let sender = info.sender.clone();

    nonpayable(&info).map_err(|_| ContractError::InvalidFunds {})?;

    let (deps, rewards) = update_rewards(deps, env.clone(), sender.clone())?;

    USER_STAKE.update(deps.storage, sender.clone(), |res| -> StdResult<_> {
        let mut stake = match res {
            Some(stake) => stake,
            None => UserStake::default(),
        };

        stake.staked_amounts = stake.staked_amounts.checked_sub(amount).unwrap();

        Ok(stake)
    })?;

    TOTAL_STAKED
        .update(deps.storage, |balance| -> StdResult<Uint128> {
            Ok(balance.checked_sub(amount).unwrap())
        })
        .unwrap();

    let response = create_distribute_message_and_update_response(
        Response::new(),
        config.fee_collector.to_string(),
        config.reward_denom,
        rewards,
        env.contract.address.to_string(),
    )
    .unwrap();

    let msg_unstake = MsgSend {
        from_address: env.contract.address.to_string(),
        to_address: sender.to_string(),
        amount: vec![Coin {
            denom: config.deposit_denom,
            amount: amount.into(),
        }],
    };

    Ok(response
        .add_message(msg_unstake)
        .add_event(Event::new("unstake").add_attributes([
            ("amount", &amount.to_string()),
            ("user", &sender.to_string()),
        ])))
}

use crate::state::{CONFIG, OWNER, REWARDS_PER_TOKEN, STATE, TOTAL_STAKED, USER_STAKE};

use cosmwasm_std::{Addr, Deps, Env, StdResult, Uint128};
use margined_common::errors::ContractError;
use margined_protocol::staking::{
    ConfigResponse, StateResponse, TotalStakedResponse, UserStakedResponse,
};

pub fn query_owner(deps: Deps) -> Result<Addr, ContractError> {
    if let Some(owner) = OWNER.get(deps)? {
        Ok(owner)
    } else {
        Err(ContractError::NoOwner {})
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage).unwrap();

    Ok(ConfigResponse {
        fee_collector: config.fee_collector,
        deposit_denom: config.deposit_denom,
        deposit_decimals: config.deposit_decimals,
        reward_denom: config.reward_denom,
        reward_decimals: config.reward_decimals,
        tokens_per_interval: config.tokens_per_interval,
    })
}

pub fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage).unwrap();

    Ok(StateResponse {
        is_open: state.is_open,
        last_distribution: state.last_distribution,
    })
}

pub fn query_total_staked_amount(deps: Deps) -> StdResult<TotalStakedResponse> {
    let total_staked = TOTAL_STAKED.load(deps.storage)?;

    Ok(TotalStakedResponse {
        amount: total_staked,
    })
}

pub fn query_user_staked_amount(deps: Deps, address: String) -> StdResult<UserStakedResponse> {
    let user = deps.api.addr_validate(&address)?;
    let user_stake = USER_STAKE.may_load(deps.storage, user)?;

    match user_stake {
        Some(stake) => Ok(UserStakedResponse {
            staked_amounts: stake.staked_amounts,
            claimable_rewards: stake.claimable_rewards,
            previous_cumulative_rewards_per_token: stake.previous_cumulative_rewards_per_token,
            cumulative_rewards: stake.cumulative_rewards,
        }),
        None => Ok(UserStakedResponse {
            staked_amounts: Uint128::zero(),
            claimable_rewards: Uint128::zero(),
            previous_cumulative_rewards_per_token: Uint128::zero(),
            cumulative_rewards: Uint128::zero(),
        }),
    }
}

pub fn query_pending_rewards(deps: Deps, env: Env) -> StdResult<Uint128> {
    let state = STATE.load(deps.storage).unwrap();
    let config = CONFIG.load(deps.storage).unwrap();

    if state.last_distribution == env.block.time {
        return Ok(Uint128::zero());
    };

    let delta =
        Uint128::from((env.block.time.seconds() - state.last_distribution.seconds()) as u128);

    let pending_rewards = delta.checked_mul(config.tokens_per_interval).unwrap();

    Ok(pending_rewards)
}

pub fn query_claimable(deps: Deps, env: Env, address: String) -> StdResult<Uint128> {
    let config = CONFIG.load(deps.storage).unwrap();
    let decimal_places = 10u128.pow(config.reward_decimals);

    let user = deps.api.addr_validate(&address)?;

    let stake = USER_STAKE.load(deps.storage, user).unwrap_or_default();
    if stake.staked_amounts.is_zero() {
        return Ok(Uint128::zero());
    };

    let pending_rewards = query_pending_rewards(deps, env)?
        .checked_mul(decimal_places.into())
        .unwrap();

    let total_staked = TOTAL_STAKED.load(deps.storage)?;
    let rewards_per_token = REWARDS_PER_TOKEN.load(deps.storage)?;

    let next_reward_per_token = rewards_per_token
        .checked_add(pending_rewards.checked_div(total_staked).unwrap())
        .unwrap();

    let latest_rewards = stake
        .staked_amounts
        .checked_mul(
            next_reward_per_token
                .checked_sub(stake.previous_cumulative_rewards_per_token)
                .unwrap(),
        )
        .unwrap()
        .checked_div(decimal_places.into())
        .unwrap();

    Ok(stake.claimable_rewards.checked_add(latest_rewards).unwrap())
}

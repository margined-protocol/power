use crate::{
    helpers::{calculate_denormalized_mark, calculate_index, wrapped_pow},
    state::STATE,
};

use cosmwasm_std::{Decimal, Deps, DepsMut, Env, StdResult};
use margined_protocol::power::FUNDING_PERIOD;
use num::Zero;

pub const MAX_TWAP_PERIOD: u64 = 48 * 60 * 60; // TWAP from pool can be no longer than 48 hours

pub fn apply_funding_rate(deps: DepsMut, env: Env) -> StdResult<Decimal> {
    let state = STATE.load(deps.storage).unwrap();

    // normalisation factor is only updated onces per block
    if state.last_funding_update.seconds() == env.block.time.seconds() {
        return Ok(state.normalisation_factor);
    }

    let normalisation_factor = calculate_normalisation_factor(deps.as_ref(), env.clone())?;

    STATE.update(deps.storage, |mut state| -> StdResult<_> {
        state.normalisation_factor = normalisation_factor;
        state.last_funding_update = env.block.time;

        Ok(state)
    })?;

    Ok(normalisation_factor)
}

pub fn calculate_normalisation_factor(deps: Deps, env: Env) -> StdResult<Decimal> {
    let state = STATE.load(deps.storage).unwrap();

    let funding_period = env
        .block
        .time
        .minus_seconds(state.last_funding_update.seconds());

    let period = funding_period.seconds().min(MAX_TWAP_PERIOD);

    // NOTE: pools must have a TWAP available for past 48 hours
    let start_time = if period < MAX_TWAP_PERIOD {
        state.last_funding_update
    } else {
        env.block.time.minus_seconds(MAX_TWAP_PERIOD)
    };

    if period.is_zero() {
        return Ok(state.normalisation_factor);
    };

    let mut mark =
        calculate_denormalized_mark(deps, start_time, state.normalisation_factor).unwrap();

    let index = calculate_index(deps, start_time).unwrap();

    let r_funding = Decimal::from_ratio(funding_period.seconds(), FUNDING_PERIOD);

    // check that the mark price is between upper and lower bounds of 140% and 80% of the index price
    let lower_bound = index * Decimal::percent(80);
    let upper_bound = index * Decimal::percent(140);

    if mark < lower_bound {
        mark = lower_bound;
    } else if mark > upper_bound {
        mark = upper_bound;
    };

    // normFactor(new) = multiplier * normFactor(old)
    // multiplier = (index/mark)^rFunding
    let base = index.checked_div(mark).unwrap();
    let multiplier = wrapped_pow(base, r_funding).unwrap();

    Ok(multiplier * state.normalisation_factor)
}

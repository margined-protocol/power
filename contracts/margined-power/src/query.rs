use crate::{
    funding::calculate_normalisation_factor,
    helpers::calculate_denormalized_mark,
    queries::{get_pool_twap, get_scaled_pool_twap},
    state::{CONFIG, OWNER, STATE},
    vault::{is_vault_safe, VAULTS, VAULTS_COUNTER},
};

use cosmwasm_std::{Addr, Decimal, Deps, Env, Order, StdError, StdResult, Timestamp};
use cw_storage_plus::Bound;
use margined_common::errors::ContractError;
use margined_protocol::power::{ConfigResponse, StateResponse, UserVaultsResponse, VaultResponse};

const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 50;

fn calculate_start_time(env: Env, period: u64) -> Timestamp {
    env.block.time.minus_seconds(period)
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage).unwrap();

    Ok(ConfigResponse {
        query_contract: config.query_contract,
        fee_pool_contract: config.fee_pool_contract,
        fee_rate: config.fee_rate,
        power_denom: config.power_denom,
        base_denom: config.base_denom,
        base_pool: config.base_pool,
        power_pool: config.power_pool,
        funding_period: config.funding_period,
        base_decimals: config.base_decimals,
        power_decimals: config.power_decimals,
    })
}

pub fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage).unwrap();

    Ok(StateResponse {
        is_open: state.is_open,
        is_paused: state.is_paused,
        last_pause: state.last_pause,
        normalisation_factor: state.normalisation_factor,
        last_funding_update: state.last_funding_update,
    })
}

pub fn query_owner(deps: Deps) -> Result<Addr, ContractError> {
    if let Some(owner) = OWNER.get(deps)? {
        Ok(owner)
    } else {
        Err(ContractError::NoOwner {})
    }
}

pub fn get_normalisation_factor(deps: Deps, env: Env) -> StdResult<Decimal> {
    let res = calculate_normalisation_factor(deps, env)?;

    Ok(res)
}

pub fn get_index(deps: Deps, env: Env, period: u64) -> StdResult<Decimal> {
    let config = CONFIG.load(deps.storage).unwrap();

    let start_time = calculate_start_time(env, period);

    let quote_price = get_scaled_pool_twap(
        &deps,
        config.base_pool.id,
        config.base_denom.clone(),
        config.base_pool.quote_denom,
        start_time,
    )
    .unwrap();

    let index = quote_price.checked_mul(quote_price).unwrap();

    Ok(index)
}

pub fn get_unscaled_index(deps: Deps, env: Env, period: u64) -> StdResult<Decimal> {
    let config = CONFIG.load(deps.storage).unwrap();

    let start_time = calculate_start_time(env, period);

    let quote_price = get_pool_twap(
        &deps,
        config.base_pool.id,
        config.base_denom.clone(),
        config.base_pool.quote_denom,
        start_time,
    )
    .unwrap();

    let index = quote_price.checked_mul(quote_price).unwrap();

    Ok(index)
}

pub fn get_denormalised_mark(deps: Deps, env: Env, period: u64) -> StdResult<Decimal> {
    let start_time = calculate_start_time(env.clone(), period);

    let normalisation_factor = calculate_normalisation_factor(deps, env)?;

    let result = calculate_denormalized_mark(deps, start_time, normalisation_factor)?;

    Ok(result)
}

pub fn get_denormalised_mark_for_funding(deps: Deps, env: Env, period: u64) -> StdResult<Decimal> {
    let start_time = calculate_start_time(env, period);

    let state = STATE.load(deps.storage).unwrap();

    let result = calculate_denormalized_mark(deps, start_time, state.normalisation_factor)?;

    Ok(result)
}

pub fn get_check_vault(deps: Deps, env: Env, vault_id: u64) -> StdResult<bool> {
    let config = CONFIG.load(deps.storage).unwrap();
    let normalisation_factor = calculate_normalisation_factor(deps, env.clone())?;

    let result =
        is_vault_safe(deps, config, vault_id, normalisation_factor, env.block.time).unwrap();

    Ok(result)
}

pub fn get_vault(deps: Deps, vault_id: u64) -> StdResult<VaultResponse> {
    let vault = VAULTS.may_load(deps.storage, &vault_id)?;
    if let Some(vault) = vault {
        Ok(VaultResponse {
            operator: vault.operator,
            collateral: vault.collateral,
            short_amount: vault.short_amount,
        })
    } else {
        Err(StdError::generic_err("Vault not found"))
    }
}

pub fn get_next_vault_id(deps: Deps) -> StdResult<u64> {
    let current_index = VAULTS_COUNTER.may_load(deps.storage)?.unwrap_or(0);

    Ok(current_index + 1u64)
}

pub fn get_user_vaults(
    deps: Deps,
    owner: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<UserVaultsResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|s| Bound::ExclusiveRaw(s.to_be_bytes().into()));

    let owner_addr = deps.api.addr_validate(&owner)?;
    let vaults: Vec<u64> = VAULTS
        .idx
        .owner
        .prefix(owner_addr)
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .collect::<StdResult<Vec<_>>>()?;

    Ok(UserVaultsResponse { vaults })
}

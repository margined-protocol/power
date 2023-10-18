use crate::{
    handle::{
        handle_apply_funding, handle_burn_power_perp, handle_close_short, handle_deposit,
        handle_liquidation, handle_mint_power_perp, handle_open_contract, handle_open_short,
        handle_pause, handle_unpause, handle_update_config, handle_withdrawal,
    },
    query::{
        get_check_vault, get_denormalised_mark, get_denormalised_mark_for_funding, get_index,
        get_next_vault_id, get_normalisation_factor, get_unscaled_index, get_user_vaults,
        get_vault, query_config, query_owner, query_state,
    },
    reply::{handle_close_short_reply, handle_open_short_reply, handle_open_short_swap_reply},
    state::{Config, State, CONFIG, OWNER, OWNERSHIP_PROPOSAL, STATE},
};

use cosmwasm_std::{
    entry_point, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdError, StdResult,
};
use cw2::set_contract_version;
use margined_common::{
    common::{check_denom_exists_in_pool, check_denom_metadata},
    errors::ContractError,
    ownership::{
        get_ownership_proposal, handle_claim_ownership, handle_ownership_proposal,
        handle_ownership_proposal_rejection,
    },
};
use margined_protocol::power::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, Pool, QueryMsg, FUNDING_PERIOD,
};
use std::str::FromStr;

pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const OPEN_SHORT_REPLY_ID: u64 = 1u64;
pub const OPEN_SHORT_SWAP_REPLY_ID: u64 = 2u64;
pub const CLOSE_SHORT_REPLY_ID: u64 = 3u64;
pub const CLOSE_SHORT_SWAP_REPLY_ID: u64 = 4u64;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(
        deps.storage,
        format!("crates.io:{CONTRACT_NAME}"),
        CONTRACT_VERSION,
    )?;

    let config = Config {
        fee_rate: Decimal::from_str(&msg.fee_rate)?,
        fee_pool_contract: deps.api.addr_validate(&msg.fee_pool)?,
        query_contract: deps.api.addr_validate(&msg.query_contract)?,
        power_denom: msg.power_denom.clone(),
        base_denom: msg.base_denom,
        base_pool: Pool {
            id: msg.base_pool_id,
            quote_denom: msg.base_pool_quote,
        },
        power_pool: Pool {
            id: msg.power_pool_id,
            quote_denom: msg.power_denom,
        },
        funding_period: FUNDING_PERIOD,
        base_decimals: msg.base_decimals,
        power_decimals: msg.power_decimals,
    };

    config.validate()?;

    CONFIG.save(deps.storage, &config)?;

    // validate denoms exist
    check_denom_metadata(deps.as_ref(), &config.base_denom)
        .map_err(|_| ContractError::InvalidDenom(config.base_denom.clone()))?;
    check_denom_metadata(deps.as_ref(), &config.power_denom)
        .map_err(|_| ContractError::InvalidDenom(config.power_denom.clone()))?;

    // validate denoms are present in pool
    check_denom_exists_in_pool(deps.as_ref(), config.base_pool.id, &config.base_denom)
        .map_err(ContractError::Std)?;
    check_denom_exists_in_pool(deps.as_ref(), config.power_pool.id, &config.base_denom)
        .map_err(ContractError::Std)?;
    check_denom_exists_in_pool(deps.as_ref(), config.power_pool.id, &config.power_denom)
        .map_err(ContractError::Std)?;

    STATE.save(
        deps.storage,
        &State {
            is_open: false,
            is_paused: true,
            last_pause: env.block.time,
            normalisation_factor: Decimal::one(),
            last_funding_update: env.block.time,
        },
    )?;

    OWNER.set(deps, Some(info.sender))?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        OPEN_SHORT_REPLY_ID => handle_open_short_reply(deps, env, msg),
        OPEN_SHORT_SWAP_REPLY_ID => handle_open_short_swap_reply(deps, env, msg),
        CLOSE_SHORT_REPLY_ID => handle_close_short_reply(deps, env, msg),

        _ => Err(ContractError::UnknownReplyId(msg.id)),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SetOpen {} => handle_open_contract(deps, env, info),
        ExecuteMsg::MintPowerPerp {
            amount,
            vault_id,
            rebase,
        } => handle_mint_power_perp(deps, env, info, amount, vault_id, rebase),
        ExecuteMsg::BurnPowerPerp {
            amount_to_withdraw,
            vault_id,
        } => handle_burn_power_perp(deps, env, info, amount_to_withdraw, vault_id),
        ExecuteMsg::OpenShort { amount, vault_id } => {
            handle_open_short(deps, env, info, amount, vault_id)
        }
        ExecuteMsg::CloseShort {
            amount_to_burn,
            amount_to_withdraw,
            vault_id,
        } => handle_close_short(
            deps,
            env,
            info,
            amount_to_burn,
            amount_to_withdraw,
            vault_id,
        ),
        ExecuteMsg::Deposit { vault_id } => handle_deposit(deps, env, info, vault_id),
        ExecuteMsg::Withdraw { amount, vault_id } => {
            handle_withdrawal(deps, env, info, amount, vault_id)
        }
        ExecuteMsg::Liquidate {
            vault_id,
            max_debt_amount,
        } => handle_liquidation(deps, env, info, max_debt_amount, vault_id),
        ExecuteMsg::ApplyFunding { .. } => handle_apply_funding(deps, env, info),
        ExecuteMsg::UpdateConfig { fee_rate, fee_pool } => {
            handle_update_config(deps, info, fee_rate, fee_pool)
        }
        ExecuteMsg::Pause {} => handle_pause(deps, env, info),
        ExecuteMsg::UnPause {} => handle_unpause(deps, env, info),
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
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Owner {} => {
            to_binary(&query_owner(deps).map_err(|err| StdError::generic_err(err.to_string()))?)
        }
        QueryMsg::GetNormalisationFactor {} => to_binary(&get_normalisation_factor(deps, env)?),
        QueryMsg::GetIndex { period } => to_binary(&get_index(deps, env, period)?),
        QueryMsg::GetUnscaledIndex { period } => to_binary(&get_unscaled_index(deps, env, period)?),
        QueryMsg::GetDenormalisedMark { period } => {
            to_binary(&get_denormalised_mark(deps, env, period)?)
        }
        QueryMsg::GetDenormalisedMarkFunding { period } => {
            to_binary(&get_denormalised_mark_for_funding(deps, env, period)?)
        }
        QueryMsg::GetVault { vault_id } => to_binary(&get_vault(deps, vault_id)?),
        QueryMsg::GetNextVaultId {} => to_binary(&get_next_vault_id(deps)?),
        QueryMsg::GetUserVaults {
            user,
            start_after,
            limit,
        } => to_binary(&get_user_vaults(deps, user, start_after, limit)?),
        QueryMsg::GetOwnershipProposal {} => {
            to_binary(&get_ownership_proposal(deps, OWNERSHIP_PROPOSAL)?)
        }
        QueryMsg::CheckVault { vault_id } => to_binary(&get_check_vault(deps, env, vault_id)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new())
}

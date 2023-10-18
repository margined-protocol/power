use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ensure, ensure_ne, Addr, Decimal, StdError, StdResult, Timestamp, Uint128};
use cw_controllers::Admin;
use cw_storage_plus::Item;
use margined_common::ownership::OwnerProposal;
use margined_protocol::power::{Pool, FUNDING_PERIOD};

pub const OWNER: Admin = Admin::new("owner");
pub const OWNERSHIP_PROPOSAL: Item<OwnerProposal> = Item::new("ownership_proposals");

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");

pub const TMP_CACHE: Item<TmpCacheValues> = Item::new("tmp_cache");

pub const LIQUIDATION_BOUNTY: Decimal = Decimal::raw(1_100_000_000_000_000_000u128); // 110%
pub const INDEX_SCALE: u128 = 10_000u128; // 1e4

pub const WEEK_IN_SECONDS: u64 = 7 * 24 * 60 * 60; // 24 hours
pub const TWAP_PERIOD: u64 = 420; // 420 seconds (7 minutes)

#[cw_serde]
pub struct Config {
    pub query_contract: Addr, // The contract that wraps the querier interface, useful for testing
    pub fee_pool_contract: Addr, // The address where fees are sent
    pub fee_rate: Decimal,    // The fee rate
    pub power_denom: String,  // Subdenom of the power perp native token, e.g. atom^2
    pub base_denom: String,   // Subdenom of the underlying native token, e.g. atom
    pub base_pool: Pool, // Pool of the underlying to quote, e.g. atom:usdc, defined on instantiation
    pub power_pool: Pool, // Pool of the underlying to power, e.g. atom:atom^2, defined during contract opening
    pub funding_period: u64, // Funding period in seconds
    pub base_decimals: u32, // Decimals of the underying token
    pub power_decimals: u32, // Decimals of the power perp token
}

impl Config {
    pub fn validate(&self) -> StdResult<()> {
        ensure!(
            self.base_decimals > 0 && self.base_decimals <= 18,
            StdError::generic_err("Invalid base decimals")
        );

        ensure!(
            self.power_decimals > 0 && self.power_decimals <= 18,
            StdError::generic_err("Invalid power decimals")
        );

        ensure!(
            self.fee_rate < Decimal::one(),
            StdError::generic_err("Invalid fee rate")
        );

        ensure!(
            self.funding_period > 0 && self.funding_period <= 2 * FUNDING_PERIOD,
            StdError::generic_err(format!(
                "Invalid funding period, must be between 0 and {} seconds",
                (2 * FUNDING_PERIOD)
            ))
        );

        ensure_ne!(
            self.power_denom,
            self.base_denom,
            StdError::generic_err("Invalid base and power denom must be different")
        );

        ensure_ne!(
            self.power_pool.id,
            self.base_pool.id,
            StdError::generic_err("Invalid base and power pool id must be different")
        );

        Ok(())
    }
}

#[cw_serde]
pub struct State {
    pub is_open: bool,                  // Whether the contract is open
    pub is_paused: bool,                // Whether the contract is paused
    pub last_pause: Timestamp,          // Last time contract was paused
    pub normalisation_factor: Decimal,  // Normalisation factor
    pub last_funding_update: Timestamp, // Last funding update timestamp
}

impl State {
    pub fn is_open_and_unpaused(&self) -> StdResult<()> {
        ensure!(
            self.is_open,
            StdError::generic_err("Cannot perform action as contract is not open")
        );

        ensure!(
            !self.is_paused,
            StdError::generic_err("Cannot perform action as contract is paused")
        );

        Ok(())
    }
}

#[cw_serde]
#[derive(Default)]
pub struct TmpCacheValues {
    pub total_supply: Option<Uint128>,
    pub balance: Option<Uint128>,
    pub amount_to_swap: Option<Uint128>,
    pub amount_to_withdraw: Option<Uint128>,
    pub sender: Option<Addr>,
    pub vault_id: Option<u64>,
}

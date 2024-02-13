use crate::utils::check_duplicates;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ensure, ensure_ne, Addr, Decimal, StdError, StdResult, Timestamp, Uint128};
use cw_controllers::Admin;
use cw_storage_plus::{Item, Map};
use margined_common::ownership::OwnerProposal;
use margined_protocol::power::{Asset, Pool, StakeAsset, FUNDING_PERIOD};

pub const OWNER: Admin = Admin::new("owner");
pub const OWNERSHIP_PROPOSAL: Item<OwnerProposal> = Item::new("ownership_proposals");

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const STAKE_ASSETS: Map<String, StakeAsset> = Map::new("stake_assets");

pub const TMP_CACHE: Item<TmpCacheValues> = Item::new("tmp_cache");

pub const LIQUIDATION_BOUNTY: Decimal = Decimal::raw(1_100_000_000_000_000_000u128); // 110%
pub const STAKED_ASSET_MULTIPLIER: Decimal = Decimal::raw(2_000_000_000_000_000_000u128); // 2
pub const DEFAULT_LIMIT: u64 = 500u64;

#[cw_serde]
pub struct Config {
    pub query_contract: Addr,
    pub fee_pool_contract: Addr,
    pub fee_rate: Decimal,
    pub base_asset: Asset,
    pub power_asset: Asset,
    pub stake_assets: Option<Vec<StakeAsset>>,
    pub base_pool: Pool,
    pub power_pool: Pool,
    pub funding_period: u64,
    pub index_scale: u64,
    pub min_collateral_amount: Decimal,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            query_contract: Addr::unchecked(""),
            fee_pool_contract: Addr::unchecked(""),
            fee_rate: Decimal::zero(),
            base_asset: Asset::default(),
            power_asset: Asset::default(),
            stake_assets: None,
            base_pool: Pool::default(),
            power_pool: Pool::default(),
            funding_period: 0,
            index_scale: 1, // Assuming '1' is a sensible default
            min_collateral_amount: Decimal::zero(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> StdResult<()> {
        ensure!(
            self.base_asset.decimals > 0 && self.base_asset.decimals <= 18,
            StdError::generic_err("Invalid base decimals")
        );

        ensure!(
            self.power_asset.decimals > 0 && self.power_asset.decimals <= 18,
            StdError::generic_err("Invalid power decimals")
        );

        if let Some(stake_assets) = self.stake_assets.clone() {
            check_duplicates(&stake_assets)?;

            for asset in stake_assets.iter() {
                ensure!(
                    asset.decimals > 0 && asset.decimals <= 18,
                    StdError::generic_err(format!(
                        "Invalid stake asset ({}) decimals",
                        asset.denom
                    ))
                );
            }
        }

        ensure!(
            self.fee_rate < Decimal::one(),
            StdError::generic_err("Invalid fee rate")
        );

        ensure!(
            self.min_collateral_amount != Decimal::zero(),
            StdError::generic_err("Minimum collateral amount cannot be zero")
        );

        ensure!(
            matches!(self.index_scale, 10 | 100 | 1000 | 10000 | 100000 | 1000000),
            StdError::generic_err("Invalid index scale")
        );

        ensure!(
            self.funding_period > 0 && self.funding_period <= 2 * FUNDING_PERIOD,
            StdError::generic_err(format!(
                "Invalid funding period, must be between 0 and {} seconds",
                (2 * FUNDING_PERIOD)
            ))
        );

        ensure_ne!(
            self.power_asset.denom,
            self.base_asset.denom,
            StdError::generic_err("Invalid base and power denom must be different")
        );

        ensure_ne!(
            self.power_pool.id,
            self.base_pool.id,
            StdError::generic_err("Invalid base and power pool id must be different")
        );

        Ok(())
    }

    pub fn is_stake_enabled(&self) -> bool {
        self.stake_assets.is_some()
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

    pub fn is_paused(&self) -> StdResult<()> {
        ensure!(
            self.is_open,
            StdError::generic_err("Cannot perform action as contract is not open")
        );

        ensure!(
            self.is_paused,
            StdError::generic_err("Cannot perform action as contract is unpaused")
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
    pub amount_to_burn: Option<Uint128>,
    pub sender: Option<Addr>,
    pub slippage: Option<Decimal>,
    pub vault_id: Option<u64>,
}

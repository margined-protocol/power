use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};
use std::fmt;

pub const FUNDING_PERIOD: u64 = 420 * 60 * 60; // 420 hours

#[cw_serde]
pub struct InstantiateMsg {
    pub fee_rate: String,
    pub fee_pool: String,
    pub query_contract: String,
    pub base_denom: String,
    pub power_denom: String,
    pub base_pool_id: u64,
    pub base_pool_quote: String,
    pub power_pool_id: u64,
    pub base_decimals: u32,
    pub power_decimals: u32,
    pub stake_assets: Option<Vec<StakeAsset>>,
    pub index_scale: u64,
    pub min_collateral_amount: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    SetOpen {},
    MintPowerPerp {
        amount: Uint128,
        vault_id: Option<u64>,
        rebase: bool,
    },
    BurnPowerPerp {
        amount_to_withdraw: Option<Uint128>,
        vault_id: u64,
    },
    OpenShort {
        amount: Uint128,
        vault_id: Option<u64>,
        slippage: Option<Decimal>,
    },
    CloseShort {
        amount_to_burn: Uint128,
        amount_to_withdraw: Option<Uint128>,
        vault_id: u64,
    },
    Deposit {
        vault_id: u64,
    },
    Withdraw {
        amount: Uint128,
        vault_id: u64,
    },
    Liquidate {
        max_debt_amount: Uint128,
        vault_id: u64,
    },
    FlashLiquidate {
        vault_id: u64,
        slippage: Option<Decimal>,
    },
    ApplyFunding {},
    UpdateConfig {
        fee_rate: Option<String>,
        fee_pool: Option<String>,
    },
    Pause {},
    UnPause {},
    ProposeNewOwner {
        new_owner: String,
        duration: u64,
    },
    MigrateVaults {
        start_after: Option<u64>,
        limit: Option<u64>,
    },
    RejectOwner {},
    RemoveEmptyVaults {
        start_after: Option<u64>,
        limit: Option<u64>,
    },
    ClaimOwnership {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(StateResponse)]
    State {},
    #[returns(Addr)]
    Owner {},
    #[returns(Decimal)]
    GetNormalisationFactor {},
    #[returns(Decimal)]
    GetIndex { period: u64 },
    #[returns(Decimal)]
    GetUnscaledIndex { period: u64 },
    #[returns(Decimal)]
    GetDenormalisedMark { period: u64 },
    #[returns(Decimal)]
    GetDenormalisedMarkFunding { period: u64 },
    #[returns(VaultResponse)]
    GetVault { vault_id: u64 },
    #[returns(UserVaultsResponse)]
    GetUserVaults {
        user: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    #[returns(u64)]
    GetNextVaultId {},
    #[returns(OwnerProposalResponse)]
    GetOwnershipProposal {},
    #[returns(LiquidationAmountResponse)]
    GetLiquidationAmount { vault_id: u64 },
    #[returns(bool)]
    CheckVault { vault_id: u64 },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct VaultResponse {
    pub operator: Addr,
    pub collateral: Uint128,
    pub short_amount: Uint128,
    pub vault_type: VaultType,
    pub collateral_ratio: Decimal,
}

#[cw_serde]
pub struct UserVaultsResponse {
    pub vaults: Vec<u64>,
}

#[cw_serde]
pub struct ConfigResponse {
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
    pub version: String,
}

#[cw_serde]
pub struct LiquidationAmountResponse {
    pub liquidation_amount: Uint128,
    pub collateral_to_pay: Uint128,
    pub debt_to_repay: Uint128,
}

#[cw_serde]
pub struct StateResponse {
    pub is_open: bool,
    pub is_paused: bool,
    pub last_pause: Timestamp,
    pub normalisation_factor: Decimal,
    pub last_funding_update: Timestamp,
}

#[cw_serde]
pub struct OwnerProposalResponse {
    pub owner: Addr,
    pub expiry: u64,
}

#[derive(Default)]
#[cw_serde]
pub struct Pool {
    pub id: u64,
    pub base_denom: String,
    pub quote_denom: String,
}

#[derive(Default)]
#[cw_serde]
pub struct Asset {
    pub denom: String,
    pub decimals: u32,
}

#[derive(Default)]
#[cw_serde]
pub struct StakeAsset {
    pub denom: String,
    pub decimals: u32,
    pub pool: Pool,
}

#[cw_serde]
pub enum VaultType {
    Default,
    Staked { denom: String },
}

impl fmt::Display for VaultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VaultType::Default => write!(f, "Default"),
            VaultType::Staked { .. } => write!(f, "Staked"),
        }
    }
}

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};

pub const FUNDING_PERIOD: u64 = 420 * 60 * 60; // 420 hours

#[cw_serde]
pub struct InstantiateMsg {
    pub fee_rate: String,        // rate of fees charge, must be less than 1
    pub fee_pool: String,        // address of fee pool contract
    pub query_contract: String,  // query contract that wraps native querier
    pub base_denom: String,      // denom of the underlying token, e.g. atom
    pub power_denom: String,     // denom of the power token, e.g. atom^2
    pub base_pool_id: u64,       // id of the pool of the underlying to quote, e.g. atom:usdc
    pub base_pool_quote: String, // denom of the base pool quote asset, e.g. usdc
    pub power_pool_id: u64,      // id of the pool of the underlying to power, e.g. atom:atom^2
    pub base_decimals: u32,      // decimals of the underlying token
    pub power_decimals: u32,     // decimals of the power perp token
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
    RejectOwner {},
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
    pub power_denom: String,
    pub base_denom: String,
    pub base_pool: Pool,
    pub power_pool: Pool,
    pub funding_period: u64,
    pub base_decimals: u32,
    pub power_decimals: u32,
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

#[cw_serde]
pub struct Pool {
    pub id: u64,
    pub quote_denom: String,
}

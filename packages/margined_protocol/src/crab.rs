use crate::power::Pool;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub power_contract: String,
    pub query_contract: String,
    pub fee_pool_contract: String,
    pub fee_rate: String,
    pub power_denom: String,
    pub base_denom: String,
    pub base_pool_id: u64,
    pub base_pool_quote: String,
    pub power_pool_id: u64,
    pub power_pool_quote: String,
    pub base_decimals: u32,
    pub power_decimals: u32,
}

#[cw_serde]
#[allow(clippy::large_enum_variant)]
pub enum ExecuteMsg {
    ClaimOwnership {},
    Deposit {},
    FlashDeposit {},
    FlashWithdraw {},
    HedgeOTC {},
    Pause {},
    ProposeNewOwner { new_owner: String, duration: u64 },
    RedeemShortShutdown {},
    RejectOwner {},
    SetOpen {},
    TransferVault {},
    Withdraw {},
    WithdrawShutdown {},
    UpdateConfig { new_config: UpdateConfig },
    UnPause {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(StateResponse)]
    State {},
    #[returns(StubResponse)]
    CheckPriceHedge {},
    #[returns(StubResponse)]
    CheckTimeHedge {},
    #[returns(StubResponse)]
    DomainSeparator {},
    #[returns(OwnerProposalResponse)]
    GetVaultDetails {},
    #[returns(StubResponse)]
    GetOwnershipProposal {},
    #[returns(StubResponse)]
    GetWsqueethFromCrabAmount {},
    #[returns(StubResponse)]
    Nonce {},
    #[returns(Addr)]
    Owner {},
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct ConfigResponse {
    pub power_contract: Addr,
    pub query_contract: Addr,
    pub fee_pool_contract: Addr,
    pub power_denom: String,
    pub base_denom: String,
    pub base_pool: Pool,
    pub power_pool: Pool,
    pub base_decimals: u32,
    pub power_decimals: u32,
    pub fee_rate: Decimal,
    pub hedge_price_threshold: Uint128,
    pub hedge_time_threshold: u64,
    pub hedging_twap_period: u64,
    pub strategy_cap: Uint128,
    pub strategy_denom: String,
    pub version: String,
}

#[derive(Default)]
#[cw_serde]
pub struct UpdateConfig {
    pub power_contract: Option<Addr>,
    pub query_contract: Option<Addr>,
    pub fee_pool_contract: Option<Addr>,
    pub power_denom: Option<String>,
    pub base_denom: Option<String>,
    pub base_pool: Option<Pool>,
    pub power_pool: Option<Pool>,
    pub base_decimals: Option<u32>,
    pub power_decimals: Option<u32>,
    pub fee_rate: Option<Decimal>,
    pub hedge_price_threshold: Option<Uint128>,
    pub hedge_time_threshold: Option<u64>,
    pub hedging_twap_period: Option<u64>,
    pub strategy_cap: Option<Uint128>,
}

#[cw_serde]
pub struct StateResponse {
    pub is_open: bool,
    pub is_paused: bool,
    pub last_pause: Timestamp,
    pub time_at_last_hedge: Timestamp,
    pub price_at_last_hedge: Decimal,
    pub strategy_vault_id: u64,
}

#[derive(Default)]
#[cw_serde]
pub struct UpdateStateResponse {
    pub is_open: Option<bool>,
    pub is_paused: Option<bool>,
    pub last_pause: Option<Timestamp>,
    pub time_at_last_hedge: Option<Decimal>,
    pub price_at_last_hedge: Option<u64>,
    pub strategy_vault_id: Option<u64>,
}

#[cw_serde]
pub struct OwnerProposalResponse {
    pub owner: Addr,
    pub expiry: u64,
}

#[cw_serde]
pub struct StubResponse {}

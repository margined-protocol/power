use crate::helpers::store_code;

use cosmwasm_std::{coin, Addr, Decimal, Uint128};
use margined_protocol::crab::InstantiateMsg;
use osmosis_test_tube::{OsmosisTestApp, SigningAccount, Wasm};
pub const MOCK_POWER_ADDR: &str = "osmo1cnj84q49sp4sd3tsacdw9p4zvyd8y46f2248ndq2edve3fqa8krs9jds9g";
pub const MOCK_QUERY_ADDR: &str = "osmo1cnj84q49sp4sd3tsacdw9p4zvyd8y46f2248ndq2edve3fqa8krs9jds9g";
pub const MOCK_FEE_POOL_CONTRACT: &str = "osmo1tj5a2z96vfy8av78926pgs3x774dvhzgxayue0";
pub const MOCK_POWER_DENOM: &str = "factory/osmo1qc5pen6am58wxuj58vw97m72vv5tp74remsul7/uosmoexp";
pub const MOCK_BASE_POOL_ID: u64 = 5u64;
pub const MOCK_POWER_POOL_ID: u64 = 75u64;
pub const MOCK_BASE_DECIMALS: u32 = 6u32;
pub const MOCK_POWER_DECIMALS: u32 = 6u32;
pub const MOCK_BASE_POOL_QUOTE_DENOM: &str =
    "ibc/6F34E1BD664C36CE49ACC28E60D62559A5F96C4F9A6CCE4FC5A67B2852E24CFE";
pub const MOCK_FEE_RATE: Decimal = Decimal::zero();
pub const MOCK_BASE_DENOM: &str = "uosmo";
pub const MOCK_HEDGING_TWAP_PERIOD: u64 = 420u64;
pub const MOCK_HEDGE_PRICE_THRESHOLD: Uint128 = Uint128::new(200_000_000_000_000_000u128);
pub const MOCK_HEDGE_TIME_THRESHOLD: u64 = 1800u64;
pub const MOCK_STRATEGY_CAP: Uint128 = Uint128::new(10000000000000000000000u128);

pub struct ContractInfo {
    pub addr: Addr,
    pub id: u64,
}

pub struct CrabEnv {
    pub app: OsmosisTestApp,
    pub signer: SigningAccount,
    pub traders: Vec<SigningAccount>,
}

impl CrabEnv {
    pub fn new() -> Self {
        let app = OsmosisTestApp::new();

        let signer = app
            .init_account(&[coin(1_000_000_000_000_000_000, "uosmo")])
            .unwrap();

        let mut traders: Vec<SigningAccount> = Vec::new();
        for _ in 0..10 {
            traders.push(
                app.init_account(&[coin(1_000_000_000_000_000_000, "uosmo")])
                    .unwrap(),
            );
        }

        Self {
            app,
            signer,
            traders,
        }
    }

    pub fn deploy_crab_contract(&self, wasm: &Wasm<OsmosisTestApp>) -> String {
        let code_id = store_code(wasm, &self.signer, "margined_crab".to_string());
        let msg = InstantiateMsg {
            power_contract: MOCK_POWER_ADDR.to_string(),
            query_contract: MOCK_QUERY_ADDR.to_string(),
            fee_pool_contract: MOCK_QUERY_ADDR.to_string(),
            fee_rate: MOCK_FEE_RATE.to_string(),
            power_denom: MOCK_POWER_DENOM.to_string(),
            base_denom: MOCK_BASE_DENOM.to_string(),
            base_pool_id: MOCK_BASE_POOL_ID,
            base_pool_quote: MOCK_BASE_POOL_QUOTE_DENOM.to_string(),
            power_pool_id: MOCK_POWER_POOL_ID,
            power_pool_quote: MOCK_POWER_DENOM.to_string(),
            base_decimals: MOCK_BASE_DECIMALS,
            power_decimals: MOCK_POWER_DECIMALS,
        };

        wasm.instantiate(
            code_id,
            &msg,
            None,
            Some("margined-crab-contract"),
            &[],
            &self.signer,
        )
        .unwrap()
        .data
        .address
    }
}

impl Default for CrabEnv {
    fn default() -> Self {
        Self::new()
    }
}

use crate::helpers::store_code;

use cosmwasm_std::{coin, Addr, Uint128};
use margined_protocol::{
    collector::{
        ExecuteMsg as FeeCollectorExecuteMsg, InstantiateMsg as FeeCollectorInstantiateMsg,
    },
    staking::InstantiateMsg,
};
use osmosis_test_tube::{
    osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest, Bank, Module, OsmosisTestApp,
    SigningAccount, Wasm,
};
use std::{collections::HashMap, str::FromStr};

pub const ONE: u128 = 1_000_000; // 1.0@6dp
pub const SCALE_FACTOR: u128 = 10_000;
pub const BASE_PRICE: u128 = 3_000_000_000; // 3000.0@6dp
pub const POWER_PRICE: u128 = 3_010_000_000; // 3010.0@6dp
pub const SCALED_POWER_PRICE: u128 = 30_100_000; // 0.3010@6dp

pub struct ContractInfo {
    pub addr: Addr,
    pub id: u64,
}

pub struct StakingEnv {
    pub app: OsmosisTestApp,
    pub signer: SigningAccount,
    pub handler: SigningAccount,
    pub empty: SigningAccount,
    pub traders: Vec<SigningAccount>,
    pub denoms: HashMap<String, String>,
}

impl StakingEnv {
    pub fn new() -> Self {
        let app = OsmosisTestApp::new();

        let mut denoms = HashMap::new();
        denoms.insert("base".to_string(), "uosmo".to_string());
        denoms.insert("reward".to_string(), "uusdc".to_string());
        denoms.insert("deposit".to_string(), "umrg".to_string());

        let signer = app
            .init_account(&[
                coin(1_000_000_000_000_000_000, "uosmo"),
                coin(1_000_000_000_000, "uusdc"),
                coin(1_000_000_000_000, "token3"),
                coin(1_000_000_000_000, "token4"),
                coin(1_000_000_000_000, "token5"),
            ])
            .unwrap();

        let handler = app.init_account(&[]).unwrap();

        let trader1 = app
            .init_account(&[
                coin(1_000_000_000_000_000_000, "uosmo"),
                coin(1_000_000_000_000, "uusdc"),
                coin(1_000_000_000, "umrg"),
            ])
            .unwrap();

        let trader2 = app
            .init_account(&[
                coin(1_000_000_000_000_000_000, "uosmo"),
                coin(1_000_000_000_000, "uusdc"),
                coin(1_000_000_000, "umrg"),
            ])
            .unwrap();

        let empty = app.init_account(&[coin(1_000_000_000, "umrg")]).unwrap();

        Self {
            app,
            signer,
            handler,
            empty,
            traders: vec![trader1, trader2],
            denoms,
        }
    }

    pub fn deploy_staking_contracts(&self, wasm: &Wasm<OsmosisTestApp>) -> (String, String) {
        let code_id = store_code(wasm, &self.signer, "margined_collector".to_string());
        let fee_collector_address = wasm
            .instantiate(
                code_id,
                &FeeCollectorInstantiateMsg {},
                None,
                Some("margined-fee-collector"),
                &[coin(1_000_000_000_000, self.denoms["base"].clone())],
                &self.signer,
            )
            .unwrap()
            .data
            .address;

        let code_id = store_code(wasm, &self.signer, "margined_staking".to_string());
        let staking_address = wasm
            .instantiate(
                code_id,
                &InstantiateMsg {
                    fee_collector: fee_collector_address.clone(),
                    deposit_denom: self.denoms["deposit"].clone(),
                    reward_denom: self.denoms["reward"].clone(),
                    deposit_decimals: 6u32,
                    reward_decimals: 6u32,
                    tokens_per_interval: 1_000_000u128.into(),
                },
                None,
                Some("margined-staking-contract"),
                &[coin(1_000_000_000_000, self.denoms["base"].clone())],
                &self.signer,
            )
            .unwrap()
            .data
            .address;

        // add the reward token as a token
        {
            wasm.execute(
                fee_collector_address.as_str(),
                &FeeCollectorExecuteMsg::AddToken {
                    token: self.denoms["reward"].clone(),
                },
                &[],
                &self.signer,
            )
            .unwrap();
        }

        // update the collector to have the staking contract as an auth
        {
            wasm.execute(
                fee_collector_address.as_str(),
                &FeeCollectorExecuteMsg::UpdateWhitelist {
                    address: staking_address.clone(),
                },
                &[],
                &self.signer,
            )
            .unwrap();
        }

        (staking_address, fee_collector_address)
    }

    pub fn deploy_staking_contract(
        &self,
        wasm: &Wasm<OsmosisTestApp>,
        contract_name: String,
        fee_collector: String,
    ) -> String {
        let code_id = store_code(wasm, &self.signer, contract_name);
        wasm.instantiate(
            code_id,
            &InstantiateMsg {
                fee_collector,
                deposit_denom: self.denoms["deposit"].clone(),
                reward_denom: self.denoms["reward"].clone(),
                deposit_decimals: 6u32,
                reward_decimals: 6u32,
                tokens_per_interval: 1_000_000u128.into(),
            },
            None,
            Some("margined-staking-contract"),
            &[coin(1_000_000_000_000, self.denoms["base"].clone())],
            &self.signer,
        )
        .unwrap()
        .data
        .address
    }

    pub fn deploy_fee_collector_contract(
        &self,
        wasm: &Wasm<OsmosisTestApp>,
        contract_name: String,
    ) -> String {
        let code_id = store_code(wasm, &self.signer, contract_name);
        wasm.instantiate(
            code_id,
            &FeeCollectorInstantiateMsg {},
            None,
            Some("margined-collector-contract"),
            &[],
            &self.signer,
        )
        .unwrap()
        .data
        .address
    }

    pub fn get_balance(&self, address: String, denom: String) -> Uint128 {
        let bank = Bank::new(&self.app);

        let response = bank
            .query_balance(&QueryBalanceRequest { address, denom })
            .unwrap();

        match response.balance {
            Some(balance) => Uint128::from_str(&balance.amount).unwrap(),
            None => Uint128::zero(),
        }
    }
}

impl Default for StakingEnv {
    fn default() -> Self {
        Self::new()
    }
}

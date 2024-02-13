use crate::helpers::store_code;

use cosmrs::proto::{
    cosmos::params::v1beta1::{ParamChange, ParameterChangeProposal},
    traits::Message,
};
use cosmwasm_std::{coin, Addr, Decimal, Uint128};
use margined_protocol::{
    crab::InstantiateMsg as CrabInstantiateMsg,
    power::{ExecuteMsg, InstantiateMsg, Pool as PowerPool, StakeAsset},
    query::{InstantiateMsg as QueryInstantiateMsg, QueryMsg as QueryQueryMsg},
};
use mock_query::contract::ExecuteMsg as MockQueryExecuteMsg;
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::{
            bank::v1beta1::{MsgSend, QueryBalanceRequest, QueryTotalSupplyRequest},
            base::v1beta1::Coin,
        },
        osmosis::{
            concentratedliquidity::v1beta1::{
                CreateConcentratedLiquidityPoolsProposal, MsgCreatePosition, Pool, PoolRecord,
                PoolsRequest,
            },
            poolmanager::v1beta1::SpotPriceRequest,
            tokenfactory::v1beta1::{MsgChangeAdmin, MsgCreateDenom},
        },
    },
    Account, Bank, ConcentratedLiquidity, Gamm, GovWithAppAccess, Module, OsmosisTestApp,
    PoolManager, SigningAccount, TokenFactory, Wasm,
};
use std::{collections::HashMap, str::FromStr};

pub const ONE: u128 = 1_000_000; // 1.0@6dp
pub const SCALE_FACTOR: u128 = 10_000; // 1e4
pub const BASE_PRICE: u128 = 3_000_000_000; // 3000.0@6dp
pub const POWER_PRICE: u128 = 3_010_000_000; // 3010.0@6dp
pub const STAKE_PRICE: u128 = 1_100_000; // 1.1@6dp
pub const SCALED_POWER_PRICE: u128 = 30_100_000; // 0.3010@6dp
pub const MAX_TWAP_PERIOD: u64 = 48 * 60 * 60;
pub struct ContractInfo {
    pub addr: Addr,
    pub id: u64,
}

pub struct PowerEnv {
    pub app: OsmosisTestApp,
    pub signer: SigningAccount,
    pub owner: SigningAccount, // owns the pools
    pub fee_pool: SigningAccount,
    pub traders: Vec<SigningAccount>,
    pub liquidator: SigningAccount,
    pub base_pool_id: u64,
    pub power_pool_id: u64,
    pub stake_pool_id: u64,
    pub milk_pool_id: u64,
    pub denoms: HashMap<String, String>,
}

impl PowerEnv {
    pub fn new() -> Self {
        let app = OsmosisTestApp::new();

        let bank = Bank::new(&app);
        let concentrated_liquidity = ConcentratedLiquidity::new(&app);
        let gamm = Gamm::new(&app);
        let gov = GovWithAppAccess::new(&app);
        let token = TokenFactory::new(&app);

        let mut denoms = HashMap::new();
        denoms.insert("quote".to_string(), "usdc".to_string());
        denoms.insert("base".to_string(), "ubase".to_string());
        denoms.insert("stake".to_string(), "stbase".to_string());
        denoms.insert("milk".to_string(), "milkbase".to_string());
        denoms.insert("gas".to_string(), "uosmo".to_string());

        let signer = app
            .init_account(&[
                coin(1_000_000_000_000_000_000, "uosmo"),
                coin(1_000_000_000_000_000, "usdc"),
                coin(1_000_000_000_000_000_000_000_000, "ubase"),
                coin(1_000_000_000_000_000_000_000_000, "stbase"),
                coin(1_000_000_000_000_000_000_000_000, "milkbase"),
            ])
            .unwrap();

        let fee_pool = app.init_account(&[]).unwrap();

        let mut traders: Vec<SigningAccount> = Vec::new();
        for _ in 0..10 {
            traders.push(
                app.init_account(&[
                    coin(1_000_000_000_000_000_000, "uosmo"),
                    coin(1_000_000_000_000, "usdc"),
                    coin(1_000_000_000_000_000, "ubase"),
                    coin(1_000_000_000_000_000, "stbase"),
                    coin(1_000_000_000_000_000, "milkbase"),
                ])
                .unwrap(),
            );
        }

        let liquidator = app
            .init_account(&[
                coin(1_000_000_000_000_000_000, "uosmo"),
                coin(1_000_000_000_000, "usdc"),
                coin(1_000_000_000_000_000, "ubase"),
                coin(1_000_000_000_000_000, "stbase"),
                coin(1_000_000_000_000_000, "milkbase"),
            ])
            .unwrap();

        let power_denom = token
            .create_denom(
                MsgCreateDenom {
                    sender: signer.address(),
                    subdenom: "sqosmo".to_string(),
                },
                &signer,
            )
            .unwrap()
            .data
            .new_token_denom;
        denoms.insert("power".to_string(), power_denom);

        let owner = app
            .init_account(&[
                coin(1_000_000_000_000_000_000, denoms["gas"].clone()),
                coin(1_000_000_000_000, denoms["base"].clone()),
                coin(1_000_000_000_000, denoms["stake"].clone()),
                coin(1_000_000_000_000, denoms["milk"].clone()),
                coin(3_000_000_000_000, denoms["quote"].clone()),
                coin(3_000_000_000_000, denoms["power"].clone()),
            ])
            .unwrap();

        bank.send(
            MsgSend {
                to_address: signer.address(),
                from_address: owner.address(),
                amount: vec![Coin {
                    amount: "1000000000000".to_string(),
                    denom: denoms["power"].clone(),
                }],
            },
            &owner,
        )
        .unwrap();

        let base_pool_id = gamm
            .create_basic_pool(
                &[
                    coin(1_000_000_000, denoms["base"].clone()),
                    coin(3_000_000_000_000, denoms["quote"].clone()),
                ],
                &owner,
            )
            .unwrap()
            .data
            .pool_id;

        let stake_pool_id = gamm
            .create_basic_pool(
                &[
                    coin(900_000_000, denoms["stake"].clone()),
                    coin(1_000_000_000, denoms["base"].clone()),
                ],
                &owner,
            )
            .unwrap()
            .data
            .pool_id;

        let milk_pool_id = gamm
            .create_basic_pool(
                &[
                    coin(1_100_000_000, denoms["milk"].clone()),
                    coin(1_000_000_000, denoms["base"].clone()),
                ],
                &owner,
            )
            .unwrap()
            .data
            .pool_id;

        // update the parameters so we can have no gas paying token as base
        gov.propose_and_execute(
            "/cosmos.params.v1beta1.ParameterChangeProposal".to_string(),
            ParameterChangeProposal {
                title: "Update authorized quote denoms".to_string(),
                description: "Add ubase as an authorized quote denom".to_string(),
                changes: vec![ParamChange {
                    subspace: "poolmanager".to_string(),
                    key: "AuthorizedQuoteDenoms".to_string(),
                    value: "[\"usdc\", \"ubase\"]".to_string(),
                }],
            },
            owner.address(),
            false,
            &owner,
        )
        .unwrap();

        gov.propose_and_execute(
            CreateConcentratedLiquidityPoolsProposal::TYPE_URL.to_string(),
            CreateConcentratedLiquidityPoolsProposal {
                title: "Create concentrated uosmo:expuosmo pool".to_string(),
                description: "Create concentrated uosmo:expuosmo pool, so that we can trade it"
                    .to_string(),
                pool_records: vec![PoolRecord {
                    denom0: denoms["power"].clone(), // base
                    denom1: denoms["base"].clone(),  // quote
                    tick_spacing: 100,
                    spread_factor: "0".to_string(),
                }],
            },
            owner.address(),
            false,
            &owner,
        )
        .unwrap();

        let pools = concentrated_liquidity
            .query_pools(&PoolsRequest { pagination: None })
            .unwrap();

        let pool = Pool::decode(pools.pools[0].value.as_slice()).unwrap();
        let power_pool_id = pool.id;

        // Make full range
        concentrated_liquidity
            .create_position(
                MsgCreatePosition {
                    pool_id: power_pool_id,
                    sender: owner.address(),
                    lower_tick: -108000000i64,
                    upper_tick: 342000000i64,
                    tokens_provided: vec![
                        Coin {
                            denom: denoms["power"].clone(),
                            amount: "1_000_000".to_string(),
                        },
                        Coin {
                            denom: denoms["base"].clone(),
                            amount: "300_000".to_string(),
                        },
                    ],
                    token_min_amount0: "0".to_string(),
                    token_min_amount1: "0".to_string(),
                },
                &owner,
            )
            .unwrap();

        Self {
            app,
            signer,
            fee_pool,
            owner,
            traders,
            liquidator,
            base_pool_id,
            power_pool_id,
            stake_pool_id,
            milk_pool_id,
            denoms,
        }
    }

    // TODO: this is potentially not the best way to do this, it could
    // be better to have a base and implementations like apollo zappers
    // but it's fine for now.
    pub fn deploy_query_contracts(&self, wasm: &Wasm<OsmosisTestApp>, is_mock: bool) -> String {
        let code_id = if is_mock {
            store_code(wasm, &self.signer, "mock_query".to_string())
        } else {
            store_code(wasm, &self.signer, "margined_query".to_string())
        };

        wasm.instantiate(
            code_id,
            &QueryInstantiateMsg {},
            None,
            Some("margined-query-contract"),
            &[coin(10_000_000, "uosmo")],
            &self.signer,
        )
        .unwrap()
        .data
        .address
    }

    pub fn get_power_price(&self, wasm: &Wasm<OsmosisTestApp>, query_address: String) -> Decimal {
        wasm.query(
            &query_address,
            &QueryQueryMsg::GetArithmeticTwapToNow {
                pool_id: self.base_pool_id,
                base_asset: self.denoms["base"].to_string(),
                quote_asset: self.denoms["quote"].to_string(),
                start_time: self.app.get_block_timestamp(),
            },
        )
        .unwrap()
    }

    pub fn deploy_crab(
        &self,
        wasm: &Wasm<OsmosisTestApp>,
        power_address: String,
        query_address: String,
    ) -> String {
        let code_id = store_code(wasm, &self.signer, "margined_crab".to_string());
        wasm.instantiate(
            code_id,
            &CrabInstantiateMsg {
                power_contract: power_address,
                query_contract: query_address,
                fee_pool_contract: self.fee_pool.address(),
                fee_rate: "0.0".to_string(), // 0%
                power_denom: self.denoms["power"].clone(),
                base_denom: self.denoms["base"].clone(),
                base_pool_id: self.base_pool_id,
                base_pool_quote: self.denoms["quote"].clone(),
                power_pool_id: self.power_pool_id,
                power_pool_quote: "usdc".to_string(),
                base_decimals: 6u32,
                power_decimals: 6u32,
            },
            None,
            Some("margined-crab-contract"),
            &[coin(300_000_000u128, "ubase")], // 300.00
            //&[],
            &self.signer,
        )
        .unwrap()
        .data
        .address
    }

    // - Deploy power
    // - Deploy crab
    // - Set fee_rate on power to 0.0
    // - Apply funding
    // - Set crab to open
    pub fn setup_crab(
        &self,
        wasm: &Wasm<OsmosisTestApp>,
        is_mock: bool,
        power_fee: String,
    ) -> (String, String, String) {
        let concentrated_liquidity = ConcentratedLiquidity::new(&self.app);

        // Add more liquidity for testing
        // LP from 0.2 to 0.65
        // 1 - (0.000001 * 800000) = 0.2 @-800000
        // 1 - (0.000001 * 350000) = 0.6 @-350000
        concentrated_liquidity
            .create_position(
                MsgCreatePosition {
                    pool_id: self.power_pool_id,
                    sender: self.owner.address(),
                    lower_tick: -7500000i64,
                    upper_tick: 750000i64,
                    tokens_provided: vec![
                        Coin {
                            denom: self.denoms["power"].clone(),
                            amount: "1_000_000_000_000".to_string(),
                        },
                        Coin {
                            denom: self.denoms["base"].clone(),
                            amount: "300_000_000_000".to_string(),
                        },
                    ],
                    token_min_amount0: "0".to_string(),
                    token_min_amount1: "0".to_string(),
                },
                &self.owner,
            )
            .unwrap();
        let (power_address, query_address) =
            self.deploy_power(wasm, "margined-power".to_string(), false, is_mock);

        if is_mock {
            // Set the oracle price to 300_000 (0.3)
            wasm.execute(
                &query_address,
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: self.base_pool_id,
                    price: Decimal::from_atomics(300_000u128, 6u32).unwrap(),
                },
                &[],
                &self.signer,
            )
            .unwrap();
            wasm.execute(
                &query_address,
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: self.power_pool_id,
                    price: Decimal::from_atomics(300_000u128, 6u32).unwrap(),
                },
                &[],
                &self.signer,
            )
            .unwrap();
        }
        self.app.increase_time(MAX_TWAP_PERIOD + 1);

        wasm.execute(
            &power_address,
            &ExecuteMsg::UpdateConfig {
                fee_rate: Some(power_fee),
                fee_pool: None,
            },
            &[],
            &self.signer,
        )
        .unwrap();

        wasm.execute(
            &power_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &self.signer,
        )
        .unwrap();

        let crab_address = self.deploy_crab(wasm, power_address.clone(), query_address.clone());

        wasm.execute(&crab_address, &ExecuteMsg::SetOpen {}, &[], &self.signer)
            .unwrap();

        (power_address, query_address, crab_address)
    }

    pub fn create_new_pool(&self, denom0: String, denom1: String, owner: &SigningAccount) -> u64 {
        let gov = GovWithAppAccess::new(&self.app);
        let concentrated_liquidity = ConcentratedLiquidity::new(&self.app);

        gov.propose_and_execute(
            CreateConcentratedLiquidityPoolsProposal::TYPE_URL.to_string(),
            CreateConcentratedLiquidityPoolsProposal {
                title: "Create concentrated uosmo:expuosmo pool".to_string(),
                description: "Create concentrated uosmo:expuosmo pool, so that we can trade it"
                    .to_string(),
                pool_records: vec![PoolRecord {
                    denom0, // base
                    denom1, // quote
                    tick_spacing: 100,
                    spread_factor: "0".to_string(),
                }],
            },
            owner.address(),
            false,
            owner,
        )
        .unwrap();

        let pools = concentrated_liquidity
            .query_pools(&PoolsRequest { pagination: None })
            .unwrap();

        let pool = Pool::decode(pools.pools.last().unwrap().value.as_slice()).unwrap();

        pool.id
    }

    pub fn create_position(
        &self,
        lower_tick: String,
        upper_tick: String,
        base_amount: String,
        power_amount: String,
    ) {
        let concentrated_liquidity = ConcentratedLiquidity::new(&self.app);

        concentrated_liquidity
            .create_position(
                MsgCreatePosition {
                    pool_id: self.power_pool_id,
                    sender: self.owner.address(),
                    lower_tick: i64::from_str(&lower_tick).unwrap(),
                    upper_tick: i64::from_str(&upper_tick).unwrap(),
                    tokens_provided: vec![
                        Coin {
                            denom: self.denoms["power"].clone(),
                            amount: power_amount,
                        },
                        Coin {
                            denom: self.denoms["base"].clone(),
                            amount: base_amount,
                        },
                    ],
                    token_min_amount0: "0".to_string(),
                    token_min_amount1: "0".to_string(),
                },
                &self.owner,
            )
            .unwrap();
    }

    pub fn deploy_power(
        &self,
        wasm: &Wasm<OsmosisTestApp>,
        contract_name: String,
        use_staked_assets: bool,
        is_mock: bool,
    ) -> (String, String) {
        let token = TokenFactory::new(&self.app);

        let query_address = self.deploy_query_contracts(wasm, is_mock);

        let stake_assets = if use_staked_assets {
            Some(vec![
                StakeAsset {
                    denom: self.denoms["stake"].clone(),
                    pool: PowerPool {
                        id: self.stake_pool_id,
                        base_denom: self.denoms["stake"].clone(),
                        quote_denom: self.denoms["base"].clone(),
                    },
                    decimals: 6u32,
                },
                StakeAsset {
                    denom: self.denoms["milk"].clone(),
                    pool: PowerPool {
                        id: self.milk_pool_id,
                        base_denom: self.denoms["milk"].clone(),
                        quote_denom: self.denoms["base"].clone(),
                    },
                    decimals: 6u32,
                },
            ])
        } else {
            None
        };

        let code_id = store_code(wasm, &self.signer, contract_name);
        let perp_address = wasm
            .instantiate(
                code_id,
                &InstantiateMsg {
                    fee_pool: self.fee_pool.address(),
                    fee_rate: "0.0".to_string(), // 0%
                    query_contract: query_address.clone(),
                    power_denom: self.denoms["power"].clone(),
                    base_denom: self.denoms["base"].clone(),
                    stake_assets,
                    base_pool_id: self.base_pool_id,
                    base_pool_quote: self.denoms["quote"].clone(),
                    power_pool_id: self.power_pool_id,
                    base_decimals: 6u32,
                    power_decimals: 6u32,
                    index_scale: SCALE_FACTOR as u64,
                    min_collateral_amount: "0.5".to_string(),
                },
                None,
                Some("margined-power-contract"),
                &[],
                &self.signer,
            )
            .unwrap()
            .data
            .address;

        token
            .change_admin(
                MsgChangeAdmin {
                    sender: self.signer.address(),
                    new_admin: perp_address.clone(),
                    denom: self.denoms["power"].clone(),
                },
                &self.signer,
            )
            .unwrap();

        wasm.execute(&perp_address, &ExecuteMsg::SetOpen {}, &[], &self.signer)
            .unwrap();

        (perp_address, query_address)
    }

    pub fn set_oracle_price(
        &self,
        wasm: &Wasm<OsmosisTestApp>,
        query_address: String,
        pool_id: u64,
        price: Decimal,
    ) {
        wasm.execute(
            &query_address,
            &MockQueryExecuteMsg::AppendPrice { pool_id, price },
            &[],
            &self.signer,
        )
        .unwrap();
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

    pub fn get_total_supply(&self, denom: String) -> Uint128 {
        let bank = Bank::new(&self.app);

        let response = bank
            .query_total_supply(&QueryTotalSupplyRequest { pagination: None })
            .unwrap()
            .supply
            .into_iter()
            .find(|coin| coin.denom == denom)
            .unwrap();

        Uint128::from_str(&response.amount).unwrap_or(Uint128::zero())
    }

    pub fn calculate_target_power_price(&self, normalisation_factor: Decimal) -> Decimal {
        let pool_manager = PoolManager::new(&self.app);

        let res = pool_manager
            .query_spot_price(&SpotPriceRequest {
                pool_id: self.base_pool_id,
                base_asset_denom: self.denoms["base"].clone(),
                quote_asset_denom: self.denoms["quote"].clone(),
            })
            .unwrap();
        let index_price = Decimal::from_str(&res.spot_price).unwrap();

        let mark_price = index_price * index_price;
        let scale_factor = Decimal::from_atomics(SCALE_FACTOR, 0u32).unwrap();

        (index_price / mark_price) * scale_factor / normalisation_factor
    }

    pub fn price_to_tick(&self, price: Decimal, tick_interval: Uint128) -> String {
        let mut exponent = Decimal::from_str("0.000001").unwrap();
        let mut tick = Decimal::zero();
        let ticks_per_increment = Decimal::from_atomics(9_000_000u128, 0u32).unwrap();
        let ten = Decimal::from_atomics(10u128, 0u32).unwrap();

        // NOTE: this is unfinished and probably doesnt work for less than 1
        let result = match price.cmp(&Decimal::one()) {
            std::cmp::Ordering::Greater => {
                let mut total_price = Decimal::one();

                while total_price < price {
                    total_price += ticks_per_increment * exponent;

                    tick += ticks_per_increment;
                    exponent *= ten;
                }

                let delta = price.abs_diff(total_price);

                tick - (delta / (exponent / ten))
            }
            std::cmp::Ordering::Less => {
                let mut total_price = Decimal::one();

                while total_price < price {
                    total_price += ticks_per_increment * exponent;
                    tick += ticks_per_increment;
                    exponent /= ten;
                }

                let delta = price.abs_diff(total_price);
                tick + (delta / (exponent / ten))
            }
            std::cmp::Ordering::Equal => Decimal::zero(),
        };

        let value = (result.to_uint_floor() / tick_interval) * tick_interval;

        if price < Decimal::one() {
            format!("{}{}", "-", value)
        } else {
            value.to_string()
        }
    }
}

impl Default for PowerEnv {
    fn default() -> Self {
        Self::new()
    }
}

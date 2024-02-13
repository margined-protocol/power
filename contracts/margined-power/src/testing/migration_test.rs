use crate::{
    contract::{CONTRACT_NAME, CONTRACT_VERSION},
    migrate::OldVaultType,
    testing::test_utils::MOCK_FEE_POOL_ADDR,
};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coin, Addr, Decimal, Uint128};
use margined_protocol::power::{
    Asset, ConfigResponse, ExecuteMsg, MigrateMsg, Pool, QueryMsg, StakeAsset, VaultResponse,
    VaultType,
};
use margined_testing::{
    helpers::store_code,
    power_env::{PowerEnv, BASE_PRICE, SCALED_POWER_PRICE, SCALE_FACTOR},
};
use mock_query::contract::ExecuteMsg as MockQueryExecuteMsg;
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmwasm::wasm::v1::{MsgMigrateContract, MsgMigrateContractResponse},
        osmosis::tokenfactory::v1beta1::MsgChangeAdmin,
    },
    Account, ExecuteResponse, Module, Runner, TokenFactory, Wasm,
};
use std::str::FromStr;

#[cw_serde]
pub struct OldVaultResponse {
    pub operator: Addr,
    pub collateral: Uint128,
    pub short_amount: Uint128,
    pub collateral_ratio: Decimal,
    pub vault_type: OldVaultType,
}

#[cw_serde]
pub struct OldInstantiateMsg {
    pub fee_rate: String,
    pub fee_pool: String,
    pub query_contract: String,
    pub base_denom: String,
    pub power_denom: String,
    pub stake_denom: Option<String>,
    pub base_pool_id: u64,
    pub base_pool_quote: String,
    pub power_pool_id: u64,
    pub stake_pool_id: Option<u64>,
    pub base_decimals: u32,
    pub power_decimals: u32,
    pub stake_decimals: Option<u32>,
    pub index_scale: u64,
    pub min_collateral_amount: String,
}

#[cw_serde]
pub struct OldConfigResponse {
    pub query_contract: Addr,
    pub fee_pool_contract: Addr,
    pub fee_rate: Decimal,
    pub base_asset: Asset,
    pub power_asset: Asset,
    pub stake_asset: Option<Asset>,
    pub base_pool: Pool,
    pub power_pool: Pool,
    pub stake_pool: Option<Pool>,
    pub funding_period: u64,
    pub index_scale: u64,
    pub min_collateral_amount: Decimal,
    pub version: String,
}

#[test]
fn test_migration() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let token = TokenFactory::new(&env.app);

    let wasm_byte_code =
        std::fs::read("./src/testing/test_artifacts/margined_power_v017.wasm").unwrap();
    let code_id = wasm
        .store_code(&wasm_byte_code, None, &env.owner)
        .unwrap()
        .data
        .code_id;

    let query_address = env.deploy_query_contracts(&wasm, true);

    wasm.execute(
        query_address.as_ref(),
        &MockQueryExecuteMsg::AppendPrice {
            pool_id: env.base_pool_id,
            price: Decimal::from_atomics(BASE_PRICE, 6u32).unwrap(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    wasm.execute(
        query_address.as_ref(),
        &MockQueryExecuteMsg::AppendPrice {
            pool_id: env.power_pool_id,
            price: Decimal::from_atomics(SCALED_POWER_PRICE, 6u32).unwrap(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    wasm.execute(
        query_address.as_ref(),
        &MockQueryExecuteMsg::AppendPrice {
            pool_id: env.stake_pool_id,
            price: Decimal::one(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let address = wasm
        .instantiate(
            code_id,
            &OldInstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: query_address.clone(),
                power_denom: env.denoms["power"].clone(),
                base_denom: env.denoms["base"].clone(),
                base_pool_id: env.base_pool_id,
                base_pool_quote: env.denoms["quote"].clone(),
                power_pool_id: env.power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: 10_000u64,
                min_collateral_amount: "0.5".to_string(),
                stake_decimals: Some(6u32),
                stake_denom: Some(env.denoms["stake"].clone()),
                stake_pool_id: Some(env.stake_pool_id),
            },
            Some(&env.signer.address()),
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &env.signer,
        )
        .unwrap()
        .data
        .address;

    token
        .change_admin(
            MsgChangeAdmin {
                sender: env.signer.address(),
                new_admin: address.clone(),
                denom: env.denoms["power"].clone(),
            },
            &env.signer,
        )
        .unwrap();

    wasm.execute(&address, &ExecuteMsg::SetOpen {}, &[], &env.signer)
        .unwrap();

    let config: OldConfigResponse = wasm.query(&address, &QueryMsg::Config {}).unwrap();
    assert_eq!(
        config,
        OldConfigResponse {
            query_contract: Addr::unchecked(query_address.clone()),
            base_asset: Asset {
                denom: env.denoms["base"].clone(),
                decimals: 6u32
            },
            power_asset: Asset {
                denom: env.denoms["power"].clone(),
                decimals: 6u32
            },
            stake_asset: Some(Asset {
                denom: env.denoms["stake"].clone(),
                decimals: 6u32
            }),
            base_pool: Pool {
                id: env.base_pool_id,
                base_denom: env.denoms["base"].clone(),
                quote_denom: env.denoms["quote"].clone(),
            },
            power_pool: Pool {
                id: env.power_pool_id,
                base_denom: env.denoms["base"].clone(),
                quote_denom: env.denoms["power"].clone(),
            },
            stake_pool: Some(Pool {
                id: env.stake_pool_id,
                base_denom: env.denoms["stake"].clone(),
                quote_denom: env.denoms["base"].clone(),
            }),
            funding_period: 1512000u64,
            fee_pool_contract: Addr::unchecked(MOCK_FEE_POOL_ADDR.to_string()),
            fee_rate: Decimal::from_str("0.1".to_string().as_str()).unwrap(),
            index_scale: 10_000u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            version: "0.1.7".to_string(),
        }
    );

    {
        env.app.increase_time(1u64);
        wasm.execute(
            &address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(1_000_000u128),
                vault_id: None,
                rebase: false,
            },
            &[coin(10_000_000, env.denoms["base"].clone())],
            &env.traders[1],
        )
        .unwrap();

        env.app.increase_time(1u64);
        wasm.execute(
            &address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(1_000_000u128),
                vault_id: None,
                rebase: false,
            },
            &[coin(10_000_000, env.denoms["base"].clone())],
            &env.traders[2],
        )
        .unwrap();

        env.app.increase_time(1u64);
        wasm.execute(
            &address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(1_000_000u128),
                vault_id: None,
                rebase: false,
            },
            &[coin(10_000_000, env.denoms["base"].clone())],
            &env.traders[3],
        )
        .unwrap();

        env.app.increase_time(1u64);
        wasm.execute(
            &address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(1_000_000u128),
                vault_id: None,
                rebase: false,
            },
            &[coin(10_000_000, env.denoms["stake"].clone())],
            &env.traders[4],
        )
        .unwrap();
    }

    let vault: OldVaultResponse = wasm
        .query(&address, &QueryMsg::GetVault { vault_id: 1u64 })
        .unwrap();

    assert_eq!(
        vault,
        OldVaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::from(6_990_000u128),
            short_amount: Uint128::from(1_000_000u128),
            collateral_ratio: Decimal::from_str("23.300124433687052162").unwrap(),
            vault_type: OldVaultType::Default,
        }
    );

    let vault: OldVaultResponse = wasm
        .query(&address, &QueryMsg::GetVault { vault_id: 4u64 })
        .unwrap();

    assert_eq!(
        vault,
        OldVaultResponse {
            operator: Addr::unchecked(env.traders[4].address()),
            collateral: Uint128::from(3_980_000u128),
            short_amount: Uint128::from(1_000_000u128),
            collateral_ratio: Decimal::from_str("13.266737517321096939").unwrap(),
            vault_type: OldVaultType::Staked,
        }
    );

    let code_id: u64 = store_code(&wasm, &env.signer, CONTRACT_NAME.to_string());
    let _: ExecuteResponse<MsgMigrateContractResponse> = env
        .app
        .execute(
            MsgMigrateContract {
                sender: env.signer.address(),
                contract: address.clone(),
                code_id,
                msg: serde_json::to_vec(&MigrateMsg {}).unwrap(),
            },
            "/cosmwasm.wasm.v1.MsgMigrateContract",
            &env.signer,
        )
        .unwrap();

    let config: ConfigResponse = wasm.query(&address, &QueryMsg::Config {}).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            query_contract: Addr::unchecked(query_address),
            base_asset: Asset {
                denom: env.denoms["base"].clone(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: env.denoms["power"].clone(),
                decimals: 6u32,
            },
            base_pool: Pool {
                id: env.base_pool_id,
                base_denom: env.denoms["base"].clone(),
                quote_denom: env.denoms["quote"].clone(),
            },
            power_pool: Pool {
                id: env.power_pool_id,
                base_denom: env.denoms["base"].clone(),
                quote_denom: env.denoms["power"].clone(),
            },
            stake_assets: Some(vec![StakeAsset {
                denom: env.denoms["stake"].clone(),
                decimals: 6u32,
                pool: Pool {
                    id: env.stake_pool_id,
                    base_denom: env.denoms["stake"].clone(),
                    quote_denom: env.denoms["base"].clone(),
                },
            }]),
            funding_period: 1512000u64,
            fee_pool_contract: Addr::unchecked(MOCK_FEE_POOL_ADDR.to_string()),
            fee_rate: Decimal::from_str("0.1".to_string().as_str()).unwrap(),
            index_scale: SCALE_FACTOR as u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            version: CONTRACT_VERSION.to_string(),
        }
    );

    wasm.execute(
        &address,
        &ExecuteMsg::MigrateVaults {
            start_after: None,
            limit: None,
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let vault: VaultResponse = wasm
        .query(&address, &QueryMsg::GetVault { vault_id: 1u64 })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::from(6_990_000u128),
            short_amount: Uint128::from(1_000_000u128),
            collateral_ratio: Decimal::from_str("23.300202205078920431").unwrap(),
            vault_type: VaultType::Default,
        }
    );

    let vault: VaultResponse = wasm
        .query(&address, &QueryMsg::GetVault { vault_id: 4u64 })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[4].address()),
            collateral: Uint128::from(3_980_000u128),
            short_amount: Uint128::from(1_000_000u128),
            collateral_ratio: Decimal::from_str("13.266781799172260846").unwrap(),
            vault_type: VaultType::Staked {
                denom: env.denoms["stake"].clone()
            },
        }
    );

    {
        wasm.execute(&address, &ExecuteMsg::UnPause {}, &[], &env.signer)
            .unwrap();
        wasm.execute(
            &address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(1_000_000u128),
                vault_id: None,
                rebase: false,
            },
            &[coin(10_000_000, env.denoms["stake"].clone())],
            &env.traders[4],
        )
        .unwrap();
    }
}

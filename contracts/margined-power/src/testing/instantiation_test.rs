use crate::{
    contract::{CONTRACT_NAME, CONTRACT_VERSION},
    state::State,
    testing::test_utils::{MOCK_FEE_POOL_ADDR, MOCK_QUERY_ADDR},
};

use cosmwasm_std::{coin, Addr, Decimal};
use margined_protocol::power::{Asset, ConfigResponse, InstantiateMsg, Pool, QueryMsg, StakeAsset};
use margined_testing::{
    helpers::store_code,
    power_env::{PowerEnv, SCALE_FACTOR},
};
use osmosis_test_tube::{Account, Module, Wasm};
use std::str::FromStr;

#[test]
fn test_instantiation() {
    let PowerEnv {
        app,
        signer,
        denoms,
        base_pool_id,
        power_pool_id,
        .. // other fields
    } = PowerEnv::new();

    let wasm = Wasm::new(&app);

    let code_id = store_code(&wasm, &signer, CONTRACT_NAME.to_string());
    let address = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["power"].clone(),
                base_denom: denoms["base"].clone(),
                stake_assets: None,
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap()
        .data
        .address;

    let timestamp = app.get_block_timestamp();

    let config: ConfigResponse = wasm.query(&address, &QueryMsg::Config {}).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            query_contract: Addr::unchecked(MOCK_QUERY_ADDR.to_string()),
            base_asset: Asset {
                denom: denoms["base"].clone(),
                decimals: 6u32
            },
            power_asset: Asset {
                denom: denoms["power"].clone(),
                decimals: 6u32
            },
            stake_assets: None,
            base_pool: Pool {
                id: base_pool_id,
                base_denom: denoms["base"].clone(),
                quote_denom: denoms["quote"].clone(),
            },
            power_pool: Pool {
                id: power_pool_id,
                base_denom: denoms["base"].clone(),
                quote_denom: denoms["power"].clone(),
            },
            funding_period: 1512000u64,
            fee_pool_contract: Addr::unchecked(MOCK_FEE_POOL_ADDR.to_string()),
            fee_rate: Decimal::from_str("0.1".to_string().as_str()).unwrap(),
            index_scale: SCALE_FACTOR as u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            version: CONTRACT_VERSION.to_string(),
        }
    );

    let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
    assert_eq!(
        state,
        State {
            is_open: false,
            is_paused: true,
            normalisation_factor: Decimal::one(),
            last_funding_update: timestamp,
            last_pause: timestamp,
        }
    );
}

#[test]
fn test_fail_instantiation_token_not_in_pool() {
    let PowerEnv {
        app,
        signer,
        denoms,
        base_pool_id,
        power_pool_id,
        .. // other fields
    } = PowerEnv::new();

    let wasm = Wasm::new(&app);

    let code_id = store_code(&wasm, &signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["base"].clone(),
                base_denom: denoms["power"].clone(),
                stake_assets: None,
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        format!("execute error: failed to execute message; message index: 0: Generic error: Denom \"factory/{}/sqosmo\" in pool id: 1: instantiate wasm contract failed", signer.address())
    );
}

#[test]
fn test_fail_instantiation_index_scale_invalid() {
    let PowerEnv {
        app,
        signer,
        denoms,
        base_pool_id,
        power_pool_id,
        .. // other fields
    } = PowerEnv::new();

    let wasm = Wasm::new(&app);

    let code_id = store_code(&wasm, &signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["power"].clone(),
                base_denom: denoms["base"].clone(),
                stake_assets: None,
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: 0u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: Invalid index scale: instantiate wasm contract failed"
    );

    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["power"].clone(),
                base_denom: denoms["base"].clone(),
                stake_assets: None,
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: 10_001u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: Invalid index scale: instantiate wasm contract failed"
    );

    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["power"].clone(),
                base_denom: denoms["base"].clone(),
                stake_assets: None,
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: Minimum collateral amount cannot be zero: instantiate wasm contract failed"
    );
}

#[test]
fn test_fail_instantiation_zero_liquidity_in_pool() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);

    let new_power_pool_id = env.create_new_pool(
        env.denoms["power"].clone(),
        env.denoms["base"].clone(),
        &env.owner,
    );

    let code_id = store_code(&wasm, &env.signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: env.denoms["power"].clone(),
                base_denom: env.denoms["base"].clone(),
                stake_assets: None,
                base_pool_id: new_power_pool_id,
                base_pool_quote: env.denoms["quote"].clone(),
                power_pool_id: env.power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &env.signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: No liquidity in pool id: 5: instantiate wasm contract failed"
    );
}

#[test]
fn test_fail_instantiation_identical_base_and_power_pool_ids() {
    let PowerEnv {
        app,
        signer,
        denoms,
        base_pool_id,
        .. // other fields
    } = PowerEnv::new();

    let wasm = Wasm::new(&app);

    let code_id = store_code(&wasm, &signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["power"].clone(),
                base_denom: denoms["base"].clone(),
                stake_assets: None,
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id: base_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: Invalid base and power pool id must be different: instantiate wasm contract failed"
    );
}

#[test]
fn test_fail_instantiation_identical_base_and_power() {
    let PowerEnv {
        app,
        signer,
        denoms,
        base_pool_id,
        power_pool_id,
        .. // other fields
    } = PowerEnv::new();

    let wasm = Wasm::new(&app);

    let code_id = store_code(&wasm, &signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["base"].clone(),
                base_denom: denoms["base"].clone(),
                stake_assets: None,
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: Invalid base and power denom must be different: instantiate wasm contract failed"
    );
}

#[test]
fn test_fail_instantiation_duplicate_staked_assets() {
    let PowerEnv {
        app,
        signer,
        denoms,
        base_pool_id,
        power_pool_id,
        stake_pool_id,
        .. // other fields
    } = PowerEnv::new();

    let wasm = Wasm::new(&app);

    let code_id = store_code(&wasm, &signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: MOCK_QUERY_ADDR.to_string(),
                power_denom: denoms["base"].clone(),
                base_denom: denoms["base"].clone(),
                stake_assets: Some(vec![
                    StakeAsset {
                        denom: denoms["stake"].clone(),
                        decimals: 6u32,
                        pool: Pool {
                            id: stake_pool_id,
                            base_denom: denoms["stake"].clone(),
                            quote_denom: denoms["base"].clone(),
                        },
                    },
                    StakeAsset {
                        denom: denoms["stake"].clone(),
                        decimals: 6u32,
                        pool: Pool {
                            id: stake_pool_id,
                            base_denom: denoms["stake"].clone(),
                            quote_denom: denoms["base"].clone(),
                        },
                    },
                ]),
                base_pool_id,
                base_pool_quote: denoms["quote"].clone(),
                power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: Duplicate asset denom: stbase: instantiate wasm contract failed"
    );
}

use crate::{contract::CONTRACT_NAME, state::State, testing::test_utils::MOCK_FEE_POOL_ADDR};

use cosmwasm_std::{coin, Addr};
use margined_protocol::power::{ExecuteMsg, InstantiateMsg, QueryMsg};
use margined_testing::{
    helpers::store_code,
    power_env::{PowerEnv, SCALE_FACTOR},
};
use osmosis_test_tube::{
    osmosis_std::types::osmosis::tokenfactory::v1beta1::MsgChangeAdmin, Account, Module,
    RunnerError, TokenFactory, Wasm,
};

#[test]
fn test_initialise_contract() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let token = TokenFactory::new(&env.app);

    let query_address = env.deploy_query_contracts(&wasm, false);

    let code_id = store_code(&wasm, &env.signer, CONTRACT_NAME.to_string());
    let address = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: query_address,
                power_denom: env.denoms["power"].clone(),
                base_denom: env.denoms["base"].clone(),
                stake_assets: None,
                base_pool_id: env.base_pool_id,
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
        .unwrap()
        .data
        .address;

    wasm.execute(&address, &ExecuteMsg::SetOpen {}, &[], &env.signer)
        .unwrap_err();

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

    let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
    assert!(state.is_open);

    let owner: Addr = wasm.query(&address, &QueryMsg::Owner {}).unwrap();
    assert_eq!(owner, env.signer.address());
}

#[test]
fn test_initialise_contract_base_does_not_exist() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);

    let query_address = env.deploy_query_contracts(&wasm, false);

    let code_id = store_code(&wasm, &env.signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: query_address,
                power_denom: env.denoms["power"].clone(),
                base_denom: "wBTC".to_string(),
                stake_assets: None,
                base_pool_id: env.base_pool_id,
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
        err,
        RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Generic error: Denom \"wBTC\" in pool id: 1: instantiate wasm contract failed".to_string()
        }
    );
}

#[test]
fn test_initialise_contract_power_does_not_exist() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);

    let query_address = env.deploy_query_contracts(&wasm, false);

    let code_id = store_code(&wasm, &env.signer, CONTRACT_NAME.to_string());
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: query_address,
                power_denom: "wBTC".to_string(),
                base_denom: env.denoms["power"].clone(),
                stake_assets: None,
                base_pool_id: env.base_pool_id,
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
        err,
        RunnerError::ExecuteError {
            msg: format!("failed to execute message; message index: 0: Generic error: Denom \"factory/{}/sqosmo\" in pool id: 1: instantiate wasm contract failed", env.signer.address())
        }
    );
}

use crate::{contract::CONTRACT_NAME, state::Config};

use cosmwasm_std::{coins, Decimal, Uint128};
use margined_protocol::power::{
    ConfigResponse, ExecuteMsg, QueryMsg, StateResponse, UserVaultsResponse,
};
use margined_testing::power_env::{PowerEnv, BASE_PRICE, SCALED_POWER_PRICE};
use mock_query::contract::ExecuteMsg as MockQueryExecuteMsg;
use osmosis_test_tube::{Account, Module, Wasm};
use std::str::FromStr;

#[test]
fn test_query() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true);

    let config: Config = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();

    // add prices to mock pools
    {
        wasm.execute(
            config.query_contract.as_ref(),
            &MockQueryExecuteMsg::AppendPrice {
                pool_id: env.base_pool_id,
                price: Decimal::from_str("3000.0").unwrap(),
            },
            &[],
            &env.signer,
        )
        .unwrap();
        wasm.execute(
            config.query_contract.as_ref(),
            &MockQueryExecuteMsg::AppendPrice {
                pool_id: env.power_pool_id,
                price: Decimal::from_str("3030.0").unwrap(),
            },
            &[],
            &env.signer,
        )
        .unwrap();
    }

    let index: Decimal = wasm
        .query(&perp_address, &QueryMsg::GetIndex { period: 1u64 })
        .unwrap();
    assert_eq!(index, Decimal::from_str("0.09").unwrap());

    let unscaled_index: Decimal = wasm
        .query(&perp_address, &QueryMsg::GetUnscaledIndex { period: 1u64 })
        .unwrap();
    assert_eq!(unscaled_index, Decimal::from_str("9000000").unwrap());

    let denomalised_mark: Decimal = wasm
        .query(
            &perp_address,
            &QueryMsg::GetDenormalisedMark { period: 1u64 },
        )
        .unwrap();
    assert_eq!(
        denomalised_mark,
        Decimal::from_str("909.004045530105750834").unwrap()
    );

    let denomalised_mark_funding: Decimal = wasm
        .query(
            &perp_address,
            &QueryMsg::GetDenormalisedMarkFunding { period: 1u64 },
        )
        .unwrap();
    assert_eq!(denomalised_mark_funding, Decimal::from_str("909").unwrap());
}

#[test]
fn test_vault_queries() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
    let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

    assert_eq!(Decimal::one(), state.normalisation_factor);

    wasm.execute(
        config.query_contract.as_ref(),
        &MockQueryExecuteMsg::AppendPrice {
            pool_id: env.base_pool_id,
            price: Decimal::from_atomics(BASE_PRICE, 6u32).unwrap(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    wasm.execute(
        config.query_contract.as_ref(),
        &MockQueryExecuteMsg::AppendPrice {
            pool_id: env.power_pool_id,
            price: Decimal::from_atomics(SCALED_POWER_PRICE, 6u32).unwrap(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    wasm.execute(
        &perp_address,
        &ExecuteMsg::ApplyFunding {},
        &[],
        &env.signer,
    )
    .unwrap();

    // check next vault id
    {
        let next_vault_id: u64 = wasm
            .query(&perp_address, &QueryMsg::GetNextVaultId {})
            .unwrap();
        assert_eq!(next_vault_id, 1u64);
    }

    // mint 20 vaults for user
    {
        (0..20).for_each(|_| {
            env.app.increase_time(5u64);

            let collateral = coins(1_000_000u128, &env.denoms["base"]);
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(1_000_000u128),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[0],
            )
            .unwrap();
        });
    }

    // check next vault id
    {
        let next_vault_id: u64 = wasm
            .query(&perp_address, &QueryMsg::GetNextVaultId {})
            .unwrap();
        assert_eq!(next_vault_id, 21u64);
    }

    // get user vaults
    {
        let response: UserVaultsResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetUserVaults {
                    user: env.traders[0].address(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        let expected_result: Vec<u64> = (1..=10).collect();
        assert_eq!(response.vaults, expected_result);
    }

    // get user vaults - start after
    {
        let response: UserVaultsResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetUserVaults {
                    user: env.traders[0].address(),
                    start_after: Some(10u64),
                    limit: None,
                },
            )
            .unwrap();
        let expected_result: Vec<u64> = (11..=20).collect();
        assert_eq!(response.vaults, expected_result);
    }

    // get user vaults - limit
    {
        let response: UserVaultsResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetUserVaults {
                    user: env.traders[0].address(),
                    start_after: Some(5u64),
                    limit: Some(5u32),
                },
            )
            .unwrap();
        let expected_result: Vec<u64> = (6..=10).collect();
        assert_eq!(response.vaults, expected_result);
    }
}

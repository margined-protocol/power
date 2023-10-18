use crate::{
    contract::CONTRACT_NAME,
    state::{Config, State},
};

use cosmwasm_std::{coins, Decimal, Uint128};
use margined_protocol::power::{ExecuteMsg, QueryMsg};
use margined_testing::{helpers::parse_event_attribute, power_env::PowerEnv};
use mock_query::contract::ExecuteMsg as MockQueryExecuteMsg;
use osmosis_test_tube::{Module, Wasm};
use std::str::FromStr;

#[test]
fn test_funding_actions() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true);

    let config: Config = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
    let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

    assert_eq!(Decimal::one(), state.normalisation_factor);

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

    wasm.execute(
        &perp_address,
        &ExecuteMsg::ApplyFunding {},
        &[],
        &env.signer,
    )
    .unwrap();

    // NORMALISATION FACTOR TESTS
    {
        // should apply the correct normalisation factor for funding
        {
            let state_before: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            assert_eq!(
                Decimal::raw(999_994_436_783_524_723u128),
                state_before.normalisation_factor
            );

            env.app.increase_time(10_795u64); // 3 hours

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(997_593_962_860_445_984u128);

            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );
        }

        // normalisation factor changes should be bounded above
        {
            // set the prices
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
                    price: Decimal::from_str("2000.0").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            env.app.increase_time(10_785u64); // 3 hours (minus 15 seconds as there are 3 preceeding blocks)

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(995_199_251_244_479_588u128);

            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );
        }

        // normalisation factor changes should be bounded below
        {
            // set the prices
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
                    price: Decimal::from_str("6000.0").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            env.app.increase_time(10_785u64); // 3 hours (minus 15 seconds as there are 3 preceeding blocks)

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(992_810_288_103_280_623u128);
            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );
        }

        // calling apply funding with small time delta should not affect the normalisation factor
        {
            // set the prices
            wasm.execute(
                config.query_contract.as_ref(),
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: env.base_pool_id,
                    price: Decimal::from_str("3030.0").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            wasm.execute(
                config.query_contract.as_ref(),
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: env.power_pool_id,
                    price: Decimal::from_str("3000.0").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            // apply funding 0
            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            let expected_normalisation_factor: Decimal = Decimal::raw(992_806_974_302_083_222u128);

            assert_eq!(expected_normalisation_factor, state.normalisation_factor);

            // apply funding 1
            env.app.increase_time(10u64);
            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            let expected_normalisation_factor: Decimal = Decimal::raw(992_803_660_511_946_624u128);

            assert_eq!(expected_normalisation_factor, state.normalisation_factor);

            // apply funding 2
            env.app.increase_time(10u64);
            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            let expected_normalisation_factor: Decimal = Decimal::raw(992_800_346_732_870_791u128);

            assert_eq!(expected_normalisation_factor, state.normalisation_factor);
        }
    }

    // FUNDING COLLATERALISATION TESTS
    {
        let mut vault_id: u64;
        // Set prices to original values
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

        // Max power to mint = eth:usd * collateral_ratio
        let collateral_amount = Uint128::from(50_000_000u128); // 50@6dp
        let max_power_to_mint = Uint128::from(111_111_112u128); // 111.111112@6dp
        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: max_power_to_mint,
                    vault_id: None,
                    rebase: false,
                },
                &coins(collateral_amount.u128(), env.denoms["base"].to_string()),
                &env.traders[0],
            )
            .unwrap();

        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        let expected_amount_can_mint = Uint128::from(1_345_538u128);
        // should revert if minting too much power after funding
        {
            env.app.increase_time(21600u64); // 6hours
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: expected_amount_can_mint + Uint128::from(1u128),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[],
                &env.traders[0],
            )
            .unwrap_err();
        }

        // should mint more wpower after funding
        {
            env.app.increase_time(1u64);
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: expected_amount_can_mint,
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[],
                &env.traders[0],
            )
            .unwrap();
        }

        env.app.increase_time(5u64);

        // Prepare vault
        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: max_power_to_mint,
                    vault_id: None,
                    rebase: false,
                },
                &coins(collateral_amount.u128(), env.denoms["base"].to_string()),
                &env.traders[0],
            )
            .unwrap();

        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        env.app.increase_time(10795u64); // 3hours

        let max_collateral_to_remove = Uint128::from(717_000u128);
        // should revert when attempting to withdraw too much collateral
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::Withdraw {
                    amount: max_collateral_to_remove + Uint128::from(1u128),
                    vault_id,
                },
                &[],
                &env.traders[0],
            )
            .unwrap_err();
        }

        // should be able to withdraw more collateral after funding
        {
            // move one block forward
            env.app.increase_time(5u64);

            wasm.execute(
                &perp_address,
                &ExecuteMsg::Withdraw {
                    amount: max_collateral_to_remove,
                    vault_id,
                },
                &[],
                &env.traders[0],
            )
            .unwrap();
        }
    }

    // EXTREME CASES FOR NORMALISATION FACTOR
    {
        // Should get capped normalisation factor when mark = 0
        {
            let state_before: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            assert_eq!(
                Decimal::raw(985_657_792_683_450_661u128),
                state_before.normalisation_factor
            );

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
                    price: Decimal::from_str("0.0").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            env.app.increase_time(10_790u64); // 3 hours

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(987_230_796_556_633_421u128);

            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );
        }

        // Should get capped normalisation factor when index = 0
        {
            let state_before: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            assert_eq!(
                Decimal::raw(987_230_796_556_633_421u128),
                state_before.normalisation_factor
            );

            wasm.execute(
                config.query_contract.as_ref(),
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: env.base_pool_id,
                    price: Decimal::from_str("0.0001").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            wasm.execute(
                config.query_contract.as_ref(),
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: env.power_pool_id,
                    price: Decimal::from_str("3000").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            env.app.increase_time(10_790u64); // 3 hours

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(984_859_865_721_874_489u128);

            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );
        }

        // calling appyfunding() 2 * 12hrs should be equivocal to 1 * 24hrs
        {
            wasm.execute(
                config.query_contract.as_ref(),
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: env.base_pool_id,
                    price: Decimal::from_str("3000").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            wasm.execute(
                config.query_contract.as_ref(),
                &MockQueryExecuteMsg::AppendPrice {
                    pool_id: env.power_pool_id,
                    price: Decimal::from_str("3024.177466").unwrap(),
                },
                &[],
                &env.signer,
            )
            .unwrap();

            env.app.increase_time(86_390u64); // 24 hours

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(966_103_783_878_107_701u128);

            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );

            env.app.increase_time(43_195u64); // 12 hours

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(956_860_653_194_208_855u128);

            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );

            env.app.increase_time(43_195u64); // 12 hours

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.signer,
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            let expected_normalisation_factor: Decimal = Decimal::raw(947_705_955_519_542_909u128);

            assert_eq!(
                expected_normalisation_factor,
                state_after.normalisation_factor
            );
        }
    }
}

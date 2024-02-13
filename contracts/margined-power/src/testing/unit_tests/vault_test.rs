use crate::{contract::CONTRACT_NAME, state::State};

use cosmwasm_std::{coin, Decimal, Uint128};
use margined_protocol::power::{ConfigResponse, ExecuteMsg, QueryMsg};
use margined_testing::{helpers::parse_event_attribute, power_env::PowerEnv};
use osmosis_test_tube::{Module, Wasm};
use std::str::FromStr;

#[test]
fn test_check_vault_is_safe() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
    let state_initial: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
    let mut vault_id = 0u64;

    assert_eq!(Decimal::one(), state_initial.normalisation_factor);

    env.set_oracle_price(
        &wasm,
        config.query_contract.to_string(),
        env.base_pool_id,
        Decimal::from_str("3000.0").unwrap(),
    );
    env.set_oracle_price(
        &wasm,
        config.query_contract.to_string(),
        env.power_pool_id,
        Decimal::from_str("3030.0").unwrap(),
    );

    // should return true if vault does not exist
    {
        let is_safe: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(is_safe);
    }

    // should return true if vault has no short
    {
        let mint_amount = Uint128::from(45_000_000u128);
        let collateral_amount = Uint128::from(45_000_000u128);

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: mint_amount,

                    vault_id: None,
                    rebase: false,
                },
                &[coin(
                    collateral_amount.into(),
                    env.denoms["base"].to_string(),
                )],
                &env.signer,
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        env.app.increase_time(1u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::BurnPowerPerp {
                amount_to_withdraw: None,
                vault_id,
            },
            &[coin(mint_amount.into(), env.denoms["power"].to_string())],
            &env.signer,
        )
        .unwrap();

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(res);
    }

    // should mint perfect amount
    {
        env.app.increase_time(1u64);
        let mint_amount = Uint128::from(100_000_000u128);
        let collateral_amount = Uint128::from(45_000_000u128);
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: mint_amount,
                    vault_id: None,
                    rebase: false,
                },
                &[coin(
                    collateral_amount.into(),
                    env.denoms["base"].to_string(),
                )],
                &env.signer,
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(res);
    }

    // increase the price and make vault insolvent
    {
        env.set_oracle_price(
            &wasm,
            config.query_contract.to_string(),
            env.base_pool_id,
            Decimal::from_str("3010.0").unwrap(),
        );

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(!res);
    }

    // Funding rate should make the vault solvent as time passes
    {
        env.set_oracle_price(
            &wasm,
            config.query_contract.to_string(),
            env.base_pool_id,
            Decimal::from_str("3030.0").unwrap(),
        );

        env.app.increase_time(89856); // 1.04 * 86400 (1.04 days in seconds)

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(res);
    }
}

#[test]
fn test_check_vault_is_safe_staked_assets() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
    let state_initial: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
    let mut vault_id = 0u64;

    assert_eq!(Decimal::one(), state_initial.normalisation_factor);

    env.set_oracle_price(
        &wasm,
        config.query_contract.to_string(),
        env.base_pool_id,
        Decimal::from_str("3000.0").unwrap(),
    );
    env.set_oracle_price(
        &wasm,
        config.query_contract.to_string(),
        env.power_pool_id,
        Decimal::from_str("3030.0").unwrap(),
    );
    env.set_oracle_price(
        &wasm,
        config.query_contract.to_string(),
        env.stake_pool_id,
        Decimal::from_str("1.1").unwrap(),
    );

    // should return true if vault does not exist
    {
        let is_safe: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(is_safe);
    }

    // should return true if vault has no short
    {
        let mint_amount = Uint128::from(45_000_000u128);
        let collateral_amount = Uint128::from(45_000_000u128);

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: mint_amount,

                    vault_id: None,
                    rebase: false,
                },
                &[coin(
                    collateral_amount.into(),
                    env.denoms["stake"].to_string(),
                )],
                &env.signer,
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        env.app.increase_time(1u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::BurnPowerPerp {
                amount_to_withdraw: None,
                vault_id,
            },
            &[coin(mint_amount.into(), env.denoms["power"].to_string())],
            &env.signer,
        )
        .unwrap();

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(res);
    }

    // should mint perfect amount
    {
        env.app.increase_time(1u64);
        let mint_amount = Uint128::from(100_000_000u128);
        let collateral_amount = Uint128::from(45_000_000u128);
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: mint_amount,
                    vault_id: None,
                    rebase: false,
                },
                &[coin(
                    collateral_amount.into(),
                    env.denoms["stake"].to_string(),
                )],
                &env.signer,
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(res);
    }

    // increase the price, decrease stake assets value and make vault insolvent
    {
        env.set_oracle_price(
            &wasm,
            config.query_contract.to_string(),
            env.base_pool_id,
            Decimal::from_str("3010.0").unwrap(),
        );
        env.set_oracle_price(
            &wasm,
            config.query_contract.to_string(),
            env.stake_pool_id,
            Decimal::from_str("0.9").unwrap(),
        );

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(!res);
    }

    // Funding rate should make the vault solvent as time passes
    {
        env.set_oracle_price(
            &wasm,
            config.query_contract.to_string(),
            env.base_pool_id,
            Decimal::from_str("3030.0").unwrap(),
        );
        env.set_oracle_price(
            &wasm,
            config.query_contract.to_string(),
            env.stake_pool_id,
            Decimal::from_str("1.0").unwrap(),
        );

        env.app.increase_time(89856); // 1.04 * 86400 (1.04 days in seconds)

        let res: bool = wasm
            .query(&perp_address, &QueryMsg::CheckVault { vault_id })
            .unwrap();
        assert!(res);
    }
}

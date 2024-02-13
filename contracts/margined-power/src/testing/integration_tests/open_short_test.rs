use crate::contract::CONTRACT_NAME;

use cosmwasm_std::{coin, Addr, Decimal, Uint128};
use margined_protocol::power::{ExecuteMsg, QueryMsg, StateResponse, VaultResponse, VaultType};
use margined_testing::{helpers::parse_event_attribute, power_env::PowerEnv};
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::bank::v1beta1::MsgSend, cosmos::base::v1beta1::Coin,
        osmosis::concentratedliquidity::v1beta1 as CLTypes,
    },
    Account, Bank, ConcentratedLiquidity, Module, RunnerError, Wasm,
};
use std::str::FromStr;

const VAULT_COLLATERAL: u128 = 910_000u128;
const STAKE_VAULT_COLLATERAL: u128 = 819_000u128;
const VAULT_MINT_AMOUNT: u128 = 2_000_000u128;

#[test]
fn test_open_short() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    // get traders initial balances
    let trader_base_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        );
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    // open vault id 1
    {
        let open_short_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        let power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        let vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        assert_eq!(
            vault,
            VaultResponse {
                operator: Addr::unchecked(env.traders[1].address()),
                collateral: Uint128::from(VAULT_COLLATERAL),
                short_amount: Uint128::from(VAULT_MINT_AMOUNT),
                vault_type: VaultType::Default,
                collateral_ratio: Decimal::from_str("1.585702325818538239").unwrap(),
            }
        );

        let trader_base_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
        let trader_power_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

        assert_eq!(
            trader_base_balance_end,
            trader_base_balance_start - Uint128::from(VAULT_COLLATERAL) + power_exposure
        );
        assert!(trader_power_balance_end.is_zero());
        assert!(trader_power_balance_start.is_zero());
    }
}

#[test]
fn test_open_short_staked_assets() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, false);

    // get traders initial balances
    let trader_base_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());
    let trader_stake_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["stake"].clone());

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        );
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    // open vault id 1
    {
        let open_short_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: None,
                },
                &[coin(STAKE_VAULT_COLLATERAL, env.denoms["stake"].clone())],
                &env.traders[1],
            )
            .unwrap();

        let power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        let vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        assert_eq!(
            vault,
            VaultResponse {
                operator: Addr::unchecked(env.traders[1].address()),
                collateral: Uint128::from(STAKE_VAULT_COLLATERAL),
                short_amount: Uint128::from(VAULT_MINT_AMOUNT),
                vault_type: VaultType::Staked {
                    denom: env.denoms["stake"].clone()
                },
                collateral_ratio: Decimal::from_str("1.585702324232835913").unwrap(),
            }
        );

        let trader_base_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
        let trader_power_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["power"].clone());
        let trader_stake_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["stake"].clone());

        assert_eq!(
            trader_base_balance_end,
            trader_base_balance_start + power_exposure
        );
        assert_eq!(
            trader_stake_balance_end,
            trader_stake_balance_start - Uint128::from(STAKE_VAULT_COLLATERAL)
        );
        assert!(trader_power_balance_end.is_zero());
        assert!(trader_power_balance_start.is_zero());
    }
}

#[test]
fn test_open_short_existing_vault() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    // get traders initial balances
    let trader_base_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        )
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    let vault_id: u64;
    // open vault id 1
    {
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // perform open short to the original vault
    {
        env.app.increase_time(1u64);

        let open_short_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: Some(vault_id),
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        let power_exposure = Uint128::from_str(
            &parse_event_attribute(open_short_response.events, "token_swapped", "tokens_out")
                .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        assert_eq!(
            vault,
            VaultResponse {
                operator: Addr::unchecked(env.traders[1].address()),
                collateral: Uint128::from(VAULT_COLLATERAL * 2),
                short_amount: Uint128::from(VAULT_MINT_AMOUNT * 2),
                vault_type: VaultType::Default,
                collateral_ratio: Decimal::from_str("1.585704442925746501").unwrap(),
            }
        );

        let trader_base_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
        let trader_power_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

        assert_eq!(
            trader_base_balance_end,
            trader_base_balance_start - Uint128::from(VAULT_COLLATERAL * 2) + power_exposure
        );
        assert_eq!(trader_power_balance_end, Uint128::from(VAULT_MINT_AMOUNT));
        assert!(trader_power_balance_start.is_zero());
    }
}

#[test]
fn test_open_short_existing_vault_staked_asset() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, false);

    // get traders initial balances
    let trader_base_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());
    let trader_stake_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["stake"].clone());

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        )
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    let vault_id: u64;
    // open vault id 1
    {
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(STAKE_VAULT_COLLATERAL, env.denoms["stake"].clone())],
                &env.traders[1],
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // try to increase with base asset
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: Some(vault_id),
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();

        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Vault Type: expected Staked, got Default - vault_id: 1: execute wasm contract failed".to_string()
        });
    }

    // perform open short to the original vault
    {
        env.app.increase_time(1u64);

        let open_short_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: Some(vault_id),
                    slippage: None,
                },
                &[coin(STAKE_VAULT_COLLATERAL, env.denoms["stake"].clone())],
                &env.traders[1],
            )
            .unwrap();

        let power_exposure = Uint128::from_str(
            &parse_event_attribute(open_short_response.events, "token_swapped", "tokens_out")
                .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        assert_eq!(
            vault,
            VaultResponse {
                operator: Addr::unchecked(env.traders[1].address()),
                collateral: Uint128::from(STAKE_VAULT_COLLATERAL * 2),
                short_amount: Uint128::from(VAULT_MINT_AMOUNT * 2),
                vault_type: VaultType::Staked {
                    denom: env.denoms["stake"].clone()
                },
                collateral_ratio: Decimal::from_str("1.585706205776055089").unwrap(),
            }
        );

        let trader_base_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
        let trader_power_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["power"].clone());
        let trader_stake_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["stake"].clone());

        assert_eq!(
            trader_stake_balance_end,
            trader_stake_balance_start - Uint128::from(STAKE_VAULT_COLLATERAL * 2)
        );
        assert_eq!(
            trader_base_balance_end,
            trader_base_balance_start + power_exposure
        );
        assert_eq!(trader_power_balance_end, Uint128::from(VAULT_MINT_AMOUNT));
        assert!(trader_power_balance_start.is_zero());
    }
}

#[test]
fn test_fail_open_short_existing_vault_staked_asset() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, false);

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        )
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    let vault_id: u64;
    // open vault id 1
    {
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(STAKE_VAULT_COLLATERAL, env.denoms["stake"].clone())],
                &env.traders[1],
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // try to increase with milk asset
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: Some(vault_id),
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["milk"].clone())],
                &env.traders[1],
            )
            .unwrap_err();

        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Vault Type: expected Staked, got Staked - vault_id: 1: execute wasm contract failed".to_string()
        });
    }
}

#[test]
fn test_fail_open_short_existing_vault_incorrect_user() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        )
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    let vault_id: u64;
    // open vault id 1
    {
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // perform open short to the original vault
    {
        env.app.increase_time(1u64);

        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: Some(vault_id),
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[0],
            )
            .unwrap_err();

        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
        });
    }
}

#[test]
fn test_fail_open_short_insufficient_funds() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    let trader = env
        .app
        .init_account(&[coin(1_000_000_000_000_000_000, "uosmo")])
        .unwrap();

    bank.send(
        MsgSend {
            from_address: env.signer.address(),
            to_address: trader.address(),
            amount: vec![Coin {
                denom: env.denoms["base"].to_string(),
                amount: VAULT_COLLATERAL.to_string(),
            }],
        },
        &env.signer,
    )
    .unwrap();

    // apply funding
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        )
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    // open vault id 1
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL + 1u128, env.denoms["base"].clone())],
                &trader,
            )
            .unwrap_err();
        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: 910000ubase is smaller than 910001ubase: insufficient funds".to_string()
        });
    }
}

#[test]
fn test_fail_open_short_incorrect_funds() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        )
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    // open vault id 1
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL + 1u128, env.denoms["gas"].clone())],
                &env.traders[0],
            )
            .unwrap_err();
        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
        });
    }
}

#[test]
fn test_fail_open_short_insufficient_liquidity() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    // remove position and make a much less liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            "-7500000".to_string(),
            "750000".to_string(),
            "300_000".to_string(),
            "1_000_000".to_string(),
        );
    }
    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    let trader = env
        .app
        .init_account(&[coin(1_000_000_000_000_000_000, "uosmo")])
        .unwrap();

    bank.send(
        MsgSend {
            from_address: env.signer.address(),
            to_address: trader.address(),
            amount: vec![Coin {
                denom: env.denoms["base"].to_string(),
                amount: VAULT_COLLATERAL.to_string(),
            }],
        },
        &env.signer,
    )
    .unwrap();

    // apply funding
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // open vault id 1
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: None,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &trader,
            )
            .unwrap_err();
        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: dispatch: submessages: reply: dispatch: submessages: ran out of ticks for pool (4) during swap".to_string()
        });
    }
}

#[test]
fn test_fail_open_short_slippage_exceeded() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    // get traders initial balances
    let trader_base_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    // apply funding
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::ApplyFunding {},
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // owner closes position and makes a much liquid one
    {
        let res = concentrated_liquidity
            .query_user_positions(&CLTypes::UserPositionsRequest {
                pool_id: env.power_pool_id,
                address: env.owner.address(),
                pagination: None,
            })
            .unwrap();

        let position = res.positions[0].clone().position.unwrap();

        concentrated_liquidity
            .withdraw_position(
                CLTypes::MsgWithdrawPosition {
                    position_id: position.position_id,
                    sender: env.owner.address(),
                    liquidity_amount: position.liquidity,
                },
                &env.owner,
            )
            .unwrap();

        env.app.increase_time(10u64);

        let state: StateResponse = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        let target_price_power = env.calculate_target_power_price(state.normalisation_factor);

        let target_price = Decimal::one().checked_div(target_price_power).unwrap();

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(90), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(110), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412 =
        env.create_position(
            lower_tick,
            upper_tick,
            "3_000_000_000".to_string(),
            "1_000_000_000".to_string(),
        );
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    // fail slippage too high
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: Some(Decimal::from_ratio(2u128, 1u128)),
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();
        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: dispatch: submessages: reply: Generic error: Slippage cannot be greater than 1: execute wasm contract failed".to_string()
        });
    }

    // fail slippage too small
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: Some(Decimal::from_ratio(1u128, 1_000u128)),
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();
        assert_eq!(err,
                RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: dispatch: submessages: reply: dispatch: submessages: token amount calculated (660006) is lesser than min amount (5994000)".to_string()
            });
    }

    // open vault id 1
    {
        let open_short_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::OpenShort {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    slippage: Some(Decimal::from_ratio(99u128, 100u128)),
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        let power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        let vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        assert_eq!(
            vault,
            VaultResponse {
                operator: Addr::unchecked(env.traders[1].address()),
                collateral: Uint128::from(VAULT_COLLATERAL),
                short_amount: Uint128::from(VAULT_MINT_AMOUNT),
                vault_type: VaultType::Default,
                collateral_ratio: Decimal::from_str("1.585705854687819671").unwrap(),
            }
        );

        let trader_base_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
        let trader_power_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

        assert_eq!(
            trader_base_balance_end,
            trader_base_balance_start - Uint128::from(VAULT_COLLATERAL) + power_exposure
        );
        assert!(trader_power_balance_end.is_zero());
        assert!(trader_power_balance_start.is_zero());
    }
}

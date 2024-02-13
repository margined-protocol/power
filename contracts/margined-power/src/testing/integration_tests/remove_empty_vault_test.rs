use crate::contract::CONTRACT_NAME;

use cosmwasm_std::{coin, Decimal, Uint128};
use margined_protocol::power::{ExecuteMsg, QueryMsg, StateResponse, UserVaultsResponse};
use margined_testing::{helpers::parse_event_attribute, power_env::PowerEnv};
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::{bank::v1beta1::MsgSend, base::v1beta1::Coin as BaseCoin},
        osmosis::concentratedliquidity::v1beta1 as CLTypes,
    },
    Account, Bank, ConcentratedLiquidity, Module, RunnerError, Wasm,
};
use std::str::FromStr;

const VAULT_COLLATERAL: u128 = 910_000u128;
const VAULT_MINT_AMOUNT: u128 = 2_000_000u128;

#[test]
fn test_remove_empty_vault() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
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

    // open short
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

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // mint
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(VAULT_MINT_AMOUNT),
                vault_id: None,
                rebase: false,
            },
            &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
            &env.traders[2],
        )
        .unwrap();
    }

    bank.send(
        MsgSend {
            from_address: env.traders[2].address(),
            to_address: env.traders[1].address(),
            amount: vec![BaseCoin {
                amount: VAULT_MINT_AMOUNT.to_string(),
                denom: env.denoms["power"].clone(),
            }],
        },
        &env.traders[2],
    )
    .unwrap();

    // close short
    {
        env.app.increase_time(1u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::CloseShort {
                amount_to_burn: Uint128::zero(),
                amount_to_withdraw: Some((VAULT_COLLATERAL).into()),
                vault_id,
            },
            &[coin(VAULT_MINT_AMOUNT, env.denoms["power"].clone())],
            &env.traders[1],
        )
        .unwrap();
    }

    {
        let response: UserVaultsResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetUserVaults {
                    user: env.traders[1].address(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(response.vaults.len(), 1usize);
    }

    // remove vault
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::RemoveEmptyVaults {
                start_after: None,
                limit: None,
            },
            &[],
            &env.traders[0],
        )
        .unwrap();
    }

    {
        let response: UserVaultsResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetUserVaults {
                    user: env.traders[1].address(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(response.vaults.len(), 0usize);
    }
}

#[test]
fn test_fail_remove_empty_vault_has_short_exposure() {
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

    // open short
    {
        wasm.execute(
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
    }

    // try to close vault before it's empty
    {
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::RemoveEmptyVaults {
                    start_after: None,
                    limit: None,
                },
                &[],
                &env.traders[1],
            )
            .unwrap_err();

        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg:"failed to execute message; message index: 0: No empty vaults found: execute wasm contract failed".to_string(),
            }
        );
    }
}

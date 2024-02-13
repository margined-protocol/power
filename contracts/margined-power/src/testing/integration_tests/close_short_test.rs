use crate::contract::CONTRACT_NAME;

use cosmwasm_std::{coin, Addr, Decimal, Uint128};
use margined_protocol::power::{ExecuteMsg, QueryMsg, StateResponse, VaultResponse, VaultType};
use margined_testing::{helpers::parse_event_attribute, power_env::PowerEnv};
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::{bank::v1beta1::MsgSend, base::v1beta1::Coin as BaseCoin},
        osmosis::concentratedliquidity::v1beta1 as CLTypes,
        osmosis::poolmanager::v1beta1 as PMTypes,
    },
    Account, Bank, ConcentratedLiquidity, Module, PoolManager, RunnerError, Wasm,
};
use std::str::FromStr;

const VAULT_COLLATERAL: u128 = 910_000u128;
const STAKED_VAULT_COLLATERAL: u128 = 819_000u128;
const VAULT_MINT_AMOUNT: u128 = 2_000_000u128;

#[test]
fn test_close_short() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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
    let power_exposure: Uint128;
    let pnl: Uint128;

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

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!("{}{}", vault.short_amount, env.denoms["power"].clone()),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        pnl = amount_to_swap
            .checked_sub(power_exposure.u128())
            .unwrap()
            .into();

        wasm.execute(
            &perp_address,
            &ExecuteMsg::CloseShort {
                amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                amount_to_withdraw: Some(VAULT_COLLATERAL.into()),
                vault_id,
            },
            &[coin(amount_to_swap, env.denoms["base"].clone())],
            &env.traders[1],
        )
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::zero(),
            short_amount: Uint128::from(1u128), // dust is left over
            vault_type: VaultType::Default,
            collateral_ratio: Decimal::zero(),
        }
    );

    let trader_base_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    assert_eq!(trader_base_balance_end, trader_base_balance_start - pnl);
    assert_eq!(trader_power_balance_end, trader_power_balance_start);
}

#[test]
fn test_close_short_staked_assets() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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
    let power_exposure: Uint128;
    let pnl: Uint128;

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
                &[coin(STAKED_VAULT_COLLATERAL, env.denoms["stake"].clone())],
                &env.traders[1],
            )
            .unwrap();

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!("{}{}", vault.short_amount, env.denoms["power"].clone()),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        pnl = amount_to_swap
            .checked_sub(power_exposure.u128())
            .unwrap()
            .into();

        wasm.execute(
            &perp_address,
            &ExecuteMsg::CloseShort {
                amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                amount_to_withdraw: Some(STAKED_VAULT_COLLATERAL.into()),
                vault_id,
            },
            &[coin(amount_to_swap, env.denoms["base"].clone())],
            &env.traders[1],
        )
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::zero(),
            short_amount: Uint128::from(1u128), // dust is left over
            vault_type: VaultType::Staked {
                denom: env.denoms["stake"].clone()
            },
            collateral_ratio: Decimal::zero(),
        }
    );

    let trader_base_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());
    let trader_stake_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["stake"].clone());

    assert_eq!(trader_base_balance_end, trader_base_balance_start - pnl);
    assert_eq!(trader_power_balance_end, trader_power_balance_start);
    assert_eq!(trader_stake_balance_end, trader_stake_balance_start);
}

#[test]
fn test_close_short_with_unsold_power_asset() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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
    let power_exposure: Uint128;
    let pnl: Uint128;

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

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    let vault_open_short: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    // mint
    {
        env.app.increase_time(1u64);
        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(VAULT_MINT_AMOUNT),
                vault_id: Some(vault_id),
                rebase: false,
            },
            &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
            &env.traders[1],
        )
        .unwrap();
    }

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!(
                        "{}{}",
                        vault_open_short.short_amount,
                        env.denoms["power"].clone()
                    ),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        pnl = amount_to_swap
            .checked_sub(power_exposure.u128())
            .unwrap()
            .into();

        wasm.execute(
            &perp_address,
            &ExecuteMsg::CloseShort {
                amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                amount_to_withdraw: Some((VAULT_COLLATERAL * 2).into()),
                vault_id,
            },
            &[
                coin(VAULT_MINT_AMOUNT, env.denoms["power"].clone()),
                coin(amount_to_swap, env.denoms["base"].clone()),
            ],
            &env.traders[1],
        )
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::zero(),
            short_amount: Uint128::from(1u128), // dust is left over
            vault_type: VaultType::Default,
            collateral_ratio: Decimal::zero(),
        }
    );

    let trader_base_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    assert_eq!(trader_base_balance_end, trader_base_balance_start - pnl);
    assert_eq!(trader_power_balance_end, trader_power_balance_start);
}

#[test]
fn test_partial_close_short_with_unsold_power_asset() {
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
                vault_id: Some(vault_id),
                rebase: false,
            },
            &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
            &env.traders[1],
        )
        .unwrap();
    }

    // get traders initial balances
    let trader_base_balance_before: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_before: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

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
            collateral_ratio: Decimal::from_str("1.585704085222694305").unwrap(),
        }
    );

    let trader_base_balance_after: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_after: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    assert_eq!(
        trader_power_balance_after,
        trader_power_balance_before - Uint128::from(VAULT_MINT_AMOUNT)
    );
    assert_eq!(
        trader_base_balance_after,
        trader_base_balance_before + Uint128::from(VAULT_COLLATERAL)
    )
}

#[test]
fn test_close_short_with_unsold_power_asset_with_tokens_not_minted() {
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

    // get traders initial balances
    let trader_base_balance_before: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_before: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

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

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::zero(),
            short_amount: Uint128::zero(),
            vault_type: VaultType::Default,
            collateral_ratio: Decimal::zero(),
        }
    );

    let trader_base_balance_after: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_after: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    assert_eq!(
        trader_power_balance_after,
        trader_power_balance_before - Uint128::from(VAULT_MINT_AMOUNT)
    );
    assert_eq!(
        trader_base_balance_after,
        trader_base_balance_before + Uint128::from(VAULT_COLLATERAL)
    )
}

#[test]
fn test_close_short_no_withdrawal() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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
    let power_exposure: Uint128;
    let pnl: Uint128;

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

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!("{}{}", vault.short_amount, env.denoms["power"].clone()),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        pnl = amount_to_swap
            .checked_sub(power_exposure.u128())
            .unwrap()
            .into();

        wasm.execute(
            &perp_address,
            &ExecuteMsg::CloseShort {
                amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                amount_to_withdraw: None,
                vault_id,
            },
            &[coin(amount_to_swap, env.denoms["base"].clone())],
            &env.traders[1],
        )
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::from(VAULT_COLLATERAL),
            short_amount: Uint128::from(1u128), // dust is left over
            vault_type: VaultType::Default,
            collateral_ratio: Decimal::from_str("3171406.41104704247786809").unwrap(),
        }
    );

    let trader_base_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    assert_eq!(
        trader_base_balance_end,
        trader_base_balance_start - pnl - Uint128::from(VAULT_COLLATERAL)
    );
    assert_eq!(trader_power_balance_end, trader_power_balance_start);
}

#[test]
fn test_close_short_additional_funds_sent() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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
    let power_exposure: Uint128;
    let pnl: Uint128;

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

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    let contract_balance_before = env.get_balance(perp_address.clone(), env.denoms["base"].clone());

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!("{}{}", vault.short_amount, env.denoms["power"].clone()),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        pnl = amount_to_swap
            .checked_sub(power_exposure.u128())
            .unwrap()
            .into();

        wasm.execute(
            &perp_address,
            &ExecuteMsg::CloseShort {
                amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                amount_to_withdraw: Some(VAULT_COLLATERAL.into()),
                vault_id,
            },
            &[coin(
                amount_to_swap + 1_000_000u128,
                env.denoms["base"].clone(),
            )],
            &env.traders[1],
        )
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    assert_eq!(
        vault,
        VaultResponse {
            operator: Addr::unchecked(env.traders[1].address()),
            collateral: Uint128::zero(),
            short_amount: Uint128::from(1u128), // dust is left over
            vault_type: VaultType::Default,
            collateral_ratio: Decimal::zero(),
        }
    );

    let contract_balance_after = env.get_balance(perp_address, env.denoms["base"].clone());

    let trader_base_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_power_balance_end: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    assert_eq!(trader_base_balance_end, trader_base_balance_start - pnl);
    assert_eq!(trader_power_balance_end, trader_power_balance_start);
    assert_eq!(contract_balance_before, Uint128::from(VAULT_COLLATERAL));
    assert!(contract_balance_after.is_zero());
}

#[test]
fn test_fail_close_short_insufficient_funds() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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
    let power_exposure: Uint128;

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

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!("{}{}", vault.short_amount, env.denoms["power"].clone()),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::CloseShort {
                    amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                    amount_to_withdraw: Some(VAULT_COLLATERAL.into()),
                    vault_id,
                },
                &[coin(amount_to_swap - 1u128, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();

        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: format!("failed to execute message; message index: 0: dispatch: submessages: token amount calculated ({}) is greater than max amount ({})", amount_to_swap, power_exposure)
        });
    }
}

#[test]
fn test_fail_close_short_burn_greater_than_vault() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(50), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(150), 100u128.into());

        // lower tick: 3.1 = 1/3.1 = 0.32258
        // lower tick: 3.4 = 1/3.4 = 0.29412
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

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!("{}{}", vault.short_amount, env.denoms["power"].clone()),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::CloseShort {
                    amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT) + Uint128::one(),
                    amount_to_withdraw: None,
                    vault_id,
                },
                &[coin(amount_to_swap, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();

        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Generic error: Cannot burn more funds or collateral than in vault: execute wasm contract failed".to_string()
        });
    }
}

#[test]
fn test_fail_close_short_withdraw_greater_than_vault() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

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

        let lower_tick = env.price_to_tick(target_price * Decimal::percent(50), 100u128.into());
        let upper_tick = env.price_to_tick(target_price * Decimal::percent(150), 100u128.into());

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

    let vault: VaultResponse = wasm
        .query(&perp_address, &QueryMsg::GetVault { vault_id })
        .unwrap();

    // close short
    {
        env.app.increase_time(1u64);

        let res = pool_manager
            .query_single_pool_swap_exact_amount_out(
                &PMTypes::EstimateSinglePoolSwapExactAmountOutRequest {
                    pool_id: env.power_pool_id,
                    token_in_denom: env.denoms["base"].clone(),
                    token_out: format!("{}{}", vault.short_amount, env.denoms["power"].clone()),
                },
            )
            .unwrap();

        let amount_to_swap = u128::from_str(&res.token_in_amount).unwrap();

        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::CloseShort {
                    amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                    amount_to_withdraw: Some(Uint128::from(VAULT_COLLATERAL) + Uint128::one()),
                    vault_id,
                },
                &[coin(amount_to_swap, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();

        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Generic error: Cannot burn more funds or collateral than in vault: execute wasm contract failed".to_string()
        });
    }
}

#[test]
fn test_fail_close_short_incorrect_sender() {
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
    let power_exposure: Uint128;

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

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // close short
    {
        env.app.increase_time(1u64);

        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::CloseShort {
                    amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                    amount_to_withdraw: Some(VAULT_COLLATERAL.into()),
                    vault_id,
                },
                &[coin(power_exposure.u128(), env.denoms["base"].clone())],
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
fn test_fail_close_short_incorrect_funds() {
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
    let power_exposure: Uint128;

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

        power_exposure = Uint128::from_str(
            &parse_event_attribute(
                open_short_response.events.clone(),
                "token_swapped",
                "tokens_out",
            )
            .replace(&env.denoms["base"], ""),
        )
        .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            open_short_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // close short
    {
        env.app.increase_time(1u64);

        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::CloseShort {
                    amount_to_burn: Uint128::from(VAULT_MINT_AMOUNT),
                    amount_to_withdraw: Some(VAULT_COLLATERAL.into()),
                    vault_id,
                },
                &[coin(power_exposure.u128(), env.denoms["gas"].clone())],
                &env.traders[1],
            )
            .unwrap_err();

        assert_eq!(err,
            RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
        });
    }
}

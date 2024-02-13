use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};

use cosmwasm_std::{coin, Addr, Decimal, Uint128};
use margined_protocol::power::{
    Asset, ConfigResponse, ExecuteMsg, Pool, QueryMsg, StateResponse, VaultResponse,
};
use margined_testing::{
    helpers::parse_event_attribute,
    power_env::{PowerEnv, SCALE_FACTOR},
};
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::bank::v1beta1::MsgSend,
        cosmos::base::v1beta1::Coin,
        osmosis::concentratedliquidity::v1beta1 as CLTypes,
        osmosis::poolmanager::v1beta1::{
            MsgSwapExactAmountIn, MsgSwapExactAmountOut, SwapAmountInRoute, SwapAmountOutRoute,
            TotalPoolLiquidityRequest,
        },
    },
    Account, Bank, ConcentratedLiquidity, Module, PoolManager, Wasm,
};
use std::str::FromStr;

const COLLATERAL_AMOUNT: u128 = 100_000_000u128; // 100.0
const SHORT_AMOUNT: u128 = 166_666_667u128; // 166.666667

#[test]
fn test_end_to_end_flow() {
    let env: PowerEnv = PowerEnv::new();

    // move two days forward
    env.app.increase_time(172800u64);

    let bank = Bank::new(&env.app);
    let concentrated_liquidity = ConcentratedLiquidity::new(&env.app);
    let wasm = Wasm::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

    let (perp_address, query_address) =
        env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    let vault_id_short: u64;

    // get traders initial balances
    let trader_long_base_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
    let trader_long_power_balance_start: Uint128 =
        env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

    let trader_short_base_balance_start: Uint128 =
        env.get_balance(env.traders[0].address(), env.denoms["base"].clone());
    let trader_short_power_balance_start: Uint128 =
        env.get_balance(env.traders[0].address(), env.denoms["power"].clone());

    // update the config
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::UpdateConfig {
                fee_rate: Some("0.01".to_string()),
                fee_pool: None,
            },
            &[],
            &env.signer,
        )
        .unwrap();
    }

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
        concentrated_liquidity
            .create_position(
                CLTypes::MsgCreatePosition {
                    pool_id: env.power_pool_id,
                    sender: env.owner.address(),
                    lower_tick: i64::from_str(&lower_tick).unwrap(),
                    upper_tick: i64::from_str(&upper_tick).unwrap(),
                    tokens_provided: vec![
                        Coin {
                            denom: env.denoms["power"].clone(),
                            amount: "1_000_000_000".to_string(),
                        },
                        Coin {
                            denom: env.denoms["base"].clone(),
                            amount: "3_000_000_000".to_string(),
                        },
                    ],
                    token_min_amount0: "0".to_string(),
                    token_min_amount1: "0".to_string(),
                },
                &env.owner,
            )
            .unwrap();
    }

    // we increase time else the functions get unhappy
    env.app.increase_time(200000u64);

    // user goes short - fails as zero collateral, vault not safe
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(SHORT_AMOUNT),
                vault_id: None,
                rebase: false,
            },
            &[],
            &env.traders[0],
        )
        .unwrap_err();
    }

    // user goes short - succeeds
    {
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(SHORT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(COLLATERAL_AMOUNT, env.denoms["base"].clone())],
                &env.traders[0],
            )
            .unwrap();

        vault_id_short = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let res: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_short,
                },
            )
            .unwrap();
        assert!(res);
    }

    let vault_initial: VaultResponse = wasm
        .query(
            &perp_address,
            &QueryMsg::GetVault {
                vault_id: vault_id_short,
            },
        )
        .unwrap();

    // user withdraws all the collateral - fails, as vault is unsafe
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::Withdraw {
                amount: Uint128::from(COLLATERAL_AMOUNT),
                vault_id: vault_id_short,
            },
            &[],
            &env.traders[0],
        )
        .unwrap_err();

        let res: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_short,
                },
            )
            .unwrap();
        assert!(res);
    }

    // user withdraws some collateral - success as vault remains safe
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::Withdraw {
                amount: Uint128::from(17_000_000u128),
                vault_id: vault_id_short,
            },
            &[],
            &env.traders[0],
        )
        .unwrap();

        let res: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_short,
                },
            )
            .unwrap();
        assert!(res);
    }

    // user withdraws additional collateral - fails as vault is unsafe
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::Withdraw {
                amount: Uint128::from(3_000_000u128),
                vault_id: vault_id_short,
            },
            &[],
            &env.traders[0],
        )
        .unwrap_err();
    }

    // user withdraws additional collateral - succeeds as it is the exact limit
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::Withdraw {
                amount: Uint128::from(2_000_000u128),
                vault_id: vault_id_short,
            },
            &[],
            &env.traders[0],
        )
        .unwrap();
    }

    // user deposits some collateral
    {
        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::Deposit {
                vault_id: vault_id_short,
            },
            &[coin(19_000_000u128, env.denoms["base"].clone())],
            &env.traders[0],
        )
        .unwrap();

        let res: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_short,
                },
            )
            .unwrap();
        assert!(res);
    }

    let vault_after: VaultResponse = wasm
        .query(
            &perp_address,
            &QueryMsg::GetVault {
                vault_id: vault_id_short,
            },
        )
        .unwrap();
    assert_eq!(vault_initial.operator, vault_after.operator);
    assert_eq!(vault_initial.collateral, vault_after.collateral);
    assert_eq!(vault_initial.short_amount, vault_after.short_amount);

    // user sells the power token
    {
        pool_manager
            .swap_exact_amount_in(
                MsgSwapExactAmountIn {
                    sender: env.traders[0].address(),
                    routes: vec![SwapAmountInRoute {
                        pool_id: env.power_pool_id,
                        token_out_denom: env.denoms["base"].clone(),
                    }],
                    token_in: Some(Coin {
                        amount: SHORT_AMOUNT.to_string(),
                        denom: env.denoms["power"].clone(),
                    }),
                    token_out_min_amount: SHORT_AMOUNT.checked_div(4u128).unwrap().to_string(),
                },
                &env.traders[0],
            )
            .unwrap();

        let balance_after_liquidity_provision =
            env.get_balance(env.traders[1].address(), env.denoms["power"].clone());
        assert!(balance_after_liquidity_provision.is_zero());
    }

    // trader buys the power token
    {
        let balance_before = env.get_balance(env.traders[1].address(), env.denoms["power"].clone());
        assert!(balance_before.is_zero());

        let liquidity_to_buy = SHORT_AMOUNT.checked_div(3u128).unwrap();

        pool_manager
            .swap_exact_amount_out(
                MsgSwapExactAmountOut {
                    sender: env.traders[1].address(),
                    routes: vec![SwapAmountOutRoute {
                        pool_id: env.power_pool_id,
                        token_in_denom: env.denoms["base"].clone(),
                    }],
                    token_out: Some(Coin {
                        amount: liquidity_to_buy.to_string(),
                        denom: env.denoms["power"].clone(),
                    }),
                    token_in_max_amount: liquidity_to_buy.checked_div(3u128).unwrap().to_string(),
                },
                &env.traders[1],
            )
            .unwrap();
    }

    // make vault unsafe by pushing base price higher 2x
    {
        let res = pool_manager
            .query_total_liquidity(&TotalPoolLiquidityRequest {
                pool_id: env.base_pool_id,
            })
            .unwrap();

        let liquidity_to_sell = Uint128::from_str(
            res.liquidity
                .iter()
                .find(|l| l.denom == env.denoms["quote"])
                .unwrap()
                .amount
                .as_str(),
        )
        .unwrap()
        .checked_div(24u128.into())
        .unwrap()
        .checked_mul(10u128.into())
        .unwrap();

        pool_manager
            .swap_exact_amount_in(
                MsgSwapExactAmountIn {
                    sender: env.signer.address(),
                    routes: vec![SwapAmountInRoute {
                        pool_id: env.base_pool_id,
                        token_out_denom: env.denoms["base"].clone(),
                    }],
                    token_in: Some(Coin {
                        amount: liquidity_to_sell.to_string(),
                        denom: env.denoms["quote"].clone(),
                    }),
                    token_out_min_amount: "1".to_string(),
                },
                &env.signer,
            )
            .unwrap();

        // increase time to affect the TWAP
        env.app.increase_time(3600u64);

        let vault_is_safe: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_short,
                },
            )
            .unwrap();
        assert!(!vault_is_safe);
    }

    // liquidate the short vault
    {
        let vault_before: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_short,
                },
            )
            .unwrap();

        let amount_to_send = vault_before.short_amount.checked_div(2u128.into()).unwrap();
        bank.send(
            MsgSend {
                from_address: env.owner.address(),
                to_address: env.traders[2].address(),
                amount: vec![Coin {
                    denom: env.denoms["power"].clone(),
                    amount: amount_to_send.to_string(),
                }],
            },
            &env.owner,
        )
        .unwrap();

        let balance = env.get_balance(env.traders[2].address(), env.denoms["power"].clone());
        assert_eq!(balance, amount_to_send);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: vault_id_short,
                max_debt_amount: vault_before.short_amount,
            },
            &[],
            &env.traders[2],
        )
        .unwrap();

        let balance = env.get_balance(env.traders[2].address(), env.denoms["power"].clone());
        assert!(balance.is_zero());
    }

    // short trader closes vault
    {
        let vault: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_short,
                },
            )
            .unwrap();

        pool_manager
            .swap_exact_amount_out(
                MsgSwapExactAmountOut {
                    sender: env.traders[0].address(),
                    routes: vec![SwapAmountOutRoute {
                        pool_id: env.power_pool_id,
                        token_in_denom: env.denoms["base"].clone(),
                    }],
                    token_out: Some(Coin {
                        amount: vault.short_amount.to_string(),
                        denom: env.denoms["power"].clone(),
                    }),
                    token_in_max_amount: "10000000000".to_string(),
                },
                &env.traders[0],
            )
            .unwrap();

        let balance = env.get_balance(env.traders[0].address(), env.denoms["power"].clone());
        assert_eq!(balance, vault.short_amount);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::BurnPowerPerp {
                vault_id: vault_id_short,
                amount_to_withdraw: None,
            },
            &[coin(vault.short_amount.u128(), env.denoms["power"].clone())],
            &env.traders[0],
        )
        .unwrap();

        let balance = env.get_balance(env.traders[0].address(), env.denoms["power"].clone());
        assert!(balance.is_zero());
    }

    // long trader sells all remaining power tokens
    {
        let balance = env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

        pool_manager
            .swap_exact_amount_in(
                MsgSwapExactAmountIn {
                    sender: env.traders[1].address(),
                    routes: vec![SwapAmountInRoute {
                        pool_id: env.power_pool_id,
                        token_out_denom: env.denoms["base"].clone(),
                    }],
                    token_in: Some(Coin {
                        amount: balance.to_string(),
                        denom: env.denoms["power"].clone(),
                    }),
                    token_out_min_amount: "1".to_string(),
                },
                &env.traders[1],
            )
            .unwrap();
    }

    // check final balances
    {
        let trader_long_base_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["base"].clone());
        let trader_long_power_balance_end: Uint128 =
            env.get_balance(env.traders[1].address(), env.denoms["power"].clone());

        let trader_short_base_balance_end: Uint128 =
            env.get_balance(env.traders[0].address(), env.denoms["base"].clone());
        let trader_short_power_balance_end: Uint128 =
            env.get_balance(env.traders[0].address(), env.denoms["power"].clone());

        // trader who went long should have gained funds
        assert!(trader_long_base_balance_start < trader_long_base_balance_end);
        assert!(trader_long_power_balance_start.is_zero());
        assert!(trader_long_power_balance_end.is_zero());

        // trader who went short should have lost funds
        assert!(trader_short_base_balance_end < trader_short_base_balance_start);
        assert!(trader_short_power_balance_start.is_zero());
        assert!(trader_short_power_balance_end.is_zero());
    }

    // Validate all queries work as anticipated
    {
        let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
        assert_eq!(
            config,
            ConfigResponse {
                fee_rate: Decimal::percent(1u64),
                fee_pool_contract: Addr::unchecked(env.fee_pool.address()),
                query_contract: Addr::unchecked(query_address),
                base_asset: Asset {
                    denom: env.denoms["base"].clone(),
                    decimals: 6u32,
                },
                power_asset: Asset {
                    denom: env.denoms["power"].clone(),
                    decimals: 6u32,
                },
                stake_assets: None,
                base_pool: Pool {
                    id: env.base_pool_id,
                    base_denom: env.denoms["base"].clone(),
                    quote_denom: env.denoms["quote"].clone()
                },
                power_pool: Pool {
                    id: env.power_pool_id,
                    base_denom: env.denoms["base"].clone(),
                    quote_denom: env.denoms["power"].clone()
                },
                funding_period: 1512000u64,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: Decimal::from_str("0.5").unwrap(),
                version: CONTRACT_VERSION.to_string(),
            }
        );
    }
}

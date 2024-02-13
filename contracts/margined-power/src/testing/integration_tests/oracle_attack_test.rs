use crate::contract::CONTRACT_NAME;

use cosmwasm_std::{coin, Decimal, Uint128};
use margined_protocol::power::{ExecuteMsg, QueryMsg};
use margined_testing::{helpers::parse_event_attribute, power_env::PowerEnv};
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::base::v1beta1::Coin,
        osmosis::poolmanager::v1beta1::{
            MsgSwapExactAmountIn, SwapAmountInRoute, TotalPoolLiquidityRequest,
        },
    },
    Account, Module, PoolManager, Wasm,
};
use std::str::FromStr;

const MIN_COLLATERAL: u128 = 45_000_000u128;
const VAULT_COLLATERAL: u128 = 60_000_000u128;
const VAULT_MINT_AMOUNT: u128 = 100_000_000u128;

#[test]
fn test_scenario_base_price_spikes_100_percent() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    let vault_id: u64;

    // prepare the vault with collateral ratio 2x
    {
        env.app.increase_time(1u64);

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[0],
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // push base price higher 2x
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

        env.app.increase_time(1u64);
    }

    // 1 second post base price spike
    {
        // index price is updated if requesting with period 1
        {
            let new_index_price: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetUnscaledIndex { period: 1 })
                .unwrap();

            assert_eq!(
                new_index_price,
                Decimal::from_str("36037509.7422128125038025").unwrap()
            );
        }

        // vaults remains safes because of TWAP
        {
            let is_safe: bool = wasm
                .query(&perp_address, &QueryMsg::CheckVault { vault_id })
                .unwrap();
            assert!(is_safe);
        }

        // can still mint with the same amount of collateral (becase of TWAP)
        {
            wasm.execute(
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
        }

        // can still mint with the same amount of collateral (becase of TWAP)
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(MIN_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();
        }
    }

    // 3 minutes post base price spike
    {
        // increase time
        env.app.increase_time(3 * 60);

        // index price is updated if requesting with period 1
        {
            let new_index_price: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetUnscaledIndex { period: 180 })
                .unwrap();

            assert_eq!(
                new_index_price,
                Decimal::from_str("36037509.7422128125038025").unwrap()
            );
        }

        // vaults becomes unsafe
        {
            let is_safe: bool = wasm
                .query(&perp_address, &QueryMsg::CheckVault { vault_id })
                .unwrap();
            assert!(!is_safe);
        }

        // should revert when trying to mint with same amount of collateral as before
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();
        }
    }
}

#[test]
fn test_scenario_base_price_crashes_50_percent() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, false);

    let vault_id: u64;

    // prepare the vault with collateral ratio 2x
    {
        env.app.increase_time(1u64);

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[0],
            )
            .unwrap();

        vault_id = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // drop base price lower 0.5x
    {
        let res = pool_manager
            .query_total_liquidity(&TotalPoolLiquidityRequest {
                pool_id: env.base_pool_id,
            })
            .unwrap();

        let liquidity_to_buy = Uint128::from_str(
            res.liquidity
                .iter()
                .find(|l| l.denom == env.denoms["base"])
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
                        token_out_denom: env.denoms["quote"].clone(),
                    }],
                    token_in: Some(Coin {
                        amount: liquidity_to_buy.to_string(),
                        denom: env.denoms["base"].clone(),
                    }),
                    token_out_min_amount: "1".to_string(),
                },
                &env.signer,
            )
            .unwrap();

        env.app.increase_time(1u64);
    }

    // 1 second post base price crash
    {
        // index price is updated if requesting with period 1
        {
            let new_index_price: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetUnscaledIndex { period: 1 })
                .unwrap();

            assert_eq!(
                new_index_price,
                Decimal::from_str("2247658.1219443176555625").unwrap()
            );
        }

        // vaults remains safes because of TWAP
        {
            let is_safe: bool = wasm
                .query(&perp_address, &QueryMsg::CheckVault { vault_id })
                .unwrap();
            assert!(is_safe);
        }

        // can still mint with the same amount of collateral (becase of TWAP)
        {
            let attack_mint_amount = Uint128::from(VAULT_MINT_AMOUNT)
                .checked_mul(101u128.into())
                .unwrap()
                .checked_mul(100u128.into())
                .unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: attack_mint_amount,
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();
        }
    }

    // 1 minute post base price crash
    {
        // increase time
        env.app.increase_time(60);

        // index price is updated if requesting with period 60
        {
            let new_index_price: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetUnscaledIndex { period: 60 })
                .unwrap();

            assert_eq!(
                new_index_price,
                Decimal::from_str("2247658.1219443176555625").unwrap()
            );
        }

        // will be able to mint more power
        {
            let attack_super_high_mint_amount = Uint128::from(VAULT_MINT_AMOUNT)
                .checked_mul(120u128.into())
                .unwrap()
                .checked_mul(100u128.into())
                .unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: attack_super_high_mint_amount,
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap_err();
        }
    }
}

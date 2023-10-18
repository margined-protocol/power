use crate::{contract::CONTRACT_NAME, vault::Vault};

use cosmwasm_std::{coin, Uint128};
use margined_protocol::power::{ExecuteMsg, QueryMsg};
use margined_testing::{
    helpers::{is_similar, parse_event_attribute},
    power_env::PowerEnv,
};
use osmosis_test_tube::{
    osmosis_std::{
        shim::Timestamp,
        types::{
            cosmos::base::v1beta1::Coin,
            osmosis::poolmanager::v1beta1::{
                MsgSwapExactAmountIn, MsgSwapExactAmountOut, SpotPriceRequest, SwapAmountInRoute,
                SwapAmountOutRoute, TotalPoolLiquidityRequest,
            },
            osmosis::twap::v1beta1 as TwapTypes,
        },
    },
    Account, Module, PoolManager, Twap, Wasm,
};
use std::str::FromStr;

const VAULT_0_COLLATERAL: u128 = 45_100_000u128;
const VAULT_0_MINT_AMOUNT: u128 = 100_000_000u128;

const VAULT_1_COLLATERAL: u128 = 910_000u128;
const VAULT_1_MINT_AMOUNT: u128 = 2_000_000u128;

const VAULT_2_COLLATERAL: u128 = 700_000u128;
const VAULT_2_MINT_AMOUNT: u128 = 1_000_000u128;

#[test]
fn test_liquidate_normal_vault_when_price_is_2x() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let twap = Twap::new(&env.app);
    let pool_manager = PoolManager::new(&env.app);

    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false);

    let vault_id_0: u64;
    let vault_id_1: u64;
    let vault_id_2: u64;

    // open vault id 0
    {
        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_0_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_0_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[0],
            )
            .unwrap();

        vault_id_0 = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // open vault id 1
    {
        env.app.increase_time(1u64);

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_1_MINT_AMOUNT),
                    vault_id: None,
                    rebase: false,
                },
                &[coin(VAULT_1_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        vault_id_1 = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // open vault id 2
    {
        env.app.increase_time(1u64);

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(VAULT_2_MINT_AMOUNT),
                    vault_id: None,
                    rebase: true,
                },
                &[coin(VAULT_2_COLLATERAL, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        vault_id_2 = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // validate spot prices
    {
        let timestamp =
            cosmwasm_std::Timestamp::from_nanos((env.app.get_block_time_nanos()) as u64);
        let start_time = Timestamp {
            seconds: timestamp.seconds() as i64 - 5i64,
            nanos: timestamp.subsec_nanos() as i32,
        };

        let new_base_price = twap
            .query_arithmetic_twap_to_now(&TwapTypes::ArithmeticTwapToNowRequest {
                pool_id: env.base_pool_id,
                base_asset: env.denoms["base"].clone(),
                quote_asset: env.denoms["quote"].clone(),
                start_time: Some(start_time.clone()),
            })
            .unwrap();

        let new_power_price = twap
            .query_arithmetic_twap_to_now(&TwapTypes::ArithmeticTwapToNowRequest {
                pool_id: env.power_pool_id,
                base_asset: env.denoms["power"].clone(),
                quote_asset: env.denoms["base"].clone(),
                start_time: Some(start_time),
            })
            .unwrap();

        assert_eq!(new_base_price.arithmetic_twap, "3000000000000000000000");
        assert_eq!(new_power_price.arithmetic_twap, "300000000000000000");
    }

    // push power price higher 2x
    {
        pool_manager
            .query_spot_price(&SpotPriceRequest {
                pool_id: env.power_pool_id,
                base_asset_denom: env.denoms["base"].clone(),
                quote_asset_denom: env.denoms["power"].clone(),
            })
            .unwrap();

        let res = pool_manager
            .query_total_liquidity(&TotalPoolLiquidityRequest {
                pool_id: env.power_pool_id,
            })
            .unwrap();

        let liquidity_to_buy = Uint128::from_str(
            res.liquidity
                .iter()
                .find(|l| l.denom == env.denoms["power"])
                .unwrap()
                .amount
                .as_str(),
        )
        .unwrap()
        .checked_div(2u128.into())
        .unwrap();

        pool_manager
            .swap_exact_amount_out(
                MsgSwapExactAmountOut {
                    sender: env.signer.address(),
                    routes: vec![SwapAmountOutRoute {
                        pool_id: env.power_pool_id,
                        token_in_denom: env.denoms["base"].clone(),
                    }],
                    token_out: Some(Coin {
                        amount: liquidity_to_buy.to_string(),
                        denom: env.denoms["power"].clone(),
                    }),
                    token_in_max_amount: "10000000000".to_string(),
                },
                &env.signer,
            )
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
    }

    // increase block time to ensure TWAP is updated
    {
        env.app.increase_time(3600u64);

        let now = cosmwasm_std::Timestamp::from_nanos((env.app.get_block_time_nanos()) as u64);
        let now = Timestamp {
            seconds: now.seconds() as i64 - 3_600i64,
            nanos: now.subsec_nanos() as i32,
        };

        let new_base_price = twap
            .query_arithmetic_twap_to_now(&TwapTypes::ArithmeticTwapToNowRequest {
                pool_id: env.base_pool_id,
                base_asset: env.denoms["base"].clone(),
                quote_asset: env.denoms["quote"].clone(),
                start_time: Some(now.clone()),
            })
            .unwrap();

        let new_power_price = twap
            .query_arithmetic_twap_to_now(&TwapTypes::ArithmeticTwapToNowRequest {
                pool_id: env.power_pool_id,
                base_asset: env.denoms["power"].clone(),
                quote_asset: env.denoms["base"].clone(),
                start_time: Some(now),
            })
            .unwrap();

        assert_eq!(new_base_price.arithmetic_twap, "6003124998050000000000");
        assert_eq!(new_power_price.arithmetic_twap, "600147863638629204");
    }

    // prepare liqudiator to liquidate vault 0 and vault 1
    {
        let timestamp =
            cosmwasm_std::Timestamp::from_nanos((env.app.get_block_time_nanos()) as u64);
        let start_time = Timestamp {
            seconds: timestamp.seconds() as i64 - 600i64,
            nanos: timestamp.subsec_nanos() as i32,
        };

        let new_base_price = twap
            .query_arithmetic_twap_to_now(&TwapTypes::ArithmeticTwapToNowRequest {
                pool_id: env.base_pool_id,
                base_asset: env.denoms["base"].clone(),
                quote_asset: env.denoms["quote"].clone(),
                start_time: Some(start_time),
            })
            .unwrap();

        let vault_before: Vault = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_0,
                },
            )
            .unwrap();

        let mint_amount = vault_before.short_amount.checked_mul(2u128.into()).unwrap();
        let collateral_required = mint_amount
            .checked_mul(new_base_price.arithmetic_twap.parse::<Uint128>().unwrap())
            .unwrap()
            .checked_div(1_000_000_000_000_000_000u128.into())
            .unwrap()
            .checked_mul(2u128.into())
            .unwrap();

        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: mint_amount,
                vault_id: None,
                rebase: false,
            },
            &[coin(collateral_required.into(), env.denoms["base"].clone())],
            &env.liquidator,
        )
        .unwrap();
    }

    // liquidate vault 0
    {
        let timestamp =
            cosmwasm_std::Timestamp::from_nanos((env.app.get_block_time_nanos()) as u64);
        let start_time = Timestamp {
            seconds: timestamp.seconds() as i64 - 600i64,
            nanos: timestamp.subsec_nanos() as i32,
        };

        let twap_response = twap
            .query_arithmetic_twap_to_now(&TwapTypes::ArithmeticTwapToNowRequest {
                pool_id: env.power_pool_id,
                base_asset: env.denoms["power"].clone(),
                quote_asset: env.denoms["base"].clone(),
                start_time: Some(start_time),
            })
            .unwrap();
        let new_power_price = Uint128::from_str(twap_response.arithmetic_twap.as_str())
            .unwrap()
            .checked_div(1_000_000_000_000u128.into())
            .unwrap();

        println!("new_power_price: {}", new_power_price);

        let vault_before: Vault = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_0,
                },
            )
            .unwrap();

        // state before liquidation
        let liquidator_power_before =
            env.get_balance(env.liquidator.address(), env.denoms["power"].clone());
        let liquidator_base_before =
            env.get_balance(env.liquidator.address(), env.denoms["base"].clone());

        // check vault is unsafe
        let is_safe: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_0,
                },
            )
            .unwrap();
        assert!(!is_safe);

        let power_to_liquidate = vault_before.short_amount.checked_div(2u128.into()).unwrap();

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: vault_id_0,
                max_debt_amount: power_to_liquidate,
            },
            &[],
            &env.liquidator,
        )
        .unwrap();

        let collateral_to_receive = new_power_price
            .checked_mul(power_to_liquidate)
            .unwrap()
            .checked_div(1_000_000u128.into())
            .unwrap()
            .checked_mul(11u128.into())
            .unwrap()
            .checked_div(10u128.into())
            .unwrap();

        let vault_after: Vault = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_0,
                },
            )
            .unwrap();

        // state after liquidation
        let liquidator_power_after =
            env.get_balance(env.liquidator.address(), env.denoms["power"].clone());
        let liquidator_base_after =
            env.get_balance(env.liquidator.address(), env.denoms["base"].clone());

        assert!(is_similar(
            collateral_to_receive,
            liquidator_base_after
                .checked_sub(liquidator_base_before)
                .unwrap(),
            100u128.into()
        ));
        assert_eq!(
            vault_before
                .short_amount
                .checked_sub(vault_after.short_amount)
                .unwrap(),
            liquidator_power_before
                .checked_sub(liquidator_power_after)
                .unwrap()
        );
    }

    // liquidate vault 1, get full collateral amount from the vault
    {
        let vault_before: Vault = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();

        // state before liquidation
        let liquidator_power_before =
            env.get_balance(env.liquidator.address(), env.denoms["power"].clone());
        let liquidator_base_before =
            env.get_balance(env.liquidator.address(), env.denoms["base"].clone());

        // check vault is unsafe
        let is_safe: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();
        assert!(!is_safe);

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: vault_id_1,
                max_debt_amount: vault_before.short_amount,
            },
            &[],
            &env.liquidator,
        )
        .unwrap();

        let vault_after: Vault = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();

        // state after liquidation
        let liquidator_power_after =
            env.get_balance(env.liquidator.address(), env.denoms["power"].clone());
        let liquidator_base_after =
            env.get_balance(env.liquidator.address(), env.denoms["base"].clone());

        assert_eq!(
            vault_before.collateral,
            liquidator_base_after
                .checked_sub(liquidator_base_before)
                .unwrap(),
        );
        assert_eq!(
            vault_before
                .short_amount
                .checked_sub(vault_after.short_amount)
                .unwrap(),
            liquidator_power_before
                .checked_sub(liquidator_power_after)
                .unwrap()
        );
    }

    // liquidate vault 2, get expected payout
    {
        let vault_before: Vault = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_2,
                },
            )
            .unwrap();

        // state before liquidation
        let liquidator_power_before =
            env.get_balance(env.liquidator.address(), env.denoms["power"].clone());
        let liquidator_base_before =
            env.get_balance(env.liquidator.address(), env.denoms["base"].clone());

        // check vault is unsafe
        let is_safe: bool = wasm
            .query(
                &perp_address,
                &QueryMsg::CheckVault {
                    vault_id: vault_id_2,
                },
            )
            .unwrap();
        assert!(!is_safe);

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: vault_id_2,
                max_debt_amount: vault_before.short_amount,
            },
            &[],
            &env.liquidator,
        )
        .unwrap();

        let vault_after: Vault = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_2,
                },
            )
            .unwrap();

        // state after liquidation
        let liquidator_power_after =
            env.get_balance(env.liquidator.address(), env.denoms["power"].clone());
        let liquidator_base_after =
            env.get_balance(env.liquidator.address(), env.denoms["base"].clone());

        let timestamp =
            cosmwasm_std::Timestamp::from_nanos((env.app.get_block_time_nanos()) as u64);
        let start_time = Timestamp {
            seconds: timestamp.seconds() as i64 - 600i64,
            nanos: timestamp.subsec_nanos() as i32,
        };

        let twap_response = twap
            .query_arithmetic_twap_to_now(&TwapTypes::ArithmeticTwapToNowRequest {
                pool_id: env.power_pool_id,
                base_asset: env.denoms["power"].clone(),
                quote_asset: env.denoms["base"].clone(),
                start_time: Some(start_time),
            })
            .unwrap();
        let new_power_price = Uint128::from_str(twap_response.arithmetic_twap.as_str())
            .unwrap()
            .checked_div(1_000_000_000_000u128.into())
            .unwrap();

        let collateral_to_receive = new_power_price
            .checked_mul(vault_before.short_amount)
            .unwrap()
            .checked_div(1_000_000u128.into())
            .unwrap()
            .checked_mul(11u128.into())
            .unwrap()
            .checked_div(10u128.into())
            .unwrap();

        assert!(is_similar(
            collateral_to_receive,
            liquidator_base_after
                .checked_sub(liquidator_base_before)
                .unwrap(),
            10u128.into()
        ));
        assert_eq!(vault_after.short_amount, Uint128::zero());
        assert_eq!(
            vault_before
                .short_amount
                .checked_sub(vault_after.short_amount)
                .unwrap(),
            liquidator_power_before
                .checked_sub(liquidator_power_after)
                .unwrap()
        );
        assert_eq!(vault_after.collateral, Uint128::from(39_838u128));
    }
}

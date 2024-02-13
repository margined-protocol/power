use crate::{contract::CONTRACT_NAME, state::State};

use cosmwasm_std::{coin, Decimal, Uint128};
use margined_protocol::power::{ConfigResponse, ExecuteMsg, QueryMsg, VaultResponse};
use margined_testing::{
    helpers::parse_event_attribute,
    power_env::{PowerEnv, BASE_PRICE, ONE, SCALE_FACTOR},
};
use mock_query::contract::ExecuteMsg as MockQueryExecuteMsg;
use osmosis_test_tube::{
    osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest, Account, Bank, Module,
    RunnerError, Wasm,
};
use std::str::FromStr;

#[test]
fn test_liquidation_profitable() {
    pub const SCALED_POWER_PRICE: u128 = 303_000 * ONE / SCALE_FACTOR;

    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
    let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

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

    let vault_id_1: u64;
    let vault_id_2: u64;

    // open vault id 1
    {
        let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        assert_eq!(Decimal::one(), state.normalisation_factor);
        let mint_amount = 100_000_000u128;
        let deposit_amount = 45_000_049u128;
        let power_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: env.traders[0].address(),
                denom: env.denoms["power"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(mint_amount),
                    vault_id: None,
                    rebase: true,
                },
                &[coin(deposit_amount, env.denoms["base"].clone())],
                &env.traders[0],
            )
            .unwrap();

        vault_id_1 = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let power_balance_after = bank
            .query_balance(&QueryBalanceRequest {
                address: env.traders[0].address(),
                denom: env.denoms["power"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        let vault_after: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();

        assert_eq!(
            100_000_400u128 + u128::from_str(&power_balance_before).unwrap(),
            u128::from_str(&power_balance_after).unwrap()
        );
        assert_eq!(Uint128::from(100_000_400u128), vault_after.short_amount);
    }

    // open vault id 2
    {
        env.app.increase_time(1u64);
        let mint_amount = 2_000_000u128;
        let deposit_amount = 900_000u128;

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(mint_amount),
                    vault_id: None,
                    rebase: true,
                },
                &[coin(deposit_amount, env.denoms["base"].clone())],
                &env.traders[1],
            )
            .unwrap();

        vault_id_2 = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();

        let power_balance_after = bank
            .query_balance(&QueryBalanceRequest {
                address: env.traders[1].address(),
                denom: env.denoms["power"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        let vault_after: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_2,
                },
            )
            .unwrap();

        assert_eq!(2_000_010u128, u128::from_str(&power_balance_after).unwrap());
        assert_eq!(Uint128::from(2_000_010u128), vault_after.short_amount);
    }

    // should revert liquidating vault id 0
    {
        let liquidate_response = wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: 0,
                max_debt_amount: Uint128::from(1_000_000u128),
            },
            &[],
            &env.signer,
        );

        assert!(liquidate_response.is_err());
        assert_eq!(
            liquidate_response.unwrap_err(),
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Vault 0 does not exist, cannot perform operation: execute wasm contract failed".to_string()
            }
        );
    }

    // should revert liquidate vault id greater than max vaults
    {
        let liquidate_response = wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: 10,
                max_debt_amount: Uint128::from(1_000_000u128),
            },
            &[],
            &env.signer,
        );

        assert!(liquidate_response.is_err());
        assert_eq!(
            liquidate_response.unwrap_err(),
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Vault 10 does not exist, cannot perform operation: execute wasm contract failed".to_string()
            }
        );
    }

    // should revert liquidating a safu vault
    {
        let vault_before: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();

        let liquidate_response = wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: vault_id_1,
                max_debt_amount: vault_before.short_amount,
            },
            &[],
            &env.signer,
        );

        assert!(liquidate_response.is_err());
        assert_eq!(
            liquidate_response.unwrap_err(),
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Vault is safe, cannot be liquidated: execute wasm contract failed".to_string()
            }
        );
    }

    // set base price to make the vault underwater
    {
        pub const SCALED_POWER_PRICE: u128 = 4_040 * ONE / SCALE_FACTOR;
        pub const BASE_PRICE: u128 = 4_000_000_000;
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
    }

    // should revert if the vault becomes dust after liquidation
    {
        // TODO: we don't have the concept of dust, (yet)
    }

    // should allow liquidation a whole vault if only liquidating half will make it a dust vault
    {
        let vault_before: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_2,
                },
            )
            .unwrap();
        let max_debt_amount = vault_before.short_amount;

        let liquidate_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::Liquidate {
                    vault_id: vault_id_2,
                    max_debt_amount,
                },
                &[],
                &env.signer,
            )
            .unwrap();

        let collateral_to_pay = Uint128::from_str(&parse_event_attribute(
            liquidate_response.events.clone(),
            "wasm-liquidation",
            "collateral_to_pay",
        ))
        .unwrap();
        assert_eq!(Uint128::from(888_804u128), collateral_to_pay);

        let liquidation_amount = Uint128::from_str(&parse_event_attribute(
            liquidate_response.events,
            "wasm-liquidation",
            "liquidation_amount",
        ))
        .unwrap();
        assert_eq!(Uint128::from(2_000_010u128), liquidation_amount);

        let vault_after: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_2,
                },
            )
            .unwrap();

        assert_eq!(Uint128::zero(), vault_after.short_amount);
        assert_eq!(Uint128::from(11_196u128), vault_after.collateral);
    }

    // liquidate unsafe vault (vault id 1)
    {
        env.app.increase_time(1u64);
        let liquidator_power_before = Uint128::from_str(
            &bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount,
        )
        .unwrap();

        let vault_before: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();
        let max_debt_amount = vault_before.short_amount.checked_div(2u128.into()).unwrap();

        let liquidate_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::Liquidate {
                    vault_id: vault_id_1,
                    max_debt_amount: max_debt_amount + Uint128::from(10u128),
                },
                &[],
                &env.signer,
            )
            .unwrap();

        let collateral_to_pay = Uint128::from_str(&parse_event_attribute(
            liquidate_response.events.clone(),
            "wasm-liquidation",
            "collateral_to_pay",
        ))
        .unwrap();
        assert_eq!(Uint128::from(22_220_088u128), collateral_to_pay);

        let liquidation_amount = Uint128::from_str(&parse_event_attribute(
            liquidate_response.events,
            "wasm-liquidation",
            "liquidation_amount",
        ))
        .unwrap();
        assert_eq!(Uint128::from(50_000_200u128), liquidation_amount);

        let liquidator_power_after = Uint128::from_str(
            &bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount,
        )
        .unwrap();

        let vault_after: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();

        assert_eq!(Uint128::from(50_000_200u128), vault_after.short_amount);
        assert_eq!(Uint128::from(22_779_961u128), vault_after.collateral);
        assert_eq!(
            liquidator_power_after + max_debt_amount,
            liquidator_power_before
        );
    }
}

#[test]
fn test_liquidation_unprofitable() {
    pub const SCALED_POWER_PRICE: u128 = 303_000 * ONE / SCALE_FACTOR;

    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
    let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

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

    let vault_id_1: u64;
    // open vault id 1
    {
        let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

        assert_eq!(Decimal::one(), state.normalisation_factor);
        let mint_amount = 100_000_000u128;
        let deposit_amount = 45_000_049u128;

        let mint_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(mint_amount),
                    vault_id: None,
                    rebase: true,
                },
                &[coin(deposit_amount, env.denoms["base"].clone())],
                &env.traders[0],
            )
            .unwrap();

        vault_id_1 = u64::from_str(&parse_event_attribute(
            mint_response.events,
            "wasm-mint",
            "vault_id",
        ))
        .unwrap();
    }

    // set price to be unprofitable
    {
        pub const SCALED_POWER_PRICE: u128 = 909_000 * ONE / SCALE_FACTOR;
        pub const BASE_PRICE: u128 = 9_000_000_000_000;

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
    }

    // should revert if vault is paying out all collateral, but there is debt remaining
    {
        env.app.increase_time(1u64);
        let vault: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();
        let max_debt_amount = vault.short_amount.checked_sub(11u128.into()).unwrap();

        wasm.execute(
            &perp_address,
            &ExecuteMsg::Liquidate {
                vault_id: vault_id_1,
                max_debt_amount: max_debt_amount + Uint128::from(10u128),
            },
            &[],
            &env.signer,
        )
        .unwrap_err();
    }

    // can fully liquidate an underwater vault even if it is not profitable
    {
        env.app.increase_time(1u64);
        let vault: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();
        let max_debt_amount = vault.short_amount;

        let liquidator_base_before = Uint128::from_str(
            &bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["base"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount,
        )
        .unwrap();

        let liquidator_power_before = Uint128::from_str(
            &bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount,
        )
        .unwrap();

        let liquidate_response = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::Liquidate {
                    vault_id: vault_id_1,
                    max_debt_amount,
                },
                &[],
                &env.signer,
            )
            .unwrap();

        let collateral_to_pay = Uint128::from_str(&parse_event_attribute(
            liquidate_response.events.clone(),
            "wasm-liquidation",
            "collateral_to_pay",
        ))
        .unwrap();
        assert_eq!(Uint128::from(45_000_049u128), collateral_to_pay);

        let liquidation_amount = Uint128::from_str(&parse_event_attribute(
            liquidate_response.events,
            "wasm-liquidation",
            "liquidation_amount",
        ))
        .unwrap();
        assert_eq!(Uint128::from(100_000_400u128), liquidation_amount);

        let liquidator_base_after = Uint128::from_str(
            &bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["base"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount,
        )
        .unwrap();
        let liquidator_power_after = Uint128::from_str(
            &bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount,
        )
        .unwrap();

        let vault_after: VaultResponse = wasm
            .query(
                &perp_address,
                &QueryMsg::GetVault {
                    vault_id: vault_id_1,
                },
            )
            .unwrap();
        assert_eq!(Uint128::zero(), vault_after.short_amount);
        assert_eq!(Uint128::zero(), vault_after.collateral);
        assert_eq!(
            liquidator_power_before
                .checked_sub(liquidator_power_after)
                .unwrap(),
            max_debt_amount
        );
        assert_eq!(
            liquidator_base_after
                .checked_sub(liquidator_base_before)
                .unwrap(),
            collateral_to_pay
        );
    }
}

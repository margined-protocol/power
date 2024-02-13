use crate::contract::CONTRACT_NAME;

use cosmwasm_std::{assert_approx_eq, coin, coins, Decimal, Uint128};
use margined_protocol::power::{
    ConfigResponse, ExecuteMsg, QueryMsg, StateResponse, VaultResponse,
};
use margined_testing::{
    helpers::parse_event_attribute,
    power_env::{PowerEnv, BASE_PRICE, SCALED_POWER_PRICE, STAKE_PRICE},
};
use mock_query::contract::ExecuteMsg as MockQueryExecuteMsg;
use osmosis_test_tube::{
    osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest, Account, Bank, Module,
    RunnerError, Wasm,
};
use std::str::FromStr;

#[test]
fn test_combined_actions() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, true);

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

    let mut vault_id: u64;

    // Open, deposit and mint
    {
        // should revert if the vault has too little collateral
        {
            let mint_amount = Uint128::from(100_000u128);
            let collateral_amount = Uint128::from(450_000u128);

            env.app.increase_time(1u64);

            let funds = coins(collateral_amount.u128(), &env.denoms["base"]);
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: mint_amount,
                        vault_id: None,
                        rebase: false,
                    },
                    &funds,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault is below minimum collateral amount (0.5 base denom): execute wasm contract failed".to_string()
                }
            );
        }

        // should open vault, deposit and mint in the same transaction (or not for us)
        {
            let amount = 100_000_000u128;
            let collateral = coins(45_000_000u128, &env.denoms["base"]);
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.traders[0].address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            env.app.increase_time(5u64);

            let res = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::from(amount),
                        vault_id: None,
                        rebase: false,
                    },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap();

            vault_id =
                u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.traders[0].address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                amount + u128::from_str(&power_balance_before).unwrap(),
                u128::from_str(&power_balance_after).unwrap()
            );
            assert_eq!(Uint128::from(amount), vault.short_amount);
        }
    }

    // Deposit and mint
    {
        // should revert if tries to deposit collateral using mint power perp
        {
            env.app.increase_time(5u64);

            let collateral = coins(45_000_000u128, &env.denoms["base"]);
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::zero(),
                        vault_id: Some(vault_id),
                        rebase: false,
                    },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Zero mint not supported: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if tries to deposit staked asset collateral using mint power perp
        {
            env.app.increase_time(5u64);

            let collateral = coins(45_000_000u128, &env.denoms["stake"]);
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Deposit { vault_id },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
                }
            );
        }

        // should deposit and mint in same transaction
        {
            env.app.increase_time(5u64);

            let amount = 1_000_000u128;
            let collateral = coins(4_500_000u128, &env.denoms["base"]);
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &collateral,
                &env.traders[0],
            )
            .unwrap();

            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let power_balance = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.traders[0].address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            assert_eq!(Uint128::from(101_000_000u128), vault.short_amount);
            assert_eq!(Uint128::from(49_500_000u128), vault.collateral);
            assert_eq!(
                Uint128::from(101_000_000u128),
                Uint128::from_str(&power_balance).unwrap()
            );
        }

        // should only mint if deposit is 0
        {
            env.app.increase_time(5u64);

            let amount = 1_000u128;

            let before: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[],
                &env.traders[0],
            )
            .unwrap();

            let after: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                before.short_amount + Uint128::from(amount),
                after.short_amount
            );
            assert_eq!(before.collateral, after.collateral);
        }

        // should not deposit if mint is zero
        {
            env.app.increase_time(5u64);

            let collateral = coins(100_000u128, &env.denoms["base"]);

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::zero(),
                        vault_id: Some(vault_id),
                        rebase: false,
                    },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Zero mint not supported: execute wasm contract failed".to_string()
                }
            );
        }

        // nothing happens if both zero
        {
            env.app.increase_time(5u64);

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::zero(),
                        vault_id: Some(vault_id),
                        rebase: false,
                    },
                    &[],
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Zero mint not supported: execute wasm contract failed".to_string()
                }
            );
        }
    }

    // Burn and withdraw
    {
        // mint power for trader 1 to withdraw
        env.app.increase_time(5u64);

        let amount = 100_000_000u128;
        let collateral = coins(45_000_000u128, &env.denoms["base"]);

        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[1],
            )
            .unwrap();
        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        // should burn and withdraw
        let before: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let burn_amount = 50_000_000u128;
        let withdraw_amount = 22_500_000u128;

        let collateral = vec![coin(burn_amount, &env.denoms["power"])];

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::BurnPowerPerp {
                amount_to_withdraw: Some(withdraw_amount.into()),
                vault_id,
            },
            &collateral,
            &env.traders[1],
        )
        .unwrap();

        let after: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        assert_eq!(
            before.short_amount - Uint128::from(burn_amount),
            after.short_amount
        );
        assert_eq!(
            before.collateral - Uint128::from(withdraw_amount),
            after.collateral
        );
    }
}

#[test]
fn test_combined_actions_staked_asset() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, true);

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
        config.query_contract.as_ref(),
        &MockQueryExecuteMsg::AppendPrice {
            pool_id: env.stake_pool_id,
            price: Decimal::from_atomics(STAKE_PRICE, 6u32).unwrap(),
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

    let mut vault_id: u64;

    // Open, deposit and mint
    {
        // should revert if the vault has too little collateral
        {
            let mint_amount = Uint128::from(100_000u128);
            let collateral_amount = Uint128::from(450_000u128);

            env.app.increase_time(1u64);

            let funds = coins(collateral_amount.u128(), &env.denoms["stake"]);
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: mint_amount,
                        vault_id: None,
                        rebase: false,
                    },
                    &funds,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault is below minimum collateral amount (0.5 base denom): execute wasm contract failed".to_string()
                }
            );
        }

        // should open vault, deposit and mint in the same transaction (or not for us)
        {
            let amount = 100_000_000u128;
            let collateral = coins(45_000_000u128, &env.denoms["stake"]);
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.traders[0].address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            env.app.increase_time(5u64);

            let res = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::from(amount),
                        vault_id: None,
                        rebase: false,
                    },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap();

            vault_id =
                u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.traders[0].address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                amount + u128::from_str(&power_balance_before).unwrap(),
                u128::from_str(&power_balance_after).unwrap()
            );
            assert_eq!(Uint128::from(amount), vault.short_amount);
        }
    }

    // Deposit and mint
    {
        // should revert if tries to deposit collateral using mint power perp
        {
            env.app.increase_time(5u64);

            let collateral = coins(45_000_000u128, &env.denoms["stake"]);
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::zero(),
                        vault_id: Some(vault_id),
                        rebase: false,
                    },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Zero mint not supported: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if tries to deposit base collateral
        {
            env.app.increase_time(5u64);

            let collateral = coins(45_000_000u128, &env.denoms["base"]);
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Deposit { vault_id },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                        err,
                        RunnerError::ExecuteError {
                            msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
                        }
                    );
        }

        // should deposit and mint in same transaction
        {
            env.app.increase_time(5u64);

            let amount = 1_000_000u128;
            let collateral = coins(4_500_000u128, &env.denoms["stake"]);
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &collateral,
                &env.traders[0],
            )
            .unwrap();

            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let power_balance = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.traders[0].address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            assert_eq!(Uint128::from(101_000_000u128), vault.short_amount);
            assert_eq!(Uint128::from(49_500_000u128), vault.collateral);
            assert_eq!(
                Uint128::from(101_000_000u128),
                Uint128::from_str(&power_balance).unwrap()
            );
        }

        // should only mint if deposit is 0
        {
            env.app.increase_time(5u64);

            let amount = 1_000u128;

            let before: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[],
                &env.traders[0],
            )
            .unwrap();

            let after: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                before.short_amount + Uint128::from(amount),
                after.short_amount
            );
            assert_eq!(before.collateral, after.collateral);
        }

        // should not deposit if mint is zero
        {
            env.app.increase_time(5u64);

            let collateral = coins(100_000u128, &env.denoms["stake"]);

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::zero(),
                        vault_id: Some(vault_id),
                        rebase: false,
                    },
                    &collateral,
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Zero mint not supported: execute wasm contract failed".to_string()
                }
            );
        }

        // nothing happens if both zero
        {
            env.app.increase_time(5u64);

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::zero(),
                        vault_id: Some(vault_id),
                        rebase: false,
                    },
                    &[],
                    &env.traders[0],
                )
                .unwrap_err();

            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Zero mint not supported: execute wasm contract failed".to_string()
                }
            );
        }
    }

    // Burn and withdraw
    {
        // mint power for trader 1 to withdraw
        env.app.increase_time(5u64);

        let amount = 100_000_000u128;
        let collateral = coins(45_000_000u128, &env.denoms["stake"]);

        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[1],
            )
            .unwrap();
        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        // should burn and withdraw
        let before: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let burn_amount = 50_000_000u128;
        let withdraw_amount = 22_500_000u128;

        let collateral = vec![coin(burn_amount, &env.denoms["power"])];

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::BurnPowerPerp {
                amount_to_withdraw: Some(withdraw_amount.into()),
                vault_id,
            },
            &collateral,
            &env.traders[1],
        )
        .unwrap();

        let after: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        assert_eq!(
            before.short_amount - Uint128::from(burn_amount),
            after.short_amount
        );
        assert_eq!(
            before.collateral - Uint128::from(withdraw_amount),
            after.collateral
        );
    }
}

#[test]
fn test_deposit_and_withdraw_with_fee() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, true);

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

    let mut vault_id: u64;

    // Should be able to set the fee rate
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::UpdateConfig {
                fee_rate: Some("0.001".to_string()),
                fee_pool: None,
            },
            &[],
            &env.signer,
        )
        .unwrap();
        let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();

        assert_eq!(config.fee_rate, Decimal::from_str("0.001").unwrap());
    }

    // Should revert if collateral is sent to an empty vault
    {
        let amount = 500_000u128;

        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &[],
                &env.traders[1],
            )
            .unwrap_err();
        assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Must send collateral to new create new vault: execute wasm contract failed".to_string()
                }
            );
    }

    // Should revert if vault is unable to pay fee amount from attached amount or vault collateral
    {
        let amount = 500_000u128;
        let collateral = vec![coin(1u128, &env.denoms["base"])];
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[1],
            )
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Generic error: Cannot subtract more collateral than deposited: execute wasm contract failed".to_string()
            }
        );
    }

    // Should charge fee on mint power perp amount from deposit amount
    {
        let amount = 500_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / 1_000_000u128 / 1_000u128;
        let collateral = coins(550_000u128 + expect_fees, &env.denoms["base"]);

        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[1],
            )
            .unwrap();
        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["base"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(vault.short_amount, Uint128::from(amount));
        assert_eq!(vault.collateral, Uint128::from(550_000u128));
        assert_eq!(
            Uint128::from_str(&fee_pool_balance).unwrap(),
            Uint128::from(expect_fees)
        );
    }

    // Should charge fee on mint power perp amount from vault collateral
    {
        let vault_before: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["base"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        let amount = 500_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / 1_000_000u128 / 1_000u128;

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(amount),
                vault_id: Some(vault_id),
                rebase: false,
            },
            &[],
            &env.traders[1],
        )
        .unwrap();

        // should burn and withdraw
        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["base"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(
            vault_before.short_amount + Uint128::from(amount),
            vault.short_amount
        );
        assert_eq!(
            Uint128::from_str(&fee_pool_balance_before).unwrap() + Uint128::from(expect_fees),
            Uint128::from_str(&fee_pool_balance).unwrap()
        );
    }

    // Should charge fee on mint power perp amount from deposit amount - 0.1
    {
        let amount = 100_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / 1_000_000u128 / 1_000u128;
        let collateral = coins(550_000u128 + expect_fees, &env.denoms["base"]);

        let fee_pool_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["base"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        env.app.increase_time(5u64);

        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[0],
            )
            .unwrap();
        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["base"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(vault.short_amount, Uint128::from(amount));
        assert_eq!(vault.collateral, Uint128::from(550_000u128));
        assert_eq!(
            Uint128::from_str(&fee_pool_balance).unwrap(),
            Uint128::from_str(&fee_pool_balance_before).unwrap() + Uint128::from(expect_fees)
        );
    }

    // Should charge fee on mint power perp amount from vault collateral - 0.1
    {
        let vault_before: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["base"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        let amount = 100_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / 1_000_000u128 / 1_000u128;

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(amount),
                vault_id: Some(vault_id),
                rebase: false,
            },
            &[],
            &env.traders[0],
        )
        .unwrap();

        // should burn and withdraw
        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["base"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(
            vault_before.short_amount + Uint128::from(amount),
            vault.short_amount
        );
        assert_eq!(
            Uint128::from_str(&fee_pool_balance_before).unwrap() + Uint128::from(expect_fees),
            Uint128::from_str(&fee_pool_balance).unwrap()
        );
    }

    // Should set fee to 0
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::UpdateConfig {
                fee_rate: Some("0.0".to_string()),
                fee_pool: None,
            },
            &[],
            &env.signer,
        )
        .unwrap();
        let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();

        assert_eq!(config.fee_rate, Decimal::zero());
    }
}

#[test]
fn test_deposit_and_withdraw_with_fee_staked_assets() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, true);

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
        config.query_contract.as_ref(),
        &MockQueryExecuteMsg::AppendPrice {
            pool_id: env.stake_pool_id,
            price: Decimal::from_atomics(STAKE_PRICE, 6u32).unwrap(),
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

    let mut vault_id: u64;

    // Should be able to set the fee rate
    {
        wasm.execute(
            &perp_address,
            &ExecuteMsg::UpdateConfig {
                fee_rate: Some("0.001".to_string()),
                fee_pool: None,
            },
            &[],
            &env.signer,
        )
        .unwrap();
        let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();

        assert_eq!(config.fee_rate, Decimal::from_str("0.001").unwrap());
    }

    // Should revert if vault is unable to pay fee amount from attached amount or vault collateral
    {
        let amount = 500_000u128;
        let collateral = vec![coin(1u128, &env.denoms["stake"])];
        let err = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[1],
            )
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Generic error: Cannot subtract more collateral than deposited: execute wasm contract failed".to_string()
            }
        );
    }

    // Should charge fee on mint power perp amount from deposit amount
    {
        let amount = 500_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / STAKE_PRICE / 1_000u128;
        let collateral = coins(550_000u128 + (2 * expect_fees), &env.denoms["stake"]);

        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[1],
            )
            .unwrap();
        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["stake"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(vault.short_amount, Uint128::from(amount));
        assert_approx_eq!(vault.collateral, Uint128::from(550_000u128), "100");
        assert_approx_eq!(
            Uint128::from_str(&fee_pool_balance).unwrap(),
            Uint128::from(2 * expect_fees),
            "100"
        );
    }

    // Should charge fee on mint power perp amount from vault collateral
    {
        let vault_before: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["stake"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        let amount = 500_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / STAKE_PRICE / 1_000u128;

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(amount),
                vault_id: Some(vault_id),
                rebase: false,
            },
            &[],
            &env.traders[1],
        )
        .unwrap();

        // should burn and withdraw
        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["stake"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(
            vault_before.short_amount + Uint128::from(amount),
            vault.short_amount
        );
        assert_approx_eq!(
            Uint128::from_str(&fee_pool_balance_before).unwrap() + Uint128::from(2 * expect_fees),
            Uint128::from_str(&fee_pool_balance).unwrap(),
            "100"
        );
    }

    // Should charge fee on mint power perp amount from deposit amount - 0.1
    {
        let amount = 100_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / STAKE_PRICE / 1_000u128;
        let collateral = coins(550_000u128 + (2 * expect_fees), &env.denoms["stake"]);

        let fee_pool_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["stake"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        env.app.increase_time(5u64);

        let res = wasm
            .execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: None,
                    rebase: false,
                },
                &collateral,
                &env.traders[0],
            )
            .unwrap();
        vault_id =
            u64::from_str(&parse_event_attribute(res.events, "wasm-mint", "vault_id")).unwrap();

        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["stake"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(vault.short_amount, Uint128::from(amount));
        assert_approx_eq!(vault.collateral, Uint128::from(550_000u128), "100");
        assert_approx_eq!(
            Uint128::from_str(&fee_pool_balance).unwrap(),
            Uint128::from_str(&fee_pool_balance_before).unwrap() + Uint128::from(2 * expect_fees),
            "100"
        );
    }

    // Should charge fee on mint power perp amount from vault collateral - 0.1
    {
        let vault_before: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["stake"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        let amount = 100_000u128;
        let expect_fees = SCALED_POWER_PRICE * amount / STAKE_PRICE / 1_000u128;

        env.app.increase_time(5u64);

        wasm.execute(
            &perp_address,
            &ExecuteMsg::MintPowerPerp {
                amount: Uint128::from(amount),
                vault_id: Some(vault_id),
                rebase: false,
            },
            &[],
            &env.traders[0],
        )
        .unwrap();

        // should burn and withdraw
        let vault: VaultResponse = wasm
            .query(&perp_address, &QueryMsg::GetVault { vault_id })
            .unwrap();

        let fee_pool_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: env.fee_pool.address(),
                denom: env.denoms["stake"].clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount;

        assert_eq!(
            vault_before.short_amount + Uint128::from(amount),
            vault.short_amount
        );
        assert_approx_eq!(
            Uint128::from_str(&fee_pool_balance_before).unwrap() + Uint128::from(2 * expect_fees),
            Uint128::from_str(&fee_pool_balance).unwrap(),
            "100"
        );
    }
}

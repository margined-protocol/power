use crate::{
    contract::{CONTRACT_NAME, CONTRACT_VERSION},
    state::State,
};

use cosmwasm_std::{coin, Addr, Decimal, Uint128};
use margined_protocol::power::{
    Asset, ConfigResponse, ExecuteMsg, InstantiateMsg, Pool, QueryMsg, StakeAsset, VaultResponse,
    FUNDING_PERIOD,
};
use margined_testing::{
    helpers::{parse_event_attribute, store_code},
    power_env::{PowerEnv, ONE, SCALE_FACTOR, STAKE_PRICE},
};
use mock_query::contract::ExecuteMsg as MockQueryExecuteMsg;
use osmosis_test_tube::{
    osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest, Account, Bank, Module,
    RunnerError, Wasm,
};
use std::str::FromStr;

#[test]
fn test_deployment() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let (perp_address, query_address) =
        env.deploy_power(&wasm, CONTRACT_NAME.to_string(), false, true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
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
            price: Decimal::from_str("3010.0").unwrap(),
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

    // check power deployment
    {
        let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
        assert_eq!(
            ConfigResponse {
                query_contract: Addr::unchecked(query_address),
                fee_pool_contract: Addr::unchecked(env.fee_pool.address()),
                fee_rate: Decimal::zero(),
                base_asset: Asset {
                    denom: env.denoms["base"].clone(),
                    decimals: 6u32
                },
                power_asset: Asset {
                    denom: env.denoms["power"].clone(),
                    decimals: 6u32
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
                funding_period: FUNDING_PERIOD,
                index_scale: 10_000u64,
                min_collateral_amount: Decimal::from_ratio(1u128, 2u128),
                version: CONTRACT_VERSION.to_string(),
            },
            config
        );
    }
}

#[test]
fn test_deployment_staked_assets() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let (perp_address, query_address) =
        env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
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
            price: Decimal::from_str("3010.0").unwrap(),
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

    // check power deployment
    {
        let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
        assert_eq!(
            ConfigResponse {
                query_contract: Addr::unchecked(query_address),
                fee_pool_contract: Addr::unchecked(env.fee_pool.address()),
                fee_rate: Decimal::zero(),
                base_asset: Asset {
                    denom: env.denoms["base"].clone(),
                    decimals: 6u32
                },
                power_asset: Asset {
                    denom: env.denoms["power"].clone(),
                    decimals: 6u32
                },
                stake_assets: Some(vec![
                    StakeAsset {
                        denom: env.denoms["stake"].clone(),
                        decimals: 6u32,
                        pool: Pool {
                            id: env.stake_pool_id,
                            base_denom: env.denoms["stake"].clone(),
                            quote_denom: env.denoms["base"].clone()
                        }
                    },
                    StakeAsset {
                        denom: env.denoms["milk"].clone(),
                        decimals: 6u32,
                        pool: Pool {
                            id: env.milk_pool_id,
                            base_denom: env.denoms["milk"].clone(),
                            quote_denom: env.denoms["base"].clone()
                        }
                    }
                ]),
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

                funding_period: FUNDING_PERIOD,
                index_scale: 10_000u64,
                min_collateral_amount: Decimal::from_ratio(1u128, 2u128),
                version: CONTRACT_VERSION.to_string(),
            },
            config
        );
    }
}

#[test]
fn test_fail_deployment_invalid_config() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);

    let query_address = env.deploy_query_contracts(&wasm, true);

    let code_id = store_code(&wasm, &env.signer, "margined-power".to_string());

    // invalid config
    let err = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: env.fee_pool.address(),
                fee_rate: "0.0".to_string(), // 0%
                query_contract: query_address,
                power_denom: env.denoms["power"].clone(),
                base_denom: env.denoms["base"].clone(),
                stake_assets: None,
                base_pool_id: env.base_pool_id,
                base_pool_quote: env.denoms["quote"].clone(),
                power_pool_id: env.power_pool_id,
                base_decimals: 160u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[],
            &env.signer,
        )
        .unwrap_err();

    assert_eq!("execute error: failed to execute message; message index: 0: Generic error: Invalid base decimals: instantiate wasm contract failed", err.to_string());
}

#[test]
fn test_basic_actions() {
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
            price: Decimal::from_str("3000.0").unwrap(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    const SCALED_POWER_PRICE: u128 = 3_010 * ONE / SCALE_FACTOR;
    const SCALED_BASE_PRICE: u128 = 3_000 * ONE / SCALE_FACTOR;
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

    // read_basic_properties
    {
        // should be able to get normalisation factor
        {
            // increase timestamp
            env.app.increase_time(30u64);

            let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            let normalisation_factor: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetNormalisationFactor {})
                .unwrap();
            assert!(state.normalisation_factor > normalisation_factor);

            // increase timestamp
            env.app.increase_time(30u64);

            let normalisation_factor_after: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetNormalisationFactor {})
                .unwrap();
            assert!(normalisation_factor > normalisation_factor_after);
        }

        // should allow anyone to call apply funding
        {
            let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            let expected_normalisation_factor: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetNormalisationFactor {})
                .unwrap();
            assert!(state.normalisation_factor > expected_normalisation_factor);

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.traders[1], // one of the trading accounts should be able to call this
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            assert!(
                state_after
                    .normalisation_factor
                    .abs_diff(expected_normalisation_factor)
                    < Decimal::from_str("0.00001").unwrap()
            );
            assert!(state.normalisation_factor > state_after.normalisation_factor);
        }

        // TODO: need to add multiple function execution
        // // should not update funding twice in a single block
        // {
        //     wasm.execute_multiple(contract, msg, funds, signer)
        // }

        // should be able to get index and mark price used for funding
        {
            env.app.increase_time(30u64);

            let mark_price: Decimal = wasm
                .query(
                    &perp_address,
                    &QueryMsg::GetDenormalisedMark { period: 30u64 },
                )
                .unwrap();
            let mark_price_funding: Decimal = wasm
                .query(
                    &perp_address,
                    &QueryMsg::GetDenormalisedMarkFunding { period: 30u64 },
                )
                .unwrap();

            let expected_mark = Decimal::from_atomics(SCALED_BASE_PRICE, 6u32).unwrap()
                * Decimal::from_atomics(SCALED_POWER_PRICE, 6u32).unwrap();

            assert!(mark_price.abs_diff(expected_mark) < Decimal::from_atomics(3u128, 0).unwrap());
            assert!(
                mark_price_funding.abs_diff(expected_mark)
                    < Decimal::from_atomics(3u128, 0).unwrap()
            );
        }

        // should be able to get scaled index
        {
            let index: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetUnscaledIndex { period: 30u64 })
                .unwrap();

            let eth_squared =
                Decimal::from_str("3000.0").unwrap() * Decimal::from_str("3000.0").unwrap();

            assert_eq!(index, eth_squared);
        }

        // TODO: should revert when sending eth to controller from an EOA
        // not sure this is quite possible in CosmWasm
    }

    let vault_id: u64;
    // mint: open vault
    {
        // should be able to open a vaults
        {
            let mint_response = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::from(500_000u128),
                        vault_id: None,
                        rebase: false,
                    },
                    &[coin(500_000u64.into(), env.denoms["base"].to_string())],
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
    }

    // deposit: deposit collateral
    {
        // should revert when trying to deposit to vault 0
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id: 0u64 },
                &[coin(1u64.into(), env.denoms["base"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when trying to deposit to non-existent vault
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id: 10u64 },
                &[coin(1u64.into(), env.denoms["base"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when trying to mint to non-existent vault
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(100u128),
                    vault_id: Some(10u64),
                    rebase: false,
                },
                &[coin(100u64.into(), env.denoms["base"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when trying to deposit to vault where info.sender is not operator
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id },
                &[coin(1u64.into(), env.denoms["base"].to_string())],
                &env.traders[0],
            )
            .unwrap_err();
        }

        // should be able to deposit collateral
        {
            let deposit_amount = 45_000_000u128;
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address.clone(),
                    denom: env.denoms["base"].to_string(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_before: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id },
                &[coin(deposit_amount, env.denoms["base"].to_string())],
                &env.signer,
            )
            .unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address.clone(),
                    denom: env.denoms["base"].to_string(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_after: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                deposit_amount + u128::from_str(&power_balance_before).unwrap(),
                u128::from_str(&power_balance_after).unwrap()
            );
            assert_eq!(
                Uint128::from(deposit_amount) + vault_before.collateral,
                vault_after.collateral
            );
        }

        // should not be able to deposit zero collateral
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Deposit { vault_id },
                    &[coin(0u128, env.denoms["base"].to_string())],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "sentFunds: invalid coins".to_string()
                }
            );

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Deposit { vault_id },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
                }
            );
        }
    }

    // mint: mint power tokens
    {
        // should revert if not called by operator
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(100u128),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[coin(100u128, env.denoms["base"].to_string())],
                &env.traders[0],
            )
            .unwrap_err();
        }

        // should revert if vault does not exist
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(100u128),
                    vault_id: Some(110u64),
                    rebase: false,
                },
                &[coin(100u128, env.denoms["base"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should be able to mint power token
        {
            let amount = 100_000_000u128;
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_before: VaultResponse = wasm
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
                &env.signer,
            )
            .unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_after: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                amount + u128::from_str(&power_balance_before).unwrap(),
                u128::from_str(&power_balance_after).unwrap()
            );
            assert_eq!(
                Uint128::from(amount) + vault_before.short_amount,
                vault_after.short_amount
            );
        }

        // should revert if minting more than collateral ratio
        {
            let amount = 100_000_000u128;

            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[],
                &env.signer,
            )
            .unwrap_err();
        }
    }

    // burn: burn power token
    {
        // should revert if no funds are sent
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: None,
                        vault_id,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert when trying to burn for vault 0
        {
            let funds = vec![coin(100u128, env.denoms["power"].clone())];
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(100u128)),
                        vault_id: 0u64,
                    },
                    &funds,
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Vault 0 does not exist, cannot perform operation: execute wasm contract failed".to_string()
            }
        );
        }

        // should revert when trying to burn more than minted
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let funds = vec![coin(
                vault.short_amount.u128() + 1u128,
                env.denoms["power"].clone(),
            )];

            // dont assert this error as the address change
            wasm.execute(
                &perp_address,
                &ExecuteMsg::BurnPowerPerp {
                    amount_to_withdraw: Some(Uint128::from(1u128)),
                    vault_id,
                },
                &funds,
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when if not operator of vault
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let funds = vec![coin(vault.short_amount.u128(), env.denoms["power"].clone())];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(1u128)),
                        vault_id,
                    },
                    &funds,
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                        err,
                        RunnerError::ExecuteError {
                            msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                        }
                    );
        }

        // should revert if vault is underwater
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let funds = vec![coin(
                vault.short_amount.u128() / 2,
                env.denoms["power"].clone(),
            )];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(vault.collateral.u128())),
                        vault_id,
                    },
                    &funds,
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault is not safe, cannot perform operation: execute wasm contract failed".to_string()
                }
            );
        }

        // TODO: should revert if vault after burning is dust
        {}

        // should revert if trying to withdraw would make vault underwater
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: vault.collateral,
                        vault_id,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault is not safe, cannot perform operation: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if different account tries to burn power token, with balance
        {
            let funds = vec![coin(1000u128, env.denoms["power"].clone())];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(1u128)),
                        vault_id,
                    },
                    &funds,
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if different account tries to withdraw collateral
        {
            let funds = vec![coin(1000u128, env.denoms["power"].clone())];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: None,
                        vault_id,
                    },
                    &funds,
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                }
            );
        }

        // should be able to burn power token
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let burn_amount = vault.short_amount.u128();
            let withdraw_amount = 5u128;

            let funds = vec![coin(burn_amount, env.denoms["power"].clone())];

            wasm.execute(
                &perp_address,
                &ExecuteMsg::BurnPowerPerp {
                    amount_to_withdraw: Some(withdraw_amount.into()),
                    vault_id,
                },
                &funds,
                &env.signer,
            )
            .unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            assert_eq!(
                u128::from_str(&power_balance_after).unwrap(),
                u128::from_str(&power_balance_before).unwrap() - burn_amount
            );
        }
    }

    // withdraw: remove collateral
    {
        // should revert if removing from vault 0
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: Uint128::from(100u128),
                        vault_id: 0u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault 0 does not exist, cannot perform operation: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if caller is not the operator
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: Uint128::from(100u128),
                        vault_id,
                    },
                    &[],
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if withdrawing more collateral than deposited
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: vault.collateral + Uint128::from(1u128),
                        vault_id,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot subtract more collateral than deposited: execute wasm contract failed".to_string()
                }
            );
        }

        // TODO: should revert if withdrawing would leave dust
        {}

        // should be able to remove collateral
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();
            let balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address.clone(),
                    denom: env.denoms["base"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let withdraw_amount = vault.collateral.checked_div(2u128.into()).unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::Withdraw {
                    amount: withdraw_amount,
                    vault_id,
                },
                &[],
                &env.signer,
            )
            .unwrap();

            let balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address,
                    denom: env.denoms["base"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            assert_eq!(
                u128::from_str(&balance_after).unwrap() + withdraw_amount.u128(),
                u128::from_str(&balance_before).unwrap()
            );
        }

        // TODO: close when empty, need to think really how to do this or why
    }
}

#[test]
fn test_basic_actions_staked_assets() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);
    let (perp_address, _) = env.deploy_power(&wasm, CONTRACT_NAME.to_string(), true, true);

    let config: ConfigResponse = wasm.query(&perp_address, &QueryMsg::Config {}).unwrap();
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

    const SCALED_POWER_PRICE: u128 = 3_010 * ONE / SCALE_FACTOR;
    const SCALED_BASE_PRICE: u128 = 3_000 * ONE / SCALE_FACTOR;
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

    // read_basic_properties
    {
        // should be able to get normalisation factor
        {
            // increase timestamp
            env.app.increase_time(30u64);

            let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            let normalisation_factor: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetNormalisationFactor {})
                .unwrap();
            assert!(state.normalisation_factor > normalisation_factor);

            // increase timestamp
            env.app.increase_time(30u64);

            let normalisation_factor_after: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetNormalisationFactor {})
                .unwrap();
            assert!(normalisation_factor > normalisation_factor_after);
        }

        // should allow anyone to call apply funding
        {
            let state: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();
            let expected_normalisation_factor: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetNormalisationFactor {})
                .unwrap();
            assert!(state.normalisation_factor > expected_normalisation_factor);

            wasm.execute(
                &perp_address,
                &ExecuteMsg::ApplyFunding {},
                &[],
                &env.traders[1], // one of the trading accounts should be able to call this
            )
            .unwrap();

            let state_after: State = wasm.query(&perp_address, &QueryMsg::State {}).unwrap();

            assert!(
                state_after
                    .normalisation_factor
                    .abs_diff(expected_normalisation_factor)
                    < Decimal::from_str("0.00001").unwrap()
            );
            assert!(state.normalisation_factor > state_after.normalisation_factor);
        }

        // TODO: need to add multiple function execution
        // // should not update funding twice in a single block
        // {
        //     wasm.execute_multiple(contract, msg, funds, signer)
        // }

        // should be able to get index and mark price used for funding
        {
            env.app.increase_time(30u64);

            let mark_price: Decimal = wasm
                .query(
                    &perp_address,
                    &QueryMsg::GetDenormalisedMark { period: 30u64 },
                )
                .unwrap();
            let mark_price_funding: Decimal = wasm
                .query(
                    &perp_address,
                    &QueryMsg::GetDenormalisedMarkFunding { period: 30u64 },
                )
                .unwrap();

            let expected_mark = Decimal::from_atomics(SCALED_BASE_PRICE, 6u32).unwrap()
                * Decimal::from_atomics(SCALED_POWER_PRICE, 6u32).unwrap();

            assert!(mark_price.abs_diff(expected_mark) < Decimal::from_atomics(3u128, 0).unwrap());
            assert!(
                mark_price_funding.abs_diff(expected_mark)
                    < Decimal::from_atomics(3u128, 0).unwrap()
            );
        }

        // should be able to get scaled index
        {
            let index: Decimal = wasm
                .query(&perp_address, &QueryMsg::GetUnscaledIndex { period: 30u64 })
                .unwrap();

            let eth_squared =
                Decimal::from_str("3000.0").unwrap() * Decimal::from_str("3000.0").unwrap();

            assert_eq!(index, eth_squared);
        }

        // TODO: should revert when sending eth to controller from an EOA
        // not sure this is quite possible in CosmWasm
    }

    let vault_id: u64;
    // mint: open vault
    {
        // should be able to open a vaults
        {
            let mint_response = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::from(500_000u128),
                        vault_id: None,
                        rebase: false,
                    },
                    &[coin(500_000u64.into(), env.denoms["stake"].to_string())],
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
    }

    // deposit: deposit collateral
    {
        // should revert when trying to deposit to vault 0
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id: 0u64 },
                &[coin(1u64.into(), env.denoms["stake"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when trying to deposit to non-existent vault
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id: 10u64 },
                &[coin(1u64.into(), env.denoms["stake"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when trying to mint to non-existent vault
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(100u128),
                    vault_id: Some(10u64),
                    rebase: false,
                },
                &[coin(100u64.into(), env.denoms["stake"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when trying to deposit to vault where info.sender is not operator
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id },
                &[coin(1u64.into(), env.denoms["stake"].to_string())],
                &env.traders[0],
            )
            .unwrap_err();
        }

        // should be able to deposit collateral
        {
            let deposit_amount = 45_000_000u128;
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address.clone(),
                    denom: env.denoms["stake"].to_string(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_before: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::Deposit { vault_id },
                &[coin(deposit_amount, env.denoms["stake"].to_string())],
                &env.signer,
            )
            .unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address.clone(),
                    denom: env.denoms["stake"].to_string(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_after: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                deposit_amount + u128::from_str(&power_balance_before).unwrap(),
                u128::from_str(&power_balance_after).unwrap()
            );
            assert_eq!(
                Uint128::from(deposit_amount) + vault_before.collateral,
                vault_after.collateral
            );
        }

        // should not be able to deposit zero collateral
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Deposit { vault_id },
                    &[coin(0u128, env.denoms["stake"].to_string())],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "sentFunds: invalid coins".to_string()
                }
            );

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Deposit { vault_id },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
                }
            );
        }
    }

    // mint: mint power tokens
    {
        // should revert if not called by operator
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(100u128),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[coin(100u128, env.denoms["stake"].to_string())],
                &env.traders[0],
            )
            .unwrap_err();
        }

        // should revert if vault does not exist
        {
            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(100u128),
                    vault_id: Some(110u64),
                    rebase: false,
                },
                &[coin(100u128, env.denoms["stake"].to_string())],
                &env.signer,
            )
            .unwrap_err();
        }

        // should be able to mint power token
        {
            let amount = 100_000_000u128;
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_before: VaultResponse = wasm
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
                &env.signer,
            )
            .unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let vault_after: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            assert_eq!(
                amount + u128::from_str(&power_balance_before).unwrap(),
                u128::from_str(&power_balance_after).unwrap()
            );
            assert_eq!(
                Uint128::from(amount) + vault_before.short_amount,
                vault_after.short_amount
            );
        }

        // should revert if minting more than collateral ratio
        {
            let amount = 100_000_000u128;

            wasm.execute(
                &perp_address,
                &ExecuteMsg::MintPowerPerp {
                    amount: Uint128::from(amount),
                    vault_id: Some(vault_id),
                    rebase: false,
                },
                &[],
                &env.signer,
            )
            .unwrap_err();
        }
    }

    // burn: burn power token
    {
        // should revert if no funds are sent
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: None,
                        vault_id,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Invalid funds: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert when trying to burn for vault 0
        {
            let funds = vec![coin(100u128, env.denoms["power"].clone())];
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(100u128)),
                        vault_id: 0u64,
                    },
                    &funds,
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Vault 0 does not exist, cannot perform operation: execute wasm contract failed".to_string()
            }
        );
        }

        // should revert when trying to burn more than minted
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let funds = vec![coin(
                vault.short_amount.u128() + 1u128,
                env.denoms["power"].clone(),
            )];

            // dont assert this error as the address change
            wasm.execute(
                &perp_address,
                &ExecuteMsg::BurnPowerPerp {
                    amount_to_withdraw: Some(Uint128::from(1u128)),
                    vault_id,
                },
                &funds,
                &env.signer,
            )
            .unwrap_err();
        }

        // should revert when if not operator of vault
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let funds = vec![coin(vault.short_amount.u128(), env.denoms["power"].clone())];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(1u128)),
                        vault_id,
                    },
                    &funds,
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                        err,
                        RunnerError::ExecuteError {
                            msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                        }
                    );
        }

        // should revert if vault is underwater
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let funds = vec![coin(
                vault.short_amount.u128() / 2,
                env.denoms["power"].clone(),
            )];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(vault.collateral.u128())),
                        vault_id,
                    },
                    &funds,
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault is not safe, cannot perform operation: execute wasm contract failed".to_string()
                }
            );
        }

        // TODO: should revert if vault after burning is dust
        {}

        // should revert if trying to withdraw would make vault underwater
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: vault.collateral,
                        vault_id,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault is not safe, cannot perform operation: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if different account tries to burn power token, with balance
        {
            let funds = vec![coin(1000u128, env.denoms["power"].clone())];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: Some(Uint128::from(1u128)),
                        vault_id,
                    },
                    &funds,
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if different account tries to withdraw collateral
        {
            let funds = vec![coin(1000u128, env.denoms["power"].clone())];

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: None,
                        vault_id,
                    },
                    &funds,
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                }
            );
        }

        // should be able to burn power token
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();
            let power_balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let burn_amount = vault.short_amount.u128();
            let withdraw_amount = 5u128;

            let funds = vec![coin(burn_amount, env.denoms["power"].clone())];

            wasm.execute(
                &perp_address,
                &ExecuteMsg::BurnPowerPerp {
                    amount_to_withdraw: Some(withdraw_amount.into()),
                    vault_id,
                },
                &funds,
                &env.signer,
            )
            .unwrap();

            let power_balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: env.signer.address(),
                    denom: env.denoms["power"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            assert_eq!(
                u128::from_str(&power_balance_after).unwrap(),
                u128::from_str(&power_balance_before).unwrap() - burn_amount
            );
        }
    }

    // withdraw: remove collateral
    {
        // should revert if removing from vault 0
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: Uint128::from(100u128),
                        vault_id: 0u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Vault 0 does not exist, cannot perform operation: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if caller is not the operator
        {
            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: Uint128::from(100u128),
                        vault_id,
                    },
                    &[],
                    &env.owner,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: operator does not match: execute wasm contract failed".to_string()
                }
            );
        }

        // should revert if withdrawing more collateral than deposited
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();

            let err = wasm
                .execute(
                    &perp_address,
                    &ExecuteMsg::Withdraw {
                        amount: vault.collateral + Uint128::from(1u128),
                        vault_id,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot subtract more collateral than deposited: execute wasm contract failed".to_string()
                }
            );
        }

        // TODO: should revert if withdrawing would leave dust
        {}

        // should be able to remove collateral
        {
            let vault: VaultResponse = wasm
                .query(&perp_address, &QueryMsg::GetVault { vault_id })
                .unwrap();
            let balance_before = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address.clone(),
                    denom: env.denoms["stake"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            let withdraw_amount = vault.collateral.checked_div(2u128.into()).unwrap();

            wasm.execute(
                &perp_address,
                &ExecuteMsg::Withdraw {
                    amount: withdraw_amount,
                    vault_id,
                },
                &[],
                &env.signer,
            )
            .unwrap();

            let balance_after = bank
                .query_balance(&QueryBalanceRequest {
                    address: perp_address,
                    denom: env.denoms["stake"].clone(),
                })
                .unwrap()
                .balance
                .unwrap()
                .amount;

            assert_eq!(
                u128::from_str(&balance_after).unwrap() + withdraw_amount.u128(),
                u128::from_str(&balance_before).unwrap()
            );
        }

        // TODO: close when empty, need to think really how to do this or why
    }
}

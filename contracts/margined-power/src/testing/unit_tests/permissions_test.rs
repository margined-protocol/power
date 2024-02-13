use crate::{contract::CONTRACT_NAME, state::State, testing::test_utils::MOCK_FEE_POOL_ADDR};

use cosmwasm_std::{coin, Decimal, Uint128};
use margined_protocol::power::{ExecuteMsg, InstantiateMsg, QueryMsg};
use margined_testing::{
    helpers::store_code,
    power_env::{PowerEnv, SCALE_FACTOR},
};
use osmosis_test_tube::{
    osmosis_std::types::osmosis::tokenfactory::v1beta1::MsgChangeAdmin, Account, Module,
    RunnerError, TokenFactory, Wasm,
};

#[test]
fn test_permissions() {
    let env = PowerEnv::new();

    let token = TokenFactory::new(&env.app);
    let wasm = Wasm::new(&env.app);

    let query_address = env.deploy_query_contracts(&wasm, false);

    let code_id = store_code(&wasm, &env.signer, CONTRACT_NAME.to_string());
    let address = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: query_address,
                power_denom: env.denoms["power"].clone(),
                base_denom: env.denoms["base"].clone(),
                stake_assets: None,
                base_pool_id: env.base_pool_id,
                base_pool_quote: env.denoms["quote"].clone(),
                power_pool_id: env.power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &env.signer,
        )
        .unwrap()
        .data
        .address;

    let mut timestamp = env.app.get_block_timestamp();

    let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
    assert_eq!(
        state,
        State {
            is_open: false,
            is_paused: true,
            normalisation_factor: Decimal::one(),
            last_funding_update: timestamp,
            last_pause: timestamp,
        }
    );

    // check permissions when contract is not open and is paused
    {
        // pause should fail, contract not set open
        {
            let err = wasm
                .execute(&address, &ExecuteMsg::Pause {}, &[], &env.signer)
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }

        // unpause should fail, contract not set open
        {
            let err = wasm
                .execute(&address, &ExecuteMsg::UnPause {}, &[], &env.signer)
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }

        // mint should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::from(1_000_000u128),
                        vault_id: None,
                        rebase: false,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }

        // burn should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: None,
                        vault_id: 1u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }

        // deposit should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::Deposit { vault_id: 1u64 },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }

        // withdraw should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::Withdraw {
                        amount: Uint128::from(1_000_000u64),
                        vault_id: 1u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }
        // liquidation should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::Liquidate {
                        max_debt_amount: Uint128::from(1_000_000u64),
                        vault_id: 1u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }
    }

    // set contract open, should fail as not admin
    {
        let err = wasm
            .execute(&address, &ExecuteMsg::SetOpen {}, &[], &env.signer)
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Contract is not admin of the power token: execute wasm contract failed".to_string()
            }
        );
    }

    // set contract open
    {
        token
            .change_admin(
                MsgChangeAdmin {
                    sender: env.signer.address(),
                    new_admin: address.clone(),
                    denom: env.denoms["power"].clone(),
                },
                &env.signer,
            )
            .unwrap();

        wasm.execute(&address, &ExecuteMsg::SetOpen {}, &[], &env.signer)
            .unwrap();

        timestamp = env.app.get_block_timestamp();

        let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
        assert_eq!(
            state,
            State {
                is_open: true,
                is_paused: false,
                normalisation_factor: Decimal::one(),
                last_funding_update: timestamp,
                last_pause: timestamp,
            }
        );
    }

    // set contract open, should fail as already open
    {
        let err = wasm
            .execute(&address, &ExecuteMsg::SetOpen {}, &[], &env.signer)
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Contract is already open: execute wasm contract failed".to_string()
            }
        );
    }

    // check permissions when contract is open and is paused
    {
        // pause contract
        {
            wasm.execute(&address, &ExecuteMsg::Pause {}, &[], &env.signer)
                .unwrap();

            let latest_timestamp = env.app.get_block_timestamp();

            let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
            assert_eq!(
                state,
                State {
                    is_open: true,
                    is_paused: true,
                    normalisation_factor: Decimal::one(),
                    last_funding_update: timestamp,
                    last_pause: latest_timestamp,
                }
            );
        }

        // pause should fail, contract not set open
        {
            let err = wasm
                .execute(&address, &ExecuteMsg::Pause {}, &[], &env.signer)
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is paused: execute wasm contract failed".to_string()
                }
            );
        }

        // mint should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::MintPowerPerp {
                        amount: Uint128::from(1_000_000u128),
                        vault_id: None,
                        rebase: false,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is paused: execute wasm contract failed".to_string()
                }
            );
        }

        // burn should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::BurnPowerPerp {
                        amount_to_withdraw: None,
                        vault_id: 1u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is paused: execute wasm contract failed".to_string()
                }
            );
        }

        // deposit should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::Deposit { vault_id: 1u64 },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is paused: execute wasm contract failed".to_string()
                }
            );
        }

        // withdraw should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::Withdraw {
                        amount: Uint128::from(1_000_000u64),
                        vault_id: 1u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is paused: execute wasm contract failed".to_string()
                }
            );
        }
        // liquidation should fail, contract not set open
        {
            let err = wasm
                .execute(
                    &address,
                    &ExecuteMsg::Liquidate {
                        max_debt_amount: Uint128::from(1_000_000u64),
                        vault_id: 1u64,
                    },
                    &[],
                    &env.signer,
                )
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is paused: execute wasm contract failed".to_string()
                }
            );
        }
    }
}

#[test]
fn test_pause_unpause() {
    let env = PowerEnv::new();

    let token = TokenFactory::new(&env.app);
    let wasm = Wasm::new(&env.app);

    let query_address = env.deploy_query_contracts(&wasm, false);

    let code_id = store_code(&wasm, &env.signer, CONTRACT_NAME.to_string());
    let address = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                fee_pool: MOCK_FEE_POOL_ADDR.to_string(),
                fee_rate: "0.1".to_string(),
                query_contract: query_address,
                power_denom: env.denoms["power"].clone(),
                base_denom: env.denoms["base"].clone(),
                stake_assets: None,
                base_pool_id: env.base_pool_id,
                base_pool_quote: env.denoms["quote"].clone(),
                power_pool_id: env.power_pool_id,
                base_decimals: 6u32,
                power_decimals: 6u32,
                index_scale: SCALE_FACTOR as u64,
                min_collateral_amount: "0.5".to_string(),
            },
            None,
            Some("margined-power-contract"),
            &[coin(10_000_000, "uosmo")],
            &env.signer,
        )
        .unwrap()
        .data
        .address;

    let mut timestamp = env.app.get_block_timestamp();

    let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
    assert_eq!(
        state,
        State {
            is_open: false,
            is_paused: true,
            normalisation_factor: Decimal::one(),
            last_funding_update: timestamp,
            last_pause: timestamp,
        }
    );

    // check permissions when contract is not open and is paused
    {
        // pause should fail, contract not set open
        {
            let err = wasm
                .execute(&address, &ExecuteMsg::Pause {}, &[], &env.signer)
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }

        // unpause should fail, contract not set open
        {
            let err = wasm
                .execute(&address, &ExecuteMsg::UnPause {}, &[], &env.signer)
                .unwrap_err();
            assert_eq!(
                err,
                RunnerError::ExecuteError {
                    msg: "failed to execute message; message index: 0: Cannot perform action as contract is not open: execute wasm contract failed".to_string()
                }
            );
        }
    }

    // set contract open
    {
        token
            .change_admin(
                MsgChangeAdmin {
                    sender: env.signer.address(),
                    new_admin: address.clone(),
                    denom: env.denoms["power"].clone(),
                },
                &env.signer,
            )
            .unwrap();

        wasm.execute(&address, &ExecuteMsg::SetOpen {}, &[], &env.signer)
            .unwrap();

        timestamp = env.app.get_block_timestamp();

        let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
        assert_eq!(
            state,
            State {
                is_open: true,
                is_paused: false,
                normalisation_factor: Decimal::one(),
                last_funding_update: timestamp,
                last_pause: timestamp,
            }
        );
    }

    // check permissions when contract is open and is paused
    let latest_timestamp = env.app.get_block_timestamp();

    // pause contract
    {
        wasm.execute(&address, &ExecuteMsg::Pause {}, &[], &env.signer)
            .unwrap();

        let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
        assert_eq!(
            state,
            State {
                is_open: true,
                is_paused: true,
                normalisation_factor: Decimal::one(),
                last_funding_update: timestamp,
                last_pause: latest_timestamp.plus_seconds(5u64),
            }
        );
    }

    // pause should fail, contract already paused
    {
        let err = wasm
            .execute(&address, &ExecuteMsg::Pause {}, &[], &env.signer)
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Generic error: Cannot perform action as contract is paused: execute wasm contract failed".to_string()
            }
        );
    }

    // unpause should fail, timer has not expired
    {
        let err = wasm
            .execute(&address, &ExecuteMsg::UnPause {}, &[], &env.traders[0])
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Unpause delay not expired: execute wasm contract failed".to_string()
            }
        );
    }

    env.app.increase_time(7 * 24 * 60 * 60);

    // unpause should pass
    {
        wasm.execute(&address, &ExecuteMsg::UnPause {}, &[], &env.traders[0])
            .unwrap();

        let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
        assert_eq!(
            state,
            State {
                is_open: true,
                is_paused: false,
                normalisation_factor: Decimal::one(),
                last_funding_update: timestamp,
                last_pause: latest_timestamp.plus_seconds(5u64),
            }
        );
    }

    // check permissions when contract is open and is paused using the admin
    let latest_timestamp = env.app.get_block_timestamp();

    // pause contract
    {
        wasm.execute(&address, &ExecuteMsg::Pause {}, &[], &env.signer)
            .unwrap();

        let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
        assert_eq!(
            state,
            State {
                is_open: true,
                is_paused: true,
                normalisation_factor: Decimal::one(),
                last_funding_update: timestamp,
                last_pause: latest_timestamp.plus_seconds(5u64),
            }
        );
    }
    // unpause should pass
    {
        wasm.execute(&address, &ExecuteMsg::UnPause {}, &[], &env.signer)
            .unwrap();

        let state: State = wasm.query(&address, &QueryMsg::State {}).unwrap();
        assert_eq!(
            state,
            State {
                is_open: true,
                is_paused: false,
                normalisation_factor: Decimal::one(),
                last_funding_update: timestamp,
                last_pause: latest_timestamp.plus_seconds(5u64),
            }
        );
    }
}

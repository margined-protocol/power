use cosmwasm_std::{Addr, Uint128};
use margined_protocol::collector::{
    AllTokenResponse, ExecuteMsg, QueryMsg, TokenLengthResponse, TokenResponse, WhitelistResponse,
};
use margined_testing::staking_env::StakingEnv;
use osmosis_test_tube::{
    osmosis_std::types::cosmos::{bank::v1beta1::MsgSend, base::v1beta1::Coin},
    Account, Bank, Module, Wasm,
};

#[test]
fn test_instantiation() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    let owner: Addr = wasm.query(&fee_collector, &QueryMsg::Owner {}).unwrap();
    assert_eq!(owner, env.signer.address());
}

#[test]
fn test_update_whitelist() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // update the whitelist
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::UpdateWhitelist {
            address: env.traders[0].address(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let res: WhitelistResponse = wasm
        .query(&fee_collector, &QueryMsg::GetWhitelist {})
        .unwrap();
    assert_eq!(res.address, Some(Addr::unchecked(env.traders[0].address())));
}

#[test]
fn test_query_token() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // add token to tokenlist here
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // query if the token has been added
    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "uusdc".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(is_token);
}

#[test]
fn test_query_all_token() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // check to see that there are no tokens listed
    let res: AllTokenResponse = wasm
        .query(&fee_collector, &QueryMsg::GetTokenList { limit: None })
        .unwrap();

    assert!(res.token_list.is_empty());

    // add a token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // add another token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uosmo".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // check for the added tokens
    let res: AllTokenResponse = wasm
        .query(&fee_collector, &QueryMsg::GetTokenList { limit: None })
        .unwrap();

    assert_eq!(
        res.token_list,
        vec!["uusdc".to_string(), "uosmo".to_string(),]
    );
}

#[test]
fn test_add_token() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // check to see that there are no tokens listed
    let res: AllTokenResponse = wasm
        .query(&fee_collector, &QueryMsg::GetTokenList { limit: None })
        .unwrap();

    assert!(res.token_list.is_empty());

    // query the token we want to add
    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "uusdc".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(!is_token);

    // add a token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // check for the added token
    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "uusdc".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(is_token);
}

#[test]
fn test_add_token_twice() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // add a token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // add a token again!
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::AddToken {
                token: "uusdc".to_string(),
            },
            &[],
            &env.signer,
        )
        .unwrap_err();

    assert_eq!(
        res.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: This token is already added: execute wasm contract failed"
    );
}

#[test]
fn test_add_second_token() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // check to see that there are no tokens listed
    let res: AllTokenResponse = wasm
        .query(&fee_collector, &QueryMsg::GetTokenList { limit: None })
        .unwrap();

    assert!(res.token_list.is_empty());

    // add a token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // add another token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uosmo".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // check for the added token
    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "uosmo".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(is_token);
}

#[test]
fn test_remove_token() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // check to see that there are no tokens listed
    let res: AllTokenResponse = wasm
        .query(&fee_collector, &QueryMsg::GetTokenList { limit: None })
        .unwrap();

    assert!(res.token_list.is_empty());

    // add a token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "uusdc".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(is_token);

    // remove the first token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::RemoveToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // check that the first token is not there
    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "uusdc".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(!is_token);
}

#[test]
fn test_remove_non_existing_token() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // add a token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "uusdc".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(is_token);

    // check that the first token is not there
    let res: TokenResponse = wasm
        .query(
            &fee_collector,
            &QueryMsg::IsToken {
                token: "token2".to_string(),
            },
        )
        .unwrap();
    let is_token = res.is_token;

    assert!(!is_token);

    // remove a token which isn't stored
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::RemoveToken {
                token: "token2".to_string(),
            },
            &[],
            &env.signer,
        )
        .unwrap_err();

    assert_eq!(
        res.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: This token has not been added: execute wasm contract failed"
    )
}

#[test]
fn test_token_capacity() {
    // for the purpose of this test, TOKEN_LIMIT is set to 3 (so four exceeds it!)
    // instantiate contract here
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    let tokens: Vec<String> = vec![
        "uusdc".to_string(),
        "uosmo".to_string(),
        "token3".to_string(),
        "token4".to_string(),
    ];

    // add three tokens
    for n in 1..4 {
        wasm.execute(
            &fee_collector,
            &ExecuteMsg::AddToken {
                token: tokens[n - 1].clone(),
            },
            &[],
            &env.signer,
        )
        .unwrap();
    }

    // try to add a fourth token
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::AddToken {
                token: "token5".to_string(),
            },
            &[],
            &env.signer,
        )
        .unwrap_err();
    assert_eq!(
        res.to_string(),
        "execute error: failed to execute message; message index: 0: Generic error: The token capacity is already reached: execute wasm contract failed"
    );
}

#[test]
fn test_token_length() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // check to see that there are no tokens listed
    let res: AllTokenResponse = wasm
        .query(&fee_collector, &QueryMsg::GetTokenList { limit: None })
        .unwrap();

    assert!(res.token_list.is_empty());

    // add a token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uusdc".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // add another token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uosmo".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // check for the second added token
    let res: TokenLengthResponse = wasm
        .query(&fee_collector, &QueryMsg::GetTokenLength {})
        .unwrap();

    assert_eq!(res.length, 2usize);
}

#[test]
fn test_send_native_token() {
    // Using the native token, we only work to 6dp

    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // give funds to the fee pool contract
    bank.send(
        MsgSend {
            from_address: env.signer.address(),
            to_address: fee_collector.clone(),
            amount: vec![Coin {
                amount: (5_000 * 10u128.pow(6)).to_string(),
                denom: "uosmo".to_string(),
            }],
        },
        &env.signer,
    )
    .unwrap();

    // add the token so we can send funds with it
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uosmo".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // query balance of bob
    let balance = env.get_balance(env.empty.address(), "uosmo".to_string());
    assert_eq!(balance, Uint128::zero());

    // query balance of contract
    let balance = env.get_balance(fee_collector.clone(), "uosmo".to_string());
    assert_eq!(balance, Uint128::from(5_000u128 * 10u128.pow(6)));

    // send token
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::SendToken {
            token: "uosmo".to_string(),
            amount: Uint128::from(1000u128 * 10u128.pow(6)),
            recipient: env.empty.address(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // query new balance of intended recipient
    let balance = env.get_balance(env.empty.address(), "uosmo".to_string());
    assert_eq!(balance, Uint128::from(1_000u128 * 10u128.pow(6)));

    // Query new contract balance
    let balance = env.get_balance(fee_collector, "uosmo".to_string());
    assert_eq!(balance, Uint128::from(4000u128 * 10u128.pow(6)));
}

#[test]
fn test_send_native_token_unsupported_token() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // give funds to the fee pool contract
    bank.send(
        MsgSend {
            from_address: env.signer.address(),
            to_address: fee_collector.clone(),
            amount: vec![Coin {
                amount: (5_000u128 * 10u128.pow(6)).to_string(),
                denom: "uosmo".to_string(),
            }],
        },
        &env.signer,
    )
    .unwrap();

    // try to send token - note this fails because we have not added the token to the token list, so it is not accepted/supported yet
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::SendToken {
                token: "uosmo".to_string(),
                amount: Uint128::from(1000u128 * 10u128.pow(6)),
                recipient: env.empty.address(),
            },
            &[],
            &env.signer,
        )
        .unwrap_err();
    assert_eq!(
        "execute error: failed to execute message; message index: 0: Token denom 'uosmo' is not supported: execute wasm contract failed",
        res.to_string()
    );
}

#[test]
fn test_send_native_token_insufficient_balance() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);
    let bank = Bank::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // give funds to the fee pool contract
    bank.send(
        MsgSend {
            from_address: env.signer.address(),
            to_address: fee_collector.clone(),
            amount: vec![Coin {
                amount: (1_000u128 * 10u128.pow(6)).to_string(),
                denom: "uosmo".to_string(),
            }],
        },
        &env.signer,
    )
    .unwrap();

    // add the token so we can send funds with it
    wasm.execute(
        &fee_collector,
        &ExecuteMsg::AddToken {
            token: "uosmo".to_string(),
        },
        &[],
        &env.signer,
    )
    .unwrap();

    // query balance of bob
    let balance = env.get_balance(env.empty.address(), "uosmo".to_string());
    assert_eq!(balance, Uint128::zero());

    // query balance of contract
    let balance = env.get_balance(fee_collector.clone(), "uosmo".to_string());
    assert_eq!(balance, Uint128::from(1000u128 * 10u128.pow(6)));

    // send token
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::SendToken {
                token: "uosmo".to_string(),
                amount: Uint128::from(2000u128 * 10u128.pow(6)),
                recipient: env.empty.address(),
            },
            &[],
            &env.signer,
        )
        .unwrap_err();
    assert_eq!(
        "execute error: failed to execute message; message index: 0: Insufficient balance: execute wasm contract failed".to_string(),
        res.to_string()
    );
    // query new balance of intended recipient
    let balance = env.get_balance(env.empty.address(), "uosmo".to_string());
    assert_eq!(balance, Uint128::zero());

    // Query new contract balance
    let balance = env.get_balance(fee_collector, "uosmo".to_string());
    assert_eq!(balance, Uint128::from(1000u128 * 10u128.pow(6)));
}

#[test]
fn test_not_owner() {
    let env = StakingEnv::new();

    let wasm = Wasm::new(&env.app);

    let fee_collector = env.deploy_fee_collector_contract(&wasm, "margined-collector".to_string());

    // try to add a token
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::AddToken {
                token: "uosmo".to_string(),
            },
            &[],
            &env.traders[0],
        )
        .unwrap_err();
    assert_eq!(res.to_string(), "execute error: failed to execute message; message index: 0: Unauthorized: execute wasm contract failed");

    // try to remove a token
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::RemoveToken {
                token: "uusdc".to_string(),
            },
            &[],
            &env.traders[0],
        )
        .unwrap_err();
    assert_eq!(res.to_string(), "execute error: failed to execute message; message index: 0: Unauthorized: execute wasm contract failed");

    // try to send money
    let res = wasm
        .execute(
            &fee_collector,
            &ExecuteMsg::SendToken {
                token: "uosmo".to_string(),
                amount: Uint128::from(2000u128 * 10u128.pow(6)),
                recipient: env.traders[0].address(),
            },
            &[],
            &env.traders[0],
        )
        .unwrap_err();
    assert_eq!("execute error: failed to execute message; message index: 0: Unauthorized: execute wasm contract failed".to_string(), res.to_string());
}

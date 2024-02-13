use crate::{contract::CONTRACT_NAME, testing::test_utils::MOCK_FEE_POOL_ADDR};

use cosmwasm_std::{coin, Addr};
use margined_protocol::power::{ExecuteMsg, InstantiateMsg, OwnerProposalResponse, QueryMsg};
use margined_testing::{
    helpers::store_code,
    power_env::{PowerEnv, SCALE_FACTOR},
};
use osmosis_test_tube::{Account, Module, RunnerError, Wasm};

const PROPOSAL_DURATION: u64 = 1000;

#[test]
fn test_update_owner() {
    let env = PowerEnv::new();

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

    // claim before a proposal is made
    {
        let err = wasm
            .execute(&address, &ExecuteMsg::ClaimOwnership {}, &[], &env.signer)
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Proposal not found: execute wasm contract failed".to_string()
            }
        );
    }

    // propose new owner
    wasm.execute(
        &address,
        &ExecuteMsg::ProposeNewOwner {
            new_owner: env.traders[0].address(),
            duration: PROPOSAL_DURATION,
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let owner: Addr = wasm.query(&address, &QueryMsg::Owner {}).unwrap();
    assert_eq!(owner, env.signer.address());

    // reject claim by incorrect new owner
    {
        let err = wasm
            .execute(&address, &ExecuteMsg::ClaimOwnership {}, &[], &env.signer)
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Unauthorized: execute wasm contract failed".to_string()
            }
        );
    }

    // let proposal expire
    env.app.increase_time(PROPOSAL_DURATION + 1);

    // proposal fails due to expiry
    {
        let err = wasm
            .execute(
                &address,
                &ExecuteMsg::ClaimOwnership {},
                &[],
                &env.traders[0],
            )
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Expired: execute wasm contract failed".to_string()
            }
        );
    }

    let owner: Addr = wasm.query(&address, &QueryMsg::Owner {}).unwrap();
    assert_eq!(owner, env.signer.address());

    // propose new owner
    wasm.execute(
        &address,
        &ExecuteMsg::ProposeNewOwner {
            new_owner: env.traders[0].address(),
            duration: PROPOSAL_DURATION,
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let owner: Addr = wasm.query(&address, &QueryMsg::Owner {}).unwrap();
    assert_eq!(owner, env.signer.address());

    // proposal fails due to expiry
    {
        let err = wasm
            .execute(&address, &ExecuteMsg::RejectOwner {}, &[], &env.traders[0])
            .unwrap_err();
        assert_eq!(
            err,
            RunnerError::ExecuteError {
                msg: "failed to execute message; message index: 0: Unauthorized: execute wasm contract failed".to_string()
            }
        );
    }

    // proposal fails due to expiry
    {
        wasm.execute(&address, &ExecuteMsg::RejectOwner {}, &[], &env.signer)
            .unwrap();
    }

    // propose new owner
    wasm.execute(
        &address,
        &ExecuteMsg::ProposeNewOwner {
            new_owner: env.traders[0].address(),
            duration: PROPOSAL_DURATION,
        },
        &[],
        &env.signer,
    )
    .unwrap();

    let block_time = env.app.get_block_time_seconds();

    let owner: Addr = wasm.query(&address, &QueryMsg::Owner {}).unwrap();
    assert_eq!(owner, env.signer.address());

    // query ownership proposal
    {
        let proposal: OwnerProposalResponse = wasm
            .query(&address, &QueryMsg::GetOwnershipProposal {})
            .unwrap();

        assert_eq!(proposal.owner, env.traders[0].address());
        assert_eq!(proposal.expiry, block_time as u64 + PROPOSAL_DURATION);
    }

    // claim ownership
    {
        wasm.execute(
            &address,
            &ExecuteMsg::ClaimOwnership {},
            &[],
            &env.traders[0],
        )
        .unwrap();
    }

    let owner: Addr = wasm.query(&address, &QueryMsg::Owner {}).unwrap();
    assert_eq!(owner, env.traders[0].address());
}

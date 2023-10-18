use crate::contract::{ExecuteMsg, InstantiateMsg, QueryMsg};

use cosmwasm_std::{coin, Decimal};
use margined_testing::setup::Setup;
use osmosis_test_tube::{Module, OsmosisTestApp, SigningAccount, Wasm};
use std::str::FromStr;

#[test]
fn test_instantiation_and_query() {
    let Setup {
        app,
        signer,
        base_pool_id,
        .. // other fields
    } = Setup::new();

    let wasm = Wasm::new(&app);

    // let timestamp_now = app.get_block_time_seconds();
    // let start_time = timestamp_now - 1i64;

    let code_id = store_code(&wasm, &signer, "mock_query".to_string());
    let address = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {},
            None,
            Some("mock-query-contract"),
            &[coin(10_000_000, "uosmo")],
            &signer,
        )
        .unwrap()
        .data
        .address;

    let pool_id = 1u64;
    let price = Decimal::from_str("1.5").unwrap();
    wasm.execute(
        &address,
        &ExecuteMsg::AppendPrice { pool_id, price },
        &[],
        &signer,
    )
    .unwrap();

    let price: Decimal = wasm
        .query(
            &address,
            &QueryMsg::GetArithmeticTwapToNow {
                pool_id: base_pool_id,
                base_asset: "uosmo".to_string(),
                quote_asset: "usdc".to_string(),
                start_time: start_time,
            },
        )
        .unwrap();
    assert_eq!(price, Decimal::from_str("1.5").unwrap());
}

fn wasm_file(contract_name: String) -> String {
    let artifacts_dir =
        std::env::var("ARTIFACTS_DIR_PATH").unwrap_or_else(|_| "artifacts".to_string());
    let snaked_name = contract_name.replace('-', "_");
    format!("../../../{artifacts_dir}/{snaked_name}-aarch64.wasm")
}

fn store_code(wasm: &Wasm<OsmosisTestApp>, owner: &SigningAccount, contract_name: String) -> u64 {
    let wasm_byte_code = std::fs::read(wasm_file(contract_name)).unwrap();
    wasm.store_code(&wasm_byte_code, None, owner)
        .unwrap()
        .data
        .code_id
}

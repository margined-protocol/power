use cosmwasm_std::{Event, Uint128};
use osmosis_test_tube::{OsmosisTestApp, SigningAccount, Wasm};

pub fn wasm_file(contract_name: String) -> String {
    let snaked_name = contract_name.replace('-', "_");

    let target = format!("../../target/wasm32-unknown-unknown/release/{snaked_name}.wasm");
    if std::path::Path::new(&target).exists() {
        target
    } else {
        let arch = std::env::consts::ARCH;

        let artifacts_dir =
            std::env::var("ARTIFACTS_DIR_PATH").unwrap_or_else(|_| "artifacts".to_string());
        format!("../../{artifacts_dir}/{snaked_name}-{arch}.wasm")
    }
}

pub fn store_code(
    wasm: &Wasm<OsmosisTestApp>,
    owner: &SigningAccount,
    contract_name: String,
) -> u64 {
    let wasm_byte_code = std::fs::read(wasm_file(contract_name)).unwrap();
    wasm.store_code(&wasm_byte_code, None, owner)
        .unwrap()
        .data
        .code_id
}

pub fn is_similar(a: Uint128, b: Uint128, epsilon: Uint128) -> bool {
    if a < b {
        (b - a) < epsilon
    } else {
        (a - b) < epsilon
    }
}

pub fn parse_event_attribute(events: Vec<Event>, event: &str, key: &str) -> String {
    events
        .iter()
        .find(|e| e.ty == event)
        .unwrap()
        .attributes
        .iter()
        .find(|e| e.key == key)
        .unwrap()
        .value
        .clone()
}

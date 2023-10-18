use cosmwasm_std::Decimal;
use margined_protocol::query::QueryMsg;
use margined_testing::power_env::PowerEnv;
use osmosis_test_tube::{Module, Wasm};
use std::str::FromStr;

#[test]
fn test_instantiation_and_queries() {
    let env = PowerEnv::new();

    let wasm = Wasm::new(&env.app);

    let contract_address = env.deploy_query_contracts(&wasm, false);

    let cw_now = cosmwasm_std::Timestamp::from_nanos((env.app.get_block_time_nanos()) as u64);

    let index: Decimal = wasm
        .query(
            &contract_address,
            &QueryMsg::GetArithmeticTwapToNow {
                pool_id: 1,
                base_asset: env.denoms["base"].clone(),
                quote_asset: env.denoms["quote"].clone(),
                start_time: cw_now,
            },
        )
        .unwrap();
    assert_eq!(index, Decimal::from_str("3000.0").unwrap());
}

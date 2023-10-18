use cosmwasm_std::{to_binary, CosmosMsg, Deps, Response, StdResult, Uint128, WasmMsg};
use margined_protocol::collector::ExecuteMsg as FeeExecuteMsg;
use osmosis_std::types::cosmos::bank::v1beta1::BankQuerier;
use std::str::FromStr;

pub fn get_bank_balance(deps: Deps, address: String, denom: String) -> Uint128 {
    let bank = BankQuerier::new(&deps.querier);

    match bank.balance(address, denom).unwrap().balance {
        Some(balance) => Uint128::from_str(balance.amount.as_str()).unwrap(),
        None => Uint128::zero(),
    }
}

pub fn create_distribute_message_and_update_response(
    mut response: Response,
    fee_collector: String,
    token: String,
    amount: Uint128,
    recipient: String,
) -> StdResult<Response> {
    if !amount.is_zero() {
        let distribute_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: fee_collector,
            msg: to_binary(&FeeExecuteMsg::SendToken {
                token,
                amount,
                recipient,
            })
            .unwrap(),
            funds: vec![],
        });

        response = response.add_message(distribute_msg);
    };

    Ok(response)
}

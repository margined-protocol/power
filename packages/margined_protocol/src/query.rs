use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Decimal, Timestamp};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub struct ExecuteMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Decimal)]
    GetArithmeticTwapToNow {
        pool_id: u64,
        base_asset: String,
        quote_asset: String,
        start_time: Timestamp,
    },
    #[returns(Option<String>)]
    GetDenomAuthority { denom: String },
}

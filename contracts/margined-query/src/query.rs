use cosmwasm_std::{Decimal, Deps, StdResult, Timestamp};
use osmosis_std::{
    shim::Timestamp as OsmosisTimestamp,
    types::osmosis::tokenfactory::v1beta1::{DenomAuthorityMetadata, TokenfactoryQuerier},
    types::osmosis::twap::v1beta1::TwapQuerier,
};
use std::str::FromStr;

pub fn get_arithmetic_twap_now(
    deps: Deps,
    pool_id: u64,
    base_asset: String,
    quote_asset: String,
    start_time: Timestamp,
) -> StdResult<Decimal> {
    let querier = TwapQuerier::new(&deps.querier);

    let start_time = OsmosisTimestamp {
        seconds: start_time.seconds() as i64,
        nanos: start_time.subsec_nanos() as i32,
    };

    let res = querier.arithmetic_twap_to_now(pool_id, base_asset, quote_asset, Some(start_time))?;

    let price = Decimal::from_str(&res.arithmetic_twap).unwrap();

    Ok(price)
}

pub fn get_denom_authority(deps: Deps, denom: String) -> StdResult<Option<String>> {
    let querier = TokenfactoryQuerier::new(&deps.querier);

    let result = querier
        .denom_authority_metadata(denom)
        .unwrap()
        .authority_metadata;

    match result {
        Some(DenomAuthorityMetadata { admin }) => Ok(Some(admin)),
        None => Ok(None),
    }
}

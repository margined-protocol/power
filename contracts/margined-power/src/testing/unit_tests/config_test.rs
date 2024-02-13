use crate::state::Config;

use cosmwasm_std::{Addr, Decimal};
use margined_protocol::power::{Asset, Pool};
use margined_testing::power_env::SCALE_FACTOR;
use std::str::FromStr;

#[test]
fn test_config_validation() {
    // invalid base decimals
    {
        let config = Config {
            fee_rate: Decimal::percent(0),
            fee_pool_contract: Addr::unchecked("fee_pool".to_string()),
            query_contract: Addr::unchecked("query".to_string()),
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 60u32,
            },
            stake_assets: None,
            base_pool: Pool {
                id: 1,
                base_denom: "base_base".to_string(),
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                base_denom: "power_base".to_string(),
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 100,
            index_scale: SCALE_FACTOR as u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
        };

        let err = config.validate().unwrap_err();
        assert_eq!(err.to_string(), "Generic error: Invalid base decimals");
    }

    // invalid power decimals
    {
        let config = Config {
            fee_rate: Decimal::percent(0),
            fee_pool_contract: Addr::unchecked("fee_pool".to_string()),
            query_contract: Addr::unchecked("query".to_string()),
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 19u32,
            },
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            stake_assets: None,
            base_pool: Pool {
                id: 1,
                base_denom: "base_base".to_string(),
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                base_denom: "power_base".to_string(),
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 100,
            index_scale: SCALE_FACTOR as u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
        };

        let err = config.validate().unwrap_err();
        assert_eq!(err.to_string(), "Generic error: Invalid power decimals");
    }

    // invalid funding period
    {
        let config = Config {
            fee_rate: Decimal::percent(0),
            fee_pool_contract: Addr::unchecked("fee_pool".to_string()),
            query_contract: Addr::unchecked("query".to_string()),
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            stake_assets: None,
            base_pool: Pool {
                id: 1,
                base_denom: "base_base".to_string(),
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                base_denom: "power_base".to_string(),
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 0,
            index_scale: SCALE_FACTOR as u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
        };

        let err = config.validate().unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Invalid funding period, must be between 0 and 3024000 seconds"
        );
    }

    // invalid base and power denom
    {
        let config = Config {
            fee_rate: Decimal::percent(0),
            fee_pool_contract: Addr::unchecked("fee_pool".to_string()),
            query_contract: Addr::unchecked("query".to_string()),
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            base_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: None,
            base_pool: Pool {
                id: 1,
                base_denom: "base_base".to_string(),
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                base_denom: "power_base".to_string(),
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 100,
            index_scale: SCALE_FACTOR as u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
        };

        let err = config.validate().unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Invalid base and power denom must be different"
        );
    }

    // invalid base and power id
    {
        let config = Config {
            fee_rate: Decimal::percent(0),
            fee_pool_contract: Addr::unchecked("fee_pool".to_string()),
            query_contract: Addr::unchecked("query".to_string()),
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            stake_assets: None,
            base_pool: Pool {
                id: 1,
                base_denom: "base_base".to_string(),
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 1,
                base_denom: "power_base".to_string(),
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 100,
            index_scale: SCALE_FACTOR as u64,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
        };

        let err = config.validate().unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Invalid base and power pool id must be different"
        );
    }
}

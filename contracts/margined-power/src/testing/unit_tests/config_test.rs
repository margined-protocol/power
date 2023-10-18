use crate::state::Config;

use cosmwasm_std::{Addr, Decimal};
use margined_protocol::power::Pool;

#[test]
fn test_config_validation() {
    // invalid base decimals
    {
        let config = Config {
            fee_rate: Decimal::percent(0),
            fee_pool_contract: Addr::unchecked("fee_pool".to_string()),
            query_contract: Addr::unchecked("query".to_string()),
            power_denom: "power".to_string(),
            base_denom: "base".to_string(),
            base_pool: Pool {
                id: 1,
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 100,
            base_decimals: 60,
            power_decimals: 6,
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
            power_denom: "power".to_string(),
            base_denom: "base".to_string(),
            base_pool: Pool {
                id: 1,
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 100,
            base_decimals: 6,
            power_decimals: 19,
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
            power_denom: "power".to_string(),
            base_denom: "base".to_string(),
            base_pool: Pool {
                id: 1,
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                quote_denom: "power_quote".to_string(),
            },
            funding_period: 0,
            base_decimals: 6,
            power_decimals: 6,
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
            power_denom: "power".to_string(),
            base_denom: "power".to_string(),
            base_pool: Pool {
                id: 1,
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 2,
                quote_denom: "base_quote".to_string(),
            },
            funding_period: 100,
            base_decimals: 6,
            power_decimals: 6,
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
            power_denom: "power".to_string(),
            base_denom: "base".to_string(),
            base_pool: Pool {
                id: 1,
                quote_denom: "base_quote".to_string(),
            },
            power_pool: Pool {
                id: 1,
                quote_denom: "base_power".to_string(),
            },
            funding_period: 100,
            base_decimals: 6,
            power_decimals: 6,
        };

        let err = config.validate().unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Invalid base and power pool id must be different"
        );
    }
}

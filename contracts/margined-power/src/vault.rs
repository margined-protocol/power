use crate::{
    helpers::get_staked_asset_decimals,
    queries::{get_scaled_pool_twap, get_stake_pool_twap},
    state::{Config, CONFIG},
    utils::decimal_to_fixed,
};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    ensure, ensure_eq, Addr, Decimal, Deps, StdError, StdResult, Storage, Timestamp, Uint128,
};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex};
use margined_common::common::TWAP_PERIOD;
use margined_protocol::power::VaultType;

pub const COLLATERAL_RATIO_NUMERATOR: Decimal = Decimal::raw(3_000_000_000_000_000_000u128); // 3
pub const COLLATERAL_RATIO_DENOMINATOR: Decimal = Decimal::raw(2_000_000_000_000_000_000u128); // 2

pub const VAULTS: IndexedMap<u64, Vault, VaultIndexes> = IndexedMap::new("vaults", INDEXES);
pub const VAULTS_COUNTER: Item<u64> = Item::new("vaults_counter");

pub const INDEXES: VaultIndexes<'_> = VaultIndexes {
    owner: MultiIndex::new(vault_operator_idx, "vaults", "vault__owner"),
};

#[cw_serde]
pub struct Vault {
    pub operator: Addr,
    pub collateral: Uint128,
    pub short_amount: Uint128,
    pub vault_type: VaultType,
}

pub struct VaultIndexes<'a> {
    pub owner: MultiIndex<'a, Addr, Vault, u64>,
}

impl<'a> IndexList<Vault> for VaultIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Vault>> + '_> {
        let v: Vec<&dyn Index<Vault>> = vec![&self.owner];
        Box::new(v.into_iter())
    }
}

pub fn vault_operator_idx(_pk: &[u8], d: &Vault) -> Addr {
    d.operator.clone()
}

impl Default for Vault {
    fn default() -> Self {
        Vault {
            operator: Addr::unchecked(""),
            collateral: Uint128::zero(),
            short_amount: Uint128::zero(),
            vault_type: VaultType::Default,
        }
    }
}

impl Vault {
    pub fn new(operator: Addr, vault_type: VaultType) -> Self {
        Vault {
            operator,
            collateral: Uint128::zero(),
            short_amount: Uint128::zero(),
            vault_type,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.collateral.is_zero() && self.short_amount.is_zero()
    }
}

pub fn check_vault(
    deps: Deps,
    config: Config,
    vault_id: u64,
    normalisation_factor: Decimal,
    block_time: Timestamp,
) -> StdResult<(bool, bool, Decimal)> {
    get_vault_status(deps, config, vault_id, normalisation_factor, block_time)
}

pub fn is_vault_safe(
    deps: Deps,
    config: Config,
    vault_id: u64,
    normalisation_factor: Decimal,
    block_time: Timestamp,
) -> StdResult<bool> {
    let (is_safe, _, _) =
        get_vault_status(deps, config, vault_id, normalisation_factor, block_time)?;

    Ok(is_safe)
}

pub fn get_vault_status(
    deps: Deps,
    config: Config,
    vault_id: u64,
    normalisation_factor: Decimal,
    block_time: Timestamp,
) -> StdResult<(bool, bool, Decimal)> {
    let start_time = block_time.minus_seconds(TWAP_PERIOD);

    let quote_price = get_scaled_pool_twap(
        &deps,
        config.base_pool.id,
        config.base_asset.denom.clone(),
        config.base_pool.quote_denom,
        start_time,
    )
    .unwrap();

    let vault = VAULTS
        .may_load(deps.storage, vault_id)
        .map(|opt| opt.unwrap_or_default())?;

    match vault.vault_type {
        VaultType::Default => get_status(&deps, vault_id, normalisation_factor, quote_price, None),
        VaultType::Staked { denom } => {
            let stake_price = get_stake_pool_twap(&deps, denom, start_time)?;

            get_status(
                &deps,
                vault_id,
                normalisation_factor,
                quote_price,
                stake_price,
            )
        }
    }
}

pub fn get_vault_type(storage: &dyn Storage, vault_id: u64) -> StdResult<VaultType> {
    let vault = VAULTS.may_load(storage, vault_id).unwrap();

    match vault {
        Some(vault) => Ok(vault.vault_type),
        None => Err(StdError::generic_err(format!(
            "vault {} does not exist",
            vault_id
        ))),
    }
}

pub fn create_vault(
    storage: &mut dyn Storage,
    operator: Addr,
    vault_type: VaultType,
) -> StdResult<u64> {
    let current_vault_count = VAULTS_COUNTER.load(storage).unwrap_or(0);
    let nonce = current_vault_count + 1;

    let vault = Vault::new(operator, vault_type);

    VAULTS.save(storage, nonce, &vault)?;
    VAULTS_COUNTER.save(storage, &nonce)?;

    Ok(nonce)
}

pub fn remove_vault(storage: &mut dyn Storage, vault_id: u64) -> StdResult<u64> {
    let current_vault_count = VAULTS_COUNTER.load(storage).unwrap_or(0);
    let nonce = current_vault_count - 1;

    let current_vault = VAULTS.load(storage, vault_id).unwrap();

    ensure!(
        current_vault.short_amount.is_zero(),
        StdError::generic_err(format!(
            "cannot remove: vault ({}) has short exposure",
            vault_id
        ))
    );

    ensure!(
        current_vault.collateral.is_zero(),
        StdError::generic_err(format!(
            "cannot remove: vault ({}) has collateral",
            vault_id
        ))
    );

    VAULTS.remove(storage, vault_id)?;
    VAULTS_COUNTER.save(storage, &nonce)?;

    Ok(nonce)
}

pub fn update_vault(
    storage: &mut dyn Storage,
    vault_id: u64,
    operator: Addr,
    vault_type: &VaultType,
    collateral: Uint128,
    short_amount: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, vault_id).unwrap();

    ensure_eq!(
        operator,
        current_vault.operator,
        StdError::generic_err("operator does not match")
    );

    ensure_eq!(
        vault_type,
        &current_vault.vault_type,
        StdError::generic_err("vault_type does not match")
    );

    let vault = Vault {
        operator,
        collateral: current_vault.collateral + collateral,
        short_amount: current_vault.short_amount + short_amount,
        vault_type: current_vault.vault_type,
    };

    VAULTS.save(storage, vault_id, &vault)?;

    Ok(())
}

pub fn check_can_burn(
    storage: &dyn Storage,
    vault_id: u64,
    operator: Addr,
    amount_to_burn: Uint128,
    collateral_to_withdraw: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, vault_id).unwrap();

    ensure_eq!(
        operator,
        current_vault.operator,
        StdError::generic_err("operator does not match")
    );

    if collateral_to_withdraw > current_vault.collateral
        || amount_to_burn > current_vault.short_amount
    {
        return Err(StdError::generic_err(
            "Cannot burn more funds or collateral than in vault",
        ));
    }

    Ok(())
}

pub fn burn_vault(
    storage: &mut dyn Storage,
    vault_id: u64,
    operator: Addr,
    collateral: Uint128,
    short_amount: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, vault_id).unwrap();

    ensure_eq!(
        operator,
        current_vault.operator,
        StdError::generic_err("operator does not match")
    );

    if collateral > current_vault.collateral || short_amount > current_vault.short_amount {
        return Err(StdError::generic_err(
            "Cannot burn more funds or collateral than in vault",
        ));
    }

    let vault = Vault {
        operator,
        collateral: current_vault.collateral - collateral,
        short_amount: current_vault.short_amount - short_amount,
        vault_type: current_vault.vault_type,
    };

    VAULTS.save(storage, vault_id, &vault)?;

    Ok(())
}

pub fn add_collateral(
    storage: &mut dyn Storage,
    vault_id: u64,
    operator: Addr,
    collateral: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, vault_id).unwrap();

    ensure_eq!(
        operator,
        current_vault.operator,
        StdError::generic_err("operator does not match")
    );

    let vault = Vault {
        operator,
        collateral: current_vault.collateral + collateral,
        short_amount: current_vault.short_amount,
        vault_type: current_vault.vault_type,
    };

    VAULTS.save(storage, vault_id, &vault)?;

    Ok(())
}

pub fn subtract_collateral(
    storage: &mut dyn Storage,
    vault_id: u64,
    operator: Addr,
    collateral: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, vault_id).unwrap();

    ensure_eq!(
        operator,
        current_vault.operator,
        StdError::generic_err("operator does not match")
    );

    if collateral > current_vault.collateral {
        return Err(StdError::generic_err(
            "Cannot subtract more collateral than deposited",
        ));
    }

    let vault = Vault {
        operator,
        collateral: current_vault.collateral - collateral,
        short_amount: current_vault.short_amount,
        vault_type: current_vault.vault_type,
    };

    VAULTS.save(storage, vault_id, &vault)?;

    Ok(())
}

pub fn get_status(
    deps: &Deps,
    vault_id: u64,
    normalisation_factor: Decimal,
    quote_price: Decimal,
    stake_price: Option<Decimal>,
) -> StdResult<(bool, bool, Decimal)> {
    let config = CONFIG.load(deps.storage)?;

    let vault = VAULTS.may_load(deps.storage, vault_id)?;
    if vault.is_none() {
        return Ok((true, false, Decimal::zero()));
    }

    let vault = vault.unwrap();

    Ok(calculate_status(
        config,
        vault,
        normalisation_factor,
        quote_price,
        stake_price,
    ))
}

fn calculate_status(
    config: Config,
    vault: Vault,
    normalisation_factor: Decimal,
    quote_price: Decimal,
    stake_price: Option<Decimal>,
) -> (bool, bool, Decimal) {
    let decimal_short_amount = Decimal::from_ratio(
        vault.short_amount,
        Uint128::from(10u128.pow(config.power_asset.decimals)),
    );

    let debt_value = decimal_short_amount
        .checked_mul(normalisation_factor)
        .unwrap()
        .checked_mul(quote_price)
        .unwrap();

    let decimal_collateral = match vault.vault_type {
        VaultType::Default => Decimal::from_ratio(
            vault.collateral,
            Uint128::from(10u128.pow(config.base_asset.decimals)),
        ),
        VaultType::Staked { denom } => {
            let stake_decimals = get_staked_asset_decimals(denom, &config).unwrap_or(6u32);
            let stake_asset_collateral =
                Decimal::from_ratio(vault.collateral, Uint128::from(10u128.pow(stake_decimals)));

            let stake_price = stake_price.unwrap_or(Decimal::zero());

            stake_asset_collateral.checked_mul(stake_price).unwrap()
        }
    };

    let adjusted_collateral = decimal_collateral
        .checked_mul(COLLATERAL_RATIO_DENOMINATOR)
        .unwrap();
    let adjusted_debt = debt_value.checked_mul(COLLATERAL_RATIO_NUMERATOR).unwrap();

    // Return to fixed point to remove rounding errors
    let adjusted_collateral = decimal_to_fixed(adjusted_collateral, config.base_asset.decimals);
    let adjusted_debt = decimal_to_fixed(adjusted_debt, config.base_asset.decimals);

    let min_collateral = decimal_to_fixed(config.min_collateral_amount, config.base_asset.decimals);

    let above_min_collateral = min_collateral <= vault.collateral;
    let is_solvent = adjusted_collateral >= adjusted_debt;

    let collateral_ratio = if debt_value.is_zero() || !above_min_collateral {
        Decimal::zero()
    } else {
        decimal_collateral / debt_value
    };

    (is_solvent, above_min_collateral, collateral_ratio)
}

#[cfg(test)]
mod tests {
    use crate::{
        state::Config,
        vault::{calculate_status, Vault},
    };

    use cosmwasm_std::{Addr, Decimal};
    use margined_protocol::power::{Asset, Pool, StakeAsset, VaultType};
    use std::str::FromStr;

    const INDEX_SCALE_FACTOR: Decimal = Decimal::raw(10_000_000_000_000_000_000_000u128); // 10,000.0

    #[test]
    fn test_calculate_status() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 3000.0
        let scaled_quote_price =
            Decimal::from_atomics(3_000_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(solvent);
        assert!(above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::from_str("1.5").unwrap());
    }

    #[test]
    fn test_calculate_status_staked_asset() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: Some(vec![StakeAsset {
                denom: "stbase".to_string(),
                decimals: 6u32,
                pool: Pool {
                    id: 1,
                    base_denom: "stbase".to_string(),
                    quote_denom: "quote".to_string(),
                },
            }]),
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 40_000_000u128.into(),    // 40.0
            short_amount: 100_000_000u128.into(), // 100.0
            vault_type: VaultType::Staked {
                denom: "stbase".to_string(),
            },
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 3000.0
        let scaled_quote_price =
            Decimal::from_atomics(3_000_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let stake_price = Decimal::from_atomics(1_125_000u128, config.base_asset.decimals).unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            Some(stake_price),
        );

        assert!(solvent);
        assert!(above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::from_str("1.5").unwrap());
    }

    #[test]
    fn test_calculate_status_staked_asset_depegs() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: Some(vec![StakeAsset {
                denom: "stbase".to_string(),
                decimals: 6u32,
                pool: Pool {
                    id: 1,
                    base_denom: "stbase".to_string(),
                    quote_denom: "quote".to_string(),
                },
            }]),
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 40_500_000u128.into(),    // 40.5
            short_amount: 100_000_000u128.into(), // 100.0
            vault_type: VaultType::Staked {
                denom: "stbase".to_string(),
            },
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 3000.0
        let scaled_quote_price =
            Decimal::from_atomics(3_000_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let stake_price = Decimal::from_atomics(562_500u128, config.base_asset.decimals).unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            Some(stake_price),
        );

        assert!(!solvent);
        assert!(above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::from_str("0.759375").unwrap());
    }

    #[test]
    fn test_calculate_status_price_doubles() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: None,
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 6000.0
        let scaled_quote_price =
            Decimal::from_atomics(6_000_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(!solvent);
        assert!(above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::from_str("0.75").unwrap());
    }

    #[test]
    fn test_calculate_status_price_halves() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 1500.0
        let scaled_quote_price =
            Decimal::from_atomics(1_500_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(solvent);
        assert!(above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::from_str("3").unwrap());
    }

    #[test]
    fn test_calculate_status_normalization_factor_makes_vault_solvent() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: Some(vec![StakeAsset {
                denom: "stbase".to_string(),
                decimals: 6u32,
                pool: Pool {
                    id: 1,
                    base_denom: "stbase".to_string(),
                    quote_denom: "quote".to_string(),
                },
            }]),
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };
        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(750_000u128, 6u32).unwrap();

        // price is 4000.0
        let scaled_quote_price =
            Decimal::from_atomics(4_000_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(solvent);
        assert!(above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::from_str("1.5").unwrap());
    }

    #[test]
    fn test_calculate_status_vault_below_min_collateral() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: Some(vec![StakeAsset {
                denom: "stbase".to_string(),
                decimals: 6u32,
                pool: Pool {
                    id: 1,
                    base_denom: "stbase".to_string(),
                    quote_denom: "quote".to_string(),
                },
            }]),
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };
        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 450_000u128.into(),     // 0.45
            short_amount: 1_000_000u128.into(), // 1.0
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(750_000u128, 6u32).unwrap();

        // price is 4000.0
        let scaled_quote_price =
            Decimal::from_atomics(4_000_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(solvent);
        assert!(!above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::zero());
    }

    #[test]
    fn test_calculate_status_vault_below_min_collateral_and_insolvent() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: Some(vec![StakeAsset {
                denom: "stbase".to_string(),
                decimals: 6u32,
                pool: Pool {
                    id: 1,
                    base_denom: "stbase".to_string(),
                    quote_denom: "quote".to_string(),
                },
            }]),
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };
        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 450_000u128.into(),     // 0.45
            short_amount: 1_000_000u128.into(), // 1.0
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(750_000u128, 6u32).unwrap();

        // price is 6000.0
        let scaled_quote_price =
            Decimal::from_atomics(6_000_000_000u128, config.base_asset.decimals)
                .unwrap()
                .checked_div(INDEX_SCALE_FACTOR)
                .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(!solvent);
        assert!(!above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::zero());
    }

    #[test]
    fn test_calculate_status_worked_example() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: Some(vec![StakeAsset {
                denom: "stbase".to_string(),
                decimals: 6u32,
                pool: Pool {
                    id: 1,
                    base_denom: "stbase".to_string(),
                    quote_denom: "quote".to_string(),
                },
            }]),
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };
        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 20_000_000u128.into(),    // 20.0
            short_amount: 102_561_000u128.into(), // 10.0
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 975.0
        let scaled_quote_price = Decimal::from_atomics(975_000_000u128, config.base_asset.decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(solvent);
        assert!(above_min_collateral);
        assert_eq!(
            collateral_ratio,
            Decimal::from_str("2.000060501830180362").unwrap()
        );
    }

    #[test]
    fn test_calculate_status_zero_short() {
        let config = Config {
            base_asset: Asset {
                denom: "base".to_string(),
                decimals: 6u32,
            },
            power_asset: Asset {
                denom: "power".to_string(),
                decimals: 6u32,
            },
            stake_assets: Some(vec![StakeAsset {
                denom: "stbase".to_string(),
                decimals: 6u32,
                pool: Pool {
                    id: 1,
                    base_denom: "stbase".to_string(),
                    quote_denom: "quote".to_string(),
                },
            }]),
            min_collateral_amount: Decimal::from_str("0.5").unwrap(),
            ..Default::default()
        };
        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 20_000_000u128.into(), // 20.0
            short_amount: 0u128.into(),
            vault_type: VaultType::Default,
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 975.0
        let scaled_quote_price = Decimal::from_atomics(975_000_000u128, config.base_asset.decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral, collateral_ratio) = calculate_status(
            config,
            vault,
            normalization_factor,
            scaled_quote_price,
            None,
        );

        assert!(solvent);
        assert!(above_min_collateral);
        assert_eq!(collateral_ratio, Decimal::zero());
    }
}

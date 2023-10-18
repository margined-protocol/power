use crate::{
    helpers::decimal_to_fixed,
    queries::get_scaled_pool_twap,
    state::{Config, CONFIG, TWAP_PERIOD},
};

use cosmwasm_std::{
    ensure_eq, Addr, Decimal, Deps, StdError, StdResult, Storage, Timestamp, Uint128,
};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const COLLATERAL_RATIO_NUMERATOR: Decimal = Decimal::raw(3_000_000_000_000_000_000u128); // 3
pub const COLLATERAL_RATIO_DENOMINATOR: Decimal = Decimal::raw(2_000_000_000_000_000_000u128); // 2
pub const MIN_COLLATERAL: Decimal = Decimal::raw(500_000_000_000_000_000u128); // 0.5

pub const VAULTS: IndexedMap<&u64, Vault, VaultIndexes> = IndexedMap::new("vaults", INDEXES);
pub const VAULTS_COUNTER: Item<u64> = Item::new("vaults_counter");

pub const INDEXES: VaultIndexes<'_> = VaultIndexes {
    owner: MultiIndex::new(vault_operator_idx, "vaults", "vault__owner"),
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Vault {
    pub operator: Addr,
    pub collateral: Uint128,
    pub short_amount: Uint128,
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

impl Vault {
    pub fn new(operator: Addr) -> Self {
        Vault {
            operator,
            collateral: Uint128::zero(),
            short_amount: Uint128::zero(),
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
) -> StdResult<(bool, bool)> {
    get_vault_status(deps, config, vault_id, normalisation_factor, block_time)
}

pub fn is_vault_safe(
    deps: Deps,
    config: Config,
    vault_id: u64,
    normalisation_factor: Decimal,
    block_time: Timestamp,
) -> StdResult<bool> {
    let (is_safe, _) = get_vault_status(deps, config, vault_id, normalisation_factor, block_time)?;

    Ok(is_safe)
}

pub fn get_vault_status(
    deps: Deps,
    config: Config,
    vault_id: u64,
    normalisation_factor: Decimal,
    block_time: Timestamp,
) -> StdResult<(bool, bool)> {
    let start_time = block_time.minus_seconds(TWAP_PERIOD);

    let quote_price = get_scaled_pool_twap(
        &deps,
        config.base_pool.id,
        config.base_denom.clone(),
        config.base_pool.quote_denom,
        start_time,
    )
    .unwrap();

    get_status(&deps, vault_id, normalisation_factor, quote_price)
}

pub fn create_vault(storage: &mut dyn Storage, operator: Addr) -> StdResult<u64> {
    let current_vault_count = VAULTS_COUNTER.load(storage).unwrap_or(0);
    let nonce = current_vault_count + 1;

    let vault = Vault::new(operator);

    VAULTS.save(storage, &nonce, &vault)?;
    VAULTS_COUNTER.save(storage, &nonce)?;

    Ok(nonce)
}

pub fn update_vault(
    storage: &mut dyn Storage,
    vault_id: u64,
    operator: Addr,
    collateral: Uint128,
    short_amount: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, &vault_id).unwrap();

    ensure_eq!(
        operator,
        current_vault.operator,
        StdError::generic_err("operator does not match")
    );

    let vault = Vault {
        operator,
        collateral: current_vault.collateral + collateral,
        short_amount: current_vault.short_amount + short_amount,
    };

    VAULTS.save(storage, &vault_id, &vault)?;

    Ok(())
}

pub fn check_can_burn(
    storage: &dyn Storage,
    vault_id: u64,
    operator: Addr,
    amount_to_burn: Uint128,
    collateral_to_withdraw: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, &vault_id).unwrap();

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
    let current_vault = VAULTS.load(storage, &vault_id).unwrap();

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
    };

    VAULTS.save(storage, &vault_id, &vault)?;

    Ok(())
}

pub fn add_collateral(
    storage: &mut dyn Storage,
    vault_id: u64,
    operator: Addr,
    collateral: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, &vault_id).unwrap();

    ensure_eq!(
        operator,
        current_vault.operator,
        StdError::generic_err("operator does not match")
    );

    let vault = Vault {
        operator,
        collateral: current_vault.collateral + collateral,
        short_amount: current_vault.short_amount,
    };

    VAULTS.save(storage, &vault_id, &vault)?;

    Ok(())
}

pub fn subtract_collateral(
    storage: &mut dyn Storage,
    vault_id: u64,
    operator: Addr,
    collateral: Uint128,
) -> StdResult<()> {
    let current_vault = VAULTS.load(storage, &vault_id).unwrap();

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
    };

    VAULTS.save(storage, &vault_id, &vault)?;

    Ok(())
}

pub fn get_status(
    deps: &Deps,
    vault_id: u64,
    normalisation_factor: Decimal,
    quote_price: Decimal,
) -> StdResult<(bool, bool)> {
    let config = CONFIG.load(deps.storage)?;

    let vault = VAULTS.may_load(deps.storage, &vault_id)?;
    if vault.is_none() {
        return Ok((true, false));
    }

    let vault = vault.unwrap();

    Ok(calculate_status(
        config.base_decimals,
        config.power_decimals,
        vault,
        normalisation_factor,
        quote_price,
    ))
}

fn calculate_status(
    base_decimals: u32,
    power_decimals: u32,
    vault: Vault,
    normalisation_factor: Decimal,
    quote_price: Decimal,
) -> (bool, bool) {
    let decimal_short_amount = Decimal::from_ratio(
        vault.short_amount,
        Uint128::from(10u128.pow(power_decimals)),
    );

    let debt_value = decimal_short_amount
        .checked_mul(normalisation_factor)
        .unwrap()
        .checked_mul(quote_price)
        .unwrap();

    let decimal_collateral =
        Decimal::from_ratio(vault.collateral, Uint128::from(10u128.pow(base_decimals)));

    let adjusted_collateral = decimal_collateral
        .checked_mul(COLLATERAL_RATIO_DENOMINATOR)
        .unwrap();
    let adjusted_debt = debt_value.checked_mul(COLLATERAL_RATIO_NUMERATOR).unwrap();

    // Return to fixed point to remove rounding errors
    let adjusted_collateral = decimal_to_fixed(adjusted_collateral, base_decimals);
    let adjusted_debt = decimal_to_fixed(adjusted_debt, base_decimals);

    let min_collateral = decimal_to_fixed(MIN_COLLATERAL, base_decimals);

    let above_min_collateral = min_collateral <= vault.collateral;
    let is_solvent = adjusted_collateral >= adjusted_debt;

    (is_solvent, above_min_collateral)
}

#[cfg(test)]
mod tests {
    use crate::vault::{calculate_status, Vault};

    use cosmwasm_std::{Addr, Decimal};

    const INDEX_SCALE_FACTOR: Decimal = Decimal::raw(10_000_000_000_000_000_000_000u128); // 10,000.0

    #[test]
    fn test_calculate_status() {
        let base_decimals = 6u32;
        let power_decimals = 6u32;

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 3000.0
        let scaled_quote_price = Decimal::from_atomics(3_000_000_000u128, base_decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral) = calculate_status(
            base_decimals,
            power_decimals,
            vault,
            normalization_factor,
            scaled_quote_price,
        );

        assert!(solvent);
        assert!(above_min_collateral);
    }

    #[test]
    fn test_calculate_status_price_doubles() {
        let base_decimals = 6u32;
        let power_decimals = 6u32;

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 6000.0
        let scaled_quote_price = Decimal::from_atomics(6_000_000_000u128, base_decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral) = calculate_status(
            base_decimals,
            power_decimals,
            vault,
            normalization_factor,
            scaled_quote_price,
        );

        assert!(!solvent);
        assert!(above_min_collateral);
    }

    #[test]
    fn test_calculate_status_price_halves() {
        let base_decimals = 6u32;
        let power_decimals = 6u32;

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 1500.0
        let scaled_quote_price = Decimal::from_atomics(1_500_000_000u128, base_decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral) = calculate_status(
            base_decimals,
            power_decimals,
            vault,
            normalization_factor,
            scaled_quote_price,
        );

        assert!(solvent);
        assert!(above_min_collateral);
    }

    #[test]
    fn test_calculate_status_normalization_factor_makes_vault_solvent() {
        let base_decimals = 6u32;
        let power_decimals = 6u32;

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 45_000_000u128.into(),    // 45.0
            short_amount: 100_000_000u128.into(), // 100.0
        };

        let normalization_factor = Decimal::from_atomics(750_000u128, 6u32).unwrap();

        // price is 4000.0
        let scaled_quote_price = Decimal::from_atomics(4_000_000_000u128, base_decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral) = calculate_status(
            base_decimals,
            power_decimals,
            vault,
            normalization_factor,
            scaled_quote_price,
        );

        assert!(solvent);
        assert!(above_min_collateral);
    }

    #[test]
    fn test_calculate_status_vault_below_min_collateral() {
        let base_decimals = 6u32;
        let power_decimals = 6u32;

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 450_000u128.into(),     // 0.45
            short_amount: 1_000_000u128.into(), // 1.0
        };

        let normalization_factor = Decimal::from_atomics(750_000u128, 6u32).unwrap();

        // price is 4000.0
        let scaled_quote_price = Decimal::from_atomics(4_000_000_000u128, base_decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral) = calculate_status(
            base_decimals,
            power_decimals,
            vault,
            normalization_factor,
            scaled_quote_price,
        );

        assert!(solvent);
        assert!(!above_min_collateral);
    }

    #[test]
    fn test_calculate_status_vault_below_min_collateral_and_insolvent() {
        let base_decimals = 6u32;
        let power_decimals = 6u32;

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 450_000u128.into(),     // 0.45
            short_amount: 1_000_000u128.into(), // 1.0
        };

        let normalization_factor = Decimal::from_atomics(750_000u128, 6u32).unwrap();

        // price is 6000.0
        let scaled_quote_price = Decimal::from_atomics(6_000_000_000u128, base_decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral) = calculate_status(
            base_decimals,
            power_decimals,
            vault,
            normalization_factor,
            scaled_quote_price,
        );

        assert!(!solvent);
        assert!(!above_min_collateral);
    }

    #[test]
    fn test_calculate_status_worked_example() {
        let base_decimals = 6u32;
        let power_decimals = 6u32;

        let vault = Vault {
            operator: Addr::unchecked(""),
            collateral: 20_000_000u128.into(),    // 20.0
            short_amount: 102_561_000u128.into(), // 10.0
        };

        let normalization_factor = Decimal::from_atomics(1u128, 0u32).unwrap();

        // price is 975.0
        let scaled_quote_price = Decimal::from_atomics(975_000_000u128, base_decimals)
            .unwrap()
            .checked_div(INDEX_SCALE_FACTOR)
            .unwrap();

        let (solvent, above_min_collateral) = calculate_status(
            base_decimals,
            power_decimals,
            vault,
            normalization_factor,
            scaled_quote_price,
        );

        assert!(solvent);
        assert!(above_min_collateral);
    }
}

use crate::utils::decimal_to_fixed;

use cosmwasm_std::{Decimal, Uint128};

#[test]
fn test_decimal_to_fixed() {
    let input = Decimal::from_atomics(1_234_567u128, 6).unwrap();
    let expected_result = Uint128::new(1_234_567u128);

    let result = decimal_to_fixed(input, 6);
    assert_eq!(result, expected_result);
}

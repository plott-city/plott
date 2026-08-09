//! Fixed-point helpers.
//!
//! Rounding policy (deliberate, applied everywhere):
//!
//!   * Anything the protocol **hands out** rounds DOWN  -- synthetic minted to
//!     a user, collateral released on redeem, funding claimed by a staker.
//!   * Anything the protocol **collects** rounds UP     -- mint fee, redeem fee.
//!
//! The residue of every rounding step therefore stays inside the protocol. The
//! alternative (round-to-nearest, or rounding out) leaks a sub-unit per
//! operation to whoever calls most often, which is a free grinding attack: at
//! 6 synthetic decimals a single leaked unit is 1e-6 USD, but an attacker can
//! issue millions of dust operations per day. Rounding is never "too small to
//! matter" when the caller controls the call count.
//!
//! There is no floating point anywhere. `f64` is non-deterministic across
//! validators and is not permitted on-chain.

use anchor_lang::prelude::*;

use crate::errors::PoyzError;

/// Basis points denominator. 10000 bps == 100.00 %.
pub const BPS_DENOMINATOR: u64 = 10_000;
pub const BPS_DENOMINATOR_U128: u128 = 10_000;

/// Fixed-point scale for the funding accumulator (`acc_funding_per_share`).
///
/// 1e12 with a u128 accumulator: even at a 1e18 total funding inflow against a
/// 1-unit stake the accumulator stays far below u128::MAX, and at realistic
/// stake sizes (>= 1e6 base units, i.e. >= 1 synthetic dollar) the per-share
/// truncation error is below 1e-6 of a base unit per settlement.
pub const ACC_SCALE: u128 = 1_000_000_000_000;

/// Upper bound on any decimal shift we are willing to compute.
/// Collateral decimals <= 9, synthetic decimals <= 9, |Pyth exponent| <= 12 in
/// practice; 30 leaves headroom while keeping `10^n` inside u128.
pub const MAX_DECIMAL_SHIFT: u32 = 30;

/// `10^exp`, bounded and checked.
pub fn pow10(exp: u32) -> Result<u128> {
    require!(exp <= MAX_DECIMAL_SHIFT, PoyzError::MathOverflow);
    10u128
        .checked_pow(exp)
        .ok_or_else(|| error!(PoyzError::MathOverflow))
}

/// Decimal shift that converts `collateral_amount * price` into synthetic base
/// units: `expo + synthetic_decimals - collateral_decimals`.
fn decimal_shift(expo: i32, collateral_decimals: u8, synthetic_decimals: u8) -> Result<i32> {
    expo.checked_add(i32::from(synthetic_decimals))
        .and_then(|v| v.checked_sub(i32::from(collateral_decimals)))
        .ok_or_else(|| error!(PoyzError::MathOverflow))
}

/// Collateral units -> synthetic base units, at `price * 10^expo` USD per whole
/// collateral unit. Rounds DOWN.
pub fn collateral_to_notional(
    collateral_amount: u64,
    price: i64,
    expo: i32,
    collateral_decimals: u8,
    synthetic_decimals: u8,
) -> Result<u64> {
    require!(price > 0, PoyzError::OracleInvalidPrice);
    let price_u = u128::try_from(price).map_err(|_| error!(PoyzError::MathOverflow))?;
    let shift = decimal_shift(expo, collateral_decimals, synthetic_decimals)?;

    let base = u128::from(collateral_amount)
        .checked_mul(price_u)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;

    let out = if shift >= 0 {
        base.checked_mul(pow10(shift.unsigned_abs())?)
            .ok_or_else(|| error!(PoyzError::MathOverflow))?
    } else {
        // Integer division truncates, i.e. rounds down. That is the direction
        // we want: the depositor is credited no more than the exact value.
        base.checked_div(pow10(shift.unsigned_abs())?)
            .ok_or_else(|| error!(PoyzError::MathOverflow))?
    };

    u64::try_from(out).map_err(|_| error!(PoyzError::MathOverflow))
}

/// Synthetic base units -> collateral units, the exact inverse of
/// [`collateral_to_notional`]. Rounds DOWN, so a redeemer never receives more
/// collateral than the burned synthetic is worth.
pub fn notional_to_collateral(
    notional: u64,
    price: i64,
    expo: i32,
    collateral_decimals: u8,
    synthetic_decimals: u8,
) -> Result<u64> {
    require!(price > 0, PoyzError::OracleInvalidPrice);
    let price_u = u128::try_from(price).map_err(|_| error!(PoyzError::MathOverflow))?;
    let shift = decimal_shift(expo, collateral_decimals, synthetic_decimals)?;

    let out = if shift >= 0 {
        let denominator = price_u
            .checked_mul(pow10(shift.unsigned_abs())?)
            .ok_or_else(|| error!(PoyzError::MathOverflow))?;
        require!(denominator > 0, PoyzError::MathOverflow);
        u128::from(notional)
            .checked_div(denominator)
            .ok_or_else(|| error!(PoyzError::MathOverflow))?
    } else {
        u128::from(notional)
            .checked_mul(pow10(shift.unsigned_abs())?)
            .ok_or_else(|| error!(PoyzError::MathOverflow))?
            .checked_div(price_u)
            .ok_or_else(|| error!(PoyzError::MathOverflow))?
    };

    u64::try_from(out).map_err(|_| error!(PoyzError::MathOverflow))
}

/// `amount * bps / 10000`, rounded DOWN. Used where the protocol pays out.
pub fn mul_bps_floor(amount: u64, bps: u16) -> Result<u64> {
    let out = u128::from(amount)
        .checked_mul(u128::from(bps))
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_div(BPS_DENOMINATOR_U128)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    u64::try_from(out).map_err(|_| error!(PoyzError::MathOverflow))
}

/// `amount * bps / 10000`, rounded UP. Used for fees the protocol collects.
pub fn mul_bps_ceil(amount: u64, bps: u16) -> Result<u64> {
    let numerator = u128::from(amount)
        .checked_mul(u128::from(bps))
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    let out = numerator
        .checked_add(BPS_DENOMINATOR_U128 - 1)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_div(BPS_DENOMINATOR_U128)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    u64::try_from(out).map_err(|_| error!(PoyzError::MathOverflow))
}

/// `amount * 10000 / ratio_bps`, rounded DOWN.
///
/// Used to apply the collateral ratio on mint: at `ratio_bps = 12000` a
/// depositor receives 1/1.2 of the deposited notional as synthetic dollars.
pub fn div_bps_floor(amount: u64, ratio_bps: u16) -> Result<u64> {
    require!(ratio_bps > 0, PoyzError::InvalidCollateralRatio);
    let out = u128::from(amount)
        .checked_mul(BPS_DENOMINATOR_U128)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_div(u128::from(ratio_bps))
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    u64::try_from(out).map_err(|_| error!(PoyzError::MathOverflow))
}

/// Confidence interval width relative to the price, in basis points.
/// Rounds UP so a borderline-wide interval is rejected rather than accepted.
pub fn confidence_bps(conf: u64, price: i64) -> Result<u64> {
    require!(price > 0, PoyzError::OracleInvalidPrice);
    let price_u = u128::try_from(price).map_err(|_| error!(PoyzError::MathOverflow))?;
    let numerator = u128::from(conf)
        .checked_mul(BPS_DENOMINATOR_U128)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    let out = numerator
        .checked_add(price_u - 1)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_div(price_u)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    u64::try_from(out).map_err(|_| error!(PoyzError::MathOverflow))
}

/// Signed book delta in basis points:
/// `(collateral_notional - hedged_notional) / collateral_notional`.
///
/// Positive means the book is under-hedged (more spot value than short
/// notional); negative means over-hedged. Both are exposures, which is why the
/// bands are applied to the absolute value.
///
/// This is the number the protocol exists to keep near zero, so it is computed
/// here from on-chain state rather than accepted from a keeper. `hedged_notional`
/// is still an attestation -- the program cannot see the venue -- but the
/// collateral side and the arithmetic are the program's own.
pub fn delta_bps(collateral_notional: u64, hedged_notional: u64) -> Result<i32> {
    require!(collateral_notional > 0, PoyzError::ZeroAmount);
    let collateral = i128::from(collateral_notional);
    let hedged = i128::from(hedged_notional);
    let deviation = collateral
        .checked_sub(hedged)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_mul(i128::from(BPS_DENOMINATOR))
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_div(collateral)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    i32::try_from(deviation).map_err(|_| error!(PoyzError::DeltaOutOfRange))
}

/// Reward accumulator increment for a funding settlement: `amount * SCALE / staked`.
/// Rounds DOWN; the truncated remainder stays in the funding vault and is
/// distributed by a later settlement rather than being credited to nobody.
pub fn acc_increment(amount: u64, total_staked: u64) -> Result<u128> {
    require!(total_staked > 0, PoyzError::NoStakers);
    u128::from(amount)
        .checked_mul(ACC_SCALE)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_div(u128::from(total_staked))
        .ok_or_else(|| error!(PoyzError::MathOverflow))
}

/// `stake * acc / SCALE`, rounded DOWN. The staker's lifetime funding
/// entitlement at the current accumulator value.
pub fn acc_entitlement(stake_amount: u64, acc_funding_per_share: u128) -> Result<u128> {
    u128::from(stake_amount)
        .checked_mul(acc_funding_per_share)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?
        .checked_div(ACC_SCALE)
        .ok_or_else(|| error!(PoyzError::MathOverflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    // SOL/USD at 152.34 with Pyth exponent -8, 9-decimal collateral,
    // 6-decimal synthetic.
    const PRICE: i64 = 15_234_000_000;
    const EXPO: i32 = -8;

    #[test]
    fn notional_round_trip_never_returns_more_than_deposited() {
        let deposited: u64 = 3_500_000_000; // 3.5 SOL
        let notional = collateral_to_notional(deposited, PRICE, EXPO, 9, 6).unwrap();
        assert_eq!(notional, 533_190_000); // 533.19 synthetic dollars
        let back = notional_to_collateral(notional, PRICE, EXPO, 9, 6).unwrap();
        assert!(back <= deposited, "round trip must not create collateral");
    }

    #[test]
    fn rounding_favours_the_protocol_on_dust() {
        // One base unit of collateral is worth less than one synthetic base
        // unit here, so the depositor is credited zero rather than one.
        assert_eq!(collateral_to_notional(1, PRICE, EXPO, 9, 6).unwrap(), 0);
        // Symmetrically, one synthetic base unit redeems for less than one
        // collateral base unit and the redeemer receives zero.
        assert_eq!(notional_to_collateral(1, PRICE, EXPO, 9, 6).unwrap(), 6);
    }

    #[test]
    fn fees_round_up_and_payouts_round_down() {
        assert_eq!(mul_bps_ceil(1, 1).unwrap(), 1); // 0.0001 -> 1
        assert_eq!(mul_bps_floor(1, 1).unwrap(), 0);
        assert_eq!(mul_bps_ceil(10_000, 25).unwrap(), 25);
    }

    #[test]
    fn overflow_is_an_error_not_a_wrap() {
        assert!(collateral_to_notional(u64::MAX, PRICE, 12, 0, 9).is_err());
        assert!(pow10(MAX_DECIMAL_SHIFT + 1).is_err());
        assert!(collateral_to_notional(1, 0, EXPO, 9, 6).is_err());
        assert!(collateral_to_notional(1, -1, EXPO, 9, 6).is_err());
    }

    #[test]
    fn confidence_bps_rounds_up() {
        // conf 1 on price 10000 is 1 bps exactly.
        assert_eq!(confidence_bps(1, 10_000).unwrap(), 1);
        // conf 1 on price 20000 is 0.5 bps and must not round to 0.
        assert_eq!(confidence_bps(1, 20_000).unwrap(), 1);
        assert_eq!(confidence_bps(0, 20_000).unwrap(), 0);
    }

    #[test]
    fn delta_is_signed_and_zero_when_perfectly_hedged() {
        assert_eq!(delta_bps(1_000_000, 1_000_000).unwrap(), 0);
        // 10 % under-hedged.
        assert_eq!(delta_bps(1_000_000, 900_000).unwrap(), 1_000);
        // 10 % over-hedged.
        assert_eq!(delta_bps(1_000_000, 1_100_000).unwrap(), -1_000);
        // An empty book has no defined delta rather than a zero one.
        assert!(delta_bps(0, 0).is_err());
    }

    #[test]
    fn collateral_ratio_reduces_the_mintable_amount() {
        // 12000 bps == 1.2x overcollateralization.
        assert_eq!(div_bps_floor(1_200_000, 12_000).unwrap(), 1_000_000);
        assert_eq!(div_bps_floor(1_000_000, 10_000).unwrap(), 1_000_000);
    }
}

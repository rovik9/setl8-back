//! Shared types placeholder: types used by more than one service. Integer-only; no wall clock.
//!
//! Only types the design document already fixes live here. Units, scales, integer widths on the
//! wire, identifiers and event/account encodings come from the contract repos through
//! `interfaces/` and are deliberately absent: `TODO(interfaces)`.

use core::fmt;

pub mod chain;

/// A trading sector. Each sector has its own evaluator process, key and DB schema.
///
/// TODO(interfaces): the canonical sector identifiers (and any on-chain encoding) come from the
/// contract repos; this enum only names the sectors the design mentions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sector {
    /// Leveraged trading (`lev`).
    Lev,
    /// Options (later).
    Options,
}

/// Position in a per-account append-only log (the `seq` of the hash chain).
///
/// TODO(interfaces): wire width and starting value are not fixed yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seq(pub u64);

impl Seq {
    /// The next sequence number, or `None` on overflow.
    #[must_use]
    pub fn next(self) -> Option<Seq> {
        self.0.checked_add(1).map(Seq)
    }
}

/// A signed integer quantity (money, price delta, PnL) in some unit fixed elsewhere.
///
/// All arithmetic is checked: overflow yields `None` and is never wrapped or saturated, so a
/// caller must decide explicitly what an overflow means.
///
/// TODO(interfaces): the unit/scale and the on-wire width are protocol details; `i128` is only
/// the widest safe in-memory choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Amount(pub i128);

impl Amount {
    /// Zero.
    pub const ZERO: Amount = Amount(0);

    /// `self + rhs`, or `None` on overflow.
    #[must_use]
    pub fn checked_add(self, rhs: Amount) -> Option<Amount> {
        self.0.checked_add(rhs.0).map(Amount)
    }

    /// `self - rhs`, or `None` on overflow.
    #[must_use]
    pub fn checked_sub(self, rhs: Amount) -> Option<Amount> {
        self.0.checked_sub(rhs.0).map(Amount)
    }

    /// `self * factor`, or `None` on overflow.
    #[must_use]
    pub fn checked_mul(self, factor: i128) -> Option<Amount> {
        self.0.checked_mul(factor).map(Amount)
    }

    /// Integer division truncating toward zero, or `None` if `divisor` is zero or on overflow.
    #[must_use]
    pub fn checked_div(self, divisor: i128) -> Option<Amount> {
        self.0.checked_div(divisor).map(Amount)
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_next_increments_and_stops_at_max() {
        assert_eq!(Seq(0).next(), Some(Seq(1)));
        assert_eq!(Seq(u64::MAX).next(), None);
    }

    #[test]
    fn amount_arithmetic_is_checked() {
        assert_eq!(Amount(2).checked_add(Amount(3)), Some(Amount(5)));
        assert_eq!(Amount(2).checked_sub(Amount(3)), Some(Amount(-1)));
        assert_eq!(Amount(i128::MAX).checked_add(Amount(1)), None);
        assert_eq!(Amount(i128::MIN).checked_sub(Amount(1)), None);
        assert_eq!(Amount(i128::MAX).checked_mul(2), None);
    }

    #[test]
    fn amount_division_truncates_toward_zero_and_rejects_zero() {
        assert_eq!(Amount(7).checked_div(2), Some(Amount(3)));
        assert_eq!(Amount(-7).checked_div(2), Some(Amount(-3)));
        assert_eq!(Amount(1).checked_div(0), None);
        assert_eq!(Amount(i128::MIN).checked_div(-1), None);
    }

    #[test]
    fn default_amount_is_zero() {
        assert_eq!(Amount::default(), Amount::ZERO);
    }
}

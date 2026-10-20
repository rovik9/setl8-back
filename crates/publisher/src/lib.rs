//! Publisher library: batching rules and finality tracking.
//!
//! Limits come from `docs/DESIGN.md`. Encoded sizes and compute costs depend on event and
//! instruction formats that arrive through `interfaces/`; callers pass them in, and this crate
//! never guesses them: `TODO(interfaces)`.

/// At most this many events per batch.
pub const MAX_EVENTS_PER_BATCH: usize = 8;
/// At most this many proofs per batch.
pub const MAX_PROOFS_PER_BATCH: usize = 3;
/// A transaction is at most this many bytes.
pub const MAX_TX_BYTES: usize = 1_232;
/// A transaction uses at most this many compute units.
pub const MAX_COMPUTE_UNITS: u32 = 200_000;

/// Why an item or transaction does not fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitError {
    /// Adding the item would exceed [`MAX_EVENTS_PER_BATCH`].
    TooManyEvents,
    /// Adding the item would exceed [`MAX_PROOFS_PER_BATCH`].
    TooManyProofs,
    /// The built transaction exceeds [`MAX_TX_BYTES`].
    TxTooLarge,
    /// The built transaction exceeds [`MAX_COMPUTE_UNITS`].
    TooManyComputeUnits,
}

/// An event that may carry proofs.
pub trait BatchItem {
    /// Number of proofs this event needs posted with it.
    fn proof_count(&self) -> usize;
}

/// Accumulates events up to the batch limits. Order is preserved.
#[derive(Debug)]
pub struct Batcher<T> {
    items: Vec<T>,
    proofs: usize,
}

impl<T> Default for Batcher<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            proofs: 0,
        }
    }
}

impl<T: BatchItem> Batcher<T> {
    /// An empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item, or hand it back with the reason it does not fit. A rejected item leaves the
    /// batch unchanged, so the caller can flush and retry it in the next batch.
    ///
    /// An item that alone needs more than [`MAX_PROOFS_PER_BATCH`] proofs can never fit and is
    /// rejected even into an empty batch.
    pub fn push(&mut self, item: T) -> Result<(), (T, LimitError)> {
        if self.items.len() >= MAX_EVENTS_PER_BATCH {
            return Err((item, LimitError::TooManyEvents));
        }
        let proofs = self.proofs.saturating_add(item.proof_count());
        if proofs > MAX_PROOFS_PER_BATCH {
            return Err((item, LimitError::TooManyProofs));
        }
        self.proofs = proofs;
        self.items.push(item);
        Ok(())
    }

    /// Number of events held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when no events are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of proofs held.
    #[must_use]
    pub fn proofs(&self) -> usize {
        self.proofs
    }

    /// Take the batch, leaving this batcher empty.
    pub fn take(&mut self) -> Vec<T> {
        self.proofs = 0;
        core::mem::take(&mut self.items)
    }
}

/// Check a built transaction against the size and compute limits.
///
/// # Errors
/// Returns the first limit exceeded.
pub fn check_tx(tx_bytes: usize, compute_units: u32) -> Result<(), LimitError> {
    if tx_bytes > MAX_TX_BYTES {
        return Err(LimitError::TxTooLarge);
    }
    if compute_units > MAX_COMPUTE_UNITS {
        return Err(LimitError::TooManyComputeUnits);
    }
    Ok(())
}

/// Chain commitment level, ordered weakest to strongest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Commitment {
    /// Seen by a node.
    Processed,
    /// Voted on by a supermajority.
    Confirmed,
    /// Irreversible.
    Finalized,
}

impl Commitment {
    /// A batch counts as posted only at finalized commitment.
    #[must_use]
    pub fn is_posted(self) -> bool {
        self == Commitment::Finalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Ev(usize);
    impl BatchItem for Ev {
        fn proof_count(&self) -> usize {
            self.0
        }
    }

    #[test]
    fn accepts_up_to_eight_events() {
        let mut b = Batcher::new();
        for _ in 0..MAX_EVENTS_PER_BATCH {
            assert!(b.push(Ev(0)).is_ok());
        }
        let (back, err) = b.push(Ev(0)).unwrap_err();
        assert_eq!((back, err), (Ev(0), LimitError::TooManyEvents));
        assert_eq!(b.len(), 8);
    }

    #[test]
    fn accepts_up_to_three_proofs() {
        let mut b = Batcher::new();
        assert!(b.push(Ev(2)).is_ok());
        assert!(b.push(Ev(1)).is_ok());
        let (_, err) = b.push(Ev(1)).unwrap_err();
        assert_eq!(err, LimitError::TooManyProofs);
        assert_eq!(b.proofs(), 3);
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn oversized_single_item_is_rejected_even_when_empty() {
        let mut b = Batcher::new();
        let (_, err) = b.push(Ev(4)).unwrap_err();
        assert_eq!(err, LimitError::TooManyProofs);
        assert!(b.is_empty());
        assert!(b.push(Ev(usize::MAX)).is_err());
    }

    #[test]
    fn take_resets_and_preserves_order() {
        let mut b = Batcher::new();
        b.push(Ev(1)).unwrap();
        b.push(Ev(0)).unwrap();
        assert_eq!(b.take(), vec![Ev(1), Ev(0)]);
        assert!(b.is_empty());
        assert_eq!(b.proofs(), 0);
        assert!(b.push(Ev(3)).is_ok());
    }

    #[test]
    fn tx_limits_are_inclusive() {
        assert_eq!(check_tx(1_232, 200_000), Ok(()));
        assert_eq!(check_tx(1_233, 0), Err(LimitError::TxTooLarge));
        assert_eq!(check_tx(0, 200_001), Err(LimitError::TooManyComputeUnits));
    }

    #[test]
    fn only_finalized_is_posted() {
        assert!(!Commitment::Processed.is_posted());
        assert!(!Commitment::Confirmed.is_posted());
        assert!(Commitment::Finalized.is_posted());
        assert!(Commitment::Processed < Commitment::Finalized);
    }
}

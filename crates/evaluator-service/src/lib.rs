//! Evaluator-service library: shard leases with fencing epochs.
//!
//! Pure state machine. Time is an input (`now`, in caller-defined ticks); nothing here reads a
//! clock. Persisting the lease (database row, bus) is the caller's concern.

/// Identifies a process competing for a lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HolderId(pub u64);

/// Fencing epoch: strictly increases every time the lease changes hands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Epoch(pub u64);

/// Why a lease operation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseError {
    /// Another holder has an unexpired lease.
    HeldByOther,
    /// The caller does not hold the lease (never did, or it lapsed or was taken over).
    NotHolder,
    /// The write carries an epoch older than the current one (a stale holder).
    StaleEpoch,
    /// The epoch counter would overflow.
    EpochOverflow,
    /// `now + ttl` would overflow.
    TimeOverflow,
}

#[derive(Clone, Copy, Debug)]
struct Held {
    holder: HolderId,
    expires_at: u64,
}

/// The lease for one shard.
#[derive(Debug, Default)]
pub struct ShardLease {
    epoch: Epoch,
    held: Option<Held>,
}

impl ShardLease {
    /// A shard nobody has held yet (epoch 0).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current fencing epoch.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// The holder at `now`, if the lease has not lapsed. A lease is live while `now < expires_at`.
    #[must_use]
    pub fn holder(&self, now: u64) -> Option<HolderId> {
        self.held.filter(|h| now < h.expires_at).map(|h| h.holder)
    }

    /// Acquire the lease for `ttl` ticks. A live lease held by the same holder is extended
    /// without changing the epoch; taking over from another holder (lapsed lease) or from
    /// nobody bumps the epoch.
    ///
    /// # Errors
    /// [`LeaseError::HeldByOther`] while another holder's lease is live; overflow errors.
    pub fn acquire(&mut self, who: HolderId, now: u64, ttl: u64) -> Result<Epoch, LeaseError> {
        let expires_at = now.checked_add(ttl).ok_or(LeaseError::TimeOverflow)?;
        match self.holder(now) {
            Some(h) if h == who => {}
            Some(_) => return Err(LeaseError::HeldByOther),
            None => {
                self.epoch = Epoch(
                    self.epoch
                        .0
                        .checked_add(1)
                        .ok_or(LeaseError::EpochOverflow)?,
                );
            }
        }
        self.held = Some(Held {
            holder: who,
            expires_at,
        });
        Ok(self.epoch)
    }

    /// Extend a live lease the caller holds.
    ///
    /// # Errors
    /// [`LeaseError::NotHolder`] if the caller does not hold a live lease.
    pub fn renew(&mut self, who: HolderId, now: u64, ttl: u64) -> Result<(), LeaseError> {
        if self.holder(now) != Some(who) {
            return Err(LeaseError::NotHolder);
        }
        let expires_at = now.checked_add(ttl).ok_or(LeaseError::TimeOverflow)?;
        self.held = Some(Held {
            holder: who,
            expires_at,
        });
        Ok(())
    }

    /// Give up the lease if held. The epoch is unchanged; the next acquirer bumps it.
    pub fn release(&mut self, who: HolderId, now: u64) {
        if self.holder(now) == Some(who) {
            self.held = None;
        }
    }

    /// Fence a write: it is accepted only if it carries the current epoch.
    ///
    /// # Errors
    /// [`LeaseError::StaleEpoch`] for an older epoch. A newer epoch than ours is also refused,
    /// as it cannot have come from this lease.
    pub fn check_fence(&self, write_epoch: Epoch) -> Result<(), LeaseError> {
        if write_epoch == self.epoch {
            Ok(())
        } else {
            Err(LeaseError::StaleEpoch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: HolderId = HolderId(1);
    const B: HolderId = HolderId(2);

    #[test]
    fn first_acquire_gets_epoch_one() {
        let mut l = ShardLease::new();
        assert_eq!(l.epoch(), Epoch(0));
        assert_eq!(l.acquire(A, 100, 10), Ok(Epoch(1)));
        assert_eq!(l.holder(105), Some(A));
    }

    #[test]
    fn other_holder_is_refused_while_live_then_takes_over_with_higher_epoch() {
        let mut l = ShardLease::new();
        l.acquire(A, 100, 10).unwrap();
        assert_eq!(l.acquire(B, 109, 10), Err(LeaseError::HeldByOther));
        assert_eq!(l.holder(110), None, "expires exactly at now + ttl");
        assert_eq!(l.acquire(B, 110, 10), Ok(Epoch(2)));
    }

    #[test]
    fn same_holder_reacquire_keeps_epoch() {
        let mut l = ShardLease::new();
        l.acquire(A, 100, 10).unwrap();
        assert_eq!(l.acquire(A, 105, 10), Ok(Epoch(1)));
        assert_eq!(l.holder(114), Some(A));
    }

    #[test]
    fn stale_holder_is_fenced_after_takeover() {
        let mut l = ShardLease::new();
        let old = l.acquire(A, 100, 10).unwrap();
        let new = l.acquire(B, 120, 10).unwrap();
        assert_eq!(l.check_fence(old), Err(LeaseError::StaleEpoch));
        assert_eq!(l.check_fence(new), Ok(()));
        assert_eq!(l.check_fence(Epoch(new.0 + 1)), Err(LeaseError::StaleEpoch));
    }

    #[test]
    fn renew_requires_a_live_lease() {
        let mut l = ShardLease::new();
        assert_eq!(l.renew(A, 0, 10), Err(LeaseError::NotHolder));
        l.acquire(A, 100, 10).unwrap();
        assert_eq!(l.renew(B, 101, 10), Err(LeaseError::NotHolder));
        assert_eq!(l.renew(A, 105, 10), Ok(()));
        assert_eq!(l.holder(114), Some(A));
        assert_eq!(l.renew(A, 115, 10), Err(LeaseError::NotHolder), "lapsed");
    }

    #[test]
    fn release_lets_another_acquire_immediately_with_new_epoch() {
        let mut l = ShardLease::new();
        l.acquire(A, 100, 10).unwrap();
        l.release(B, 101);
        assert_eq!(l.holder(101), Some(A), "non-holder cannot release");
        l.release(A, 101);
        assert_eq!(l.acquire(B, 101, 10), Ok(Epoch(2)));
    }

    #[test]
    fn overflow_is_an_error_not_a_wrap() {
        let mut l = ShardLease::new();
        assert_eq!(l.acquire(A, u64::MAX, 1), Err(LeaseError::TimeOverflow));
        assert_eq!(l.epoch(), Epoch(0), "failed acquire changes nothing");
    }
}

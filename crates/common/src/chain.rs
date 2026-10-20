//! Per-account append-only hash chain: each link has `seq`, `prev_hash` and `hash`.
//!
//! The hash function, digest width and genesis rule are protocol details, not decided here:
//! the digest is opaque bytes and hashing is injected through [`LinkHasher`].
//! `TODO(interfaces)`: fix the algorithm, the encoding of the hashed fields and the genesis
//! `prev_hash`. Until then the genesis link has no predecessor (`prev_hash == None`).

use crate::Seq;

/// Computes a link's hash from its position, predecessor hash and payload.
pub trait LinkHasher {
    /// Hash of a link. Must be deterministic.
    fn hash(&self, seq: Seq, prev_hash: Option<&[u8]>, payload: &[u8]) -> Vec<u8>;
}

/// One entry of the chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// Position in the chain, starting at `Seq(0)`.
    pub seq: Seq,
    /// Hash of the previous link; `None` for the first link.
    pub prev_hash: Option<Vec<u8>>,
    /// Opaque payload (the journal record).
    pub payload: Vec<u8>,
    /// Hash over `(seq, prev_hash, payload)`.
    pub hash: Vec<u8>,
}

/// Why verification failed, with the index of the offending link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainError {
    /// The link's `seq` is not its position.
    BadSeq(usize),
    /// The link's `prev_hash` is not the previous link's `hash` (or the first link has one).
    BadPrev(usize),
    /// The link's `hash` does not match its contents.
    BadHash(usize),
}

/// Appends links, tracking the tip.
#[derive(Debug, Default)]
pub struct ChainBuilder {
    next_seq: u64,
    tip: Option<Vec<u8>>,
}

impl ChainBuilder {
    /// A builder for a new, empty chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resume after an existing tip: the next link will have `next_seq` and `prev_hash = tip`.
    #[must_use]
    pub fn resume(next_seq: Seq, tip: Option<Vec<u8>>) -> Self {
        Self {
            next_seq: next_seq.0,
            tip,
        }
    }

    /// Append a payload, returning the new link, or `None` if `seq` would overflow.
    pub fn append<H: LinkHasher>(&mut self, hasher: &H, payload: Vec<u8>) -> Option<Link> {
        let seq = Seq(self.next_seq);
        let next = self.next_seq.checked_add(1)?;
        let hash = hasher.hash(seq, self.tip.as_deref(), &payload);
        let link = Link {
            seq,
            prev_hash: self.tip.take(),
            payload,
            hash: hash.clone(),
        };
        self.next_seq = next;
        self.tip = Some(hash);
        Some(link)
    }
}

/// Verify a chain that starts at `Seq(0)`.
///
/// # Errors
/// The first inconsistency found, with the link index.
pub fn verify<H: LinkHasher>(hasher: &H, links: &[Link]) -> Result<(), ChainError> {
    let mut prev: Option<&[u8]> = None;
    for (i, link) in links.iter().enumerate() {
        if u64::try_from(i).map_or(true, |i| link.seq != Seq(i)) {
            return Err(ChainError::BadSeq(i));
        }
        if link.prev_hash.as_deref() != prev {
            return Err(ChainError::BadPrev(i));
        }
        if hasher.hash(link.seq, link.prev_hash.as_deref(), &link.payload) != link.hash {
            return Err(ChainError::BadHash(i));
        }
        prev = Some(&link.hash);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only toy hasher (FNV-1a over a length-prefixed encoding). Not cryptographic.
    struct Toy;
    impl LinkHasher for Toy {
        fn hash(&self, seq: Seq, prev: Option<&[u8]>, payload: &[u8]) -> Vec<u8> {
            let mut buf = seq.0.to_be_bytes().to_vec();
            match prev {
                None => buf.push(0),
                Some(p) => {
                    buf.push(1);
                    buf.extend_from_slice(&(p.len() as u64).to_be_bytes());
                    buf.extend_from_slice(p);
                }
            }
            buf.extend_from_slice(payload);
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in buf {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            h.to_be_bytes().to_vec()
        }
    }

    fn build(n: usize) -> Vec<Link> {
        let mut b = ChainBuilder::new();
        (0..n)
            .map(|i| b.append(&Toy, vec![i as u8]).unwrap())
            .collect()
    }

    #[test]
    fn built_chain_verifies_and_links_up() {
        let links = build(4);
        assert_eq!(verify(&Toy, &links), Ok(()));
        assert_eq!(links[0].prev_hash, None);
        assert_eq!(links[1].prev_hash.as_ref(), Some(&links[0].hash));
        assert_eq!(verify(&Toy, &[]), Ok(()));
    }

    #[test]
    fn payload_tamper_is_detected_at_that_link() {
        let mut links = build(4);
        links[2].payload[0] ^= 1;
        assert_eq!(verify(&Toy, &links), Err(ChainError::BadHash(2)));
    }

    #[test]
    fn reordering_and_dropping_are_detected() {
        let mut links = build(4);
        links.swap(1, 2);
        assert_eq!(verify(&Toy, &links), Err(ChainError::BadSeq(1)));
        let mut links = build(4);
        links.remove(1);
        assert_eq!(verify(&Toy, &links), Err(ChainError::BadSeq(1)));
    }

    #[test]
    fn rewiring_prev_hash_is_detected() {
        let mut links = build(3);
        links[1].prev_hash = None;
        assert_eq!(verify(&Toy, &links), Err(ChainError::BadPrev(1)));
        let mut links = build(2);
        links[0].prev_hash = Some(vec![9]);
        assert_eq!(verify(&Toy, &links), Err(ChainError::BadPrev(0)));
    }

    #[test]
    fn resume_continues_the_same_chain() {
        let links = build(3);
        let tip = links[2].hash.clone();
        let mut b = ChainBuilder::resume(Seq(3), Some(tip));
        let mut all = links;
        all.push(b.append(&Toy, vec![3]).unwrap());
        assert_eq!(verify(&Toy, &all), Ok(()));
    }

    #[test]
    fn seq_overflow_returns_none_and_changes_nothing() {
        let mut b = ChainBuilder::resume(Seq(u64::MAX), Some(vec![1]));
        assert_eq!(b.append(&Toy, vec![]), None);
        assert_eq!(b.tip, Some(vec![1]));
    }
}

//! Identities that are not the same thing (E §2), and proof of possession.
//!
//! - an **executor identity** is the `Address` a task names; whoever holds
//!   its Ed25519 key may answer that task and nobody else;
//! - a **worker instance** is one running process, which may hold one
//!   executor key; an executor may run many instances;
//! - a **session** binds one instance to one executor at the control plane
//!   for a while;
//! - an **attempt** is one instance's effort at one task under one lease.
//!
//! Only the executor identity is cryptographic. Everything else is an
//! opaque, off-chain identifier that never touches consensus.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use mbongo_core::Address;

/// A 32-byte opaque identifier, hex on display.
macro_rules! opaque_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        pub struct $name(pub [u8; 32]);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", hex::encode(&self.0[..8]))
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), hex::encode(self.0))
            }
        }
    };
}

opaque_id!(
    /// One running worker process. Not an executor.
    WorkerInstanceId
);
opaque_id!(
    /// One instance's authenticated relationship with the control plane.
    SessionId
);
opaque_id!(
    /// One instance's effort at one task under one lease (E §8). Off-chain.
    AttemptId
);
opaque_id!(
    /// One execution lease (E §3). Off-chain.
    LeaseId
);
opaque_id!(
    /// One data-plane capability (F §5.3): the unit of consumption and
    /// revocation.
    CapabilityId
);
opaque_id!(
    /// One private object in the data plane. Opaque; grants nothing.
    ObjectId
);
opaque_id!(
    /// A fresh challenge for proof of possession. Single-use.
    Challenge
);

/// Deterministic source of fresh identifiers: `BLAKE3(seed || domain || n)`.
///
/// Unpredictable to anyone who does not hold the seed, distinct across
/// domains and across calls, and reproducible in a test that fixes the
/// seed. A production deployment would seed it from the OS; the harness
/// seeds it from the clock and the process id.
#[derive(Debug, Clone)]
pub struct IdSource {
    seed: [u8; 32],
    counter: u64,
}

impl IdSource {
    /// An id source over `seed`.
    pub fn new(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }

    /// The next 32 fresh bytes for `domain`.
    pub fn next(&mut self, domain: &str) -> [u8; 32] {
        self.counter += 1;
        let mut h = blake3::Hasher::new();
        h.update(&self.seed);
        h.update(domain.as_bytes());
        h.update(&self.counter.to_le_bytes());
        *h.finalize().as_bytes()
    }
}

/// The executor's Ed25519 key, held **locally by the worker process**.
///
/// This is the only cryptographic identity in the system. It signs receipts
/// and anchoring transactions (RFC 0005 §2.5, rule g, rule s) and proves
/// possession to the control plane and the data plane (E §5, F §5.3). It is
/// never serialised, never logged (`Debug` prints the public address only),
/// and never handed to the control plane: the control plane can relay a
/// signature but cannot produce one (E §11.1).
///
/// Custody in this reference implementation is a 32-byte seed in process
/// memory, supplied by the operator at start. No keystore, KMS or HSM is
/// chosen; any of them may supply the seed.
#[derive(Clone)]
pub struct ExecutorKey {
    key: SigningKey,
}

impl std::fmt::Debug for ExecutorKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecutorKey({})", self.address())
    }
}

impl ExecutorKey {
    /// A key from its 32-byte Ed25519 seed.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(seed),
        }
    }

    /// The executor identity this key controls: the `Address` a task names.
    pub fn address(&self) -> Address {
        Address(self.key.verifying_key().to_bytes())
    }

    /// Ed25519 over `message`.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.key.sign(message).to_bytes()
    }

    /// Proof of possession over a challenge, under a domain tag so that a
    /// possession proof can never be mistaken for a receipt or transaction
    /// signature, and bound to `context` so that a proof for one capability
    /// or session cannot serve another.
    pub fn prove_possession(
        &self,
        domain: &str,
        context: &[u8],
        challenge: &Challenge,
    ) -> [u8; 64] {
        self.sign(&possession_message(domain, context, challenge))
    }
}

/// The exact bytes a possession proof signs: `domain || context || challenge`.
pub fn possession_message(domain: &str, context: &[u8], challenge: &Challenge) -> Vec<u8> {
    let mut m = Vec::with_capacity(domain.len() + context.len() + 32);
    m.extend_from_slice(domain.as_bytes());
    m.extend_from_slice(context);
    m.extend_from_slice(&challenge.0);
    m
}

/// Verifies an Ed25519 signature by `address` over `message`.
pub fn verify_signature(address: &Address, message: &[u8], signature: &[u8; 64]) -> bool {
    let Ok(pk) = VerifyingKey::from_bytes(&address.0) else {
        return false;
    };
    pk.verify(message, &Signature::from_bytes(signature)).is_ok()
}

/// Verifies a possession proof by `executor` over `(domain, context, challenge)`.
pub fn verify_possession(
    executor: &Address,
    domain: &str,
    context: &[u8],
    challenge: &Challenge,
    proof: &[u8; 64],
) -> bool {
    verify_signature(
        executor,
        &possession_message(domain, context, challenge),
        proof,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_key_debug_never_shows_the_seed() {
        let key = ExecutorKey::from_seed(&[7u8; 32]);
        let shown = format!("{key:?}");
        assert!(shown.contains(&key.address().to_string()));
        assert!(!shown.contains(&hex::encode([7u8; 32])));
    }

    #[test]
    fn possession_is_bound_to_domain_context_and_challenge() {
        let key = ExecutorKey::from_seed(&[1u8; 32]);
        let other = ExecutorKey::from_seed(&[2u8; 32]);
        let ch = Challenge([9u8; 32]);
        let proof = key.prove_possession("d", b"ctx", &ch);
        assert!(verify_possession(&key.address(), "d", b"ctx", &ch, &proof));
        assert!(!verify_possession(
            &other.address(),
            "d",
            b"ctx",
            &ch,
            &proof
        ));
        assert!(!verify_possession(&key.address(), "e", b"ctx", &ch, &proof));
        assert!(!verify_possession(&key.address(), "d", b"ctz", &ch, &proof));
        assert!(!verify_possession(
            &key.address(),
            "d",
            b"ctx",
            &Challenge([8u8; 32]),
            &proof
        ));
    }

    #[test]
    fn id_source_is_deterministic_and_distinct() {
        let mut a = IdSource::new([3u8; 32]);
        let mut b = IdSource::new([3u8; 32]);
        let x = a.next("lease");
        assert_eq!(x, b.next("lease"));
        assert_ne!(x, a.next("lease"));
        let mut c = IdSource::new([3u8; 32]);
        assert_ne!(x, c.next("attempt"));
    }
}

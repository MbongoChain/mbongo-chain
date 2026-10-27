//! Request authentication for the MVP HTTP front (MVP-C).
//!
//! Every request is signed by the client with the Ed25519 key of the chain
//! account that submits (or submitted) the task — the same key that signs
//! the `ComputeTask` transaction. The signature covers the method, the path,
//! a timestamp, a random per-request nonce and the BLAKE3 digest of the
//! exact body bytes:
//!
//! ```text
//! message = "mbongo-mvp-http:v1\n" METHOD "\n" PATH "\n" TIMESTAMP "\n" NONCE "\n" hex(BLAKE3(body))
//! headers  x-mbongo-address    64 lowercase hex (the account address)
//!          x-mbongo-timestamp  unix seconds
//!          x-mbongo-nonce      32 lowercase hex (16 random bytes)
//!          x-mbongo-signature  128 lowercase hex
//! ```
//!
//! Ed25519 signatures are deterministic: without the nonce, two identical
//! polls in the same second would carry the same signature and the second
//! would be refused as a replay.
//!
//! The front accepts a timestamp within ±[`MAX_SKEW_SECS`] of its clock and
//! refuses a `(signer, nonce)` pair it has already accepted in that window
//! ([`ReplayGuard`]). Who may do what
//! is decided elsewhere, by the chain: only the task's on-chain submitter may
//! register it, read its status or retrieve its result. No key or secret is
//! ever sent; nothing here is logged.

use std::collections::HashMap;

use mbongo_core::Address;

use crate::data_plane::LocalKey;
use crate::identity::verify_signature;

/// Domain of the signing message.
pub const AUTH_DOMAIN: &str = "mbongo-mvp-http:v1";
/// Header carrying the signer's address.
pub const HEADER_ADDRESS: &str = "x-mbongo-address";
/// Header carrying the signing timestamp.
pub const HEADER_TIMESTAMP: &str = "x-mbongo-timestamp";
/// Header carrying the request nonce.
pub const HEADER_NONCE: &str = "x-mbongo-nonce";
/// Header carrying the signature.
pub const HEADER_SIGNATURE: &str = "x-mbongo-signature";
/// Accepted clock skew, both directions.
pub const MAX_SKEW_SECS: u64 = 120;

/// The exact bytes a request signature covers.
pub fn signing_message(
    method: &str,
    path: &str,
    timestamp: u64,
    nonce: &[u8; 16],
    body: &[u8],
) -> Vec<u8> {
    let digest = hex::encode(blake3::hash(body).as_bytes());
    let nonce = hex::encode(nonce);
    format!("{AUTH_DOMAIN}\n{method}\n{path}\n{timestamp}\n{nonce}\n{digest}").into_bytes()
}

/// The four authentication headers for a request.
pub fn sign_request(
    key: &LocalKey,
    method: &str,
    path: &str,
    timestamp: u64,
    nonce: &[u8; 16],
    body: &[u8],
) -> [(&'static str, String); 4] {
    let sig = key.sign(&signing_message(method, path, timestamp, nonce, body));
    [
        (HEADER_ADDRESS, hex::encode(key.address().0)),
        (HEADER_TIMESTAMP, timestamp.to_string()),
        (HEADER_NONCE, hex::encode(nonce)),
        (HEADER_SIGNATURE, hex::encode(sig)),
    ]
}

/// Why a request was not authenticated. Carries no request content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// A header is missing.
    #[error("missing authentication header")]
    Missing,
    /// A header is not in the expected format.
    #[error("malformed authentication header")]
    Malformed,
    /// The timestamp is outside the accepted window.
    #[error("stale or future timestamp")]
    Stale,
    /// The signer already used this nonce within the window.
    #[error("replayed request or reused nonce")]
    Replayed,
    /// The replay guard is full; the request was not processed.
    #[error("replay guard full")]
    Busy,
    /// The signature does not verify for the claimed address.
    #[error("bad signature")]
    BadSignature,
}

/// Most `(signer, nonce)` pairs the replay guard holds at once. Only
/// requests whose signature verified are recorded, so an unauthenticated
/// flood cannot fill it; when it is full, new requests are refused rather
/// than letting memory grow.
pub const MAX_REPLAY_ENTRIES: usize = 100_000;

/// Remembers every `(signer, nonce)` pair accepted within twice the skew
/// window — long enough to cover every timestamp the front would still
/// accept. A request reusing a pair is a replay, whatever its body: an
/// identical resubmission and a different request under a reused nonce are
/// both refused.
///
/// **Durability: process lifetime only.** The guard is in memory; a restart
/// of the service forgets it. A request captured before a restart can be
/// replayed after it only while its timestamp is still inside the
/// ±[`MAX_SKEW_SECS`] window.
#[derive(Debug, Default)]
pub struct ReplayGuard {
    seen: HashMap<(Address, [u8; 16]), u64>,
}

impl ReplayGuard {
    fn check_and_record(
        &mut self,
        signer: Address,
        nonce: [u8; 16],
        now: u64,
    ) -> Result<(), AuthError> {
        self.seen.retain(|_, ts| ts.saturating_add(2 * MAX_SKEW_SECS) >= now);
        if self.seen.contains_key(&(signer, nonce)) {
            return Err(AuthError::Replayed);
        }
        if self.seen.len() >= MAX_REPLAY_ENTRIES {
            return Err(AuthError::Busy);
        }
        self.seen.insert((signer, nonce), now);
        Ok(())
    }

    /// Pairs currently remembered.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether nothing is remembered.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

fn parse_hex<const N: usize>(s: &str) -> Result<[u8; N], AuthError> {
    let v = hex::decode(s).map_err(|_| AuthError::Malformed)?;
    v.try_into().map_err(|_| AuthError::Malformed)
}

/// The four authentication header values as received.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthHeaders<'a> {
    /// `x-mbongo-address`.
    pub address: Option<&'a str>,
    /// `x-mbongo-timestamp`.
    pub timestamp: Option<&'a str>,
    /// `x-mbongo-nonce`.
    pub nonce: Option<&'a str>,
    /// `x-mbongo-signature`.
    pub signature: Option<&'a str>,
}

/// Verifies a request and returns the authenticated address.
pub fn verify_request(
    headers: AuthHeaders<'_>,
    method: &str,
    path: &str,
    body: &[u8],
    now: u64,
    replay: &mut ReplayGuard,
) -> Result<Address, AuthError> {
    let (Some(a), Some(t), Some(n), Some(s)) = (
        headers.address,
        headers.timestamp,
        headers.nonce,
        headers.signature,
    ) else {
        return Err(AuthError::Missing);
    };
    let address = Address(parse_hex::<32>(a)?);
    let ts: u64 = t.parse().map_err(|_| AuthError::Malformed)?;
    let nonce = parse_hex::<16>(n)?;
    let sig = parse_hex::<64>(s)?;
    if ts.abs_diff(now) > MAX_SKEW_SECS {
        return Err(AuthError::Stale);
    }
    let message = signing_message(method, path, ts, &nonce, body);
    if !verify_signature(&address, &message, &sig) {
        return Err(AuthError::BadSignature);
    }
    replay.check_and_record(address, nonce, now)?;
    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hd<'a>(h: &'a [(&'static str, String); 4]) -> AuthHeaders<'a> {
        AuthHeaders {
            address: Some(&h[0].1),
            timestamp: Some(&h[1].1),
            nonce: Some(&h[2].1),
            signature: Some(&h[3].1),
        }
    }

    #[test]
    fn signed_request_round_trips_and_binds_every_field() {
        let key = LocalKey::from_seed(&[7u8; 32]);
        let h = sign_request(&key, "PUT", "/v1/inputs", 1_000, &[9u8; 16], b"body");
        let mut g = ReplayGuard::default();
        let ok = verify_request(hd(&h), "PUT", "/v1/inputs", b"body", 1_010, &mut g);
        assert_eq!(ok, Ok(key.address()));
        assert_eq!(
            verify_request(hd(&h), "PUT", "/v1/inputs", b"body", 1_010, &mut g),
            Err(AuthError::Replayed)
        );
        let mut g = ReplayGuard::default();
        for (m, p, b) in [
            ("GET", "/v1/inputs", &b"body"[..]),
            ("PUT", "/v1/other", &b"body"[..]),
            ("PUT", "/v1/inputs", &b"bodY"[..]),
        ] {
            assert_eq!(
                verify_request(hd(&h), m, p, b, 1_000, &mut g),
                Err(AuthError::BadSignature)
            );
        }
        let mut other_nonce = h.clone();
        other_nonce[2].1 = hex::encode([8u8; 16]);
        assert_eq!(
            verify_request(
                hd(&other_nonce),
                "PUT",
                "/v1/inputs",
                b"body",
                1_000,
                &mut g
            ),
            Err(AuthError::BadSignature)
        );
        assert_eq!(
            verify_request(
                hd(&h),
                "PUT",
                "/v1/inputs",
                b"body",
                1_000 + MAX_SKEW_SECS + 1,
                &mut g
            ),
            Err(AuthError::Stale)
        );
        assert_eq!(
            verify_request(AuthHeaders::default(), "PUT", "/", b"", 0, &mut g),
            Err(AuthError::Missing)
        );
    }
}

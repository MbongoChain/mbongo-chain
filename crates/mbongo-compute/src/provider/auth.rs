//! Recipient-bound request signatures for the provider interface.
//!
//! A client signs each request with its chain key, for one provider. The
//! signature covers the provider's identity, the method, the path, the body
//! digest, a validity window and a unique request identifier, under a domain
//! of its own. A request captured by one provider is therefore useless at
//! another. The frozen MVP format ([`crate::mvp::auth`]) is untouched.
//!
//! Who may act on a task is still decided by the chain: only the committed
//! task's `submitter` may register it, read its status or fetch its result.

use mbongo_core::Address;

use crate::data_plane::LocalKey;
use crate::identity::verify_signature;

/// Domain of the signing message.
pub const AUTH_DOMAIN: &str = "mbongo-provider-http:v1";
/// The signer's address.
pub const HEADER_ADDRESS: &str = "x-mbongo-address";
/// The provider the request is for.
pub const HEADER_RECIPIENT: &str = "x-mbongo-recipient";
/// Start of the validity window, unix seconds.
pub const HEADER_ISSUED_AT: &str = "x-mbongo-issued-at";
/// End of the validity window, unix seconds.
pub const HEADER_EXPIRES_AT: &str = "x-mbongo-expires-at";
/// The unique request identifier.
pub const HEADER_REQUEST_ID: &str = "x-mbongo-request-id";
/// The signature.
pub const HEADER_SIGNATURE: &str = "x-mbongo-signature";
/// Longest validity window a provider accepts.
pub const MAX_WINDOW_SECS: u64 = 120;
/// Clock difference tolerated at the start of the window.
pub const MAX_CLOCK_SKEW_SECS: u64 = 30;
/// Most request identifiers held at once. Only requests whose signature
/// verified are recorded, so an unauthenticated flood cannot fill it; when it
/// is full, requests are refused, not let through.
pub const MAX_REPLAY_ENTRIES: usize = 100_000;

/// What a signature covers.
#[derive(Debug, Clone, Copy)]
pub struct SignedRequest<'a> {
    /// The provider the request is for.
    pub recipient: Address,
    /// HTTP method.
    pub method: &'a str,
    /// Request path.
    pub path: &'a str,
    /// Request body.
    pub body: &'a [u8],
    /// Start of the validity window.
    pub issued_at: u64,
    /// End of the validity window.
    pub expires_at: u64,
    /// Unique request identifier.
    pub request_id: [u8; 16],
}

/// The exact bytes a request signature covers.
pub fn signing_message(r: &SignedRequest<'_>) -> Vec<u8> {
    format!(
        "{AUTH_DOMAIN}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        hex::encode(r.recipient.0),
        r.method,
        r.path,
        hex::encode(blake3::hash(r.body).as_bytes()),
        r.issued_at,
        r.expires_at,
        hex::encode(r.request_id),
    )
    .into_bytes()
}

/// The authentication headers for a request.
pub fn sign_request(key: &LocalKey, r: &SignedRequest<'_>) -> [(&'static str, String); 6] {
    [
        (HEADER_ADDRESS, hex::encode(key.address().0)),
        (HEADER_RECIPIENT, hex::encode(r.recipient.0)),
        (HEADER_ISSUED_AT, r.issued_at.to_string()),
        (HEADER_EXPIRES_AT, r.expires_at.to_string()),
        (HEADER_REQUEST_ID, hex::encode(r.request_id)),
        (HEADER_SIGNATURE, hex::encode(key.sign(&signing_message(r)))),
    ]
}

/// Why a request was refused. Each has its own code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// A header is absent.
    #[error("AUTH_MISSING")]
    Missing,
    /// A header is not well formed.
    #[error("AUTH_MALFORMED")]
    Malformed,
    /// The request is for another provider.
    #[error("AUTH_WRONG_RECIPIENT")]
    WrongRecipient,
    /// The window is empty, or longer than [`MAX_WINDOW_SECS`].
    #[error("AUTH_BAD_WINDOW")]
    BadWindow,
    /// The window has not started.
    #[error("AUTH_NOT_YET_VALID")]
    NotYetValid,
    /// The window has ended.
    #[error("AUTH_EXPIRED")]
    Expired,
    /// The signature does not verify: a wrong key or a changed request.
    #[error("AUTH_BAD_SIGNATURE")]
    BadSignature,
}

/// The authentication header values as received.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthHeaders<'a> {
    /// [`HEADER_ADDRESS`].
    pub address: Option<&'a str>,
    /// [`HEADER_RECIPIENT`].
    pub recipient: Option<&'a str>,
    /// [`HEADER_ISSUED_AT`].
    pub issued_at: Option<&'a str>,
    /// [`HEADER_EXPIRES_AT`].
    pub expires_at: Option<&'a str>,
    /// [`HEADER_REQUEST_ID`].
    pub request_id: Option<&'a str>,
    /// [`HEADER_SIGNATURE`].
    pub signature: Option<&'a str>,
}

fn parse_hex<const N: usize>(s: &str) -> Result<[u8; N], AuthError> {
    let v = hex::decode(s).map_err(|_| AuthError::Malformed)?;
    v.try_into().map_err(|_| AuthError::Malformed)
}

/// The request as this provider received it.
#[derive(Debug, Clone, Copy)]
pub struct Received<'a> {
    /// This provider's identity.
    pub provider: Address,
    /// HTTP method.
    pub method: &'a str,
    /// Request path.
    pub path: &'a str,
    /// Request body.
    pub body: &'a [u8],
    /// This provider's clock.
    pub now: u64,
}

/// A request whose signature, recipient and window verified. It is not
/// accepted yet: replay protection is durable, and belongs to
/// [`super::durable::DurableStore::accept_request`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified {
    /// The authenticated address.
    pub signer: Address,
    /// The unique request identifier.
    pub request_id: [u8; 16],
    /// End of the validity window.
    pub expires_at: u64,
}

/// Verifies a request.
pub fn verify_request(
    headers: AuthHeaders<'_>,
    request: Received<'_>,
) -> Result<Verified, AuthError> {
    let (Some(address), Some(to), Some(from_time), Some(to_time), Some(id), Some(sig)) = (
        headers.address,
        headers.recipient,
        headers.issued_at,
        headers.expires_at,
        headers.request_id,
        headers.signature,
    ) else {
        return Err(AuthError::Missing);
    };
    let signer = Address(parse_hex::<32>(address)?);
    let recipient = Address(parse_hex::<32>(to)?);
    let issued_at: u64 = from_time.parse().map_err(|_| AuthError::Malformed)?;
    let expires_at: u64 = to_time.parse().map_err(|_| AuthError::Malformed)?;
    let request_id = parse_hex::<16>(id)?;
    let signature = parse_hex::<64>(sig)?;
    if recipient != request.provider {
        return Err(AuthError::WrongRecipient);
    }
    if expires_at <= issued_at || expires_at - issued_at > MAX_WINDOW_SECS {
        return Err(AuthError::BadWindow);
    }
    if issued_at > request.now.saturating_add(MAX_CLOCK_SKEW_SECS) {
        return Err(AuthError::NotYetValid);
    }
    if request.now > expires_at {
        return Err(AuthError::Expired);
    }
    let message = signing_message(&SignedRequest {
        recipient,
        method: request.method,
        path: request.path,
        body: request.body,
        issued_at,
        expires_at,
        request_id,
    });
    if !verify_signature(&signer, &message, &signature) {
        return Err(AuthError::BadSignature);
    }
    Ok(Verified {
        signer,
        request_id,
        expires_at,
    })
}

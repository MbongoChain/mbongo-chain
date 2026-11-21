//! The genesis file and the chain identifier (RFC 0006 §1).
//!
//! A network is defined by one file, `mbongo-genesis:v1`, which must already
//! be canonical or it is refused. Canonical is defined by the RFC itself, not
//! by any JSON library, so this module reads the file byte by byte against that
//! definition instead of parsing JSON:
//!
//! ```text
//! {"allocations":[{"address":"<hex>","amount":"<decimal>"}],"authorities":["<hex>"],
//!  "name":"<name>","schema":"mbongo-genesis:v1","timestamp":<digits>}
//! ```
//!
//! (one line, with no whitespace anywhere outside strings).
//!
//! `chain_id = BLAKE3("mbongo:chain-id:v1" || file_bytes)`.
//!
//! This is a library only. Nothing in the node reads a genesis file yet
//! (S1-4 wires it in).

use crate::Address;

/// The only schema this module reads.
pub const GENESIS_SCHEMA_V1: &str = "mbongo-genesis:v1";

/// The largest genesis file accepted, in bytes.
pub const MAX_GENESIS_FILE_BYTES: usize = 1 << 20;

/// Domain of the chain identifier.
pub const CHAIN_ID_DOMAIN: &[u8] = b"mbongo:chain-id:v1";

/// Domain of the dev authority's seed.
pub const DEV_AUTHORITY_SEED_DOMAIN: &[u8] = b"mbongo:dev-authority:v1";

/// The dev genesis file, byte for byte (RFC 0006 §1, "The dev genesis").
pub const DEV_GENESIS_FILE: &[u8] = include_bytes!("../genesis/dev.json");

/// The longest network name.
const MAX_NAME_CHARS: usize = 64;

/// A 32-byte chain identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChainId(pub [u8; 32]);

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// Computes the chain identifier of a genesis file, from its exact bytes.
#[must_use]
pub fn chain_id(file_bytes: &[u8]) -> ChainId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHAIN_ID_DOMAIN);
    hasher.update(file_bytes);
    ChainId(*hasher.finalize().as_bytes())
}

/// The seed of the dev authority: `BLAKE3("mbongo:dev-authority:v1")`.
/// A public test key, never for a real network.
#[must_use]
pub fn dev_authority_seed() -> [u8; 32] {
    *blake3::hash(DEV_AUTHORITY_SEED_DOMAIN).as_bytes()
}

/// Why a genesis file is refused. Each variant is one class of RFC 0006 §1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenesisError {
    /// The file is larger than [`MAX_GENESIS_FILE_BYTES`].
    TooLarge,
    /// The file is not UTF-8.
    NotUtf8,
    /// Not the canonical form: whitespace, an escape, key order, a missing,
    /// duplicate or unknown key, a wrong type, or trailing bytes. The offset is
    /// where the file departs from it.
    NotCanonical(usize),
    /// `schema` is not `mbongo-genesis:v1`.
    Schema,
    /// `name` is not 1 to 64 characters from `[a-z0-9-]`.
    Name,
    /// `timestamp` has a leading zero or does not fit in `u64`.
    Timestamp,
    /// An allocation address is not 64 lowercase hex characters.
    Address,
    /// An amount is not a canonical decimal that fits in `u128`.
    Amount,
    /// The allocations sum to more than `u128::MAX`.
    SupplyOverflow,
    /// The allocations are not sorted ascending by address, or one repeats.
    AllocationOrder,
    /// `authorities` does not hold exactly one key.
    AuthorityCount,
    /// An authority is not 64 lowercase hex characters, or not a canonical
    /// Ed25519 point outside the small-order subgroup.
    Authority,
}

impl std::fmt::Display for GenesisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "genesis file larger than {MAX_GENESIS_FILE_BYTES} bytes"),
            Self::NotUtf8 => write!(f, "genesis file is not UTF-8"),
            Self::NotCanonical(at) => write!(f, "genesis file is not canonical at byte {at}"),
            Self::Schema => write!(f, "genesis schema is not {GENESIS_SCHEMA_V1}"),
            Self::Name => write!(f, "genesis name must be 1 to 64 characters from [a-z0-9-]"),
            Self::Timestamp => write!(f, "genesis timestamp is not a canonical u64"),
            Self::Address => write!(f, "allocation address is not 64 lowercase hex characters"),
            Self::Amount => write!(f, "allocation amount is not a canonical u128 decimal"),
            Self::SupplyOverflow => write!(f, "allocations sum to more than u128::MAX"),
            Self::AllocationOrder => write!(f, "allocations are not sorted by address, or repeat"),
            Self::AuthorityCount => write!(f, "mbongo-genesis:v1 holds exactly one authority"),
            Self::Authority => write!(f, "authority is not a usable Ed25519 public key"),
        }
    }
}

impl std::error::Error for GenesisError {}

/// A validated genesis file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Genesis {
    /// The network name.
    pub name: String,
    /// The genesis block's timestamp.
    pub timestamp: u64,
    /// The authorities: exactly one in `mbongo-genesis:v1`.
    pub authorities: Vec<Address>,
    /// Accounts created at genesis, sorted by address, each with nonce 0.
    pub allocations: Vec<(Address, u128)>,
    /// The chain identifier, from the file's exact bytes.
    pub chain_id: ChainId,
}

impl Genesis {
    /// Validates a genesis file and derives its chain identifier.
    ///
    /// # Errors
    ///
    /// The first [`GenesisError`] the file meets, in file order.
    pub fn from_file_bytes(bytes: &[u8]) -> Result<Self, GenesisError> {
        if bytes.len() > MAX_GENESIS_FILE_BYTES {
            return Err(GenesisError::TooLarge);
        }
        std::str::from_utf8(bytes).map_err(|_| GenesisError::NotUtf8)?;
        let mut r = Reader { bytes, at: 0 };

        r.literal(b"{\"allocations\":[")?;
        let mut allocations: Vec<(Address, u128)> = Vec::new();
        let mut supply: u128 = 0;
        if !r.peek(b']') {
            loop {
                r.literal(b"{\"address\":")?;
                let address = parse_hex32(r.string()?).ok_or(GenesisError::Address)?;
                r.literal(b",\"amount\":")?;
                let amount = parse_amount(r.string()?)?;
                r.literal(b"}")?;
                if allocations.last().is_some_and(|(last, _)| last.0 >= address) {
                    return Err(GenesisError::AllocationOrder);
                }
                supply = supply.checked_add(amount).ok_or(GenesisError::SupplyOverflow)?;
                allocations.push((Address(address), amount));
                if !r.comma() {
                    break;
                }
            }
        }
        r.literal(b"],\"authorities\":[")?;
        let mut authorities = Vec::new();
        if !r.peek(b']') {
            loop {
                let key = parse_hex32(r.string()?).ok_or(GenesisError::Authority)?;
                authorities.push(key);
                if !r.comma() {
                    break;
                }
            }
        }
        if authorities.len() != 1 {
            return Err(GenesisError::AuthorityCount);
        }
        if !authorities.iter().all(is_usable_public_key) {
            return Err(GenesisError::Authority);
        }
        r.literal(b"],\"name\":")?;
        let name = r.string()?;
        if name.is_empty()
            || name.len() > MAX_NAME_CHARS
            || !name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(GenesisError::Name);
        }
        r.literal(b",\"schema\":")?;
        if r.string()? != GENESIS_SCHEMA_V1 {
            return Err(GenesisError::Schema);
        }
        r.literal(b",\"timestamp\":")?;
        let timestamp = parse_timestamp(r.digits()?)?;
        r.literal(b"}")?;
        if r.at != bytes.len() {
            return Err(GenesisError::NotCanonical(r.at));
        }

        Ok(Self {
            name: name.to_owned(),
            timestamp,
            authorities: authorities.into_iter().map(Address).collect(),
            allocations,
            chain_id: chain_id(bytes),
        })
    }

    /// The built-in dev genesis.
    ///
    /// # Panics
    ///
    /// Never: the dev file is a fixed, tested part of this crate.
    #[must_use]
    pub fn dev() -> Self {
        Self::from_file_bytes(DEV_GENESIS_FILE).expect("the dev genesis file is valid")
    }

    /// The block producer: the only authority this schema allows.
    #[must_use]
    pub fn authority(&self) -> Address {
        self.authorities[0]
    }
}

/// Reads the canonical form, byte by byte.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// Consumes exactly `expected`, or fails where the file departs from it.
    fn literal(&mut self, expected: &[u8]) -> Result<(), GenesisError> {
        for (i, b) in expected.iter().enumerate() {
            if self.bytes.get(self.at + i) != Some(b) {
                return Err(GenesisError::NotCanonical(self.at + i));
            }
        }
        self.at += expected.len();
        Ok(())
    }

    fn peek(&self, b: u8) -> bool {
        self.bytes.get(self.at) == Some(&b)
    }

    /// Consumes a comma if one follows.
    fn comma(&mut self) -> bool {
        if self.peek(b',') {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// A string with no escape and no control character; returns its content.
    fn string(&mut self) -> Result<&'a str, GenesisError> {
        self.literal(b"\"")?;
        let start = self.at;
        loop {
            match self.bytes.get(self.at) {
                Some(b'"') => break,
                Some(b'\\') | None => return Err(GenesisError::NotCanonical(self.at)),
                Some(b) if *b < 0x20 => return Err(GenesisError::NotCanonical(self.at)),
                Some(_) => self.at += 1,
            }
        }
        let content = &self.bytes[start..self.at];
        self.at += 1;
        // The whole file is UTF-8 and the string ends on a quote, so this holds.
        std::str::from_utf8(content).map_err(|_| GenesisError::NotUtf8)
    }

    /// One or more ASCII digits, the form of a JSON integer without a sign.
    fn digits(&mut self) -> Result<&'a str, GenesisError> {
        let start = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        if self.at == start {
            return Err(GenesisError::NotCanonical(start));
        }
        std::str::from_utf8(&self.bytes[start..self.at]).map_err(|_| GenesisError::NotUtf8)
    }
}

/// 64 lowercase hex characters, as 32 bytes.
fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(s, &mut out).ok()?;
    Some(out)
}

/// Digits only, no leading zero unless the value is zero.
fn is_canonical_decimal(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'))
}

fn parse_amount(s: &str) -> Result<u128, GenesisError> {
    if !is_canonical_decimal(s) {
        return Err(GenesisError::Amount);
    }
    s.parse().map_err(|_| GenesisError::Amount)
}

fn parse_timestamp(s: &str) -> Result<u64, GenesisError> {
    if !is_canonical_decimal(s) {
        return Err(GenesisError::Timestamp);
    }
    s.parse().map_err(|_| GenesisError::Timestamp)
}

/// A canonical encoding of an Ed25519 point that is not of small order.
///
/// Canonical is RFC 8032 §5.1.3: `y < p`, and no sign bit when `x = 0` (that
/// is, when `y` is 1 or `p - 1`). `ed25519-dalek` decodes non-canonical
/// encodings, so this is checked first.
fn is_usable_public_key(key: &[u8; 32]) -> bool {
    let sign = key[31] >> 7;
    let mut y = *key;
    y[31] &= 0x7f;
    // y >= p = 2^255 - 19 exactly when the top 31 bytes are all ones (bit 255
    // cleared) and the lowest byte is at least 0xed.
    if y[31] == 0x7f && y[1..31].iter().all(|b| *b == 0xff) && y[0] >= 0xed {
        return false;
    }
    let y_is_one = y[0] == 1 && y[1..].iter().all(|b| *b == 0);
    let y_is_p_minus_one = y[0] == 0xec && y[1..31].iter().all(|b| *b == 0xff) && y[31] == 0x7f;
    if sign == 1 && (y_is_one || y_is_p_minus_one) {
        return false;
    }
    ed25519_dalek::VerifyingKey::from_bytes(key).is_ok_and(|k| !k.is_weak())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_over_one_mebibyte_is_refused_before_anything_else() {
        let mut big = DEV_GENESIS_FILE.to_vec();
        big.resize(MAX_GENESIS_FILE_BYTES + 1, b' ');
        assert_eq!(Genesis::from_file_bytes(&big), Err(GenesisError::TooLarge));
        // Exactly the bound is not refused for its size.
        big.truncate(MAX_GENESIS_FILE_BYTES);
        assert!(matches!(
            Genesis::from_file_bytes(&big),
            Err(GenesisError::NotCanonical(_))
        ));
    }

    #[test]
    fn the_dev_genesis_is_the_rfc_one() {
        use ed25519_dalek::SigningKey;
        let dev = Genesis::dev();
        assert_eq!(dev.name, "dev");
        assert_eq!(dev.timestamp, 0);
        let account = Address(SigningKey::from_bytes(&[0xAA; 32]).verifying_key().to_bytes());
        assert_eq!(dev.allocations, vec![(account, 1_000_000_000)]);
        let authority = SigningKey::from_bytes(&dev_authority_seed()).verifying_key();
        assert_eq!(dev.authority(), Address(authority.to_bytes()));
        assert_eq!(dev.chain_id, chain_id(DEV_GENESIS_FILE));
    }

    #[test]
    fn the_chain_id_covers_every_byte() {
        let mut other = DEV_GENESIS_FILE.to_vec();
        let at = other.len() - 2; // the timestamp digit
        other[at] = b'1';
        assert_ne!(chain_id(&other), chain_id(DEV_GENESIS_FILE));
        assert_eq!(Genesis::from_file_bytes(&other).unwrap().timestamp, 1);
    }

    #[test]
    fn the_chain_id_is_domain_separated() {
        assert_ne!(
            chain_id(DEV_GENESIS_FILE).0,
            *blake3::hash(DEV_GENESIS_FILE).as_bytes()
        );
    }
}

use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 32-byte hash used across headers and roots.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Encode, Decode)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// Returns the zero hash (all bytes zero).
    #[must_use]
    pub const fn zero() -> Self {
        Self([0u8; 32])
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl std::str::FromStr for Hash {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s).map_err(|e| e.to_string())?;
        if bytes.len() != 32 {
            return Err(format!("expected 32 bytes, got {}", bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Hash(arr))
    }
}

impl Serialize for Hash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// 32-byte address (ed25519 public key).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Encode, Decode)]
pub struct Address(pub [u8; 32]);

impl Address {
    /// Returns the zero address.
    #[must_use]
    pub const fn zero() -> Self {
        Self([0u8; 32])
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl std::str::FromStr for Address {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s).map_err(|e| e.to_string())?;
        if bytes.len() != 32 {
            return Err(format!("expected 32 bytes, got {}", bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Address(arr))
    }
}

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Supported transaction types.
///
/// Codec indexes are pinned explicitly: this enum is consensus-visible,
/// and variant order must never silently change the wire format. Indexes
/// 0–2 match the implicit v0.2 encoding byte-for-byte; index 3 is the
/// v0.3 addition (RFC 0002 §1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum TransactionType {
    /// Simple transfer from sender to receiver of `amount`.
    #[codec(index = 0)]
    Transfer,
    /// Compute task assignment/payment.
    #[codec(index = 1)]
    ComputeTask,
    /// Stake `amount` to validator or staking contract.
    #[codec(index = 2)]
    Stake,
    /// Anchor an off-chain compute receipt (RFC 0002). Carried in
    /// [`TransactionPayload::AnchorReceipt`]. Consensus rules for this
    /// type activate in RFC 0002 Phase 3; until then it is pure data.
    #[codec(index = 3)]
    AnchorReceipt,
}

/// Typed transaction payload (RFC 0002 §1.1).
///
/// Appending variants (with a new explicit index) is the extension point
/// for future transaction kinds. Codec indexes are pinned explicitly:
/// this enum is consensus-visible.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Encode, Decode)]
#[allow(clippy::cast_possible_truncation)] // codec derive casts the explicit index to u8
pub enum TransactionPayload {
    /// No payload. Required for `Transfer`, `ComputeTask`, and `Stake`
    /// transactions. Encodes as a single `0x00` byte.
    #[codec(index = 0)]
    None,
    /// A receipt to anchor. Required for `AnchorReceipt` transactions.
    /// Boxed to keep `Transaction` small for the common `None` case;
    /// SCALE encodes `Box<T>` identically to `T`, so the wire format is
    /// exactly `0x01` followed by the canonical receipt bytes.
    #[codec(index = 1)]
    AnchorReceipt(Box<crate::receipt::Receipt>),
}

/// Transaction structure (SCALE serializable) with ed25519 signature.
///
/// Canonical v0.3 SCALE field order (RFC 0002 §1): `tx_type`, `sender`,
/// `receiver`, `amount`, `nonce`, `payload`, `signature`. The payload is
/// covered by both the signing payload and the transaction hash. This
/// encoding is incompatible with v0.2 (which had no `payload` field);
/// v0.2 transaction bytes do not decode under v0.3.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct Transaction {
    /// Transaction type.
    pub tx_type: TransactionType,
    /// Sender address (ed25519 public key).
    pub sender: Address,
    /// Receiver address (depends on tx type).
    pub receiver: Address,
    /// Amount (MBO or compute units depending on type).
    pub amount: u128,
    /// Nonce to prevent replay.
    pub nonce: u64,
    /// Typed payload. [`TransactionPayload::None`] for all v0.2 types.
    pub payload: TransactionPayload,
    /// ed25519 signature over the signing payload.
    #[serde(with = "serde_arr64")]
    pub signature: [u8; 64],
}

impl Transaction {
    /// Returns SCALE-encoded signing payload (all fields except signature,
    /// in canonical order — including `payload`).
    #[must_use]
    pub fn signing_payload(&self) -> Vec<u8> {
        #[derive(Encode)]
        struct SigningFields<'a> {
            tx_type: TransactionType,
            sender: Address,
            receiver: Address,
            amount: u128,
            nonce: u64,
            payload: &'a TransactionPayload,
        }
        SigningFields {
            tx_type: self.tx_type,
            sender: self.sender,
            receiver: self.receiver,
            amount: self.amount,
            nonce: self.nonce,
            payload: &self.payload,
        }
        .encode()
    }

    /// Verifies signature using ed25519 and sender's public key.
    #[must_use]
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::{Signature, Verifier};
        let Ok(pk) = ed25519_dalek::VerifyingKey::from_bytes(&self.sender.0) else {
            return false;
        };
        let sig = Signature::from_bytes(&self.signature);
        pk.verify(&self.signing_payload(), &sig).is_ok()
    }
}

/// Block header containing chain linkage and commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct BlockHeader {
    /// Hash of the parent block.
    pub parent_hash: Hash,
    /// State root after executing this block.
    pub state_root: Hash,
    /// Blake3 commitment to the body transactions (see `compute_transactions_root`).
    pub transactions_root: Hash,
    /// Unix timestamp (seconds).
    pub timestamp: u64,
    /// Block height (genesis = 0).
    pub height: u64,
}

/// Block body containing ordered transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, Encode, Decode)]
pub struct BlockBody {
    /// Ordered list of transactions included in the block.
    pub transactions: Vec<Transaction>,
}

/// Full block with header and body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct Block {
    /// Header with metadata and commitments.
    pub header: BlockHeader,
    /// Body with transactions.
    pub body: BlockBody,
}

/// Compute a deterministic commitment over transactions.
/// This is a simple Blake3 hash over SCALE-encoded, length-prefixed transactions.
#[must_use]
pub fn compute_transactions_root(txs: &[Transaction]) -> Hash {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    for tx in txs {
        let encoded = tx.encode();
        // SCALE-encoded transactions are bounded well below u32::MAX bytes.
        #[allow(clippy::cast_possible_truncation)]
        let len = encoded.len() as u32;
        hasher.update(&len.to_le_bytes());
        hasher.update(&encoded);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Hash(out)
}

// Serde helpers for fixed-size 64-byte arrays as hex strings
pub(crate) mod serde_arr64 {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("0x{}", hex::encode(v)))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let s = String::deserialize(d)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom("expected 64 bytes"));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

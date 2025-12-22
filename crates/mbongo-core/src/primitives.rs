use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 32-byte hash used across headers and roots.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Encode, Decode)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// Returns the zero hash (all bytes zero).
    pub const fn zero() -> Self { Self([0u8; 32]) }
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
    pub const fn zero() -> Self { Self([0u8; 32]) }
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum TransactionType {
    /// Simple transfer from sender to receiver of `amount`.
    Transfer,
    /// Compute task assignment/payment.
    ComputeTask,
    /// Stake `amount` to validator or staking contract.
    Stake,
}

/// Transaction structure (SCALE serializable) with ed25519 signature.
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
    /// ed25519 signature over the signing payload.
    pub signature: [u8; 64],
}

impl Transaction {
    /// Returns SCALE-encoded signing payload (all fields except signature).
    pub fn signing_payload(&self) -> Vec<u8> {
        #[derive(Encode)]
        struct Payload {
            tx_type: TransactionType,
            sender: Address,
            receiver: Address,
            amount: u128,
            nonce: u64,
        }
        Payload {
            tx_type: self.tx_type,
            sender: self.sender,
            receiver: self.receiver,
            amount: self.amount,
            nonce: self.nonce,
        }
        .encode()
    }

    /// Verifies signature using ed25519 and sender's public key.
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::{Signature, Verifier};
        let pk = match ed25519_dalek::VerifyingKey::from_bytes(&self.sender.0) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = match Signature::from_bytes(&self.signature) {
            Ok(s) => s,
            Err(_) => return false,
        };
        pk.verify(&self.signing_payload(), &sig).is_ok()
    }
}
    }
}

/// Block header containing chain linkage and commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BlockBody {
    /// Ordered list of transactions included in the block.
    pub transactions: Vec<Transaction>,
}

/// Full block with header and body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Header with metadata and commitments.
    pub header: BlockHeader,
    /// Body with transactions.
    pub body: BlockBody,
}

/// Compute a deterministic commitment over transactions.
/// This is a simple Blake3 hash over SCALE-encoded, length-prefixed transactions.
pub fn compute_transactions_root(txs: &[Transaction]) -> Hash {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    for tx in txs {
    let encoded = tx.encode();
    let len = encoded.len() as u32;
        hasher.update(&len.to_le_bytes());
        hasher.update(&encoded);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Hash(out)
}

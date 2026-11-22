//! Cryptographic primitives for Mbongo Chain.
//!
//! This module provides hashing utilities and Merkle tree construction
//! for securing blocks and transactions.
//!
//! # Supported Hash Functions
//! - **BLAKE3**: Primary hash function, fast and secure (default)
//! - **SHA256**: For compatibility with existing systems
//!
//! # Merkle Tree
//! Binary Merkle tree implementation for transaction commitment.
//!
//! # Example
//! ```rust
//! use mbongo_core::crypto::{blake3_hash, sha256_hash, MerkleTree};
//!
//! // Hash data with BLAKE3
//! let hash = blake3_hash(b"hello world");
//!
//! // Hash data with SHA256
//! let sha_hash = sha256_hash(b"hello world");
//!
//! // Build a Merkle tree
//! let leaves = vec![
//!     blake3_hash(b"tx1"),
//!     blake3_hash(b"tx2"),
//!     blake3_hash(b"tx3"),
//! ];
//! let tree = MerkleTree::new(&leaves);
//! let root = tree.root();
//! ```

use sha2::{Digest as Sha256Digest, Sha256};

/// Hash output size in bytes (32 bytes = 256 bits)
pub const HASH_SIZE: usize = 32;

/// A 32-byte hash output
pub type HashOutput = [u8; HASH_SIZE];

/// Compute BLAKE3 hash of the input data.
///
/// BLAKE3 is the primary hash function for Mbongo Chain due to its
/// excellent performance and security properties.
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// A 32-byte hash output
///
/// # Example
/// ```rust
/// use mbongo_core::crypto::blake3_hash;
///
/// let hash = blake3_hash(b"hello world");
/// assert_eq!(hash.len(), 32);
/// ```
#[must_use]
pub fn blake3_hash(data: &[u8]) -> HashOutput {
    *blake3::hash(data).as_bytes()
}

/// Compute BLAKE3 hash of multiple data slices.
///
/// This is useful for hashing concatenated data without allocating.
///
/// # Arguments
/// * `parts` - Multiple data slices to hash together
///
/// # Returns
/// A 32-byte hash output
#[must_use]
pub fn blake3_hash_multi(parts: &[&[u8]]) -> HashOutput {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

/// Compute SHA256 hash of the input data.
///
/// SHA256 is provided for compatibility with existing blockchain systems
/// and standards (e.g., Bitcoin, Ethereum address derivation).
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// A 32-byte hash output
///
/// # Example
/// ```rust
/// use mbongo_core::crypto::sha256_hash;
///
/// let hash = sha256_hash(b"hello world");
/// assert_eq!(hash.len(), 32);
/// ```
#[must_use]
pub fn sha256_hash(data: &[u8]) -> HashOutput {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut output = [0u8; HASH_SIZE];
    output.copy_from_slice(&result);
    output
}

/// Compute SHA256 hash of multiple data slices.
///
/// # Arguments
/// * `parts` - Multiple data slices to hash together
///
/// # Returns
/// A 32-byte hash output
#[must_use]
pub fn sha256_hash_multi(parts: &[&[u8]]) -> HashOutput {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    let result = hasher.finalize();
    let mut output = [0u8; HASH_SIZE];
    output.copy_from_slice(&result);
    output
}

/// Double SHA256 hash (SHA256(SHA256(data))).
///
/// Used for Bitcoin-compatible hashing where double hashing is required.
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// A 32-byte hash output
#[must_use]
pub fn double_sha256(data: &[u8]) -> HashOutput {
    sha256_hash(&sha256_hash(data))
}

/// A binary Merkle tree for commitment to a list of data elements.
///
/// The tree uses BLAKE3 for internal hashing and supports proof generation
/// and verification.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// All nodes in the tree, stored level by level (leaves first, root last)
    nodes: Vec<HashOutput>,
    /// Number of leaves in the tree
    leaf_count: usize,
}

impl MerkleTree {
    /// Construct a new Merkle tree from a list of leaf hashes.
    ///
    /// If the number of leaves is not a power of 2, the tree is padded
    /// with zero hashes to the next power of 2.
    ///
    /// # Arguments
    /// * `leaves` - The leaf hashes to include in the tree
    ///
    /// # Returns
    /// A new `MerkleTree` instance
    ///
    /// # Panics
    /// Never panics, handles empty input gracefully.
    #[must_use]
    pub fn new(leaves: &[HashOutput]) -> Self {
        if leaves.is_empty() {
            return Self {
                nodes: vec![[0u8; HASH_SIZE]],
                leaf_count: 0,
            };
        }

        // Pad to next power of 2 (minimum 2 so a single leaf is paired with zero)
        let leaf_count = leaves.len();
        let padded_count = leaf_count.next_power_of_two().max(2);

        let mut nodes = Vec::with_capacity(2 * padded_count - 1);

        // Add leaves
        nodes.extend_from_slice(leaves);

        // Pad with zero hashes if necessary
        for _ in leaf_count..padded_count {
            nodes.push([0u8; HASH_SIZE]);
        }

        // Build tree bottom-up
        let mut level_start = 0;
        let mut level_size = padded_count;

        while level_size > 1 {
            let level_end = level_start + level_size;
            for i in (level_start..level_end).step_by(2) {
                let left = &nodes[i];
                let right = &nodes[i + 1];
                let parent = hash_pair(left, right);
                nodes.push(parent);
            }
            level_start = level_end;
            level_size /= 2;
        }

        Self { nodes, leaf_count }
    }

    /// Get the Merkle root of the tree.
    ///
    /// # Returns
    /// The root hash of the tree
    #[must_use]
    pub fn root(&self) -> HashOutput {
        *self.nodes.last().unwrap_or(&[0u8; HASH_SIZE])
    }

    /// Get the number of leaves in the tree.
    #[must_use]
    pub const fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    /// Generate a Merkle proof for a leaf at the given index.
    ///
    /// # Arguments
    /// * `leaf_index` - The index of the leaf to prove
    ///
    /// # Returns
    /// `Some(proof)` if the index is valid, `None` otherwise.
    /// The proof is a vector of sibling hashes from leaf to root.
    #[must_use]
    pub fn proof(&self, leaf_index: usize) -> Option<MerkleProof> {
        if leaf_index >= self.leaf_count || self.leaf_count == 0 {
            return None;
        }

        let padded_count = self.leaf_count.next_power_of_two();
        let mut siblings = Vec::new();
        let mut indices = Vec::new();
        let mut idx = leaf_index;
        let mut level_start = 0;
        let mut level_size = padded_count;

        while level_size > 1 {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            siblings.push(self.nodes[level_start + sibling_idx]);
            indices.push(idx % 2 == 0); // true if leaf is on left

            level_start += level_size;
            level_size /= 2;
            idx /= 2;
        }

        Some(MerkleProof {
            leaf_index,
            siblings,
            indices,
        })
    }

    /// Verify that a leaf is part of the tree.
    ///
    /// # Arguments
    /// * `leaf` - The leaf hash to verify
    /// * `proof` - The Merkle proof for the leaf
    ///
    /// # Returns
    /// `true` if the proof is valid, `false` otherwise
    #[must_use]
    pub fn verify(&self, leaf: &HashOutput, proof: &MerkleProof) -> bool {
        verify_proof(leaf, proof, &self.root())
    }
}

/// A Merkle proof for a single leaf.
#[derive(Debug, Clone)]
pub struct MerkleProof {
    /// The index of the leaf in the original list
    pub leaf_index: usize,
    /// Sibling hashes from leaf to root
    pub siblings: Vec<HashOutput>,
    /// Direction indicators: true if the current node is on the left
    pub indices: Vec<bool>,
}

/// Verify a Merkle proof against a known root.
///
/// # Arguments
/// * `leaf` - The leaf hash to verify
/// * `proof` - The Merkle proof
/// * `root` - The expected root hash
///
/// # Returns
/// `true` if the proof is valid, `false` otherwise
#[must_use]
pub fn verify_proof(leaf: &HashOutput, proof: &MerkleProof, root: &HashOutput) -> bool {
    let mut current = *leaf;

    for (sibling, is_left) in proof.siblings.iter().zip(&proof.indices) {
        current = if *is_left {
            hash_pair(&current, sibling)
        } else {
            hash_pair(sibling, &current)
        };
    }

    current == *root
}

/// Hash two nodes together to produce a parent node.
///
/// Uses BLAKE3 for internal tree hashing.
#[inline]
fn hash_pair(left: &HashOutput, right: &HashOutput) -> HashOutput {
    blake3_hash_multi(&[left, right])
}

/// Compute the Merkle root directly from leaf hashes without storing the tree.
///
/// This is more memory-efficient when only the root is needed.
///
/// # Arguments
/// * `leaves` - The leaf hashes
///
/// # Returns
/// The Merkle root hash
#[must_use]
pub fn compute_merkle_root(leaves: &[HashOutput]) -> HashOutput {
    if leaves.is_empty() {
        return [0u8; HASH_SIZE];
    }

    // Pad to next power of 2 (minimum 2 so a single leaf is paired with zero)
    let padded_count = leaves.len().next_power_of_two().max(2);
    let mut level: Vec<HashOutput> = leaves.to_vec();
    level.resize(padded_count, [0u8; HASH_SIZE]);

    while level.len() > 1 {
        let mut next_level = Vec::with_capacity(level.len() / 2);
        for chunk in level.chunks(2) {
            next_level.push(hash_pair(&chunk[0], &chunk[1]));
        }
        level = next_level;
    }

    level[0]
}

/// Convert a hash to a hexadecimal string.
#[must_use]
pub fn hash_to_hex(hash: &HashOutput) -> String {
    hex::encode(hash)
}

/// Parse a hexadecimal string to a hash.
///
/// # Errors
/// Returns `Err` if the string is not valid hex or not 64 characters.
pub fn hex_to_hash(s: &str) -> Result<HashOutput, HexError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 64 {
        return Err(HexError::InvalidLength(s.len()));
    }
    let bytes = hex::decode(s).map_err(|_| HexError::InvalidHex)?;
    let mut output = [0u8; HASH_SIZE];
    output.copy_from_slice(&bytes);
    Ok(output)
}

/// Error type for hex parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexError {
    /// Invalid hex string length
    InvalidLength(usize),
    /// Invalid hex characters
    InvalidHex,
}

impl std::fmt::Display for HexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLength(len) => write!(f, "invalid hash length: {len}, expected 64"),
            Self::InvalidHex => write!(f, "invalid hex characters"),
        }
    }
}

impl std::error::Error for HexError {}

// ============================================================================
// Strict Ed25519 verification (RFC 0006 §3)
// ============================================================================

/// The Ed25519 group order `L = 2^252 + 27742317777372353535851937790883648493`,
/// little-endian.
const GROUP_ORDER_LE: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

/// `y = 1`, the identity's `y`, little-endian with the sign bit clear.
const Y_ONE: [u8; 32] = {
    let mut y = [0u8; 32];
    y[0] = 1;
    y
};

/// `y = p - 1 = 2^255 - 20`, little-endian with the sign bit clear.
const Y_P_MINUS_ONE: [u8; 32] = {
    let mut y = [0xffu8; 32];
    y[0] = 0xec;
    y[31] = 0x7f;
    y
};

/// Whether a 32-byte little-endian scalar is below the group order.
fn scalar_is_canonical(s: &[u8; 32]) -> bool {
    for i in (0..32).rev() {
        if s[i] != GROUP_ORDER_LE[i] {
            return s[i] < GROUP_ORDER_LE[i];
        }
    }
    false // equal to L
}

/// Whether a point encoding is canonical (RFC 8032 §5.1.3): `y < p`, and
/// the sign bit is not set when `x = 0`, which happens only for `y = 1` and
/// `y = p - 1`.
fn point_encoding_is_canonical(bytes: &[u8; 32]) -> bool {
    let sign = bytes[31] >> 7;
    let mut y = *bytes;
    y[31] &= 0x7f;
    // y >= p = 2^255 - 19 exactly when the top 31 bytes are all ones
    // (bit 255 cleared) and the lowest byte is at least 0xed.
    let at_least_p = y[31] == 0x7f && y[1..31].iter().all(|b| *b == 0xff) && y[0] >= 0xed;
    if at_least_p {
        return false;
    }
    !(sign == 1 && (y == Y_ONE || y == Y_P_MINUS_ONE))
}

/// Verifies an Ed25519 signature by the strict definition of RFC 0006 §3,
/// which §2 applies to transaction and receipt signatures.
///
/// A signature is valid only if all of these hold:
/// - `s` is below the group order;
/// - the public key and `R` are canonical encodings;
/// - neither the public key nor `R` is of small order;
/// - the verification equation holds without the cofactor.
///
/// The last two checks are those of `ed25519-dalek`'s `verify_strict`. It
/// does not check canonical encoding, so that check is made here first.
/// Any malformed input is a `false` verdict, never a panic.
#[must_use]
pub fn verify_ed25519_strict(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let mut r = [0u8; 32];
    r.copy_from_slice(&signature[..32]);
    let mut s = [0u8; 32];
    s.copy_from_slice(&signature[32..]);
    if !scalar_is_canonical(&s)
        || !point_encoding_is_canonical(public_key)
        || !point_encoding_is_canonical(&r)
    {
        return false;
    }
    let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(public_key) else {
        return false;
    };
    key.verify_strict(message, &ed25519_dalek::Signature::from_bytes(signature))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known test vectors for BLAKE3
    // From: https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json
    #[test]
    fn blake3_empty_input() {
        let hash = blake3_hash(b"");
        let expected = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
        assert_eq!(hash_to_hex(&hash), expected);
    }

    #[test]
    fn blake3_hello_world() {
        let hash = blake3_hash(b"hello world");
        // BLAKE3 hash of "hello world"
        let expected = "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24";
        assert_eq!(hash_to_hex(&hash), expected);
    }

    #[test]
    fn blake3_multi_matches_concat() {
        let data1 = b"hello ";
        let data2 = b"world";
        let concat = b"hello world";

        let hash_multi = blake3_hash_multi(&[data1, data2]);
        let hash_concat = blake3_hash(concat);

        assert_eq!(hash_multi, hash_concat);
    }

    // Known test vectors for SHA256
    // From: https://www.di-mgt.com.au/sha_testvectors.html
    #[test]
    fn sha256_empty_input() {
        let hash = sha256_hash(b"");
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(hash_to_hex(&hash), expected);
    }

    #[test]
    fn sha256_abc() {
        let hash = sha256_hash(b"abc");
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(hash_to_hex(&hash), expected);
    }

    #[test]
    fn sha256_448_bits() {
        // "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        let input = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        let hash = sha256_hash(input);
        let expected = "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1";
        assert_eq!(hash_to_hex(&hash), expected);
    }

    #[test]
    fn sha256_multi_matches_concat() {
        let data1 = b"hello ";
        let data2 = b"world";
        let concat = b"hello world";

        let hash_multi = sha256_hash_multi(&[data1, data2]);
        let hash_concat = sha256_hash(concat);

        assert_eq!(hash_multi, hash_concat);
    }

    #[test]
    fn double_sha256_test() {
        let hash = double_sha256(b"hello");
        // SHA256(SHA256("hello"))
        let inner = sha256_hash(b"hello");
        let expected = sha256_hash(&inner);
        assert_eq!(hash, expected);
    }

    // Merkle tree tests
    #[test]
    fn merkle_empty_tree() {
        let tree = MerkleTree::new(&[]);
        assert_eq!(tree.leaf_count(), 0);
        assert_eq!(tree.root(), [0u8; HASH_SIZE]);
    }

    #[test]
    fn merkle_single_leaf() {
        let leaf = blake3_hash(b"single");
        let tree = MerkleTree::new(&[leaf]);
        assert_eq!(tree.leaf_count(), 1);
        // Single leaf tree: root = hash(leaf || zero)
        let expected_root = hash_pair(&leaf, &[0u8; HASH_SIZE]);
        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn merkle_two_leaves() {
        let leaf1 = blake3_hash(b"tx1");
        let leaf2 = blake3_hash(b"tx2");
        let tree = MerkleTree::new(&[leaf1, leaf2]);

        assert_eq!(tree.leaf_count(), 2);
        let expected_root = hash_pair(&leaf1, &leaf2);
        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn merkle_four_leaves() {
        let leaves: Vec<HashOutput> =
            (0..4).map(|i| blake3_hash(format!("tx{i}").as_bytes())).collect();

        let tree = MerkleTree::new(&leaves);
        assert_eq!(tree.leaf_count(), 4);

        // Manual calculation
        let h01 = hash_pair(&leaves[0], &leaves[1]);
        let h23 = hash_pair(&leaves[2], &leaves[3]);
        let expected_root = hash_pair(&h01, &h23);

        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn merkle_three_leaves_padded() {
        let leaves: Vec<HashOutput> =
            (0..3).map(|i| blake3_hash(format!("tx{i}").as_bytes())).collect();

        let tree = MerkleTree::new(&leaves);
        assert_eq!(tree.leaf_count(), 3);

        // Should be padded to 4 leaves
        let zero = [0u8; HASH_SIZE];
        let h01 = hash_pair(&leaves[0], &leaves[1]);
        let h23 = hash_pair(&leaves[2], &zero);
        let expected_root = hash_pair(&h01, &h23);

        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn merkle_proof_and_verify() {
        let leaves: Vec<HashOutput> =
            (0..4).map(|i| blake3_hash(format!("tx{i}").as_bytes())).collect();

        let tree = MerkleTree::new(&leaves);

        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.proof(i).expect("proof should exist");
            assert!(
                tree.verify(leaf, &proof),
                "proof for leaf {i} should verify"
            );
        }
    }

    #[test]
    fn merkle_proof_invalid_index() {
        let leaves: Vec<HashOutput> =
            (0..4).map(|i| blake3_hash(format!("tx{i}").as_bytes())).collect();

        let tree = MerkleTree::new(&leaves);
        assert!(tree.proof(4).is_none());
        assert!(tree.proof(100).is_none());
    }

    #[test]
    fn merkle_proof_tampered_fails() {
        let leaves: Vec<HashOutput> =
            (0..4).map(|i| blake3_hash(format!("tx{i}").as_bytes())).collect();

        let tree = MerkleTree::new(&leaves);
        let proof = tree.proof(0).unwrap();

        // Try to verify with a different leaf
        let fake_leaf = blake3_hash(b"fake");
        assert!(!tree.verify(&fake_leaf, &proof));
    }

    #[test]
    fn compute_merkle_root_matches_tree() {
        let leaves: Vec<HashOutput> =
            (0..7).map(|i| blake3_hash(format!("tx{i}").as_bytes())).collect();

        let tree = MerkleTree::new(&leaves);
        let root = compute_merkle_root(&leaves);

        assert_eq!(tree.root(), root);
    }

    #[test]
    fn hex_roundtrip() {
        let original = blake3_hash(b"test");
        let hex_str = hash_to_hex(&original);
        let parsed = hex_to_hash(&hex_str).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn hex_with_prefix() {
        let original = blake3_hash(b"test");
        let hex_str = format!("0x{}", hash_to_hex(&original));
        let parsed = hex_to_hash(&hex_str).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn hex_invalid_length() {
        assert!(matches!(
            hex_to_hash("abc"),
            Err(HexError::InvalidLength(3))
        ));
    }

    #[test]
    fn hex_invalid_chars() {
        let invalid = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
        assert!(matches!(hex_to_hash(invalid), Err(HexError::InvalidHex)));
    }

    // ── Strict Ed25519: the encoding checks at their boundaries ────────

    /// `2^255 - 19 + offset`, little-endian, sign bit clear, for `offset` in 0..19.
    fn y_at_least_p(offset: u8) -> [u8; 32] {
        let mut y = [0xffu8; 32];
        y[0] = 0xed + offset;
        y[31] = 0x7f;
        y
    }

    #[test]
    fn scalar_canonical_at_the_group_order() {
        assert!(scalar_is_canonical(&[0u8; 32]));
        let mut below = GROUP_ORDER_LE;
        below[0] -= 1;
        assert!(scalar_is_canonical(&below), "L - 1");
        assert!(!scalar_is_canonical(&GROUP_ORDER_LE), "L");
        let mut above = GROUP_ORDER_LE;
        above[0] += 1;
        assert!(!scalar_is_canonical(&above), "L + 1");
        assert!(!scalar_is_canonical(&[0xffu8; 32]), "2^256 - 1");
    }

    #[test]
    fn point_encoding_canonical_at_the_field_prime() {
        // y = p - 1 is canonical with the sign bit clear.
        assert!(point_encoding_is_canonical(&Y_P_MINUS_ONE));
        // Every y in p ..= 2^255 - 1 is not, with either sign bit.
        for offset in 0..19 {
            let mut y = y_at_least_p(offset);
            assert!(!point_encoding_is_canonical(&y), "p + {offset}");
            y[31] |= 0x80;
            assert!(
                !point_encoding_is_canonical(&y),
                "p + {offset}, sign bit set"
            );
        }
    }

    #[test]
    fn point_encoding_refuses_a_sign_bit_on_x_zero() {
        // x = 0 exactly for y = 1 and y = p - 1.
        for y in [Y_ONE, Y_P_MINUS_ONE] {
            assert!(point_encoding_is_canonical(&y));
            let mut signed = y;
            signed[31] |= 0x80;
            assert!(!point_encoding_is_canonical(&signed));
        }
        // Any other y keeps its sign bit as part of a canonical encoding.
        let mut y2 = [0u8; 32];
        y2[0] = 2;
        y2[31] = 0x80;
        assert!(point_encoding_is_canonical(&y2));
    }

    #[test]
    fn strict_verification_accepts_an_honest_signature_and_refuses_its_variants() {
        use ed25519_dalek::{Signer, SigningKey};
        let sk = SigningKey::from_bytes(&[0x2au8; 32]);
        let key = sk.verifying_key().to_bytes();
        let sig = sk.sign(b"mbongo").to_bytes();
        assert!(verify_ed25519_strict(&key, b"mbongo", &sig));
        assert!(
            !verify_ed25519_strict(&key, b"mbongO", &sig),
            "another message"
        );
        let mut flipped = sig;
        flipped[0] ^= 1;
        assert!(
            !verify_ed25519_strict(&key, b"mbongo", &flipped),
            "another R"
        );
        let mut high = sig;
        high[63] |= 0x80;
        assert!(!verify_ed25519_strict(&key, b"mbongo", &high), "s >= L");
        assert!(
            !verify_ed25519_strict(&[0u8; 32], b"mbongo", &sig),
            "small-order key"
        );
        assert!(
            !verify_ed25519_strict(&y_at_least_p(0), b"mbongo", &sig),
            "key y = p"
        );
    }
}

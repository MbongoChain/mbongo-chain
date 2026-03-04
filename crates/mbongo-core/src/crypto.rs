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
}

//! Storage module for core primitives
//! 
//! This module provides a Merkle Patricia Trie implementation for state storage
//! with insert/get/delete, state root computation, and proof generation.

pub mod trie;

pub(crate) mod util {
    use blake3::Hasher;

    pub fn hash_bytes(data: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        let mut h = Hasher::new();
        h.update(data);
        out.copy_from_slice(h.finalize().as_bytes());
        out
    }

    // Convert bytes to nibbles (0..=15 per element)
    pub fn bytes_to_nibbles(bytes: &[u8]) -> Vec<u8> {
        let mut nibbles = Vec::with_capacity(bytes.len() * 2);
        for b in bytes {
            nibbles.push(b >> 4);
            nibbles.push(b & 0x0f);
        }
        nibbles
    }

    pub fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
        let mut i = 0;
        while i < a.len() && i < b.len() && a[i] == b[i] {
            i += 1;
        }
        i
    }
}

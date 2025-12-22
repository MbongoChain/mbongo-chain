use crate::primitives::Hash;
use parity_scale_codec::{Decode, Encode};
use std::collections::HashMap;

use super::util::{bytes_to_nibbles, common_prefix_len, hash_bytes};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum Node {
    Branch {
        // 16 children (hexary), each by node hash if present
        children: [Option<Hash>; 16],
        value: Option<Vec<u8>>, // optional value at this node
    },
    Extension {
        key: Vec<u8>, // nibble-encoded path (values 0..=15)
        child: Hash,
    },
    Leaf {
        key: Vec<u8>,   // nibble-encoded path (remaining nibbles)
        value: Vec<u8>,
    },
}

fn node_hash(node: &Node) -> Hash {
    let enc = node.encode();
    Hash(hash_bytes(&enc))
}

fn empty_children() -> [Option<Hash>; 16] {
    [(); 16].map(|_| None)
}

/// Proof node captured during proof generation
#[derive(Clone, Debug)]
pub struct ProofNode {
    pub hash: Hash,
    pub encoded: Vec<u8>,
}

/// Persistent backing store for nodes. For now, we provide in-memory by default
/// and an optional RocksDB-backed implementation.
trait NodeStore {
    fn get(&self, h: &Hash) -> Option<Node>;
    fn put(&mut self, node: &Node) -> Hash;
    fn delete(&mut self, h: &Hash);
}

#[derive(Default)]
struct MemoryStore {
    // map hash -> encoded bytes
    map: HashMap<Hash, Vec<u8>>,
}

impl NodeStore for MemoryStore {
    fn get(&self, h: &Hash) -> Option<Node> {
        self.map.get(h).and_then(|b| Node::decode(&mut &b[..]).ok())
    }
    fn put(&mut self, node: &Node) -> Hash {
        let h = node_hash(node);
        self.map.insert(h, node.encode());
        h
    }
    fn delete(&mut self, h: &Hash) {
        self.map.remove(h);
    }
}

#[cfg(feature = "rocksdb")] // not enabled by default, but code compiles without feature gates
struct RocksStore {
    db: rocksdb::DB,
}

#[cfg(feature = "rocksdb")]
impl RocksStore {
    fn open(path: &str) -> anyhow::Result<Self> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        Ok(Self { db: rocksdb::DB::open(&opts, path)? })
    }
}

#[cfg(feature = "rocksdb")]
impl NodeStore for RocksStore {
    fn get(&self, h: &Hash) -> Option<Node> {
        let k = h.0;
        self.db.get(k).ok().flatten().and_then(|b| Node::decode(&mut &b[..]).ok())
    }
    fn put(&mut self, node: &Node) -> Hash {
        let enc = node.encode();
        let h = Hash(hash_bytes(&enc));
        let _ = self.db.put(h.0, &enc);
        h
    }
    fn delete(&mut self, h: &Hash) {
        let _ = self.db.delete(h.0);
    }
}

/// Merkle Patricia Trie over hex nibbles, compatible in spirit with Ethereum's MPT but
/// using SCALE encoding for internal nodes and BLAKE3 for hashing.
pub struct MerklePatriciaTrie {
    root: Option<Hash>,
    store: Box<dyn NodeStore>,
}

impl Default for MerklePatriciaTrie {
    fn default() -> Self {
        Self { root: None, store: Box::<MemoryStore>::default() }
    }
}

impl MerklePatriciaTrie {
    pub fn with_memory() -> Self { Self::default() }

    #[cfg(feature = "rocksdb")]
    pub fn with_rocksdb(path: &str) -> anyhow::Result<Self> {
        Ok(Self { root: None, store: Box::new(RocksStore::open(path)?), })
    }

    pub fn root(&self) -> Hash { self.root.unwrap_or_default() }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let nibbles = bytes_to_nibbles(key);
        let mut cur = self.root?;
        let mut idx_path = &nibbles[..];
        loop {
            let node = self.store.get(&cur)?;
            match node {
                Node::Branch { children, value } => {
                    if idx_path.is_empty() {
                        return value;
                    }
                    let nib = idx_path[0] as usize;
                    idx_path = &idx_path[1..];
                    if let Some(child) = children[nib] {
                        cur = child;
                    } else {
                        return None;
                    }
                }
                Node::Extension { key, child } => {
                    let l = common_prefix_len(&key, idx_path);
                    if l != key.len() { return None; }
                    idx_path = &idx_path[l..];
                    cur = child;
                }
                Node::Leaf { key, value } => {
                    if key == idx_path { return Some(value); }
                    return None;
                }
            }
        }
    }

    pub fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        let nibbles = bytes_to_nibbles(key);
        let new_root = match self.root {
            None => {
                let leaf = Node::Leaf { key: nibbles, value };
                self.store.put(&leaf)
            }
            Some(r) => self.insert_at(r, &nibbles, value),
        };
        self.root = Some(new_root);
    }

    fn insert_at(&mut self, node_hash: Hash, path: &[u8], value: Vec<u8>) -> Hash {
        let node = self.store.get(&node_hash).expect("node must exist");
        match node {
            Node::Leaf { key, value: old_val } => {
                let l = common_prefix_len(&key, path);
                if l == key.len() && l == path.len() {
                    // replace leaf value
                    let new_leaf = Node::Leaf { key, value };
                    self.store.put(&new_leaf)
                } else {
                    // split leaf into branch (with optional extension)
                    let mut branch = Node::Branch { children: empty_children(), value: None };
                    // existing leaf remainder
                    if l == key.len() && l < path.len() {
                        // old leaf at child of branch
                        let child_nib = path[l] as usize;
                        let new_leaf = Node::Leaf { key: path[l + 1..].to_vec(), value };
                        let new_leaf_h = self.store.put(&new_leaf);
                        if let Node::Branch { children, .. } = &mut branch { children[child_nib] = Some(new_leaf_h); }

                        let old_nib = 16; // sentinel for branch value
                        drop(old_nib);

                        let old_child_nib = key[l] as usize;
                        let old_leaf = Node::Leaf { key: key[l + 1..].to_vec(), value: old_val };
                        let old_leaf_h = self.store.put(&old_leaf);
                        if let Node::Branch { children, .. } = &mut branch { children[old_child_nib] = Some(old_leaf_h); }
                    } else if l < key.len() && l == path.len() {
                        // new value becomes branch value at exact match
                        if let Node::Branch { value: v, .. } = &mut branch { *v = Some(value); }
                        let old_child_nib = key[l] as usize;
                        let old_leaf = Node::Leaf { key: key[l + 1..].to_vec(), value: old_val };
                        let old_leaf_h = self.store.put(&old_leaf);
                        if let Node::Branch { children, .. } = &mut branch { children[old_child_nib] = Some(old_leaf_h); }
                    } else {
                        // both have remainders
                        let a_nib = path[l] as usize;
                        let b_nib = key[l] as usize;
                        let a_leaf = Node::Leaf { key: path[l + 1..].to_vec(), value };
                        let b_leaf = Node::Leaf { key: key[l + 1..].to_vec(), value: old_val };
                        let a_h = self.store.put(&a_leaf);
                        let b_h = self.store.put(&b_leaf);
                        if let Node::Branch { children, .. } = &mut branch {
                            children[a_nib] = Some(a_h);
                            children[b_nib] = Some(b_h);
                        }
                    }

                    let branch_h = self.store.put(&branch);
                    if l == 0 {
                        branch_h
                    } else {
                        // prefix into extension
                        let ext = Node::Extension { key: path[..l].to_vec(), child: branch_h };
                        self.store.put(&ext)
                    }
                }
            }
            Node::Extension { key, child } => {
                let l = common_prefix_len(&key, path);
                if l == key.len() {
                    let new_child = self.insert_at(child, &path[l..], value);
                    let new_ext = Node::Extension { key, child: new_child };
                    self.store.put(&new_ext)
                } else {
                    // split extension
                    let mut branch = Node::Branch { children: empty_children(), value: None };
                    // existing child on key remainder
                    let old_nib = key[l] as usize;
                    let old_suffix = if l + 1 <= key.len() { key[l + 1..].to_vec() } else { vec![] };
                    let old_node = if old_suffix.is_empty() { Node::Extension { key: vec![], child } } else { Node::Extension { key: old_suffix, child } };
                    let old_h = self.store.put(&old_node);
                    if let Node::Branch { children, .. } = &mut branch { children[old_nib] = Some(old_h); }

                    // new child on path remainder
                    if l == path.len() {
                        if let Node::Branch { value: v, .. } = &mut branch { *v = Some(value); }
                    } else {
                        let new_nib = path[l] as usize;
                        let new_leaf = Node::Leaf { key: path[l + 1..].to_vec(), value };
                        let new_h = self.store.put(&new_leaf);
                        if let Node::Branch { children, .. } = &mut branch { children[new_nib] = Some(new_h); }
                    }

                    let branch_h = self.store.put(&branch);
                    if l == 0 {
                        branch_h
                    } else {
                        let ext = Node::Extension { key: path[..l].to_vec(), child: branch_h };
                        self.store.put(&ext)
                    }
                }
            }
            Node::Branch { mut children, mut value: val } => {
                if path.is_empty() {
                    val = Some(value);
                } else {
                    let nib = path[0] as usize;
                    let child_h = if let Some(c) = children[nib] {
                        self.insert_at(c, &path[1..], value)
                    } else {
                        let leaf = Node::Leaf { key: path[1..].to_vec(), value };
                        self.store.put(&leaf)
                    };
                    children[nib] = Some(child_h);
                }
                let new_branch = Node::Branch { children, value: val };
                self.store.put(&new_branch)
            }
        }
    }

    pub fn delete(&mut self, key: &[u8]) -> bool {
        let nibbles = bytes_to_nibbles(key);
        if self.root.is_none() { return false; }
        let (changed, new_root) = self.delete_at(self.root.unwrap(), &nibbles);
        if changed { self.root = new_root; }
        changed
    }

    // returns (changed, new_root)
    fn delete_at(&mut self, node_hash: Hash, path: &[u8]) -> (bool, Option<Hash>) {
        let node = match self.store.get(&node_hash) { Some(n) => n, None => return (false, Some(node_hash)) };
        match node {
            Node::Leaf { key, .. } => {
                if key == path { return (true, None); }
                (false, Some(node_hash))
            }
            Node::Extension { key, child } => {
                let l = common_prefix_len(&key, path);
                if l != key.len() { return (false, Some(node_hash)); }
                let (changed, child_new) = self.delete_at(child, &path[l..]);
                if !changed { return (false, Some(node_hash)); }
                match child_new {
                    None => {
                        // extension to nothing disappears
                        (true, None)
                    }
                    Some(ch) => {
                        // If child is extension, merge keys
                        if let Some(Node::Extension { key: ck, child: gc }) = self.store.get(&ch) {
                            let mut merged = key.clone();
                            merged.extend_from_slice(&ck);
                            let ext = Node::Extension { key: merged, child: gc };
                            (true, Some(self.store.put(&ext)))
                        } else {
                            let ext = Node::Extension { key, child: ch };
                            (true, Some(self.store.put(&ext)))
                        }
                    }
                }
            }
            Node::Branch { mut children, mut value } => {
                if path.is_empty() {
                    if value.is_none() { return (false, Some(node_hash)); }
                    value = None;
                } else {
                    let nib = path[0] as usize;
                    if let Some(ch) = children[nib] {
                        let (changed, child_new) = self.delete_at(ch, &path[1..]);
                        if !changed { return (false, Some(node_hash)); }
                        children[nib] = child_new;
                    } else {
                        return (false, Some(node_hash));
                    }
                }
                // compress if possible
                let mut count = if value.is_some() { 1 } else { 0 };
                let mut last_idx = 0usize;
                for i in 0..16 {
                    if children[i].is_some() { count += 1; last_idx = i; }
                }
                if count > 1 {
                    let new_b = Node::Branch { children, value };
                    (true, Some(self.store.put(&new_b)))
                } else if count == 1 {
                    // collapse
                    if value.is_some() {
                        // only value -> convert to leaf with empty key
                        let leaf = Node::Leaf { key: vec![], value: value.unwrap() };
                        (true, Some(self.store.put(&leaf)))
                    } else {
                        // single child; merge with possible extension/leaf
                        let ch = children[last_idx].unwrap();
                        if let Some(Node::Leaf { mut key, value }) = self.store.get(&ch) {
                            let mut new_key = vec![last_idx as u8];
                            new_key.append(&mut key);
                            let leaf = Node::Leaf { key: new_key, value };
                            (true, Some(self.store.put(&leaf)))
                        } else if let Some(Node::Extension { mut key, child }) = self.store.get(&ch) {
                            let mut new_key = vec![last_idx as u8];
                            new_key.append(&mut key);
                            let ext = Node::Extension { key: new_key, child };
                            (true, Some(self.store.put(&ext)))
                        } else {
                            // child is branch
                            let ext = Node::Extension { key: vec![last_idx as u8], child: ch };
                            (true, Some(self.store.put(&ext)))
                        }
                    }
                } else {
                    // empty
                    (true, None)
                }
            }
        }
    }

    /// Returns a proof as a sequence of nodes (hash + encoded) encountered along the path.
    pub fn get_proof(&self, key: &[u8]) -> Option<Vec<ProofNode>> {
        let mut proof = Vec::new();
        let mut cur = self.root?;
        let mut path = bytes_to_nibbles(key);
        loop {
            let node = self.store.get(&cur)?;
            let enc = node.encode();
            proof.push(ProofNode { hash: node_hash(&node), encoded: enc });
            match node {
                Node::Branch { children, .. } => {
                    if path.is_empty() { return Some(proof); }
                    let nib = path.remove(0) as usize;
                    if let Some(ch) = children[nib] { cur = ch; } else { return Some(proof); }
                }
                Node::Extension { key, child } => {
                    let l = common_prefix_len(&key, &path);
                    if l != key.len() { return Some(proof); }
                    path.drain(0..l);
                    cur = child;
                }
                Node::Leaf { key, .. } => {
                    // terminal
                    let _ = key;
                    return Some(proof);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_insert_get() {
        let mut t = MerklePatriciaTrie::with_memory();
        assert_eq!(t.root(), Hash::default());
        t.insert(b"dog", b"puppy".to_vec());
        t.insert(b"do", b"verb".to_vec());
        t.insert(b"doge", b"coin".to_vec());
        assert_eq!(t.get(b"do"), Some(b"verb".to_vec()));
        assert_eq!(t.get(b"dog"), Some(b"puppy".to_vec()));
        assert_eq!(t.get(b"doge"), Some(b"coin".to_vec()));
        assert_eq!(t.get(b"cat"), None);
        assert_ne!(t.root(), Hash::default());
    }

    #[test]
    fn overwrite_and_delete() {
        let mut t = MerklePatriciaTrie::with_memory();
        t.insert(b"dog", b"puppy".to_vec());
        t.insert(b"dog", b"canine".to_vec());
        assert_eq!(t.get(b"dog"), Some(b"canine".to_vec()));
        assert!(t.delete(b"dog"));
        assert_eq!(t.get(b"dog"), None);
    }

    #[test]
    fn proof_generation_nonempty() {
        let mut t = MerklePatriciaTrie::with_memory();
        t.insert(b"alice", b"1".to_vec());
        t.insert(b"bob", b"2".to_vec());
        let proof = t.get_proof(b"alice").unwrap();
        assert!(!proof.is_empty());
        // First node must hash to its own encoding
        let first = &proof[0];
        assert_eq!(first.hash, Hash(hash_bytes(&first.encoded)));
    }
}

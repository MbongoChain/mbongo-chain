//! The state commitment (RFC 0006 §4).
//!
//! `state_root` is a hash chain over what each block changes, not a Merkle
//! root:
//!
//! ```text
//! state_root(0) = BLAKE3("mbongo:state:v1" || chain_id || SCALE(allocations))
//! state_root(h) = BLAKE3("mbongo:state:v1" || state_root(h-1) || SCALE(changes))
//! ```
//!
//! `allocations` is `Vec<(Address, u128)>` in file order, which RFC 0006 §1
//! makes ascending by address. `changes` is `Vec<(u8, [u8; 32], Vec<u8>)>`,
//! sorted ascending by kind, then by key, compared bytewise, with one entry
//! per key holding the value after the whole block:
//!
//! | Kind | Key | Value |
//! |---|---|---|
//! | 0 | Address | `SCALE(Account)` |
//! | 1 | `task_id` | `SCALE(ComputeTask)` |
//! | 2 | `task_id` | `SCALE(Receipt)` |
//!
//! Which entries a block has is decided by block execution, not here: this
//! module only holds them and hashes them. `SCALE(Account)` is a consensus
//! input from here on, so the [`Account`] encoding is locked.

use std::collections::BTreeMap;

use parity_scale_codec::Encode;

use crate::{Account, Address, ChainId, ComputeTask, Hash, Receipt};

/// The domain of every state root.
pub const STATE_DOMAIN: &[u8] = b"mbongo:state:v1";

/// Kind of an account entry: key the address, value `SCALE(Account)`.
pub const KIND_ACCOUNT: u8 = 0;
/// Kind of a task entry: key the `task_id`, value `SCALE(ComputeTask)`.
pub const KIND_TASK: u8 = 1;
/// Kind of a receipt entry: key the `task_id`, value `SCALE(Receipt)`.
pub const KIND_RECEIPT: u8 = 2;

/// What one block changes, one entry per `(kind, key)`.
///
/// Ordered by kind, then by key bytewise, which is the order of the
/// `(u8, [u8; 32])` map key, so iteration is already the canonical order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateChanges {
    entries: BTreeMap<(u8, [u8; 32]), Vec<u8>>,
}

impl StateChanges {
    /// No changes: the changes of an empty block.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `value` under `(kind, key)`, replacing an earlier value: an
    /// entry holds the value after the whole block.
    pub fn insert(&mut self, kind: u8, key: [u8; 32], value: Vec<u8>) {
        self.entries.insert((kind, key), value);
    }

    /// Records the state of the account at `address` after the block. The
    /// key is the address the state maps, never a field read back from the
    /// value.
    pub fn account(&mut self, address: &Address, account: &Account) {
        self.insert(KIND_ACCOUNT, address.0, account.encode());
    }

    /// Records a task the block registers.
    pub fn task(&mut self, task_id: [u8; 32], task: &ComputeTask) {
        self.insert(KIND_TASK, task_id, task.encode());
    }

    /// Records a receipt the block anchors.
    pub fn receipt(&mut self, receipt: &Receipt) {
        self.insert(KIND_RECEIPT, receipt.task_id, receipt.encode());
    }

    /// Whether the block changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many entries there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The entries in canonical order.
    #[must_use]
    pub fn entries(&self) -> Vec<(u8, [u8; 32], Vec<u8>)> {
        self.entries
            .iter()
            .map(|((kind, key), value)| (*kind, *key, value.clone()))
            .collect()
    }

    /// `SCALE(changes)`: the bytes the state root hashes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.entries().encode()
    }
}

/// `state_root(0)`: the root of the genesis block of `chain_id`.
#[must_use]
pub fn genesis_state_root(chain_id: &ChainId, allocations: &[(Address, u128)]) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(STATE_DOMAIN);
    hasher.update(&chain_id.0);
    hasher.update(&allocations.to_vec().encode());
    Hash(*hasher.finalize().as_bytes())
}

/// `state_root(h)`, from the parent's root and what block `h` changes.
#[must_use]
pub fn next_state_root(parent: &Hash, changes: &StateChanges) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(STATE_DOMAIN);
    hasher.update(&parent.0);
    hasher.update(&changes.encode());
    Hash(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_domain_is_the_literal_15_ascii_bytes() {
        assert_eq!(STATE_DOMAIN, b"mbongo:state:v1");
        assert_eq!(STATE_DOMAIN.len(), 15);
    }

    #[test]
    fn an_empty_block_hashes_the_one_byte_empty_vector() {
        let parent = Hash([7u8; 32]);
        let changes = StateChanges::new();
        assert_eq!(changes.encode(), vec![0u8]);
        let mut preimage = STATE_DOMAIN.to_vec();
        preimage.extend_from_slice(&parent.0);
        preimage.push(0);
        assert_eq!(
            next_state_root(&parent, &changes).0,
            *blake3::hash(&preimage).as_bytes()
        );
    }

    #[test]
    fn entries_are_ordered_by_kind_then_key_bytewise() {
        let mut changes = StateChanges::new();
        changes.insert(KIND_RECEIPT, [0u8; 32], vec![3]);
        changes.insert(KIND_ACCOUNT, [0xff; 32], vec![2]);
        changes.insert(KIND_ACCOUNT, [0x01; 32], vec![1]);
        changes.insert(KIND_TASK, [0x00; 32], vec![4]);
        let order: Vec<(u8, u8)> =
            changes.entries().iter().map(|(k, key, _)| (*k, key[0])).collect();
        assert_eq!(order, vec![(0, 0x01), (0, 0xff), (1, 0x00), (2, 0x00)]);
    }

    #[test]
    fn one_entry_per_key_holds_the_last_value() {
        let mut changes = StateChanges::new();
        changes.insert(KIND_ACCOUNT, [1u8; 32], vec![1]);
        changes.insert(KIND_ACCOUNT, [1u8; 32], vec![2]);
        assert_eq!(changes.entries(), vec![(KIND_ACCOUNT, [1u8; 32], vec![2])]);
    }

    #[test]
    fn the_entry_encoding_is_kind_key_then_compact_length_and_value() {
        let mut changes = StateChanges::new();
        changes.insert(KIND_TASK, [9u8; 32], vec![0xaa, 0xbb]);
        let mut expected = vec![0x04, KIND_TASK];
        expected.extend_from_slice(&[9u8; 32]);
        expected.extend_from_slice(&[0x08, 0xaa, 0xbb]);
        assert_eq!(changes.encode(), expected);
    }

    #[test]
    fn the_account_value_is_address_balance_and_nonce_little_endian() {
        let mut account = Account::new(Address([3u8; 32]));
        account.balance = 0x0102;
        account.nonce = 5;
        let mut changes = StateChanges::new();
        changes.account(&Address([3u8; 32]), &account);
        let (_, key, value) = &changes.entries()[0];
        assert_eq!(*key, [3u8; 32]);
        let mut expected = [3u8; 32].to_vec();
        expected.extend_from_slice(&0x0102u128.to_le_bytes());
        expected.extend_from_slice(&5u64.to_le_bytes());
        assert_eq!(*value, expected);
    }

    #[test]
    fn the_genesis_root_covers_the_chain_and_every_allocation() {
        let chain = ChainId([1u8; 32]);
        let allocations = vec![(Address([2u8; 32]), 10u128)];
        let root = genesis_state_root(&chain, &allocations);
        assert_ne!(root, genesis_state_root(&ChainId([3u8; 32]), &allocations));
        assert_ne!(
            root,
            genesis_state_root(&chain, &[(Address([2u8; 32]), 11)])
        );
        assert_ne!(root, genesis_state_root(&chain, &[]));

        let mut preimage = STATE_DOMAIN.to_vec();
        preimage.extend_from_slice(&[1u8; 32]);
        preimage.push(0x04);
        preimage.extend_from_slice(&[2u8; 32]);
        preimage.extend_from_slice(&10u128.to_le_bytes());
        assert_eq!(root.0, *blake3::hash(&preimage).as_bytes());
    }
}

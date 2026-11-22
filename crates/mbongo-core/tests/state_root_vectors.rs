//! Rust side of the shared state commitment vectors,
//! `test-vectors/state/state-root-v1.json` (RFC 0006 §4).
//!
//! The file was derived without this crate: BLAKE3 from its specification,
//! SCALE by hand, and each block's entries from a model of RFC 0006 §6. This
//! file checks that `mbongo_core::state` gives the same encodings and roots
//! from the same entries, and that every value is the encoding of what it
//! claims to be. Which entries a real block has is checked against the same
//! file by `mbongo-node` (`backend/tests/state_commitment.rs`).

use mbongo_core::state::{KIND_ACCOUNT, KIND_RECEIPT, KIND_TASK};
use mbongo_core::{
    genesis_state_root, next_state_root, Account, Address, ComputeTask, Genesis, Hash, Receipt,
    StateChanges, STATE_DOMAIN,
};
use parity_scale_codec::{Decode, Encode};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../test-vectors/state/state-root-v1.json");

fn fixture() -> Value {
    let v: Value = serde_json::from_str(FIXTURE).expect("valid JSON");
    assert_eq!(
        v["fixture_version"].as_u64(),
        Some(1),
        "unsupported fixture version"
    );
    v
}

fn bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().expect("hex string")).expect("hex")
}

fn key(v: &Value) -> [u8; 32] {
    bytes(v).try_into().expect("32 bytes")
}

fn hash(v: &Value) -> Hash {
    Hash(key(v))
}

/// The changes a vector lists, rebuilt entry by entry, after checking that
/// each value is the encoding of what the entry claims.
fn changes(entries: &Value) -> StateChanges {
    let mut changes = StateChanges::new();
    for e in entries.as_array().unwrap() {
        let kind = u8::try_from(e["kind"].as_u64().unwrap()).unwrap();
        let key = key(&e["key"]);
        let value = bytes(&e["value"]);
        match kind {
            KIND_ACCOUNT => {
                let account = Account {
                    address: Address(key),
                    balance: e["balance"].as_str().unwrap().parse().unwrap(),
                    nonce: e["nonce"].as_u64().unwrap(),
                };
                assert_eq!(
                    account.encode(),
                    value,
                    "SCALE(Account) of {}",
                    e["account"]
                );
                assert_eq!(value.len(), 56);
            }
            KIND_TASK => {
                let task = ComputeTask::decode(&mut &value[..]).expect("a task");
                assert_eq!(task.encode(), value, "canonical task bytes");
                assert_eq!(task.task_id(), key, "a task entry is keyed by its task_id");
            }
            KIND_RECEIPT => {
                let receipt = Receipt::decode(&mut &value[..]).expect("a receipt");
                assert_eq!(receipt.encode(), value, "canonical receipt bytes");
                assert_eq!(
                    receipt.task_id, key,
                    "a receipt entry is keyed by its task_id"
                );
            }
            other => panic!("unknown kind {other}"),
        }
        changes.insert(kind, key, value);
    }
    changes
}

#[test]
fn the_domain_is_the_files() {
    let v = fixture();
    assert_eq!(
        v["domain"]["ascii"].as_str().unwrap().as_bytes(),
        STATE_DOMAIN
    );
    assert_eq!(bytes(&v["domain"]["hex"]), STATE_DOMAIN);
}

#[test]
fn the_genesis_root_is_the_files() {
    let v = fixture();
    let g = &v["genesis"];
    let genesis = Genesis::from_file_bytes(g["file"].as_str().unwrap().as_bytes()).unwrap();
    assert_eq!(
        genesis.chain_id.to_string(),
        g["chain_id"].as_str().unwrap()
    );

    let listed = g["allocations"].as_array().unwrap();
    assert_eq!(listed.len(), 5, "expected 5 allocations");
    assert_eq!(genesis.allocations.len(), 5);
    for ((address, amount), a) in genesis.allocations.iter().zip(listed) {
        assert_eq!(address.0, key(&a["address"]));
        assert_eq!(amount.to_string(), a["amount"].as_str().unwrap());
    }
    let supply = genesis.allocations.iter().try_fold(0u128, |s, (_, m)| s.checked_add(*m));
    assert_eq!(supply, Some(u128::MAX), "the supply is exactly u128::MAX");

    assert_eq!(
        genesis.allocations.encode(),
        bytes(&g["allocations_encoding"])
    );
    assert_eq!(
        genesis_state_root(&genesis.chain_id, &genesis.allocations),
        hash(&g["state_root"])
    );
}

#[test]
fn every_block_root_is_the_files() {
    let v = fixture();
    let blocks = v["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 9, "expected 9 blocks");
    let counts: Vec<usize> =
        blocks.iter().map(|b| b["changes"].as_array().unwrap().len()).collect();
    assert_eq!(counts, [0, 2, 1, 2, 2, 4, 2, 0, 4], "entries per block");

    let mut parent = hash(&v["genesis"]["state_root"]);
    for (i, b) in blocks.iter().enumerate() {
        let name = b["name"].as_str().unwrap();
        assert_eq!(b["height"].as_u64(), Some(i as u64 + 1), "{name}");
        assert_eq!(
            hash(&b["parent_state_root"]),
            parent,
            "{name}: chained from the parent"
        );
        let changes = changes(&b["changes"]);
        assert_eq!(
            changes.encode(),
            bytes(&b["changes_encoding"]),
            "{name}: SCALE(changes)"
        );
        let root = next_state_root(&parent, &changes);
        assert_eq!(root, hash(&b["state_root"]), "{name}: state_root");
        parent = root;
    }
}

#[test]
fn the_listed_order_is_the_canonical_order() {
    let v = fixture();
    let mut entries = 0;
    for b in v["blocks"].as_array().unwrap().iter().chain([&v["production"]]) {
        let listed: Vec<(u8, [u8; 32])> = b["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    u8::try_from(e["kind"].as_u64().unwrap()).unwrap(),
                    key(&e["key"]),
                )
            })
            .collect();
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(listed, sorted, "{}: sorted, one entry per key", b["name"]);
        entries += listed.len();
    }
    assert_eq!(entries, 17 + 3, "entries across the file");
}

#[test]
fn the_production_root_is_that_of_the_included_candidates() {
    let v = fixture();
    let p = &v["production"];
    let candidates = p["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 5, "expected 5 candidates");
    let included: Vec<&str> = candidates
        .iter()
        .filter(|c| c["included"].as_bool().unwrap())
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(included, ["p2", "p4"]);

    let last = v["blocks"].as_array().unwrap().last().unwrap();
    assert_eq!(p["parent_state_root"], last["state_root"]);
    let changes = changes(&p["changes"]);
    assert_eq!(changes.len(), 3);
    assert_eq!(changes.encode(), bytes(&p["changes_encoding"]));
    assert_eq!(
        next_state_root(&hash(&p["parent_state_root"]), &changes),
        hash(&p["state_root"])
    );
}

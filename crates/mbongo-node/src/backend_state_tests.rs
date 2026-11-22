//! S1-6: the state commitment (RFC 0006 §4, §5 rule w) and view-based
//! block production (RFC 0006 §3, plan D-5).
//!
//! What these tests prove, each against a real `NodeBackend`:
//!
//! - the blocks of `test-vectors/state/state-root-v1.json`, derived without
//!   this code, are produced with the file's roots and entries, and every
//!   other node computes the same roots (producer and validator agree);
//! - a block whose `state_root` is wrong is refused with
//!   `StateRootMismatch`, and writes nothing;
//! - a candidate that fails in the producer's view has no effect, wherever
//!   it sits and however far its execution got;
//! - a transaction that always fails does not stop production;
//! - nodes forced to diverge stop at the first block that differs;
//! - the transfer path follows RFC 0006 §6 over every sender, receiver,
//!   nonce and boundary amount, against a reference model.

use super::tests::{
    address_of, build_valid_block, fund, make_backend, signed_anchor_tx, signed_receipt_for,
    signed_task_tx, signed_transfer, task_for,
};
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use mbongo_core::state::{KIND_ACCOUNT, KIND_RECEIPT, KIND_TASK};
use mbongo_core::StateChanges;
use mbongo_storage::InMemoryStorage;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

const VECTOR: &str = include_str!("../../../test-vectors/state/state-root-v1.json");

fn vector() -> Value {
    let v: Value = serde_json::from_str(VECTOR).expect("valid JSON");
    assert_eq!(v["fixture_version"].as_u64(), Some(1));
    v
}

fn hex_bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().expect("hex")).expect("hex")
}

fn hex32(v: &Value) -> [u8; 32] {
    hex_bytes(v).try_into().expect("32 bytes")
}

/// Signs `tx` for `chain_id` with `sk`.
fn sign_for(sk: &SigningKey, chain_id: &ChainId, mut tx: Transaction) -> Transaction {
    tx.signature = sk.sign(&tx.signing_payload(chain_id)).to_bytes();
    tx
}

fn producer_on(genesis: &Genesis) -> NodeBackend<InMemoryStorage> {
    let node = NodeBackend::new(InMemoryStorage::new(), true);
    node.ensure_genesis(genesis).unwrap();
    node
}

fn follower_on(genesis: &Genesis) -> NodeBackend<InMemoryStorage> {
    let node = NodeBackend::new(InMemoryStorage::new(), false);
    node.ensure_genesis(genesis).unwrap();
    node
}

/// Places `txs` in the pool directly, in order, as admission would have.
/// Some of them are transactions admission refuses: they stand for any
/// candidate that fails in the producer's view.
async fn pool(node: &NodeBackend<InMemoryStorage>, txs: &[Transaction]) {
    let mut pool = node.mempool.write().await;
    for tx in txs {
        pool.insert(compute_tx_hash(tx), tx.clone()).unwrap();
    }
}

async fn produce(node: &NodeBackend<InMemoryStorage>) -> Block {
    let hash: Hash = node.produce_block().await.unwrap().parse().unwrap();
    node.storage.get_block(&hash).unwrap().unwrap()
}

fn tip(node: &NodeBackend<InMemoryStorage>) -> Block {
    let h = node.storage.get_latest_height().unwrap();
    node.storage.get_block_by_height(h).unwrap().unwrap()
}

/// What `txs` change on top of `node`'s tip, every one applying.
fn changes_for(node: &NodeBackend<InMemoryStorage>, txs: &[Transaction]) -> StateChanges {
    let mut view = BlockView::new(node).unwrap();
    for (i, tx) in txs.iter().enumerate() {
        let executed = view.execute(i, tx).unwrap();
        view.commit(executed);
    }
    view.changes()
}

/// Everything a refused block could have touched.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    height: u64,
    tip: Hash,
    accounts: Vec<Option<Account>>,
    transactions: Vec<bool>,
    last_seq: u64,
    tasks: Vec<bool>,
}

fn snapshot(
    node: &NodeBackend<InMemoryStorage>,
    accounts: &[Address],
    txs: &[Transaction],
    task_ids: &[[u8; 32]],
) -> Snapshot {
    let s = &node.storage;
    Snapshot {
        height: s.get_latest_height().unwrap(),
        tip: compute_block_hash(&tip(node)),
        accounts: accounts.iter().map(|a| s.get_account(a).unwrap()).collect(),
        transactions: txs
            .iter()
            .map(|tx| s.get_transaction(&compute_tx_hash(tx)).unwrap().is_some())
            .collect(),
        last_seq: s.get_last_included_tx_seq().unwrap(),
        tasks: task_ids.iter().map(|t| s.has_task(t).unwrap()).collect(),
    }
}

// ── The shared vectors, on real nodes ─────────────────────────────────

/// The vector chain: its genesis, keys, and its transactions signed for it.
struct VectorChain {
    v: Value,
    genesis: Genesis,
    keys: BTreeMap<String, SigningKey>,
    /// Every transaction by id, once built, so a repeat is byte-identical.
    built: BTreeMap<String, Transaction>,
}

impl VectorChain {
    fn new() -> Self {
        let v = vector();
        let genesis =
            Genesis::from_file_bytes(v["genesis"]["file"].as_str().unwrap().as_bytes()).unwrap();
        assert_eq!(
            genesis.chain_id.to_string(),
            v["genesis"]["chain_id"].as_str().unwrap()
        );
        let mut keys = BTreeMap::new();
        for (name, k) in v["test_keys"].as_object().unwrap() {
            if name == "warning" {
                continue;
            }
            let sk = SigningKey::from_bytes(&hex32(&k["ed25519_seed"]));
            assert_eq!(
                sk.verifying_key().to_bytes(),
                hex32(&k["public_key"]),
                "{name}"
            );
            keys.insert(name.clone(), sk);
        }
        assert_eq!(keys.len(), 7, "expected 7 test keys");
        Self {
            v,
            genesis,
            keys,
            built: BTreeMap::new(),
        }
    }

    fn address(&self, name: &str) -> Address {
        Address(self.keys[name].verifying_key().to_bytes())
    }

    /// The transaction a vector entry describes, signed for the chain.
    fn tx(&mut self, d: &Value) -> Transaction {
        let id = d["id"].as_str().unwrap().to_owned();
        if d.get("repeat_of").is_some() {
            return self.built[&id].clone();
        }
        let sender = d["sender"].as_str().unwrap();
        let (tx_type, receiver, payload) = match d["type"].as_str().unwrap() {
            "Transfer" => (
                TransactionType::Transfer,
                self.address(d["receiver"].as_str().unwrap()),
                TransactionPayload::None,
            ),
            "Stake" => (
                TransactionType::Stake,
                self.address(d["receiver"].as_str().unwrap()),
                TransactionPayload::None,
            ),
            "ComputeTask" => {
                let name = d["task"].as_str().unwrap();
                let bytes = hex_bytes(&self.v["tasks"][name]["encoding"]);
                let task = ComputeTask::decode(&mut &bytes[..]).unwrap();
                (
                    TransactionType::ComputeTask,
                    Address::zero(),
                    TransactionPayload::ComputeTask(Box::new(task)),
                )
            }
            "AnchorReceipt" => {
                let bytes = hex_bytes(&self.v["receipt"]["encoding"]);
                let receipt = Receipt::decode(&mut &bytes[..]).unwrap();
                (
                    TransactionType::AnchorReceipt,
                    Address::zero(),
                    TransactionPayload::AnchorReceipt(Box::new(receipt)),
                )
            }
            other => panic!("type {other}"),
        };
        let tx = sign_for(
            &self.keys[sender],
            &self.genesis.chain_id,
            Transaction {
                tx_type,
                sender: self.address(sender),
                receiver,
                amount: d["amount"].as_str().unwrap().parse().unwrap(),
                nonce: d["nonce"].as_u64().unwrap(),
                payload,
                signature: [0u8; 64],
            },
        );
        self.built.insert(id, tx.clone());
        tx
    }
}

/// The entries a vector lists, as `(kind, key, value)`.
fn listed(changes: &Value) -> Vec<(u8, [u8; 32], Vec<u8>)> {
    changes
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                u8::try_from(e["kind"].as_u64().unwrap()).unwrap(),
                hex32(&e["key"]),
                hex_bytes(&e["value"]),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_vector_chain_is_produced_and_applied_with_the_files_roots() {
    let mut chain = VectorChain::new();
    let genesis = chain.genesis.clone();
    // Two producers and one follower: producers agree with each other,
    // and the follower validates each block with its own computation.
    let a = producer_on(&genesis);
    let b = producer_on(&genesis);
    let follower = follower_on(&genesis);
    let root0 = Hash(hex32(&chain.v["genesis"]["state_root"]));
    for node in [&a, &b, &follower] {
        assert_eq!(tip(node).header.state_root, root0, "state_root(0)");
    }

    let blocks = chain.v["blocks"].as_array().unwrap().clone();
    assert_eq!(blocks.len(), 9, "expected 9 vector blocks");
    let mut skipped = 0;
    for vb in &blocks {
        let name = vb["name"].as_str().unwrap();
        let txs: Vec<Transaction> =
            vb["transactions"].as_array().unwrap().iter().map(|d| chain.tx(d)).collect();
        skipped += vb["transactions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["skipped"].as_bool().unwrap())
            .count();

        // The entries this node computes are the file's, entry by entry.
        assert_eq!(
            changes_for(&a, &txs).entries(),
            listed(&vb["changes"]),
            "{name}: changes"
        );

        pool(&a, &txs).await;
        pool(&b, &txs).await;
        let produced = produce(&a).await;
        let other = produce(&b).await;
        assert_eq!(
            produced.body.transactions, txs,
            "{name}: every transaction included, in order"
        );
        let want = Hash(hex32(&vb["state_root"]));
        assert_eq!(produced.header.state_root, want, "{name}: the file's root");
        assert_eq!(
            other.header.state_root, want,
            "{name}: the other producer's root"
        );
        assert!(
            follower.handle_incoming_block(produced).is_some(),
            "{name}: the follower applies it"
        );
        assert_eq!(tip(&follower).header.state_root, want);
    }
    assert_eq!(skipped, 2, "two skipped transactions across the file");

    // Production: three candidates fail and are left out with no effect.
    let p = chain.v["production"].clone();
    let candidates = p["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 5, "expected 5 candidates");
    let txs: Vec<Transaction> = candidates.iter().map(|d| chain.tx(d)).collect();
    let included: Vec<Transaction> = candidates
        .iter()
        .zip(&txs)
        .filter(|(d, _)| d["included"].as_bool().unwrap())
        .map(|(_, tx)| tx.clone())
        .collect();
    assert_eq!(included.len(), 2);
    pool(&a, &txs).await;
    let produced = produce(&a).await;
    assert_eq!(
        produced.body.transactions, included,
        "the included candidates, in order"
    );
    assert_eq!(produced.header.state_root, Hash(hex32(&p["state_root"])));
    assert_eq!(
        changes_for(&follower, &included).entries(),
        listed(&p["changes"])
    );
    for tx in &txs {
        assert!(
            !a.is_pending(&compute_tx_hash(tx)).await,
            "the pool is empty after production"
        );
    }
    assert!(follower.handle_incoming_block(produced).is_some());

    // Every account the file names ends the same on the producer and the
    // follower.
    for name in chain.keys.keys() {
        let addr = chain.address(name);
        assert_eq!(
            a.storage.get_account(&addr).unwrap(),
            follower.storage.get_account(&addr).unwrap(),
            "{name}"
        );
    }
}

// ── Rule (w): a wrong root is refused atomically ──────────────────────

#[test]
fn a_wrong_state_root_is_refused_and_writes_nothing() {
    let backend = make_backend();
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    let alice = SigningKey::from_bytes(&[0xA1u8; 32]);
    let submitter = SigningKey::from_bytes(&[0xA2u8; 32]);
    let alice_addr = fund(&backend, &alice, 1_000);
    let submitter_addr = fund(&backend, &submitter, 0);
    let bob = Address([0xB0u8; 32]);
    let task = task_for(
        submitter_addr,
        Address([0xE0u8; 32]),
        [0x51u8; 32],
        vec![1, 2, 3],
    );
    let txs = vec![
        signed_transfer(&alice, bob, 250, 0),
        signed_task_tx(&submitter, 0, task.clone()),
    ];
    let good = build_valid_block(&backend, txs.clone());
    let right = good.header.state_root;
    assert_ne!(right, Hash::zero());
    let parent_root = tip(&backend).header.state_root;
    let without_task = backend.state_root_for(&txs[..1]).unwrap();

    let watched = [alice_addr, submitter_addr, bob];
    let before = snapshot(&backend, &watched, &txs, &[task.task_id()]);
    let mut flipped = right;
    flipped.0[31] ^= 1;
    let wrong = [
        ("one bit flipped", flipped),
        ("zero", Hash::zero()),
        ("the parent's root", parent_root),
        (
            "the root of the block without its last transaction",
            without_task,
        ),
    ];
    assert_eq!(wrong.len(), 4);
    for (what, root) in wrong {
        assert_ne!(root, right, "{what}");
        let mut block = good.clone();
        block.header.state_root = root;
        match backend.apply_block(&block) {
            Err(ApplyBlockError::StateRootMismatch { expected, got }) => {
                assert_eq!(expected, right, "{what}: the locally computed root");
                assert_eq!(got, root, "{what}");
            }
            other => panic!("{what}: expected StateRootMismatch, got {other:?}"),
        }
        assert_eq!(
            snapshot(&backend, &watched, &txs, &[task.task_id()]),
            before,
            "{what}: nothing written"
        );
    }

    // The block with the right root then applies, and writes everything.
    backend.apply_block(&good).unwrap();
    let after = snapshot(&backend, &watched, &txs, &[task.task_id()]);
    assert_eq!(after.height, before.height + 1);
    assert_eq!(after.transactions, vec![true, true]);
    assert_eq!(after.tasks, vec![true]);
    assert_eq!(after.last_seq, before.last_seq + 2);
}

#[test]
fn a_block_is_checked_against_its_own_parents_root() {
    // The same transactions on two different parents give two different
    // roots: the chain of roots, not the block alone, is committed.
    let backend = make_backend();
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    let alice = SigningKey::from_bytes(&[0xA3u8; 32]);
    fund(&backend, &alice, 1_000);
    let empty = build_valid_block(&backend, vec![]);
    let at_one = empty.header.state_root;
    assert_eq!(
        at_one,
        next_state_root(&tip(&backend).header.state_root, &StateChanges::new())
    );
    backend.apply_block(&empty).unwrap();
    let again = build_valid_block(&backend, vec![]);
    assert_ne!(
        again.header.state_root, at_one,
        "an empty block's root still chains"
    );
    let mut stale = again.clone();
    stale.header.state_root = at_one;
    assert!(matches!(
        backend.apply_block(&stale),
        Err(ApplyBlockError::StateRootMismatch { .. })
    ));
    backend.apply_block(&again).unwrap();
}

#[test]
fn a_failing_transaction_is_reported_before_rule_w() {
    // Rule (w) is evaluated after every transaction: a block with a failing
    // transaction and a wrong root reports the transaction.
    let backend = make_backend();
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    let alice = SigningKey::from_bytes(&[0xA4u8; 32]);
    fund(&backend, &alice, 10);
    let mut block = build_valid_block(
        &backend,
        vec![signed_transfer(&alice, Address([1u8; 32]), 11, 0)],
    );
    block.header.state_root = Hash([0x77u8; 32]);
    assert!(matches!(
        backend.apply_block(&block),
        Err(ApplyBlockError::InsufficientBalance(0))
    ));
}

#[test]
fn a_directory_with_a_genesis_block_from_before_s1_6_is_refused_not_rewritten() {
    // A data directory written by an S1-4 or S1-5 build holds a genesis
    // block whose state_root is zero. It is not migrated or overwritten.
    let backend = make_backend();
    let mut legacy = genesis_block(&Genesis::dev());
    legacy.header.state_root = Hash::zero();
    let legacy_hash = compute_block_hash(&legacy);
    backend
        .storage
        .write_batch(vec![
            BatchOp::PutBlock(legacy_hash, legacy.clone()),
            BatchOp::PutBlockHeightIndex(0, legacy_hash),
        ])
        .unwrap();
    let err = backend.ensure_genesis(&Genesis::dev()).unwrap_err().to_string();
    assert!(err.contains("is not the genesis block"), "{err}");
    assert_eq!(
        backend.storage.get_block_by_height(0).unwrap(),
        Some(legacy)
    );
    let dev = Genesis::dev();
    for (address, _) in &dev.allocations {
        assert_eq!(
            backend.storage.get_account(address).unwrap(),
            None,
            "no allocation written"
        );
    }
}

#[test]
fn the_genesis_root_is_state_root_zero_of_the_genesis() {
    let dev = Genesis::dev();
    let block = genesis_block(&dev);
    assert_eq!(
        block.header.state_root,
        genesis_state_root(&dev.chain_id, &dev.allocations)
    );
    assert_ne!(block.header.state_root, Hash::zero());
}

// ── Production: a failing candidate has no effect ─────────────────────

/// One way a candidate can fail in the view, after how much of its
/// execution.
struct Failure {
    name: &'static str,
    /// Accounts the failing candidate would touch, to check they are not.
    touched: Vec<Address>,
    tx: Transaction,
}

/// Every way of failing these tests cover, each from its own sender, with
/// the state it needs written to `backend`.
fn failures(backend: &NodeBackend<InMemoryStorage>) -> Vec<Failure> {
    let mut out = Vec::new();
    let key = |seed: u8, balance: u128| {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let addr = fund(backend, &sk, balance);
        (sk, addr)
    };

    // After the nonce step: the balance does not cover the amount.
    let (sk, addr) = key(0xC1, 50);
    let to = Address([0xD1u8; 32]);
    out.push(Failure {
        name: "insufficient balance",
        touched: vec![addr, to],
        tx: signed_transfer(&sk, to, 51, 0),
    });

    // A self-transfer above the balance, after the nonce step.
    let (sk, addr) = key(0xC2, 50);
    out.push(Failure {
        name: "self-transfer above balance",
        touched: vec![addr],
        tx: signed_transfer(&sk, addr, 51, 0),
    });

    // After the debit: crediting the receiver overflows.
    let (sk, addr) = key(0xC3, 50);
    let full = Address([0xD3u8; 32]);
    let mut acc = Account::new(full);
    acc.balance = u128::MAX - 10;
    backend.storage.put_account(&full, &acc).unwrap();
    out.push(Failure {
        name: "receiver overflow",
        touched: vec![addr, full],
        tx: signed_transfer(&sk, full, 11, 0),
    });

    // A nonce gap.
    let (sk, addr) = key(0xC4, 50);
    out.push(Failure {
        name: "nonce gap",
        touched: vec![addr],
        tx: signed_transfer(&sk, Address([0xD4u8; 32]), 1, 1),
    });

    // An absent sender.
    let ghost = SigningKey::from_bytes(&[0xC5u8; 32]);
    let ghost_addr = Address(ghost.verifying_key().to_bytes());
    out.push(Failure {
        name: "absent sender",
        touched: vec![ghost_addr],
        tx: signed_transfer(&ghost, Address([0xD5u8; 32]), 0, 0),
    });

    // A signature for another network.
    let (sk, addr) = key(0xC6, 50);
    let tx = sign_for(
        &sk,
        &ChainId([0x5Au8; 32]),
        Transaction {
            tx_type: TransactionType::Transfer,
            sender: addr,
            receiver: Address([0xD6u8; 32]),
            amount: 1,
            nonce: 0,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        },
    );
    out.push(Failure {
        name: "signature for another chain",
        touched: vec![addr],
        tx,
    });

    // An anchor whose receipt signature fails, after its nonce step.
    let (sk, addr) = key(0xC7, 0);
    let mut receipt = signed_receipt_for(&sk, [0x97u8; 32], vec![], 1);
    receipt.signature[0] ^= 1;
    out.push(Failure {
        name: "bad receipt signature",
        touched: vec![addr],
        tx: signed_anchor_tx(&sk, 0, receipt),
    });

    // A task whose submitter is not its sender, after the nonce step.
    let (sk, addr) = key(0xC8, 0);
    let task = task_for(
        Address([0xEEu8; 32]),
        Address([0xE1u8; 32]),
        [0x98u8; 32],
        vec![],
    );
    out.push(Failure {
        name: "submitter mismatch",
        touched: vec![addr],
        tx: signed_task_tx(&sk, 0, task),
    });

    // A receipt for a task nobody registered.
    let (sk, addr) = key(0xC9, 0);
    let receipt = signed_receipt_for(&sk, [0x99u8; 32], vec![], 1);
    out.push(Failure {
        name: "task not registered",
        touched: vec![addr],
        tx: signed_anchor_tx(&sk, 0, receipt),
    });

    assert_eq!(out.len(), 9, "expected 9 failure kinds");
    out
}

#[tokio::test]
async fn a_failing_candidate_has_no_effect_wherever_it_sits() {
    let mut cases = 0;
    for position in 0..3 {
        for index in 0..9 {
            let producer = make_backend();
            producer.ensure_genesis(&Genesis::dev()).unwrap();
            let reference = make_backend();
            reference.ensure_genesis(&Genesis::dev()).unwrap();

            let failure = failures(&producer).swap_remove(index);
            let _ = failures(&reference);
            let alice = SigningKey::from_bytes(&[0xF1u8; 32]);
            let carol = SigningKey::from_bytes(&[0xF2u8; 32]);
            for node in [&producer, &reference] {
                fund(node, &alice, 1_000);
                fund(node, &carol, 1_000);
            }
            let valid = vec![
                signed_transfer(&alice, Address([0xB1u8; 32]), 10, 0),
                signed_transfer(&carol, Address([0xB2u8; 32]), 20, 0),
            ];
            let mut candidates = valid.clone();
            candidates.insert(position, failure.tx.clone());

            let before: Vec<Option<Account>> = failure
                .touched
                .iter()
                .map(|a| producer.storage.get_account(a).unwrap())
                .collect();
            pool(&producer, &candidates).await;
            let block = produce(&producer).await;
            let name = failure.name;

            assert_eq!(
                block.body.transactions, valid,
                "{name} at {position}: left out"
            );
            // The root is that of the valid transactions alone, as a node
            // that never saw the failing one computes it.
            let expected = build_valid_block(&reference, valid.clone());
            assert_eq!(
                block.header.state_root, expected.header.state_root,
                "{name} at {position}: root"
            );
            let after: Vec<Option<Account>> = failure
                .touched
                .iter()
                .map(|a| producer.storage.get_account(a).unwrap())
                .collect();
            assert_eq!(
                after, before,
                "{name} at {position}: no effect on its accounts"
            );
            let hash = compute_tx_hash(&failure.tx);
            assert!(
                producer.storage.get_transaction(&hash).unwrap().is_none(),
                "{name}: not stored"
            );
            assert_eq!(
                producer.storage.get_last_included_tx_seq().unwrap(),
                2,
                "{name}: two sequence numbers"
            );
            assert!(!producer.is_pending(&hash).await, "{name}: evicted");
            // A follower on the same state accepts the produced block.
            reference.apply_block(&block).unwrap();
            cases += 1;
        }
    }
    assert_eq!(cases, 27, "9 failure kinds at 3 positions");
}

#[test]
fn a_failed_execution_leaves_the_view_as_it_was() {
    // The pool never holds two registrations of one task, so this is
    // driven on the view directly. The second registration fails at rule
    // (p), after its nonce step, against state only the view holds; the
    // view is exactly as before, and the block it then gives is valid.
    let backend = make_backend();
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    let s1 = SigningKey::from_bytes(&[0x81u8; 32]);
    let s2 = SigningKey::from_bytes(&[0x82u8; 32]);
    let a1 = fund(&backend, &s1, 0);
    let a2 = fund(&backend, &s2, 0);
    let task = task_for(a1, Address([0xE2u8; 32]), [0x83u8; 32], vec![9]);
    let first = signed_task_tx(&s1, 0, task.clone());
    let second = signed_task_tx(&s1, 1, task.clone());
    let other = signed_transfer(&s2, Address([0xB3u8; 32]), 0, 0);

    let mut view = BlockView::new(&backend).unwrap();
    let executed = view.execute(0, &first).unwrap();
    view.commit(executed);
    let changes = view.changes();
    let (ops, seq) = (view.ops.len(), view.last_seq);
    assert!(matches!(
        view.execute(1, &second),
        Err(ApplyBlockError::TaskRepeatedInBlock(1))
    ));
    assert_eq!(view.changes(), changes, "no effect on the view");
    assert_eq!((view.ops.len(), view.last_seq), (ops, seq));
    let executed = view.execute(1, &other).unwrap();
    view.commit(executed);

    let mut expected = StateChanges::new();
    expected.account(
        &a1,
        &Account {
            address: a1,
            balance: 0,
            nonce: 1,
        },
    );
    expected.account(
        &a2,
        &Account {
            address: a2,
            balance: 0,
            nonce: 1,
        },
    );
    let b3 = Address([0xB3u8; 32]);
    expected.account(&b3, &Account::new(b3));
    expected.task(task.task_id(), &task);
    assert_eq!(view.changes(), expected);
    let root = next_state_root(&tip(&backend).header.state_root, &view.changes());
    drop(view);

    let mut block = build_valid_block(&backend, vec![first, other]);
    assert_eq!(block.header.state_root, root);
    block.header.state_root = root;
    backend.apply_block(&block).unwrap();
    assert_eq!(
        backend.storage.get_account(&a1).unwrap().unwrap().nonce,
        1,
        "one nonce consumed"
    );
}

// ── D-5: a poison transaction does not stop production ────────────────

#[tokio::test]
async fn a_poison_transaction_does_not_stop_production() {
    let backend = make_backend();
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    let poisoner = SigningKey::from_bytes(&[0x91u8; 32]);
    fund(&backend, &poisoner, 5);
    // First in the pool, so it is selected first: before S1-6 every
    // production attempt failed on it. Its successor depends on it.
    let poison = signed_transfer(&poisoner, Address([0x92u8; 32]), 6, 0);
    let dependent = signed_transfer(&poisoner, Address([0x92u8; 32]), 1, 1);
    pool(&backend, &[poison.clone(), dependent.clone()]).await;

    let honest = SigningKey::from_bytes(&[0x93u8; 32]);
    fund(&backend, &honest, 1_000);
    let mut produced = 0;
    for round in 0..4u64 {
        let tx = signed_transfer(&honest, Address([0x94u8; 32]), 1, round);
        backend.submit_transaction(tx.clone()).await.unwrap();
        let block = produce(&backend).await;
        assert_eq!(
            block.body.transactions,
            vec![tx],
            "round {round}: the honest transaction only"
        );
        assert_eq!(block.header.height, round + 1);
        for t in [&poison, &dependent] {
            assert!(
                !backend.is_pending(&compute_tx_hash(t)).await,
                "round {round}: out of the pool"
            );
        }
        produced += 1;
    }
    assert_eq!(produced, 4, "four blocks produced in a row");
    assert_eq!(backend.mempool.read().await.len(), 0);
    assert_eq!(
        backend.storage.get_account(&address_of(&poisoner)).unwrap().unwrap().nonce,
        0,
        "the poison never applied"
    );
}

#[tokio::test]
async fn a_failed_task_and_its_receipt_both_leave_the_pool() {
    // A ComputeTask that fails in the view, and the receipt bound to it,
    // which rule (q) then refuses: neither stays to fail every block.
    let backend = make_backend();
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    let submitter = SigningKey::from_bytes(&[0x95u8; 32]);
    let executor = SigningKey::from_bytes(&[0x96u8; 32]);
    let sub = fund(&backend, &submitter, 0);
    let exe = fund(&backend, &executor, 0);
    let mut task = task_for(sub, exe, [0x97u8; 32], vec![]);
    task.version = 2; // fails rule (m) in the view
    let receipt = {
        let mut r = signed_receipt_for(&executor, task.task_id(), vec![], 1);
        r.input_commitment = task.input_commitment;
        r.signature = executor.sign(&r.receipt_hash().0).to_bytes();
        r
    };
    let task_tx = signed_task_tx(&submitter, 0, task);
    let anchor_tx = signed_anchor_tx(&executor, 0, receipt);
    pool(&backend, &[task_tx.clone(), anchor_tx.clone()]).await;
    let block = produce(&backend).await;
    assert!(block.body.transactions.is_empty());
    assert!(!backend.is_pending(&compute_tx_hash(&task_tx)).await);
    assert!(!backend.is_pending(&compute_tx_hash(&anchor_tx)).await);
}

#[tokio::test]
async fn a_produced_timestamp_is_never_below_its_parents() {
    let backend = make_backend();
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    let future = now_secs() + 1_000_000_000;
    let mut block = build_valid_block(&backend, vec![]);
    block.header.timestamp = future;
    backend.apply_block(&block).unwrap();
    let produced = produce(&backend).await;
    assert_eq!(
        produced.header.timestamp, future,
        "max(now, parent.timestamp)"
    );
    let next = produce(&backend).await;
    assert_eq!(next.header.timestamp, future);
}

// ── Divergence is detected at the first conflicting block ─────────────

#[tokio::test]
async fn nodes_that_diverge_stop_at_the_first_conflicting_block() {
    let producer = make_backend();
    let honest = NodeBackend::new(InMemoryStorage::new(), false);
    let diverged = NodeBackend::new(InMemoryStorage::new(), false);
    let a = SigningKey::from_bytes(&[0x71u8; 32]);
    let z = SigningKey::from_bytes(&[0x72u8; 32]);
    for node in [&producer, &honest, &diverged] {
        node.ensure_genesis(&Genesis::dev()).unwrap();
        fund(node, &a, 1_000);
        fund(node, &z, 1_000);
    }
    // One node holds one unit more for z: invisible until a block touches
    // z, then refused by rule (w) though every transaction is valid there.
    let z_addr = address_of(&z);
    let mut acc = diverged.storage.get_account(&z_addr).unwrap().unwrap();
    acc.balance += 1;
    diverged.storage.put_account(&z_addr, &acc).unwrap();

    let plan = [
        signed_transfer(&a, Address([0x73u8; 32]), 10, 0),
        signed_transfer(&a, Address([0x73u8; 32]), 10, 1),
        signed_transfer(&z, Address([0x74u8; 32]), 10, 0),
        signed_transfer(&a, Address([0x73u8; 32]), 10, 2),
    ];
    let mut blocks = Vec::new();
    for tx in &plan {
        producer.submit_transaction(tx.clone()).await.unwrap();
        blocks.push(produce(&producer).await);
    }
    assert_eq!(blocks.len(), 4);

    for block in &blocks {
        assert!(
            honest.handle_incoming_block(block.clone()).is_some(),
            "the honest node applies all"
        );
    }
    assert_eq!(
        compute_block_hash(&tip(&honest)),
        compute_block_hash(&tip(&producer))
    );

    assert!(diverged.handle_incoming_block(blocks[0].clone()).is_some());
    assert!(diverged.handle_incoming_block(blocks[1].clone()).is_some());
    for attempt in 0..2 {
        match diverged.apply_block(&blocks[2]) {
            Err(ApplyBlockError::StateRootMismatch { got, .. }) => {
                assert_eq!(got, blocks[2].header.state_root, "attempt {attempt}");
            }
            other => panic!("attempt {attempt}: expected StateRootMismatch, got {other:?}"),
        }
    }
    assert!(diverged.handle_incoming_block(blocks[2].clone()).is_none());
    assert_eq!(
        diverged.latest_height().unwrap(),
        2,
        "stays at the last agreed height"
    );
    assert_eq!(
        compute_block_hash(&tip(&diverged)),
        compute_block_hash(&blocks[1])
    );
    assert!(
        diverged.apply_block(&blocks[3]).is_err(),
        "and cannot skip ahead"
    );
    assert_eq!(
        diverged.storage.get_account(&z_addr).unwrap().unwrap().balance,
        1_001
    );
}

// ── RFC 0006 §6 against a reference model ─────────────────────────────

/// What one account holds in the reference model: balance and nonce.
type Held = (u128, u64);

/// The reference model's state: one mapping from address to what it holds.
type Model = HashMap<Address, Held>;

/// A literal reading of RFC 0006 §6 steps 1–5, over a map of accounts.
/// `Err` names the error the RFC gives.
fn model_transfer(
    state: &mut Model,
    sender: Address,
    receiver: Address,
    amount: u128,
    nonce: u64,
) -> Result<(), &'static str> {
    let mut s = *state.get(&sender).ok_or("InsufficientBalance")?; // 1
    if nonce != s.1 || nonce > MAX_TRANSACTION_NONCE {
        return Err("InvalidNonce"); // 2
    }
    s.1 += 1;
    if s.0 < amount {
        return Err("InsufficientBalance"); // 3
    }
    s.0 -= amount;
    state.insert(sender, s);
    let mut r = *state.get(&receiver).unwrap_or(&(0, 0)); // 4 (reads step 3's write)
    r.0 = r.0.checked_add(amount).ok_or("InsufficientBalance")?; // 5
    state.insert(receiver, r);
    Ok(())
}

fn verdict(e: &ApplyBlockError) -> &'static str {
    match e {
        ApplyBlockError::InvalidNonce(_) => "InvalidNonce",
        ApplyBlockError::InsufficientBalance(_) => "InsufficientBalance",
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn single_transfers_follow_the_reference_model_at_every_boundary() {
    const BALANCE: u128 = 1_000;
    let sender_sk = SigningKey::from_bytes(&[0x61u8; 32]);
    let sender = address_of(&sender_sk);
    // Sender: funded, empty, absent, at the last usable nonce, exhausted.
    let senders: [(&str, Option<Held>); 5] = [
        ("funded", Some((BALANCE, 3))),
        ("empty", Some((0, 0))),
        ("absent", None),
        ("last nonce", Some((BALANCE, MAX_TRANSACTION_NONCE))),
        ("nonce exhausted", Some((BALANCE, u64::MAX))),
    ];
    let rich = Address([0x62u8; 32]);
    let zero = Address([0x63u8; 32]);
    let fresh = Address([0x64u8; 32]);
    let nearly_full = Address([0x65u8; 32]);
    let receivers: [(&str, Address, Option<Held>); 5] = [
        ("self", sender, None),
        ("existing", rich, Some((500, 7))),
        ("existing at zero", zero, Some((0, 0))),
        ("absent", fresh, None),
        ("nearly full", nearly_full, Some((u128::MAX - 500, 0))),
    ];
    let amounts = [0, 1, BALANCE - 1, BALANCE, BALANCE + 1, u128::MAX];
    let types = [TransactionType::Transfer, TransactionType::Stake];

    let mut cases = 0;
    let mut outcomes: BTreeMap<&str, usize> = BTreeMap::new();
    for (sname, sstate) in senders {
        let base_nonce = sstate.map_or(0, |s| s.1);
        let nonces = [
            Some(base_nonce),
            base_nonce.checked_add(1),
            base_nonce.checked_sub(1),
        ];
        for (rname, raddr, rstate) in receivers {
            for amount in amounts {
                for nonce in nonces.into_iter().flatten() {
                    for tx_type in types {
                        let backend = make_backend();
                        backend.ensure_genesis(&Genesis::dev()).unwrap();
                        let mut model: Model = HashMap::new();
                        let mut put = |addr: Address, st: (u128, u64)| {
                            backend
                                .storage
                                .put_account(
                                    &addr,
                                    &Account {
                                        address: addr,
                                        balance: st.0,
                                        nonce: st.1,
                                    },
                                )
                                .unwrap();
                            model.insert(addr, st);
                        };
                        if let Some(st) = sstate {
                            put(sender, st);
                        }
                        if let Some(st) = rstate {
                            put(raddr, st);
                        }
                        let tx = sign_for(
                            &sender_sk,
                            &Genesis::dev().chain_id,
                            Transaction {
                                tx_type,
                                sender,
                                receiver: raddr,
                                amount,
                                nonce,
                                payload: TransactionPayload::None,
                                signature: [0u8; 64],
                            },
                        );
                        let case = format!(
                            "{sname} -> {rname}, amount {amount}, nonce {nonce}, {tx_type:?}"
                        );

                        let before_model = model.clone();
                        let expected = if raddr == sender {
                            // One account: nothing moves; the same balance check.
                            match model.get(&sender).copied() {
                                None => Err("InsufficientBalance"),
                                Some((_, n)) if nonce != n || nonce > MAX_TRANSACTION_NONCE => {
                                    Err("InvalidNonce")
                                }
                                Some((b, _)) if b < amount => Err("InsufficientBalance"),
                                Some((b, n)) => {
                                    model.insert(sender, (b, n + 1));
                                    Ok(())
                                }
                            }
                        } else {
                            model_transfer(&mut model, sender, raddr, amount, nonce)
                        };

                        // The model's root: its touched accounts after the block.
                        let mut changes = StateChanges::new();
                        if expected.is_ok() {
                            for addr in [sender, raddr] {
                                let (b, n) = model[&addr];
                                changes.account(
                                    &addr,
                                    &Account {
                                        address: addr,
                                        balance: b,
                                        nonce: n,
                                    },
                                );
                            }
                        }
                        let parent = tip(&backend);
                        let block = Block {
                            header: BlockHeader {
                                parent_hash: compute_block_hash(&parent),
                                state_root: next_state_root(&parent.header.state_root, &changes),
                                transactions_root: compute_transactions_root(std::slice::from_ref(
                                    &tx,
                                )),
                                timestamp: now_secs(),
                                height: 1,
                            },
                            body: BlockBody {
                                transactions: vec![tx],
                            },
                        };
                        let got = backend.apply_block(&block);
                        let watched: Vec<Address> = vec![sender, raddr];
                        let state: Vec<Option<Held>> = watched
                            .iter()
                            .map(|a| {
                                backend
                                    .storage
                                    .get_account(a)
                                    .unwrap()
                                    .map(|x| (x.balance, x.nonce))
                            })
                            .collect();
                        match (&expected, &got) {
                            (Ok(()), Ok(_)) => {
                                let want: Vec<Option<Held>> =
                                    watched.iter().map(|a| model.get(a).copied()).collect();
                                assert_eq!(state, want, "{case}: state");
                                // I1: the two balances move by exactly the
                                // amount, so their sum is unchanged. I2.
                                let total = |m: &Model| {
                                    watched
                                        .iter()
                                        .filter_map(|a| m.get(a))
                                        .try_fold(0u128, |s, x| s.checked_add(x.0))
                                };
                                if raddr != sender {
                                    assert_eq!(total(&model), total(&before_model), "{case}: I1");
                                }
                                assert_eq!(
                                    model[&sender].1,
                                    before_model[&sender].1 + 1,
                                    "{case}: I2"
                                );
                                *outcomes.entry("applied").or_default() += 1;
                            }
                            (Err(reason), Err(e)) => {
                                let reason: &'static str = reason;
                                assert_eq!(verdict(e), reason, "{case}: error");
                                let want: Vec<Option<Held>> =
                                    watched.iter().map(|a| before_model.get(a).copied()).collect();
                                assert_eq!(state, want, "{case}: nothing written");
                                *outcomes.entry(reason).or_default() += 1;
                            }
                            _ => panic!("{case}: model {expected:?}, node {got:?}"),
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    // 5 senders; nonces: 3 for "funded" and "last nonce", 2 for "empty" and
    // "absent" (no nonce below 0), 2 for "nonce exhausted" (none above
    // u64::MAX). 5 receivers, 6 amounts, 2 types.
    assert_eq!(cases, (3 + 2 + 2 + 3 + 2) * 5 * 6 * 2, "case count");
    assert!(outcomes["applied"] >= 50, "{outcomes:?}");
    assert!(outcomes["InvalidNonce"] >= 50, "{outcomes:?}");
    assert!(outcomes["InsufficientBalance"] >= 50, "{outcomes:?}");
}

#[tokio::test]
async fn two_transaction_blocks_follow_the_model_in_validation_and_production() {
    // Every ordered pair of transfers among three accounts, with boundary
    // amounts: validation takes the block or rejects it at the model's
    // first failure; production includes exactly the model's applicable
    // ones, with the model's root. Step 4 reading step 3 (I4) is what
    // makes the second transfer's verdict depend on the first.
    let keys: Vec<SigningKey> = (0..3).map(|i| SigningKey::from_bytes(&[0x41 + i; 32])).collect();
    let addrs: Vec<Address> = keys.iter().map(address_of).collect();
    let start = [(600u128, 0u64), (0, 0), (300, 0)];
    let amounts = [0u128, 300, 600, 900];
    let mut cases = 0;
    let mut partial = 0;
    for s1 in 0..3 {
        for r1 in 0..3 {
            for s2 in 0..3 {
                for r2 in 0..3 {
                    for a1 in amounts {
                        for a2 in amounts {
                            let n2 = u64::from(s2 == s1);
                            let txs = [
                                signed_transfer(&keys[s1], addrs[r1], a1, 0),
                                signed_transfer(&keys[s2], addrs[r2], a2, n2),
                            ];
                            // Model, strict (validation) and lenient (production).
                            let mut init: Model = HashMap::new();
                            for (i, st) in start.iter().enumerate() {
                                init.insert(addrs[i], *st);
                            }
                            let apply = |m: &mut Model, i: usize| -> bool {
                                let (s, r, a, n) = if i == 0 {
                                    (s1, r1, a1, 0)
                                } else {
                                    (s2, r2, a2, n2)
                                };
                                if s == r {
                                    let (b, nn) = m[&addrs[s]];
                                    if nn != n || b < a {
                                        return false;
                                    }
                                    m.insert(addrs[s], (b, nn + 1));
                                    return true;
                                }
                                let mut trial = m.clone();
                                if model_transfer(&mut trial, addrs[s], addrs[r], a, n).is_ok() {
                                    *m = trial;
                                    true
                                } else {
                                    false
                                }
                            };
                            let mut lenient = init.clone();
                            let included: Vec<usize> =
                                (0..2).filter(|i| apply(&mut lenient, *i)).collect();
                            let mut changes = StateChanges::new();
                            let mut touched = std::collections::BTreeSet::new();
                            for i in &included {
                                let (s, r) = if *i == 0 { (s1, r1) } else { (s2, r2) };
                                touched.insert(s);
                                touched.insert(r);
                            }
                            for i in touched {
                                let (b, n) = lenient[&addrs[i]];
                                changes.account(
                                    &addrs[i],
                                    &Account {
                                        address: addrs[i],
                                        balance: b,
                                        nonce: n,
                                    },
                                );
                            }

                            let setup = |node: &NodeBackend<InMemoryStorage>| {
                                node.ensure_genesis(&Genesis::dev()).unwrap();
                                for (i, (b, n)) in start.iter().enumerate() {
                                    node.storage
                                        .put_account(
                                            &addrs[i],
                                            &Account {
                                                address: addrs[i],
                                                balance: *b,
                                                nonce: *n,
                                            },
                                        )
                                        .unwrap();
                                }
                            };
                            let case = format!("{s1}->{r1} {a1}, {s2}->{r2} {a2}");

                            // Validation.
                            let validator = make_backend();
                            setup(&validator);
                            let mut block = build_valid_block(&validator, txs.to_vec());
                            let parent_root = tip(&validator).header.state_root;
                            block.header.state_root = next_state_root(&parent_root, &changes);
                            let strict_ok = included.len() == 2;
                            assert_eq!(
                                validator.apply_block(&block).is_ok(),
                                strict_ok,
                                "{case}: validation"
                            );

                            // Production.
                            let producer = make_backend();
                            setup(&producer);
                            pool(&producer, &txs).await;
                            let produced = produce(&producer).await;
                            let want: Vec<Transaction> =
                                included.iter().map(|i| txs[*i].clone()).collect();
                            assert_eq!(produced.body.transactions, want, "{case}: production");
                            assert_eq!(
                                produced.header.state_root,
                                next_state_root(&parent_root, &changes),
                                "{case}: root"
                            );
                            for (i, addr) in addrs.iter().enumerate() {
                                let acc = producer.storage.get_account(addr).unwrap().unwrap();
                                assert_eq!(
                                    (acc.balance, acc.nonce),
                                    lenient[addr],
                                    "{case}: account {i}"
                                );
                            }
                            let supply: u128 = addrs
                                .iter()
                                .map(|a| producer.storage.get_account(a).unwrap().unwrap().balance)
                                .sum();
                            assert_eq!(supply, 900, "{case}: I1");
                            if included.len() == 1 {
                                partial += 1;
                            }
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(
        cases,
        81 * 16,
        "every pair of transfers among three accounts, 4 amounts each"
    );
    assert!(partial > 100, "many blocks keep one of two: {partial}");
}

#[test]
fn the_kinds_are_the_rfcs() {
    assert_eq!((KIND_ACCOUNT, KIND_TASK, KIND_RECEIPT), (0, 1, 2));
}

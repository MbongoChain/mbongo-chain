//! Authenticated blocks (RFC 0006 §3, §5 rules t, u, v; plan S1-7).
//!
//! What these tests hold the node to:
//! - only the authority of the bound genesis produces a valid block, and
//!   every non-genesis block carries its strict Ed25519 signature over the
//!   whole header and this network's `chain_id`;
//! - rules (t), (u) and (v) are evaluated in that order, after height, parent
//!   and `transactions_root`, before any transaction, and a refusal writes
//!   nothing;
//! - a signature made for another network is refused, the authority key
//!   being the same;
//! - rule (w) still refuses a validly signed block with a wrong `state_root`;
//! - a node signs only with the authority's key;
//! - the shared vectors, `test-vectors/block/block-signature-v1.json`, give
//!   the same outcome here as their independent derivation states.

use super::tests::{
    address_of, build_valid_block, dev_producer, fund, make_backend, sign_block, signed_transfer,
};
use super::*;
use ed25519_dalek::SigningKey;
use mbongo_core::genesis::dev_authority_seed;
use mbongo_storage::InMemoryStorage;
use serde_json::Value;
use std::collections::BTreeMap;

const VECTORS: &str = include_str!("../../../test-vectors/block/block-signature-v1.json");

fn vectors() -> Value {
    let v: Value = serde_json::from_str(VECTORS).expect("valid JSON");
    assert_eq!(v["fixture_version"].as_u64(), Some(1));
    v
}

/// What a refusal must leave untouched.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    height: u64,
    tip: Hash,
    accounts: Vec<Option<(u128, u64)>>,
    transactions: Vec<bool>,
}

fn snapshot<S: Storage>(
    node: &NodeBackend<S>,
    watched: &[Address],
    txs: &[Transaction],
) -> Snapshot {
    let height = node.storage.get_latest_height().unwrap();
    let tip = compute_block_hash(&node.storage.get_block_by_height(height).unwrap().unwrap());
    Snapshot {
        height,
        tip,
        accounts: watched
            .iter()
            .map(|a| node.storage.get_account(a).unwrap().map(|x| (x.balance, x.nonce)))
            .collect(),
        transactions: txs
            .iter()
            .map(|tx| node.storage.get_transaction(&compute_tx_hash(tx)).unwrap().is_some())
            .collect(),
    }
}

/// A second network with the same authority key as the dev chain: its file
/// differs, so its `chain_id` does.
fn other_genesis() -> Genesis {
    let v = vectors();
    let file = v["chains"]["other"]["genesis_file"].as_str().unwrap();
    let genesis = Genesis::from_file_bytes(file.as_bytes()).unwrap();
    assert_eq!(genesis.authority(), Genesis::dev().authority());
    assert_ne!(genesis.chain_id, Genesis::dev().chain_id);
    genesis
}

fn dev_chain() -> ChainId {
    Genesis::dev().chain_id
}

/// The S1-7 rules' errors, by the names the vectors use.
fn outcome(result: &Result<Hash, ApplyBlockError>) -> &'static str {
    match result {
        Ok(_) => "accepted",
        Err(ApplyBlockError::BadParent { .. }) => "BadParent",
        Err(ApplyBlockError::UnauthorizedProducer { .. }) => "UnauthorizedProducer",
        Err(ApplyBlockError::InvalidBlockSignature) => "InvalidBlockSignature",
        Err(ApplyBlockError::InvalidTimestamp { .. }) => "InvalidTimestamp",
        Err(other) => panic!("not an outcome of these vectors: {other:?}"),
    }
}

// ── The shared vectors ─────────────────────────────────────────────────

#[test]
fn the_vector_blocks_get_the_files_outcomes_and_a_refusal_writes_nothing() {
    let v = vectors();
    let cases = v["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 24);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let chain = c["chain"].as_str().unwrap();
        let genesis = match chain {
            "dev" => Genesis::dev(),
            "other" => other_genesis(),
            _ => panic!("{name}: unknown chain"),
        };
        // The node derives the genesis block the vectors state.
        assert_eq!(
            hex::encode(compute_block_hash(&genesis_block(&genesis)).0),
            v["chains"][chain]["genesis_block_hash"].as_str().unwrap(),
            "{name}: genesis block hash"
        );
        let node = NodeBackend::new(InMemoryStorage::new(), false);
        node.ensure_genesis(&genesis).unwrap();
        let bytes = hex::decode(c["block_encoding"].as_str().unwrap()).unwrap();
        let block = Block::decode(&mut &bytes[..]).unwrap();
        let hash = compute_block_hash(&block);
        assert_eq!(
            hex::encode(hash.0),
            c["block_hash"].as_str().unwrap(),
            "{name}"
        );

        let before = snapshot(&node, &[], &[]);
        let result = node.apply_block(&block);
        let got = outcome(&result);
        assert_eq!(
            got,
            c["apply_block"].as_str().unwrap(),
            "{name}: {result:?}"
        );
        if got == "accepted" {
            assert_eq!(result.unwrap(), hash);
            assert_eq!(node.storage.get_latest_height().unwrap(), 1, "{name}");
            assert_eq!(node.storage.get_block(&hash).unwrap(), Some(block));
        } else {
            assert_eq!(snapshot(&node, &[], &[]), before, "{name}: nothing written");
            assert_eq!(node.storage.get_block(&hash).unwrap(), None, "{name}");
        }
        *seen.entry(got.to_string()).or_default() += 1;
    }
    let expected: BTreeMap<String, usize> = [
        ("accepted", 3),
        ("BadParent", 1),
        ("InvalidBlockSignature", 14),
        ("InvalidTimestamp", 2),
        ("UnauthorizedProducer", 4),
    ]
    .into_iter()
    .map(|(k, n)| (k.to_string(), n))
    .collect();
    assert_eq!(seen, expected);
}

#[test]
fn the_error_values_name_what_was_refused() {
    let v = vectors();
    let case = |name: &str| {
        v["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()
            .clone()
    };
    let decode = |c: &Value| {
        let bytes = hex::decode(c["block_encoding"].as_str().unwrap()).unwrap();
        Block::decode(&mut &bytes[..]).unwrap()
    };

    let dev = NodeBackend::new(InMemoryStorage::new(), false);
    dev.ensure_genesis(&Genesis::dev()).unwrap();
    let forged = decode(&case("unauthorized-producer-valid-signature"));
    match dev.apply_block(&forged) {
        Err(ApplyBlockError::UnauthorizedProducer { expected, got }) => {
            assert_eq!(expected, Genesis::dev().authority());
            assert_eq!(got, forged.header.producer);
        }
        other => panic!("expected UnauthorizedProducer, got {other:?}"),
    }

    let other = NodeBackend::new(InMemoryStorage::new(), false);
    other.ensure_genesis(&other_genesis()).unwrap();
    let early = decode(&case("timestamp-below-parent"));
    match other.apply_block(&early) {
        Err(ApplyBlockError::InvalidTimestamp { parent, got }) => {
            assert_eq!(parent, other_genesis().timestamp);
            assert_eq!(got, parent - 1);
        }
        other => panic!("expected InvalidTimestamp, got {other:?}"),
    }
}

// ── Each rule, on a chain with transactions ─────────────────────────────

/// A dev producer and a follower at height 1, both holding the same funded
/// account, and a valid block 2 carrying one transfer, not yet applied.
fn at_height_one() -> (
    NodeBackend<InMemoryStorage>,
    NodeBackend<InMemoryStorage>,
    Vec<Address>,
    Vec<Transaction>,
    Block,
) {
    let producer = make_backend();
    let follower = NodeBackend::new(InMemoryStorage::new(), false);
    let alice = SigningKey::from_bytes(&[0xB1u8; 32]);
    let bob = Address([0xB2u8; 32]);
    for node in [&producer, &follower] {
        node.ensure_genesis(&Genesis::dev()).unwrap();
        fund(node, &alice, 1_000);
    }
    let first = build_valid_block(&producer, vec![]);
    producer.apply_block(&first).unwrap();
    follower.apply_block(&first).unwrap();
    let txs = vec![signed_transfer(&alice, bob, 10, 0)];
    let second = build_valid_block(&follower, txs.clone());
    (
        producer,
        follower,
        vec![address_of(&alice), bob],
        txs,
        second,
    )
}

#[test]
fn rule_t_refuses_a_producer_that_is_not_the_authority() {
    let (_, follower, watched, txs, good) = at_height_one();
    let before = snapshot(&follower, &watched, &txs);
    let intruder = [0x5Du8; 32];
    let mut forged = good.clone();
    sign_block(&mut forged, &intruder, &dev_chain());
    assert!(
        forged.verify_producer_signature(&dev_chain()),
        "a correct signature by another key"
    );
    match follower.apply_block(&forged) {
        Err(ApplyBlockError::UnauthorizedProducer { expected, got }) => {
            assert_eq!(expected, Genesis::dev().authority());
            assert_eq!(got, address_of(&SigningKey::from_bytes(&intruder)));
        }
        other => panic!("expected UnauthorizedProducer, got {other:?}"),
    }
    assert_eq!(snapshot(&follower, &watched, &txs), before);
    follower.apply_block(&good).unwrap();
}

#[test]
fn rule_u_refuses_a_signature_that_does_not_verify() {
    let (_, follower, watched, txs, good) = at_height_one();
    let before = snapshot(&follower, &watched, &txs);
    let mut cases: Vec<(&str, Block)> = Vec::new();
    {
        // The authority named, another key signs.
        let mut b = good.clone();
        let sk = SigningKey::from_bytes(&[0x5Eu8; 32]);
        b.producer_signature = {
            use ed25519_dalek::Signer;
            sk.sign(&b.header.signing_hash(&dev_chain())).to_bytes()
        };
        cases.push(("another key signs", b));
    }
    {
        let mut b = good.clone();
        b.producer_signature[63] ^= 0x01;
        cases.push(("one bit of s", b));
    }
    {
        let mut b = good.clone();
        b.producer_signature[0] ^= 0x01;
        cases.push(("one bit of R", b));
    }
    {
        let mut b = good.clone();
        b.producer_signature = [0u8; 64];
        cases.push(("zero", b));
    }
    {
        // Valid for the header before it was changed: the signature covers
        // every header field.
        let mut b = good.clone();
        b.header.timestamp += 1;
        cases.push(("timestamp changed after signing", b));
    }
    {
        let mut b = good.clone();
        b.header.state_root.0[0] ^= 0x01;
        cases.push(("state_root changed after signing", b));
    }
    assert_eq!(cases.len(), 6);
    for (what, block) in cases {
        assert!(
            matches!(
                follower.apply_block(&block),
                Err(ApplyBlockError::InvalidBlockSignature)
            ),
            "{what}"
        );
        assert_eq!(snapshot(&follower, &watched, &txs), before, "{what}");
    }
    follower.apply_block(&good).unwrap();
}

#[test]
fn rule_v_refuses_a_timestamp_below_the_parents_and_has_no_upper_bound() {
    let (_, follower, watched, txs, good) = at_height_one();
    let parent = follower.storage.get_block_by_height(1).unwrap().unwrap();
    assert!(parent.header.timestamp > 0);
    let before = snapshot(&follower, &watched, &txs);
    let mut early = good.clone();
    early.header.timestamp = parent.header.timestamp - 1;
    sign_block(&mut early, &dev_authority_seed(), &dev_chain());
    match follower.apply_block(&early) {
        Err(ApplyBlockError::InvalidTimestamp { parent: p, got }) => {
            assert_eq!(p, parent.header.timestamp);
            assert_eq!(got, parent.header.timestamp - 1);
        }
        other => panic!("expected InvalidTimestamp, got {other:?}"),
    }
    assert_eq!(snapshot(&follower, &watched, &txs), before);

    // Equal to the parent's: accepted.
    let mut equal = good.clone();
    equal.header.timestamp = parent.header.timestamp;
    sign_block(&mut equal, &dev_authority_seed(), &dev_chain());
    follower.apply_block(&equal).unwrap();

    // u64::MAX: accepted, no wall-clock bound. The next block can only
    // carry u64::MAX too, and it is accepted.
    let mut last = build_valid_block(&follower, vec![]);
    last.header.timestamp = u64::MAX;
    sign_block(&mut last, &dev_authority_seed(), &dev_chain());
    follower.apply_block(&last).unwrap();
    let after = build_valid_block(&follower, vec![]);
    assert_eq!(after.header.timestamp, u64::MAX);
    follower.apply_block(&after).unwrap();
}

#[test]
fn rules_t_u_v_are_checked_in_order_before_any_transaction() {
    let (_, follower, watched, _, _) = at_height_one();
    let alice = SigningKey::from_bytes(&[0xB1u8; 32]);
    // A transaction that would fail: more than alice holds.
    let failing = vec![signed_transfer(&alice, Address([0xB3u8; 32]), 5_000, 0)];
    let base = build_valid_block(&follower, failing.clone());
    let parent_ts = follower.storage.get_block_by_height(1).unwrap().unwrap().header.timestamp;
    let before = snapshot(&follower, &watched, &failing);

    // The transaction alone: it is what fails.
    assert!(matches!(
        follower.apply_block(&base),
        Err(ApplyBlockError::InsufficientBalance(0))
    ));

    let variant = |producer_seed: [u8; 32], timestamp: u64, break_signature: bool| {
        let mut b = base.clone();
        b.header.timestamp = timestamp;
        sign_block(&mut b, &producer_seed, &dev_chain());
        if break_signature {
            b.producer_signature[40] ^= 0x01;
        }
        b
    };
    let authority = dev_authority_seed();
    let intruder = [0x5Fu8; 32];
    let late = parent_ts;
    let early = parent_ts - 1;
    let cases = [
        (
            "(v) and a failing transaction",
            variant(authority, early, false),
            "InvalidTimestamp",
        ),
        (
            "(u) and a failing transaction",
            variant(authority, late, true),
            "InvalidBlockSignature",
        ),
        (
            "(u), (v) and a failing transaction",
            variant(authority, early, true),
            "InvalidBlockSignature",
        ),
        (
            "(t) and a failing transaction",
            variant(intruder, late, false),
            "UnauthorizedProducer",
        ),
        (
            "(t), (u), (v) and a failing transaction",
            variant(intruder, early, true),
            "UnauthorizedProducer",
        ),
    ];
    assert_eq!(cases.len(), 5);
    for (what, block, want) in &cases {
        let result = follower.apply_block(block);
        assert_eq!(outcome(&result), *want, "{what}: {result:?}");
        assert_eq!(snapshot(&follower, &watched, &failing), before, "{what}");
    }

    // The earlier checks keep their precedence: a wrong transactions_root is
    // reported before (t), (u) and (v).
    let mut wrong_root = variant(intruder, early, true);
    wrong_root.header.transactions_root.0[0] ^= 0x01;
    assert!(matches!(
        follower.apply_block(&wrong_root),
        Err(ApplyBlockError::TransactionsRootMismatch)
    ));
}

#[test]
fn a_validly_signed_block_with_a_wrong_state_root_is_refused_by_rule_w() {
    let (_, follower, watched, txs, good) = at_height_one();
    let before = snapshot(&follower, &watched, &txs);
    let right = good.header.state_root;
    let mut tampered = good.clone();
    tampered.header.state_root.0[31] ^= 0x01;
    // Not re-signed: rule (u) refuses it first.
    assert!(matches!(
        follower.apply_block(&tampered),
        Err(ApplyBlockError::InvalidBlockSignature)
    ));
    // Re-signed by the authority: the signature verifies, rule (w) refuses.
    sign_block(&mut tampered, &dev_authority_seed(), &dev_chain());
    assert!(tampered.verify_producer_signature(&dev_chain()));
    match follower.apply_block(&tampered) {
        Err(ApplyBlockError::StateRootMismatch { expected, got }) => {
            assert_eq!(expected, right);
            assert_eq!(got, tampered.header.state_root);
        }
        other => panic!("expected StateRootMismatch, got {other:?}"),
    }
    assert_eq!(snapshot(&follower, &watched, &txs), before);
    follower.apply_block(&good).unwrap();
}

// ── Another network, the same authority key ────────────────────────────

#[tokio::test]
async fn a_block_signed_for_another_network_is_refused() {
    // Two networks whose genesis files name the same authority key.
    let other = other_genesis();
    let on_other = dev_producer(InMemoryStorage::new());
    on_other.ensure_genesis(&other).unwrap();
    let on_dev = NodeBackend::new(InMemoryStorage::new(), false);
    on_dev.ensure_genesis(&Genesis::dev()).unwrap();

    on_other.produce_block().await.unwrap();
    let produced = on_other.storage.get_block_by_height(1).unwrap().unwrap();
    assert!(produced.verify_producer_signature(&other.chain_id));
    assert!(!produced.verify_producer_signature(&dev_chain()));

    // Replayed whole: another parent.
    assert!(matches!(
        on_dev.apply_block(&produced),
        Err(ApplyBlockError::BadParent { .. })
    ));
    // The same block moved onto the dev chain, everything right but the
    // signature, which is the other network's: refused by rule (u).
    let mut moved = build_valid_block(&on_dev, vec![]);
    moved.header.timestamp = produced.header.timestamp;
    sign_block(&mut moved, &dev_authority_seed(), &other.chain_id);
    assert_eq!(moved.header.producer, produced.header.producer);
    assert!(matches!(
        on_dev.apply_block(&moved),
        Err(ApplyBlockError::InvalidBlockSignature)
    ));
    assert_eq!(on_dev.storage.get_latest_height().unwrap(), 0);
    // Signed for the dev chain, it applies.
    sign_block(&mut moved, &dev_authority_seed(), &dev_chain());
    on_dev.apply_block(&moved).unwrap();
}

// ── Production ─────────────────────────────────────────────────────────

#[tokio::test]
async fn a_produced_block_is_signed_by_the_authority_and_a_follower_applies_it() {
    let producer = make_backend();
    let follower = NodeBackend::new(InMemoryStorage::new(), false);
    let alice = SigningKey::from_bytes(&[0xC1u8; 32]);
    for node in [&producer, &follower] {
        node.ensure_genesis(&Genesis::dev()).unwrap();
        fund(node, &alice, 1_000);
    }
    let mut produced = 0;
    for nonce in 0..4u64 {
        producer
            .submit_transaction(signed_transfer(&alice, Address([0xC2u8; 32]), 1, nonce))
            .await
            .unwrap();
        let hash: Hash = producer.produce_block().await.unwrap().parse().unwrap();
        let block = producer.storage.get_block(&hash).unwrap().unwrap();
        assert_eq!(block.header.producer, Genesis::dev().authority());
        assert!(block.verify_producer_signature(&dev_chain()));
        assert_eq!(block.body.transactions.len(), 1);
        assert_eq!(follower.apply_block(&block).unwrap(), hash);
        produced += 1;
    }
    assert_eq!(produced, 4);
    assert_eq!(follower.storage.get_latest_height().unwrap(), 4);
}

#[tokio::test]
async fn production_needs_the_authority_key() {
    let alice = SigningKey::from_bytes(&[0xC3u8; 32]);
    let tx = signed_transfer(&alice, Address([0xC4u8; 32]), 1, 0);

    // A producer with no key, and one with a key that is not the authority.
    let keyless = NodeBackend::new(InMemoryStorage::new(), true);
    let mut wrong = NodeBackend::new(InMemoryStorage::new(), true);
    wrong.set_producer_key(&[0x5Au8; 32]);
    // A node that does not produce, even with the authority key.
    let mut follower = NodeBackend::new(InMemoryStorage::new(), false);
    follower.set_producer_key(&dev_authority_seed());
    let nodes = [
        ("no key", keyless, "no producer key"),
        ("not the authority", wrong, "is not the authority"),
        ("not a producer", follower, "not configured as producer"),
    ];
    assert_eq!(nodes.len(), 3);
    for (what, node, message) in nodes {
        node.ensure_genesis(&Genesis::dev()).unwrap();
        fund(&node, &alice, 10);
        node.submit_transaction(tx.clone()).await.unwrap();
        let before = snapshot(&node, &[address_of(&alice)], std::slice::from_ref(&tx));
        let err = node.produce_block().await.unwrap_err();
        assert!(err.to_string().contains(message), "{what}: {err}");
        assert_eq!(
            snapshot(&node, &[address_of(&alice)], std::slice::from_ref(&tx)),
            before,
            "{what}"
        );
        assert!(
            node.is_pending(&compute_tx_hash(&tx)).await,
            "{what}: still pending"
        );
    }
}

#[tokio::test]
async fn a_producer_key_of_another_network_does_not_sign_this_one() {
    // The vectors' state-root genesis names the authority of seed 0x10: a
    // node holding the dev authority key cannot produce its blocks.
    let v: Value = serde_json::from_str(include_str!(
        "../../../test-vectors/state/state-root-v1.json"
    ))
    .unwrap();
    let genesis =
        Genesis::from_file_bytes(v["genesis"]["file"].as_str().unwrap().as_bytes()).unwrap();
    assert_ne!(genesis.authority(), Genesis::dev().authority());
    let node = dev_producer(InMemoryStorage::new());
    node.ensure_genesis(&genesis).unwrap();
    let err = node.produce_block().await.unwrap_err();
    assert!(err.to_string().contains("is not the authority"), "{err}");
    assert_eq!(node.storage.get_latest_height().unwrap(), 0);
}

#[test]
fn the_genesis_block_carries_no_producer_and_no_signature() {
    for genesis in [Genesis::dev(), other_genesis()] {
        let block = genesis_block(&genesis);
        assert_eq!(block.header.producer, Address::zero());
        assert_eq!(block.producer_signature, [0u8; 64]);
        assert!(!block.verify_producer_signature(&genesis.chain_id));
    }
}

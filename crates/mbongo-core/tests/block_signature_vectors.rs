//! Rust side of the shared block-signature vectors,
//! `test-vectors/block/block-signature-v1.json` (RFC 0006 §3, §5 rules t, u
//! and v).
//!
//! The file was derived without this crate: Ed25519 from RFC 8032 in BigInt
//! arithmetic, checked against OpenSSL; BLAKE3 from its specification; SCALE
//! by hand. This file checks that `mbongo-core` gives the same header and
//! block encodings, the same block hashes and signing hashes, and the same
//! strict verdicts. What a node does with each block is checked against the
//! same file by `mbongo-node` (`backend_block_tests.rs`).

use mbongo_core::{
    chain_id, genesis_state_root, Address, Block, BlockBody, BlockHeader, ChainId, Genesis, Hash,
    BLOCK_SIGNING_DOMAIN,
};
use parity_scale_codec::{Decode, Encode};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../test-vectors/block/block-signature-v1.json");

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

fn chain_of(v: &Value, name: &str) -> ChainId {
    ChainId(key(&v["chains"][name]["chain_id"]))
}

/// The header a case lists, field by field.
fn header(h: &Value) -> BlockHeader {
    BlockHeader {
        parent_hash: Hash(key(&h["parent_hash"])),
        state_root: Hash(key(&h["state_root"])),
        transactions_root: Hash(key(&h["transactions_root"])),
        timestamp: h["timestamp"].as_str().unwrap().parse().unwrap(),
        height: h["height"].as_str().unwrap().parse().unwrap(),
        producer: Address(key(&h["producer"])),
    }
}

fn signature(c: &Value) -> [u8; 64] {
    bytes(&c["producer_signature"]).try_into().expect("64 bytes")
}

#[test]
fn the_fixture_holds_the_cases_it_states() {
    let v = fixture();
    let cases = v["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 24, "case count");
    let count = |field: &str, value: &str| {
        cases.iter().filter(|c| c[field].as_str() == Some(value)).count()
    };
    assert_eq!(count("expected", "valid"), 6);
    assert_eq!(count("expected", "invalid"), 18);
    assert_eq!(count("apply_block", "accepted"), 3);
    assert_eq!(count("apply_block", "BadParent"), 1);
    assert_eq!(count("apply_block", "UnauthorizedProducer"), 4);
    assert_eq!(count("apply_block", "InvalidBlockSignature"), 14);
    assert_eq!(count("apply_block", "InvalidTimestamp"), 2);
    assert_eq!(count("chain", "dev") + count("chain", "other"), 24);
}

#[test]
fn the_domain_is_the_rfcs() {
    let v = fixture();
    assert_eq!(BLOCK_SIGNING_DOMAIN, b"mbongo-block:v1");
    assert_eq!(bytes(&v["domain"]["hex"]), BLOCK_SIGNING_DOMAIN);
    assert_eq!(
        v["domain"]["length"].as_u64(),
        Some(BLOCK_SIGNING_DOMAIN.len() as u64)
    );
}

#[test]
fn the_chains_and_their_genesis_blocks_are_the_files() {
    let v = fixture();
    let mut checked = 0;
    for name in ["dev", "other"] {
        let c = &v["chains"][name];
        let file = c["genesis_file"].as_str().unwrap().as_bytes();
        let genesis = Genesis::from_file_bytes(file).expect("a canonical genesis file");
        assert_eq!(genesis.chain_id, chain_of(&v, name), "{name}: chain_id");
        assert_eq!(chain_id(file), genesis.chain_id);
        assert_eq!(genesis.authority().0, key(&c["authority"]), "{name}");
        assert_eq!(
            genesis_state_root(&genesis.chain_id, &genesis.allocations).0,
            key(&c["genesis_state_root"]),
            "{name}: state_root(0)"
        );
        // The genesis block: zero producer and zero signature (RFC 0006 §3).
        let block = Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: genesis_state_root(&genesis.chain_id, &genesis.allocations),
                transactions_root: mbongo_core::compute_transactions_root(&[]),
                timestamp: genesis.timestamp,
                height: 0,
                producer: Address::zero(),
            },
            body: BlockBody::default(),
            producer_signature: [0u8; 64],
        };
        assert_eq!(
            block.encode(),
            bytes(&c["genesis_block_encoding"]),
            "{name}"
        );
        assert_eq!(
            blake3::hash(&block.encode()).as_bytes(),
            &key(&c["genesis_block_hash"]),
            "{name}: genesis block hash"
        );
        checked += 1;
    }
    assert_eq!(checked, 2);
    // The dev file in the fixture is the built-in dev genesis.
    assert_eq!(
        v["chains"]["dev"]["genesis_file"].as_str().unwrap().as_bytes(),
        mbongo_core::genesis::DEV_GENESIS_FILE
    );
    // The two networks share one authority key and differ in chain_id.
    assert_eq!(
        v["chains"]["dev"]["authority"],
        v["chains"]["other"]["authority"]
    );
    assert_ne!(chain_of(&v, "dev"), chain_of(&v, "other"));
}

#[test]
fn every_encoding_hash_and_verdict_is_the_files() {
    let v = fixture();
    let cases = v["cases"].as_array().unwrap();
    let mut checked = 0;
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let chain = chain_of(&v, c["chain"].as_str().unwrap());
        let header = header(&c["header"]);

        // The header encoding, producer appended last.
        assert_eq!(header.encode(), bytes(&c["header_encoding"]), "{name}");
        assert_eq!(header.encode().len(), 144, "{name}");
        assert_eq!(
            &header.encode()[112..],
            &header.producer.0,
            "{name}: producer last"
        );
        assert_eq!(
            BlockHeader::decode(&mut &bytes(&c["header_encoding"])[..]).unwrap(),
            header
        );

        // The 32 bytes the producer signs.
        assert_eq!(
            header.signing_hash(&chain).to_vec(),
            bytes(&c["signing_hash"]),
            "{name}: signing hash"
        );

        // The block encoding, signature appended last, and its hash over
        // everything, the signature included.
        let block = Block {
            header: header.clone(),
            body: BlockBody::default(),
            producer_signature: signature(c),
        };
        let encoding = bytes(&c["block_encoding"]);
        assert_eq!(block.encode(), encoding, "{name}: block encoding");
        assert_eq!(&encoding[encoding.len() - 64..], &signature(c), "{name}");
        let mut input = &encoding[..];
        assert_eq!(Block::decode(&mut input).unwrap(), block, "{name}");
        assert!(input.is_empty(), "{name}: the decoder consumes every byte");
        assert_eq!(
            blake3::hash(&encoding).as_bytes(),
            &key(&c["block_hash"]),
            "{name}: block hash"
        );

        // The strict verdict.
        let expected = match c["expected"].as_str().unwrap() {
            "valid" => true,
            "invalid" => false,
            other => panic!("{name}: unknown verdict {other}"),
        };
        assert_eq!(
            block.verify_producer_signature(&chain),
            expected,
            "{name}: strict verdict"
        );
        assert_eq!(
            mbongo_core::crypto::verify_ed25519_strict(
                &header.producer.0,
                &header.signing_hash(&chain),
                &signature(c),
            ),
            expected,
            "{name}"
        );
        checked += 1;
    }
    assert_eq!(checked, 24);
}

#[test]
fn a_signature_is_bound_to_its_chain_id() {
    // The cross-chain case: the same header and the same authority key, two
    // networks. The signature for one is refused by the other.
    let v = fixture();
    let dev = chain_of(&v, "dev");
    let other = chain_of(&v, "other");
    let case = |name: &str| {
        v["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("case {name}"))
            .clone()
    };
    let valid = case("valid-dev");
    let replayed = case("cross-chain-signature");
    assert_eq!(valid["header"], replayed["header"], "one header");
    let h = header(&valid["header"]);
    assert_ne!(h.signing_hash(&dev), h.signing_hash(&other));
    let block = |c: &Value| Block {
        header: h.clone(),
        body: BlockBody::default(),
        producer_signature: signature(c),
    };
    assert!(block(&valid).verify_producer_signature(&dev));
    assert!(!block(&valid).verify_producer_signature(&other));
    assert!(block(&replayed).verify_producer_signature(&other));
    assert!(!block(&replayed).verify_producer_signature(&dev));
}

#[test]
fn a_block_in_the_format_before_s1_7_does_not_decode() {
    // An S1-6 block: the same fields without producer and producer_signature.
    // It must not decode as a block of this format, so that a directory or a
    // peer holding one is refused rather than misread.
    #[derive(Encode)]
    struct HeaderBeforeS17 {
        parent_hash: Hash,
        state_root: Hash,
        transactions_root: Hash,
        timestamp: u64,
        height: u64,
    }
    #[derive(Encode)]
    struct BlockBeforeS17 {
        header: HeaderBeforeS17,
        body: BlockBody,
    }
    let genesis = Genesis::dev();
    let legacy = BlockBeforeS17 {
        header: HeaderBeforeS17 {
            parent_hash: Hash::zero(),
            state_root: genesis_state_root(&genesis.chain_id, &genesis.allocations),
            transactions_root: mbongo_core::compute_transactions_root(&[]),
            timestamp: genesis.timestamp,
            height: 0,
        },
        body: BlockBody::default(),
    }
    .encode();
    assert_eq!(legacy.len(), 32 * 3 + 8 + 8 + 1);
    assert!(Block::decode(&mut &legacy[..]).is_err());
}

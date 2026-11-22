//! Rust side of the shared genesis vectors, `test-vectors/genesis/genesis-v1.json`.
//!
//! The files, chain identifiers and public keys there were derived from
//! RFC 0006 §1 without this crate: files written by hand, `chain_id` from an
//! independent BLAKE3, and keys from an independent Ed25519. This file checks
//! that `Genesis::from_file_bytes` accepts every valid file with the same
//! contents and identifier, and refuses every invalid one with its class.

use ed25519_dalek::SigningKey;
use mbongo_core::genesis::{dev_authority_seed, DEV_GENESIS_FILE};
use mbongo_core::{chain_id, Address, Genesis, GenesisError};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../test-vectors/genesis/genesis-v1.json");

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

fn address(v: &Value) -> Address {
    Address(bytes(v).try_into().expect("32 bytes"))
}

/// The class name of an error, as the fixture spells it.
fn class(e: GenesisError) -> String {
    let debug = format!("{e:?}");
    debug.split('(').next().unwrap().to_owned()
}

#[test]
fn valid_files_give_their_contents_and_chain_id() {
    let v = fixture();
    let cases = v["valid"].as_array().unwrap();
    assert_eq!(cases.len(), 4, "expected 4 valid files");
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let file = bytes(&c["file_hex"]);
        let g = Genesis::from_file_bytes(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            g.chain_id.to_string(),
            c["chain_id"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(chain_id(&file), g.chain_id, "{name}");
        let e = &c["expected"];
        assert_eq!(g.name, e["name"].as_str().unwrap(), "{name}");
        assert_eq!(
            g.timestamp.to_string(),
            e["timestamp"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(g.authorities, vec![address(&e["authority"])], "{name}");
        let want: Vec<(Address, u128)> = e["allocations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    address(&a["address"]),
                    a["amount"].as_str().unwrap().parse().unwrap(),
                )
            })
            .collect();
        assert_eq!(g.allocations, want, "{name}");
    }
}

#[test]
fn invalid_files_are_refused_with_their_class() {
    let v = fixture();
    let classes: Vec<&str> = v["error_classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    let cases = v["invalid"].as_array().unwrap();
    assert_eq!(cases.len(), 35, "expected 35 invalid files");
    let mut seen = std::collections::BTreeSet::new();
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let want = c["error"].as_str().unwrap();
        assert!(classes.contains(&want), "{name}: unknown class {want}");
        let err = Genesis::from_file_bytes(&bytes(&c["file_hex"]))
            .err()
            .unwrap_or_else(|| panic!("{name}: must be refused"));
        assert_eq!(class(err), want, "{name}: {err}");
        seen.insert(want);
    }
    // Every class but TooLarge, which needs a file over 1 MiB and has its own
    // unit test.
    assert_eq!(
        seen.len(),
        classes.len() - 1,
        "every class exercised: {seen:?}"
    );
}

#[test]
fn the_dev_genesis_is_the_committed_file_and_the_fixture_one() {
    let v = fixture();
    let dev: Vec<&Value> =
        v["valid"].as_array().unwrap().iter().filter(|c| c["name"] == "dev").collect();
    assert_eq!(dev.len(), 1);
    assert_eq!(
        bytes(&dev[0]["file_hex"]),
        DEV_GENESIS_FILE,
        "the committed dev file differs"
    );
    let d = &v["dev"];
    let account_seed: [u8; 32] = bytes(&d["account_seed"]).try_into().unwrap();
    assert_eq!(
        SigningKey::from_bytes(&account_seed).verifying_key().to_bytes(),
        address(&d["account"]).0
    );
    assert_eq!(dev_authority_seed().to_vec(), bytes(&d["authority_seed"]));
    assert_eq!(
        SigningKey::from_bytes(&dev_authority_seed()).verifying_key().to_bytes(),
        address(&d["authority"]).0
    );
    let worker_seed: [u8; 32] = bytes(&d["worker_seed"]).try_into().unwrap();
    assert_eq!(
        SigningKey::from_bytes(&worker_seed).verifying_key().to_bytes(),
        address(&d["worker"]).0
    );
    let g = Genesis::dev();
    assert_eq!(g.authority(), address(&d["authority"]));
    assert_eq!(
        g.allocations,
        vec![
            (
                address(&d["worker"]),
                d["worker_amount"].as_str().unwrap().parse().unwrap()
            ),
            (
                address(&d["account"]),
                d["amount"].as_str().unwrap().parse().unwrap()
            ),
        ]
    );
}

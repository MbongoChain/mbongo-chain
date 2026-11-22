//! Rust side of the shared strict Ed25519 vectors,
//! `test-vectors/signature/ed25519-strict-v2.json`.
//!
//! The verdicts were derived by an Ed25519 implementation written
//! independently of `ed25519-dalek` and of the SDK's library, reading the
//! strict definition of RFC 0006 §3. This file checks that the production
//! verifier reaches the same verdict on every case, both directly and through
//! `Transaction::verify_signature`. Receipt cases are consumed by
//! `mbongo-verification`, which owns receipt verification. The TypeScript
//! SDK reads the same file.

use mbongo_core::crypto::verify_ed25519_strict;
use mbongo_core::{Address, Transaction, TransactionPayload, TransactionType};
use parity_scale_codec::DecodeAll;
use serde_json::Value;

const STRICT_FIXTURE: &str = include_str!("../../../test-vectors/signature/ed25519-strict-v2.json");
const ANCHOR_FIXTURE: &str =
    include_str!("../../../test-vectors/transaction/anchor-receipt-v2.json");

fn fixture(raw: &str) -> Value {
    let v: Value = serde_json::from_str(raw).expect("valid JSON");
    assert_eq!(
        v["fixture_version"].as_u64(),
        Some(2),
        "unsupported fixture version"
    );
    v
}

/// The network a v2 fixture signs for: its `chain.chain_id`.
fn chain_of(v: &Value) -> mbongo_core::ChainId {
    let id: [u8; 32] = bytes("chain.chain_id", &v["chain"]["chain_id"]).try_into().unwrap();
    assert_eq!(
        id,
        mbongo_core::Genesis::dev().chain_id.0,
        "the fixture signs for the dev chain"
    );
    mbongo_core::ChainId(id)
}

fn bytes(field: &str, v: &Value) -> Vec<u8> {
    let s = v.as_str().unwrap_or_else(|| panic!("{field}: expected a hex string"));
    assert!(
        s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{field}: lowercase hex only"
    );
    hex::decode(s).unwrap_or_else(|e| panic!("{field}: {e}"))
}

fn arr<const N: usize>(field: &str, v: &Value) -> [u8; N] {
    bytes(field, v)
        .try_into()
        .unwrap_or_else(|_| panic!("{field}: expected {N} bytes"))
}

fn expected_valid(c: &Value) -> bool {
    match c["expected"].as_str() {
        Some("valid") => true,
        Some("invalid") => false,
        other => panic!("unexpected verdict {other:?}"),
    }
}

#[test]
fn every_case_has_the_strict_verdict() {
    let v = fixture(STRICT_FIXTURE);
    let cases = v["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 19, "expected 19 cases");
    let rules: Vec<&str> =
        v["rules"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
    let (mut valid, mut invalid) = (0, 0);
    for c in cases {
        let name = c["name"].as_str().unwrap();
        assert!(
            rules.contains(&c["rule"].as_str().unwrap()),
            "{name}: unknown rule"
        );
        let verdict = verify_ed25519_strict(
            &arr::<32>("public_key", &c["public_key"]),
            &bytes("message", &c["message"]),
            &arr::<64>("signature", &c["signature"]),
        );
        assert_eq!(verdict, expected_valid(c), "{name} ({})", c["rule"]);
        if verdict {
            valid += 1;
        } else {
            invalid += 1;
        }
    }
    assert_eq!((valid, invalid), (4, 15), "expected 4 valid and 15 invalid");
}

#[test]
fn transaction_cases_through_verify_signature() {
    let v = fixture(STRICT_FIXTURE);
    let mut seen = 0;
    for c in v["cases"].as_array().unwrap().iter().filter(|c| c["kind"] == "transaction") {
        seen += 1;
        let name = c["name"].as_str().unwrap();
        let t = &c["transaction"];
        assert_eq!(t["tx_type"], "Transfer", "{name}");
        let tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address(arr("sender", &t["sender"])),
            receiver: Address(arr("receiver", &t["receiver"])),
            amount: t["amount"].as_str().unwrap().parse().unwrap(),
            nonce: t["nonce"].as_str().unwrap().parse().unwrap(),
            payload: TransactionPayload::None,
            signature: arr("signature", &c["signature"]),
        };
        assert_eq!(
            tx.sender.0,
            arr::<32>("public_key", &c["public_key"]),
            "{name}: sender is the key"
        );
        assert_eq!(
            tx.signing_payload(&chain_of(&v)),
            bytes("message", &c["message"]),
            "{name}: message is the payload"
        );
        assert_eq!(
            tx.signed_fields(),
            bytes("transaction_fields", &c["transaction_fields"]),
            "{name}: fields"
        );
        assert_eq!(
            tx.verify_signature(&chain_of(&v)),
            expected_valid(c),
            "{name} ({})",
            c["rule"]
        );
    }
    assert_eq!(seen, 14, "expected 14 transaction cases");
}

#[test]
fn malformed_lengths_cannot_reach_the_verifier() {
    // The verifier takes fixed-width arrays, so a wrong length is refused by
    // the type before any verification. The fixture's cases must indeed not
    // fit.
    let v = fixture(STRICT_FIXTURE);
    let cases = v["malformed_lengths"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for c in cases {
        let key = bytes("public_key", &c["public_key"]);
        let sig = bytes("signature", &c["signature"]);
        let fits = <[u8; 32]>::try_from(key.as_slice()).is_ok()
            && <[u8; 64]>::try_from(sig.as_slice()).is_ok();
        assert!(!fits, "{}: must not fit the verifier's types", c["name"]);
        assert_eq!(c["expected"], "invalid");
    }
}

#[test]
fn existing_signed_transactions_still_verify() {
    let v = fixture(ANCHOR_FIXTURE);
    let valid = v["valid"].as_array().unwrap();
    assert_eq!(valid.len(), 2, "expected 2 valid anchor vectors");
    for a in valid {
        let full = bytes("full_transaction", &a["expected"]["full_transaction"]);
        let tx = Transaction::decode_all(&mut &full[..]).unwrap();
        assert!(
            tx.verify_signature(&chain_of(&v)),
            "{} must stay valid",
            a["name"]
        );
    }
}

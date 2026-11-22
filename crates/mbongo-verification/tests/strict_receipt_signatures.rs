//! Receipt cases of the shared strict Ed25519 vectors
//! (`test-vectors/signature/ed25519-strict-v2.json`), through
//! `verify_receipt_signature`, the rule (h) check. Every valid receipt of
//! `test-vectors/receipt/receipt-v1.json` must also stay valid.

use mbongo_core::Address;
use mbongo_verification::receipt::{verify_receipt_signature, Receipt};
use serde_json::Value;

const STRICT_FIXTURE: &str = include_str!("../../../test-vectors/signature/ed25519-strict-v2.json");
const RECEIPT_FIXTURE: &str = include_str!("../../../test-vectors/receipt/receipt-v1.json");

fn fixture(raw: &str) -> Value {
    let v: Value = serde_json::from_str(raw).expect("valid JSON");
    assert_eq!(
        v["fixture_version"].as_u64(),
        // The strict file is version 2 (S1-5), the receipt file version 1.
        Some(if v.get("supersedes").is_some() { 2 } else { 1 }),
        "unsupported fixture version"
    );
    v
}

fn bytes(v: &Value) -> Vec<u8> {
    let s = v.as_str().expect("hex string");
    assert!(s.len() % 2 == 0, "even-length hex");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn arr<const N: usize>(v: &Value) -> [u8; N] {
    bytes(v).try_into().unwrap_or_else(|_| panic!("expected {N} bytes"))
}

/// The receipt vector `name` of the receipt fixture, with its own signature.
fn receipt_vector(rx: &Value, name: &str) -> Receipt {
    let found: Vec<&Value> =
        rx["valid"].as_array().unwrap().iter().filter(|v| v["name"] == name).collect();
    assert_eq!(found.len(), 1, "exactly one receipt vector named {name}");
    let v = found[0];
    let m = &v["receipt"]["metadata"];
    assert_eq!(m["pattern"], "repeat");
    let byte = bytes(&m["byte"])[0];
    Receipt {
        version: u8::try_from(v["receipt"]["version"].as_u64().unwrap()).unwrap(),
        task_id: arr(&v["receipt"]["task_id"]),
        input_commitment: arr(&v["receipt"]["input_commitment"]),
        output_commitment: arr(&v["receipt"]["output_commitment"]),
        executor: Address(arr(&v["receipt"]["executor"])),
        metadata: vec![byte; usize::try_from(m["length"].as_u64().unwrap()).unwrap()],
        signature: arr(&v["expected"]["executor_signature"]),
    }
}

#[test]
fn receipt_cases_have_the_strict_verdict() {
    let v = fixture(STRICT_FIXTURE);
    let rx = fixture(RECEIPT_FIXTURE);
    let mut seen = 0;
    for c in v["cases"].as_array().unwrap().iter().filter(|c| c["kind"] == "receipt") {
        seen += 1;
        let name = c["name"].as_str().unwrap();
        let mut receipt = receipt_vector(&rx, c["receipt"]["receipt_vector"].as_str().unwrap());
        receipt.executor = Address(arr(&c["receipt"]["executor"]));
        receipt.signature = arr(&c["signature"]);
        assert_eq!(
            receipt.executor.0,
            arr::<32>(&c["public_key"]),
            "{name}: executor is the key"
        );
        assert_eq!(
            receipt.receipt_hash().0.to_vec(),
            bytes(&c["message"]),
            "{name}: message is the receipt hash"
        );
        let expected = c["expected"] == "valid";
        assert_eq!(
            verify_receipt_signature(&receipt),
            expected,
            "{name} ({})",
            c["rule"]
        );
    }
    assert_eq!(seen, 5, "expected 5 receipt cases");
}

#[test]
fn every_valid_receipt_vector_still_verifies() {
    let rx = fixture(RECEIPT_FIXTURE);
    let names: Vec<&str> = rx["valid"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert!(!names.is_empty());
    for name in &names {
        assert!(
            verify_receipt_signature(&receipt_vector(&rx, name)),
            "{name} must stay valid"
        );
    }
}

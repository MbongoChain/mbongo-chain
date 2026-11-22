//! Rust side of the shared nonce-boundary and account vectors.
//!
//! `test-vectors/transaction/nonce-boundary-v1.json` and
//! `test-vectors/account/account-v1.json` were **not** produced by this
//! crate. Byte layouts were written by hand from the field rules, integers by
//! explicit little-endian construction, signatures by an independent Ed25519
//! and hashes by an independent BLAKE3. This file is a consumer: the
//! production encoder must agree with values it did not produce. The
//! TypeScript SDK reads the same files.
//!
//! The state-transition section of the nonce fixture is consumed by the
//! `apply_block` tests in `mbongo-node`, which own block application.

use ed25519_dalek::{Signer, SigningKey};
use mbongo_core::{
    Account, Address, Receipt, Transaction, TransactionPayload, TransactionType,
    MAX_TRANSACTION_NONCE,
};
use parity_scale_codec::{DecodeAll, Encode};
use serde_json::Value;

const NONCE_FIXTURE: &str =
    include_str!("../../../test-vectors/transaction/nonce-boundary-v1.json");
const ACCOUNT_FIXTURE: &str = include_str!("../../../test-vectors/account/account-v1.json");
const RECEIPT_FIXTURE: &str = include_str!("../../../test-vectors/receipt/receipt-v1.json");

const SUPPORTED_FIXTURE_VERSION: u64 = 1;

fn doc(name: &str, raw: &str) -> Value {
    let v: Value =
        serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name}: invalid JSON: {e}"));
    assert_eq!(
        v["fixture_version"].as_u64(),
        Some(SUPPORTED_FIXTURE_VERSION),
        "{name}: unsupported fixture schema version"
    );
    v
}

fn hex_bytes(field: &str, v: &Value) -> Vec<u8> {
    let s = v.as_str().unwrap_or_else(|| panic!("{field}: expected a hex string"));
    assert!(!s.starts_with("0x"), "{field}: no 0x prefix allowed");
    assert!(
        s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{field}: lowercase hex only"
    );
    hex::decode(s).unwrap_or_else(|e| panic!("{field}: invalid hex: {e}"))
}

fn arr32(field: &str, v: &Value) -> [u8; 32] {
    hex_bytes(field, v)
        .try_into()
        .unwrap_or_else(|_| panic!("{field}: expected 32 bytes"))
}

fn arr64(field: &str, v: &Value) -> [u8; 64] {
    hex_bytes(field, v)
        .try_into()
        .unwrap_or_else(|_| panic!("{field}: expected 64 bytes"))
}

fn dec<T: std::str::FromStr>(field: &str, v: &Value) -> T {
    v.as_str()
        .unwrap_or_else(|| panic!("{field}: expected a decimal string"))
        .parse()
        .unwrap_or_else(|_| panic!("{field}: not a decimal in range"))
}

/// The fixture's `<test_key>`: the receipt fixture's TEST ONLY key.
fn test_key() -> SigningKey {
    let rx = doc("receipt fixture", RECEIPT_FIXTURE);
    let sk = SigningKey::from_bytes(&arr32(
        "test_key.ed25519_seed",
        &rx["test_key"]["ed25519_seed"],
    ));
    assert_eq!(
        sk.verifying_key().to_bytes(),
        arr32("test_key.public_key", &rx["test_key"]["public_key"]),
        "the receipt fixture's seed and public key disagree"
    );
    sk
}

fn address(field: &str, v: &Value, sk: &SigningKey) -> Address {
    match v.as_str() {
        Some("<test_key>" | "<receipt.executor>") => Address(sk.verifying_key().to_bytes()),
        _ => Address(arr32(field, v)),
    }
}

fn tx_type(name: &str) -> TransactionType {
    match name {
        "Transfer" => TransactionType::Transfer,
        "Stake" => TransactionType::Stake,
        other => panic!("unsupported plain tx_type {other}"),
    }
}

fn tx_hash(tx: &Transaction) -> [u8; 32] {
    // Mirrors compute_tx_hash in mbongo-node, which is pub(crate): BLAKE3
    // over the full SCALE encoding.
    *blake3::hash(&tx.encode()).as_bytes()
}

/// Checks one signed transaction against its `expected` block.
fn check_signed(name: &str, mut tx: Transaction, sk: &SigningKey, expected: &Value) {
    let payload = tx.signing_payload();
    assert_eq!(
        payload.len() as u64,
        expected["signing_payload_length"].as_u64().unwrap(),
        "{name}: signing payload length"
    );
    assert_eq!(
        payload,
        hex_bytes("signing_payload", &expected["signing_payload"]),
        "{name}: signing payload"
    );
    let sig = sk.sign(&payload).to_bytes();
    assert_eq!(
        sig,
        arr64("transaction_signature", &expected["transaction_signature"]),
        "{name}: transaction signature"
    );
    tx.signature = sig;
    assert!(tx.verify_signature(), "{name}: signature must verify");
    let full = tx.encode();
    assert_eq!(
        full.len() as u64,
        expected["full_transaction_length"].as_u64().unwrap(),
        "{name}: full length"
    );
    assert_eq!(
        full,
        hex_bytes("full_transaction", &expected["full_transaction"]),
        "{name}: full encoding"
    );
    assert_eq!(
        tx_hash(&tx),
        arr32("transaction_hash", &expected["transaction_hash"]),
        "{name}: hash"
    );
    let decoded = Transaction::decode_all(&mut &full[..])
        .unwrap_or_else(|e| panic!("{name}: canonical bytes must decode: {e}"));
    assert_eq!(decoded, tx, "{name}: decode round trip");
}

#[test]
fn constants_match_the_protocol_bound() {
    let v = doc("nonce fixture", NONCE_FIXTURE);
    let c = &v["constants"];
    assert_eq!(dec::<u64>("u64_max", &c["u64_max"]), u64::MAX);
    assert_eq!(
        dec::<u64>("max_transaction_nonce", &c["max_transaction_nonce"]),
        MAX_TRANSACTION_NONCE
    );
    assert_eq!(dec::<u128>("u128_max", &c["u128_max"]), u128::MAX);
}

#[test]
fn plain_transactions_encode_hash_and_sign_as_derived() {
    let v = doc("nonce fixture", NONCE_FIXTURE);
    let sk = test_key();
    let offsets = &v["signing_formula"]["offsets"];
    let vectors = v["plain"].as_array().expect("plain");
    assert_eq!(vectors.len(), 9, "expected 9 plain vectors");
    let mut at_bound = 0;
    for p in vectors {
        let name = p["name"].as_str().unwrap();
        let t = &p["transaction"];
        let tx = Transaction {
            tx_type: tx_type(t["tx_type"].as_str().unwrap()),
            sender: address("sender", &t["sender"], &sk),
            receiver: address("receiver", &t["receiver"], &sk),
            amount: dec("amount", &t["amount"]),
            nonce: dec("nonce", &t["nonce"]),
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        };
        // Field bytes at their fixed offsets.
        let payload = tx.signing_payload();
        let off = |k: &str| usize::try_from(offsets[k].as_u64().unwrap()).unwrap();
        assert_eq!(
            hex::encode(&payload[off("tx_type")..off("sender")]),
            p["expected"]["tx_type_byte"],
            "{name}"
        );
        assert_eq!(
            hex::encode(&payload[off("amount")..off("nonce")]),
            p["expected"]["amount_u128_le"],
            "{name}"
        );
        assert_eq!(
            hex::encode(&payload[off("nonce")..off("payload")]),
            p["expected"]["nonce_u64_le"],
            "{name}"
        );
        assert_eq!(
            hex::encode(&payload[off("payload")..]),
            p["expected"]["payload_byte"],
            "{name}"
        );
        if tx.nonce > MAX_TRANSACTION_NONCE {
            at_bound += 1;
        }
        check_signed(name, tx, &sk, &p["expected"]);
    }
    assert_eq!(at_bound, 2, "expected 2 plain vectors beyond the bound");
}

#[test]
fn anchor_transactions_at_the_bound_encode_as_derived() {
    let v = doc("nonce fixture", NONCE_FIXTURE);
    let rx = doc("receipt fixture", RECEIPT_FIXTURE);
    let sk = test_key();
    let vectors = v["anchor"].as_array().expect("anchor");
    assert_eq!(vectors.len(), 2, "expected 2 anchor vectors");
    for a in vectors {
        let name = a["name"].as_str().unwrap();
        let t = &a["transaction"];
        let rname = t["receipt_vector"].as_str().unwrap();
        let found: Vec<&Value> =
            rx["valid"].as_array().unwrap().iter().filter(|r| r["name"] == rname).collect();
        assert_eq!(
            found.len(),
            1,
            "{name}: exactly one receipt vector named {rname}"
        );
        let receipt_bytes = hex_bytes("full_encoding", &found[0]["expected"]["full_encoding"]);
        let receipt = Receipt::decode_all(&mut &receipt_bytes[..]).expect("receipt decodes");
        assert_eq!(
            receipt.executor.0,
            sk.verifying_key().to_bytes(),
            "{name}: executor is the test key"
        );
        let tx = Transaction {
            tx_type: TransactionType::AnchorReceipt,
            sender: address("sender", &t["sender"], &sk),
            receiver: address("receiver", &t["receiver"], &sk),
            amount: dec("amount", &t["amount"]),
            nonce: dec("nonce", &t["nonce"]),
            payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
            signature: [0u8; 64],
        };
        check_signed(name, tx, &sk, &a["expected"]);
    }
}

#[test]
fn malformed_encodings_do_not_decode() {
    let v = doc("nonce fixture", NONCE_FIXTURE);
    let cases = v["malformed_encodings"].as_array().expect("malformed_encodings");
    assert_eq!(cases.len(), 6, "expected 6 malformed encodings");
    for m in cases {
        let name = m["name"].as_str().unwrap();
        let bytes = hex_bytes(name, &m["hex"]);
        assert_eq!(
            bytes.len() as u64,
            m["hex_length"].as_u64().unwrap(),
            "{name}: length"
        );
        assert!(
            Transaction::decode_all(&mut &bytes[..]).is_err(),
            "{name}: must not decode ({})",
            m["reason"]
        );
    }
}

#[test]
fn signatures_over_the_wrong_message_do_not_verify() {
    let v = doc("nonce fixture", NONCE_FIXTURE);
    let cases = v["invalid_signatures"].as_array().expect("invalid_signatures");
    assert_eq!(cases.len(), 2, "expected 2 invalid signatures");
    for s in cases {
        let name = s["name"].as_str().unwrap();
        let base: Vec<&Value> = v["plain"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["name"] == s["base_vector"])
            .collect();
        assert_eq!(base.len(), 1, "{name}: exactly one base vector");
        let full = hex_bytes("full_transaction", &base[0]["expected"]["full_transaction"]);
        let mut tx = Transaction::decode_all(&mut &full[..]).unwrap();
        assert!(tx.verify_signature(), "{name}: the base vector verifies");
        tx.signature = arr64(name, &s["transaction_signature"]);
        assert!(
            !tx.verify_signature(),
            "{name}: must not verify ({})",
            s["reason"]
        );
    }
}

#[test]
fn nonces_outside_u64_are_not_representable() {
    let v = doc("nonce fixture", NONCE_FIXTURE);
    let cases = v["sdk_input_domain"].as_array().expect("sdk_input_domain");
    assert_eq!(cases.len(), 2, "expected 2 input-domain cases");
    for c in cases {
        let n = c["nonce"].as_str().unwrap();
        assert!(n.parse::<u64>().is_err(), "{n} must not fit in u64");
    }
}

#[test]
fn accounts_encode_as_derived() {
    let v = doc("account fixture", ACCOUNT_FIXTURE);
    let sk = test_key();
    assert_eq!(v["layout"]["length"].as_u64(), Some(56));
    let cases = v["valid"].as_array().expect("valid");
    assert_eq!(cases.len(), 4, "expected 4 account vectors");
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let a = &c["account"];
        let account = Account {
            address: address("address", &a["address"], &sk),
            balance: dec("balance", &a["balance"]),
            nonce: dec("nonce", &a["nonce"]),
        };
        let enc = account.encode();
        assert_eq!(
            enc.len() as u64,
            c["expected"]["encoding_length"].as_u64().unwrap(),
            "{name}: length"
        );
        assert_eq!(
            enc,
            hex_bytes("encoding", &c["expected"]["encoding"]),
            "{name}: encoding"
        );
        assert_eq!(
            hex::encode(&enc[32..48]),
            c["expected"]["balance_u128_le"],
            "{name}: balance bytes"
        );
        assert_eq!(
            hex::encode(&enc[48..56]),
            c["expected"]["nonce_u64_le"],
            "{name}: nonce bytes"
        );
        assert_eq!(
            Account::decode_all(&mut &enc[..]).unwrap(),
            account,
            "{name}: round trip"
        );
    }
    let bad = v["malformed_encodings"].as_array().expect("malformed_encodings");
    assert_eq!(bad.len(), 3, "expected 3 malformed accounts");
    for m in bad {
        let name = m["name"].as_str().unwrap();
        let bytes = hex_bytes(name, &m["hex"]);
        assert_eq!(
            bytes.len() as u64,
            m["hex_length"].as_u64().unwrap(),
            "{name}: length"
        );
        assert!(
            Account::decode_all(&mut &bytes[..]).is_err(),
            "{name}: must not decode"
        );
    }
}

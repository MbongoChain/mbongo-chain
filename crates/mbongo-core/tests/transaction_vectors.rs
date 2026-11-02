//! Rust side of the shared cross-language `AnchorReceipt` transaction vectors.
//!
//! The expected values in `test-vectors/transaction/anchor-receipt-v1.json`
//! were **not** produced by this crate. The signing payload was laid out by
//! hand from the field rules — a SCALE struct is its fields concatenated in
//! declaration order, an enum is one `codec(index)` byte, and fixed-width
//! integers are little-endian and never compact. The integers were built by
//! explicit little-endian construction, the signatures come from an
//! independent Ed25519, and the hashes from an independent BLAKE3.
//!
//! The only machine input was `test-vectors/receipt/receipt-v1.json`, which
//! #83 already derived independently. Receipt bytes are resolved from there
//! and never restated here, so there is exactly one receipt source of truth.
//!
//! So this file is not the encoder checking its own output. It proves the
//! production encoder agrees with values derived without it, which is what
//! makes the fixture worth anything to a second language. #85 will hold a
//! TypeScript consumer of this same file.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use mbongo_core::{Address, Receipt, Transaction, TransactionPayload, TransactionType};
use parity_scale_codec::Encode;
use serde_json::Value;

const TX_FIXTURE: &str = include_str!("../../../test-vectors/transaction/anchor-receipt-v1.json");
const RECEIPT_FIXTURE: &str = include_str!("../../../test-vectors/receipt/receipt-v1.json");

/// The fixture schema this test understands. A bump must be a deliberate
/// change here, not a silent divergence.
const SUPPORTED_FIXTURE_VERSION: u64 = 1;

fn doc(name: &str, raw: &str) -> Value {
    let v: Value =
        serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name}: invalid JSON: {e}"));
    let version = v["fixture_version"]
        .as_u64()
        .unwrap_or_else(|| panic!("{name}: fixture_version missing or not a number"));
    assert_eq!(
        version, SUPPORTED_FIXTURE_VERSION,
        "{name}: unsupported fixture schema version: this test understands {SUPPORTED_FIXTURE_VERSION}"
    );
    v
}

fn transactions() -> Value {
    doc("transaction fixture", TX_FIXTURE)
}

fn receipts() -> Value {
    doc("receipt fixture", RECEIPT_FIXTURE)
}

/// Decodes lowercase hex without a `0x` prefix, failing loudly.
fn hex_bytes(field: &str, v: &Value) -> Vec<u8> {
    let s = v.as_str().unwrap_or_else(|| panic!("{field}: expected a hex string"));
    assert!(
        !s.starts_with("0x"),
        "{field}: fixture hex must not carry an 0x prefix"
    );
    assert!(
        s.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "{field}: fixture hex must be lowercase"
    );
    hex::decode(s).unwrap_or_else(|e| panic!("{field}: invalid hex: {e}"))
}

fn fixed<const N: usize>(field: &str, v: &Value) -> [u8; N] {
    let bytes = hex_bytes(field, v);
    assert_eq!(
        bytes.len(),
        N,
        "{field}: expected {N} bytes, got {}",
        bytes.len()
    );
    bytes.try_into().expect("length checked above")
}

fn u64_at(field: &str, v: &Value) -> u64 {
    v.as_u64().unwrap_or_else(|| panic!("{field}: expected an unsigned integer"))
}

/// Expands `{pattern: "repeat", byte: "ab", length: n}` — the only pattern the
/// receipt schema defines, so anything else is a fixture error rather than
/// something to guess at.
fn metadata(field: &str, v: &Value) -> Vec<u8> {
    let pattern = v["pattern"].as_str().unwrap_or_else(|| panic!("{field}: missing pattern"));
    assert_eq!(pattern, "repeat", "{field}: unsupported metadata pattern");
    let byte = hex_bytes(&format!("{field}.byte"), &v["byte"]);
    assert_eq!(byte.len(), 1, "{field}.byte: expected exactly one byte");
    let len = u64_at(&format!("{field}.length"), &v["length"]) as usize;
    vec![byte[0]; len]
}

/// The TEST ONLY signing key, resolved from the receipt fixture rather than
/// restated here. Public seed; never a production key.
fn test_key(rdoc: &Value) -> SigningKey {
    let seed: [u8; 32] = fixed("test_key.ed25519_seed", &rdoc["test_key"]["ed25519_seed"]);
    let sk = SigningKey::from_bytes(&seed);
    let expected: [u8; 32] = fixed("test_key.public_key", &rdoc["test_key"]["public_key"]);
    assert_eq!(
        sk.verifying_key().to_bytes(),
        expected,
        "test_key: seed does not derive the public key the receipt fixture records"
    );
    sk
}

/// Resolves a named vector in the receipt fixture. Exactly one match is
/// required: zero means a dangling reference, more than one means the receipt
/// fixture has ambiguous names.
fn receipt_vector<'a>(rdoc: &'a Value, name: &str) -> &'a Value {
    let all = rdoc["valid"].as_array().expect("receipt fixture: valid is not an array");
    let matches: Vec<&Value> = all.iter().filter(|v| v["name"].as_str() == Some(name)).collect();
    assert_eq!(
        matches.len(),
        1,
        "receipt_vector {name:?}: expected exactly one match in the receipt fixture, found {}",
        matches.len()
    );
    matches[0]
}

/// Builds the signed receipt a transaction vector references, taking every
/// byte from the receipt fixture.
fn referenced_receipt(rdoc: &Value, name: &str) -> (Receipt, Vec<u8>, [u8; 32]) {
    let entry = receipt_vector(rdoc, name);
    let r = &entry["receipt"];
    let signature: [u8; 64] = fixed(
        "receipt.executor_signature",
        &entry["expected"]["executor_signature"],
    );
    let receipt = Receipt {
        version: u64_at("receipt.version", &r["version"]) as u8,
        task_id: fixed("receipt.task_id", &r["task_id"]),
        input_commitment: fixed("receipt.input_commitment", &r["input_commitment"]),
        output_commitment: fixed("receipt.output_commitment", &r["output_commitment"]),
        executor: Address(fixed("receipt.executor", &r["executor"])),
        metadata: metadata("receipt.metadata", &r["metadata"]),
        signature,
    };
    let full = hex_bytes("receipt.full_encoding", &entry["expected"]["full_encoding"]);
    let hash: [u8; 32] = fixed("receipt.receipt_hash", &entry["expected"]["receipt_hash"]);
    (receipt, full, hash)
}

/// Builds the transaction a valid vector describes. `sender` is not read from
/// the transaction fixture: consensus requires it to equal the receipt's
/// executor, so deriving it removes any chance of the two disagreeing.
fn transaction_from(entry: &Value, rdoc: &Value) -> (Transaction, Vec<u8>, [u8; 32]) {
    let t = &entry["transaction"];
    let name = t["receipt_vector"]
        .as_str()
        .expect("transaction.receipt_vector: expected a vector name");
    let (receipt, receipt_full, receipt_hash) = referenced_receipt(rdoc, name);

    let receiver = Address(fixed("transaction.receiver", &t["receiver"]));
    let amount = u128::from(u64_at("transaction.amount", &t["amount"]));
    assert_eq!(
        amount, 0,
        "a valid AnchorReceipt vector must carry amount 0"
    );
    assert_eq!(
        receiver,
        Address::zero(),
        "a valid AnchorReceipt vector must carry the zero receiver"
    );

    let tx = Transaction {
        tx_type: TransactionType::AnchorReceipt,
        sender: receipt.executor,
        receiver,
        amount,
        nonce: u64_at("transaction.nonce", &t["nonce"]),
        payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
        signature: fixed(
            "expected.transaction_signature",
            &entry["expected"]["transaction_signature"],
        ),
    };
    (tx, receipt_full, receipt_hash)
}

fn valid_vectors(tdoc: &Value) -> Vec<Value> {
    tdoc["valid"]
        .as_array()
        .expect("transaction fixture: valid is not an array")
        .clone()
}

/// The transaction hash rule, mirrored from `compute_tx_hash` in
/// `crates/mbongo-node/src/backend.rs`, which is `pub(crate)` and so cannot be
/// called from an integration test. That function is exactly this: BLAKE3 over
/// the full SCALE encoding, signature included.
fn transaction_hash(tx: &Transaction) -> [u8; 32] {
    *blake3::hash(&tx.encode()).as_bytes()
}

#[test]
fn transaction_fixture_parses() {
    let tdoc = transactions();
    let valid = valid_vectors(&tdoc);
    assert!(!valid.is_empty(), "no valid vectors");
    assert_eq!(
        tdoc["signing_formula"]["fixed_bytes_before_receipt"]
            .as_u64()
            .expect("fixed_bytes_before_receipt missing"),
        90,
        "the fixed prefix is 1 + 32 + 32 + 16 + 8 + 1 bytes"
    );
    assert_eq!(
        tdoc["discriminants"]["TransactionType::AnchorReceipt"].as_str(),
        Some("03")
    );
    assert_eq!(
        tdoc["discriminants"]["TransactionPayload::AnchorReceipt"].as_str(),
        Some("01")
    );
    for entry in &valid {
        for key in [
            "signing_payload",
            "transaction_signature",
            "full_transaction",
            "transaction_hash",
        ] {
            assert!(
                entry["expected"][key].is_string(),
                "{}: expected.{key} missing",
                entry["name"]
            );
        }
    }
}

#[test]
fn receipt_vector_reference_resolves_uniquely() {
    let (tdoc, rdoc) = (transactions(), receipts());
    for entry in valid_vectors(&tdoc) {
        let name = entry["transaction"]["receipt_vector"].as_str().expect("receipt_vector missing");
        // Panics unless exactly one receipt vector carries this name.
        let resolved = receipt_vector(&rdoc, name);
        assert_eq!(resolved["name"].as_str(), Some(name));
    }
}

#[test]
fn canonical_signing_payload_matches() {
    let (tdoc, rdoc) = (transactions(), receipts());
    for entry in valid_vectors(&tdoc) {
        let (tx, _, _) = transaction_from(&entry, &rdoc);
        let expected = hex_bytes(
            "expected.signing_payload",
            &entry["expected"]["signing_payload"],
        );
        let name = entry["name"].as_str().unwrap_or("<unnamed>");

        assert_eq!(
            tx.signing_payload(),
            expected,
            "{name}: production signing payload differs from the independently derived bytes"
        );
        assert_eq!(
            expected.len() as u64,
            u64_at(
                "expected.signing_payload_length",
                &entry["expected"]["signing_payload_length"]
            ),
            "{name}: pinned signing payload length disagrees with the pinned bytes"
        );
        assert!(
            tx.verify_signature(),
            "{name}: the pinned transaction signature does not verify"
        );
    }
}

#[test]
fn receipt_bytes_appear_at_expected_offset() {
    let (tdoc, rdoc) = (transactions(), receipts());
    for entry in valid_vectors(&tdoc) {
        let (tx, receipt_full, _) = transaction_from(&entry, &rdoc);
        let payload = tx.signing_payload();
        let offset = u64_at(
            "expected.receipt_offset",
            &entry["expected"]["receipt_offset"],
        ) as usize;
        let name = entry["name"].as_str().unwrap_or("<unnamed>");

        // Computed, not assumed: everything before the receipt is fixed-width.
        assert_eq!(offset, payload.len() - receipt_full.len(), "{name}: offset");
        assert_eq!(
            &payload[offset..],
            receipt_full.as_slice(),
            "{name}: the receipt bytes resolved from the receipt fixture are not a contiguous \
             suffix of the signing payload — a length prefix or wrapper has crept in"
        );
    }
}

#[test]
fn diagnostic_nonce_is_little_endian() {
    let (tdoc, rdoc) = (transactions(), receipts());
    for entry in valid_vectors(&tdoc) {
        let (tx, _, _) = transaction_from(&entry, &rdoc);
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        let pinned = hex_bytes("expected.nonce_u64_le", &entry["expected"]["nonce_u64_le"]);
        assert_eq!(
            pinned.len(),
            8,
            "{name}: a u64 is eight bytes, never compact"
        );
        assert_eq!(
            pinned,
            tx.nonce.to_le_bytes(),
            "{name}: pinned nonce bytes are not the little-endian encoding"
        );
        // And those exact bytes must sit in the signing payload, after
        // 1 + 32 + 32 + 16 bytes.
        let at = 1 + 32 + 32 + 16;
        assert_eq!(
            &tx.signing_payload()[at..at + 8],
            pinned.as_slice(),
            "{name}: the nonce is not encoded little-endian at its offset"
        );
    }

    // The canonical vector must keep an asymmetric nonce: an all-zero or
    // palindromic value cannot distinguish little-endian from big-endian.
    let canonical = valid_vectors(&tdoc)
        .into_iter()
        .find(|v| v["name"].as_str() == Some("canonical-diagnostic-nonce"))
        .expect("the canonical vector is missing");
    let bytes = hex_bytes("nonce_u64_le", &canonical["expected"]["nonce_u64_le"]);
    let mut reversed = bytes.clone();
    reversed.reverse();
    assert_ne!(
        bytes, reversed,
        "the canonical nonce is palindromic and cannot prove byte order"
    );
}

#[test]
fn u128_amount_is_fixed_width_little_endian() {
    let tdoc = transactions();
    let cases = tdoc["encoding_only"].as_array().expect("encoding_only is not an array");
    assert!(!cases.is_empty(), "no encoding-only diagnostic");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed>");
        assert!(
            case["warning"].as_str().is_some_and(|w| w.contains("ENCODING ONLY")),
            "{name}: an encoding-only case must say so, so nothing mistakes it for a valid anchor"
        );
        let decimal = case["amount_decimal"].as_str().unwrap_or_else(|| {
            panic!("{name}: amount_decimal must be a string, not a JSON number")
        });
        let amount: u128 = decimal
            .parse()
            .unwrap_or_else(|e| panic!("{name}: amount_decimal is not a u128: {e}"));
        assert_ne!(
            amount, 0,
            "{name}: a zero amount proves nothing about byte order"
        );

        let pinned = hex_bytes(
            "expected.amount_u128_le",
            &case["expected"]["amount_u128_le"],
        );
        assert_eq!(
            pinned.len(),
            16,
            "{name}: a u128 is sixteen bytes, never compact"
        );
        assert_eq!(
            pinned,
            amount.to_le_bytes(),
            "{name}: pinned amount bytes are not the little-endian encoding"
        );
        let mut reversed = pinned.clone();
        reversed.reverse();
        assert_ne!(
            pinned, reversed,
            "{name}: the diagnostic amount is palindromic and cannot prove byte order"
        );
    }

    // Every valid anchoring vector carries amount 0, which is why the
    // diagnostic above has to exist at all.
    for entry in valid_vectors(&tdoc) {
        assert_eq!(
            hex_bytes(
                "expected.amount_u128_le",
                &entry["expected"]["amount_u128_le"]
            ),
            0u128.to_le_bytes(),
            "a valid AnchorReceipt vector must carry amount 0"
        );
    }
}

#[test]
fn full_transaction_encoding_and_hash_match() {
    let (tdoc, rdoc) = (transactions(), receipts());
    for entry in valid_vectors(&tdoc) {
        let (tx, _, _) = transaction_from(&entry, &rdoc);
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        let expected = hex_bytes(
            "expected.full_transaction",
            &entry["expected"]["full_transaction"],
        );

        assert_eq!(tx.encode(), expected, "{name}: full SCALE encoding differs");
        assert_eq!(
            expected.len() as u64,
            u64_at(
                "expected.full_transaction_length",
                &entry["expected"]["full_transaction_length"]
            ),
            "{name}: pinned full length disagrees with the pinned bytes"
        );

        // The signature is the final field, so the signing payload is a strict
        // prefix of the full encoding.
        let payload = tx.signing_payload();
        assert_eq!(
            &expected[..payload.len()],
            payload.as_slice(),
            "{name}: prefix"
        );
        assert_eq!(
            &expected[payload.len()..],
            &tx.signature[..],
            "{name}: suffix"
        );

        let hash: [u8; 32] = fixed(
            "expected.transaction_hash",
            &entry["expected"]["transaction_hash"],
        );
        assert_eq!(
            transaction_hash(&tx),
            hash,
            "{name}: transaction hash differs from the independently derived value"
        );
    }
}

#[test]
fn transaction_signature_domain_is_distinct() {
    let (tdoc, rdoc) = (transactions(), receipts());
    let sk = test_key(&rdoc);
    let vk: VerifyingKey = sk.verifying_key();

    let canonical = valid_vectors(&tdoc)
        .into_iter()
        .find(|v| v["name"].as_str() == Some("canonical-diagnostic-nonce"))
        .expect("the canonical vector is missing");
    let (tx, _, receipt_hash) = transaction_from(&canonical, &rdoc);
    let payload = tx.signing_payload();

    let TransactionPayload::AnchorReceipt(receipt) = &tx.payload else {
        panic!("the canonical vector must carry an AnchorReceipt payload");
    };
    let receipt_sig = ed25519_dalek::Signature::from_bytes(&receipt.signature);
    let tx_sig = ed25519_dalek::Signature::from_bytes(&tx.signature);

    // One key, because anchoring requires sender == executor.
    assert_eq!(
        tx.sender, receipt.executor,
        "sender must equal the executor"
    );
    assert_eq!(
        vk.to_bytes(),
        tx.sender.0,
        "both signatures use the fixture key"
    );

    // Each signature verifies over its own message...
    assert!(
        vk.verify(&receipt_hash, &receipt_sig).is_ok(),
        "receipt over receipt hash"
    );
    assert!(
        vk.verify(&payload, &tx_sig).is_ok(),
        "transaction over signing payload"
    );

    // ...and over nothing else.
    assert!(
        vk.verify(&payload, &receipt_sig).is_err(),
        "the receipt signature must not verify over the transaction signing payload"
    );
    assert!(
        vk.verify(&receipt_hash, &tx_sig).is_err(),
        "the transaction signature must not verify over the receipt hash"
    );

    // Same key, different messages, therefore different signatures.
    assert_ne!(
        receipt.signature, tx.signature,
        "the two signatures must not be interchangeable"
    );

    // And the three values are three different things. The two hashes are both
    // 32 bytes, so compare the actual values rather than their shape.
    assert_ne!(
        receipt_hash,
        transaction_hash(&tx),
        "receipt hash vs transaction hash"
    );
    assert_ne!(
        payload.len(),
        receipt_hash.len(),
        "the transaction signing message is raw variable-length bytes, not a 32-byte digest"
    );

    // Signing the receipt hash is deterministic, so it reproduces the receipt's
    // own signature: reusing either is the same mistake.
    assert_eq!(
        sk.sign(&receipt_hash).to_bytes(),
        receipt.signature,
        "Ed25519 is deterministic here"
    );
}

#[test]
fn invalid_vectors_fail_signature_verification() {
    let (tdoc, rdoc) = (transactions(), receipts());
    let cases = tdoc["invalid"].as_array().expect("invalid is not an array");
    assert!(!cases.is_empty(), "no invalid vectors");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed>");
        let base = case["base_vector"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: base_vector missing"));
        let entry = valid_vectors(&tdoc)
            .into_iter()
            .find(|v| v["name"].as_str() == Some(base))
            .unwrap_or_else(|| panic!("{name}: base_vector {base:?} does not exist"));

        let (mut tx, _, _) = transaction_from(&entry, &rdoc);
        // Everything matches the base vector except the transaction signature.
        tx.signature = fixed("transaction_signature", &case["transaction_signature"]);

        assert_eq!(
            case["expected"]["transaction_signature_verifies"].as_bool(),
            Some(false),
            "{name}: an invalid vector must expect verification to fail"
        );
        assert!(
            !tx.verify_signature(),
            "{name}: a signature over the wrong message must not verify"
        );

        // The receipt inside is untouched and still sound: only the
        // transaction-level signature is wrong.
        let TransactionPayload::AnchorReceipt(receipt) = &tx.payload else {
            panic!("{name}: expected an AnchorReceipt payload");
        };
        let vk = VerifyingKey::from_bytes(&receipt.executor.0).expect("valid executor key");
        let receipt_sig = ed25519_dalek::Signature::from_bytes(&receipt.signature);
        assert!(
            vk.verify(receipt.receipt_hash().0.as_ref(), &receipt_sig).is_ok(),
            "{name}: the anchored receipt must remain independently valid"
        );
    }
}

#[test]
fn serialized_anchor_receipt_json_matches() {
    let (tdoc, rdoc) = (transactions(), receipts());
    let pinned = &tdoc["serialized_transaction"];
    let name = pinned["vector"].as_str().expect("serialized_transaction.vector missing");
    let entry = valid_vectors(&tdoc)
        .into_iter()
        .find(|v| v["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("serialized_transaction references unknown vector {name:?}"));

    let (tx, _, _) = transaction_from(&entry, &rdoc);
    let actual = serde_json::to_value(&tx).expect("transaction serialises");
    assert_eq!(
        actual, pinned["object"],
        "the serialised Transaction object differs from the pinned wire form"
    );

    // The mixed byte representation is the whole reason this block exists, so
    // assert the shape explicitly rather than trusting the blob comparison.
    let receipt = &actual["payload"]["AnchorReceipt"];
    for field in ["sender", "receiver", "signature"] {
        assert!(
            actual[field].as_str().is_some_and(|s| s.starts_with("0x")),
            "transaction.{field} should be an 0x hex string"
        );
    }
    for field in ["executor", "signature"] {
        assert!(
            receipt[field].as_str().is_some_and(|s| s.starts_with("0x")),
            "receipt.{field} should be an 0x hex string"
        );
    }
    for field in [
        "task_id",
        "input_commitment",
        "output_commitment",
        "metadata",
    ] {
        assert!(
            receipt[field].is_array(),
            "receipt.{field} is a plain byte array in Rust with no serde annotation, so it \
             serialises as an array of numbers, not as hex"
        );
    }
    assert_eq!(actual["tx_type"].as_str(), Some("AnchorReceipt"));
    assert!(
        actual["payload"].get("AnchorReceipt").is_some(),
        "the payload enum is externally tagged: the variant name is the key"
    );
    assert!(actual["amount"].is_number() && actual["nonce"].is_number());
}

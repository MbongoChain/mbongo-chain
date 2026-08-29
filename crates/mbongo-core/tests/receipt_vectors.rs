//! Rust side of the shared cross-language receipt vectors.
//!
//! The expected values in `test-vectors/receipt/receipt-v1.json` were **not**
//! produced by this crate. The signing-payload bytes were assembled by hand
//! from the canonical field order and the SCALE compact-length rule; the
//! hashes come from an independent BLAKE3 implementation over those bytes;
//! the key and signatures come from an independent Ed25519 implementation.
//! See the fixture README for the derivation.
//!
//! So this file is not the encoder checking its own output. It proves the
//! production encoder agrees with values derived without it — and #84 will
//! prove a TypeScript stack agrees with the same file. Two independent
//! implementations meeting on pinned constants is what makes the vectors
//! worth anything.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use mbongo_core::{Address, Receipt};
use parity_scale_codec::Encode;
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../test-vectors/receipt/receipt-v1.json");

/// The fixture schema this test understands. A bump must be a deliberate
/// change here, not a silent divergence.
const SUPPORTED_FIXTURE_VERSION: u64 = 1;

fn fixture() -> Value {
    let doc: Value = serde_json::from_str(FIXTURE).expect("fixture is not valid JSON");
    let version = doc["fixture_version"]
        .as_u64()
        .expect("fixture_version missing or not a number");
    assert_eq!(
        version, SUPPORTED_FIXTURE_VERSION,
        "unsupported fixture schema version: this test understands {SUPPORTED_FIXTURE_VERSION}"
    );
    doc
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

/// Expands `{pattern: "repeat", byte: "ab", length: n}` — the only pattern
/// the schema defines, so anything else is a fixture error rather than
/// something to guess at.
fn metadata(field: &str, v: &Value) -> Vec<u8> {
    let pattern = v["pattern"].as_str().unwrap_or_else(|| panic!("{field}: missing pattern"));
    assert_eq!(pattern, "repeat", "{field}: unsupported metadata pattern");
    let byte = hex_bytes(&format!("{field}.byte"), &v["byte"]);
    assert_eq!(byte.len(), 1, "{field}.byte: expected exactly one byte");
    let len = v["length"].as_u64().unwrap_or_else(|| panic!("{field}: missing length")) as usize;
    vec![byte[0]; len]
}

/// Builds a receipt from a fixture entry, taking the signature from the
/// fixture when present and leaving it zeroed otherwise.
fn receipt_from(entry: &Value, signature: Option<[u8; 64]>) -> Receipt {
    let r = &entry["receipt"];
    Receipt {
        version: r["version"].as_u64().expect("version") as u8,
        task_id: fixed::<32>("task_id", &r["task_id"]),
        input_commitment: fixed::<32>("input_commitment", &r["input_commitment"]),
        output_commitment: fixed::<32>("output_commitment", &r["output_commitment"]),
        executor: Address(fixed::<32>("executor", &r["executor"])),
        metadata: metadata("metadata", &r["metadata"]),
        signature: signature.unwrap_or([0u8; 64]),
    }
}

fn test_key() -> SigningKey {
    let doc = fixture();
    let seed = fixed::<32>("ed25519_seed", &doc["test_key"]["ed25519_seed"]);
    SigningKey::from_bytes(&seed)
}

#[test]
fn fixture_declares_the_expected_shape() {
    let doc = fixture();
    assert_eq!(doc["metadata_max_bytes"].as_u64(), Some(4096));
    assert_eq!(
        doc["canonical_field_order"].as_array().map(Vec::len),
        Some(7),
        "a receipt has seven fields"
    );
    assert_eq!(doc["valid"].as_array().map(Vec::len), Some(5));
    assert_eq!(doc["invalid"].as_array().map(Vec::len), Some(3));
}

#[test]
fn test_key_derives_the_pinned_public_key() {
    // The fixture's public key came from an independent Ed25519
    // implementation. If dalek disagrees, every signature vector below is
    // meaningless, so this is checked first.
    let doc = fixture();
    let expected = fixed::<32>("public_key", &doc["test_key"]["public_key"]);
    assert_eq!(test_key().verifying_key().to_bytes(), expected);
}

#[test]
fn valid_vectors_match_the_production_encoder() {
    let doc = fixture();
    let sk = test_key();

    for entry in doc["valid"].as_array().expect("valid vectors") {
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        let expected = &entry["expected"];
        let signature = fixed::<64>("executor_signature", &expected["executor_signature"]);
        let receipt = receipt_from(entry, Some(signature));

        // The executor must be the signer, or a signature check proves nothing.
        assert_eq!(
            receipt.executor.0,
            sk.verifying_key().to_bytes(),
            "{name}: executor is not the fixture test key"
        );

        let payload = receipt.signing_payload();
        let full = receipt.encode();

        assert_eq!(
            payload.len() as u64,
            expected["signing_payload_length"].as_u64().unwrap(),
            "{name}: signing payload length"
        );
        assert_eq!(
            full.len() as u64,
            expected["full_encoding_length"].as_u64().unwrap(),
            "{name}: full encoding length"
        );

        // The compact prefix sits immediately after the five fixed-width
        // fields. Its width is the thing a naive implementation gets wrong.
        let head = 1 + 32 + 32 + 32 + 32;
        let prefix = hex_bytes(
            "metadata_compact_prefix",
            &expected["metadata_compact_prefix"],
        );
        assert_eq!(
            &payload[head..head + prefix.len()],
            &prefix[..],
            "{name}: compact metadata prefix"
        );
        assert_eq!(
            payload.len(),
            head + prefix.len() + receipt.metadata.len(),
            "{name}: payload is head + prefix + metadata and nothing else"
        );

        assert_eq!(
            receipt.receipt_hash().0.to_vec(),
            hex_bytes("receipt_hash", &expected["receipt_hash"]),
            "{name}: receipt hash"
        );

        // Signature is over the raw 32 bytes of the hash.
        VerifyingKey::from_bytes(&receipt.executor.0)
            .expect("valid key")
            .verify(
                &receipt.receipt_hash().0,
                &ed25519_dalek::Signature::from_bytes(&signature),
            )
            .unwrap_or_else(|e| panic!("{name}: signature does not verify: {e}"));

        // Full byte pinning where the fixture carries it.
        if let Some(hex) = expected.get("signing_payload") {
            assert_eq!(
                payload,
                hex_bytes("signing_payload", hex),
                "{name}: payload bytes"
            );
        }
        if let Some(hex) = expected.get("full_encoding") {
            assert_eq!(full, hex_bytes("full_encoding", hex), "{name}: full bytes");
        }
    }
}

#[test]
fn signature_is_over_raw_hash_bytes_not_hex_text() {
    // A plausible mistake in another language: signing the displayed hex
    // string instead of the digest. This pins the difference.
    let doc = fixture();
    let sk = test_key();
    let entry = &doc["valid"][1];
    let receipt = receipt_from(
        entry,
        Some(fixed::<64>(
            "executor_signature",
            &entry["expected"]["executor_signature"],
        )),
    );

    let hash = receipt.receipt_hash().0;
    let over_hex = sk.sign(hex::encode(hash).as_bytes()).to_bytes();
    assert_ne!(
        over_hex, receipt.signature,
        "signing the hex text must not produce the canonical signature"
    );
}

#[test]
fn signature_stays_out_of_the_receipt_hash() {
    let doc = fixture();
    let entry = &doc["valid"][1];
    let mut receipt = receipt_from(entry, Some([0u8; 64]));
    let before = receipt.receipt_hash();
    receipt.signature = [0xFF; 64];
    assert_eq!(before, receipt.receipt_hash());
}

/// The boundary a naive implementation is most likely to get wrong: at the
/// consensus maximum the compact prefix is **two** bytes, not one.
#[test]
fn metadata_at_consensus_bound_uses_a_two_byte_compact_prefix() {
    let doc = fixture();
    let entry = doc["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["receipt"]["metadata"]["length"].as_u64() == Some(4096))
        .expect("a vector at the 4096 bound");

    let receipt = receipt_from(
        entry,
        Some(fixed::<64>(
            "executor_signature",
            &entry["expected"]["executor_signature"],
        )),
    );
    let payload = receipt.signing_payload();
    let head = 1 + 32 + 32 + 32 + 32;

    assert_eq!(&payload[head..head + 2], &[0x01, 0x40], "prefix is 01 40");
    assert_eq!(payload.len(), 4227, "signing payload length at the bound");
    assert_eq!(
        receipt.encode().len(),
        4291,
        "full encoding length at the bound"
    );
    assert_eq!(
        payload[head + 2],
        0xAB,
        "metadata starts after a two-byte prefix"
    );
}

/// The 63 -> 64 transition, where the prefix widens.
#[test]
fn compact_prefix_widens_between_sixty_three_and_sixty_four() {
    let doc = fixture();
    let head = 1 + 32 + 32 + 32 + 32;
    let by_len = |n: u64| {
        doc["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["receipt"]["metadata"]["length"].as_u64() == Some(n))
            .unwrap_or_else(|| panic!("a vector with metadata length {n}"))
            .clone()
    };

    let p63 = receipt_from(&by_len(63), None).signing_payload();
    let p64 = receipt_from(&by_len(64), None).signing_payload();

    assert_eq!(
        &p63[head..head + 1],
        &[0xFC],
        "63 is the last one-byte prefix"
    );
    assert_eq!(
        &p64[head..head + 2],
        &[0x01, 0x01],
        "64 is the first two-byte prefix"
    );
    assert_eq!(
        p64.len() - p63.len(),
        2,
        "one more byte of data, one more of prefix"
    );
}

#[test]
fn invalid_vectors_are_rejected_for_the_stated_reason() {
    let doc = fixture();
    let sk = test_key();

    for entry in doc["invalid"].as_array().expect("invalid vectors") {
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        let reason = entry["expected"]["rejected_by"].as_str().expect("rejected_by");
        let signature = entry["receipt"].get("signature").map(|s| fixed::<64>("signature", s));
        let receipt = receipt_from(entry, signature);

        match reason {
            "metadata_bound" => {
                // Encoding and hashing succeed; only the bound rejects it.
                assert_eq!(receipt.metadata.len(), 4097);
                assert!(
                    receipt.metadata.len() > doc["metadata_max_bytes"].as_u64().unwrap() as usize,
                    "{name}: must exceed the bound"
                );
                let prefix = hex_bytes(
                    "metadata_compact_prefix",
                    &entry["expected"]["metadata_compact_prefix"],
                );
                let head = 1 + 32 + 32 + 32 + 32;
                assert_eq!(
                    &receipt.signing_payload()[head..head + prefix.len()],
                    &prefix[..],
                    "{name}: encodes cleanly — the failure is the bound, not the encoding"
                );
            }
            "signature" => {
                let verified = VerifyingKey::from_bytes(&receipt.executor.0)
                    .ok()
                    .and_then(|pk| {
                        pk.verify(
                            &receipt.receipt_hash().0,
                            &ed25519_dalek::Signature::from_bytes(&receipt.signature),
                        )
                        .ok()
                    })
                    .is_some();
                assert!(!verified, "{name}: signature must not verify");
                // And it is genuinely a signature problem: re-signing the same
                // receipt with the right key makes it verify.
                let good = sk.sign(&receipt.receipt_hash().0).to_bytes();
                assert!(
                    VerifyingKey::from_bytes(&receipt.executor.0)
                        .unwrap()
                        .verify(
                            &receipt.receipt_hash().0,
                            &ed25519_dalek::Signature::from_bytes(&good)
                        )
                        .is_ok(),
                    "{name}: the receipt itself is otherwise sound"
                );
            }
            other => panic!("{name}: unknown rejection reason {other}"),
        }
    }
}

//! Rust side of the shared cross-language `ComputeTask` vectors
//! (RFC 0005 §12).
//!
//! The expected values in `test-vectors/compute-task/compute-task-v1.json`
//! were **not** produced by this crate. The canonical task bytes were laid
//! out by hand from the field rules — a SCALE struct is its fields
//! concatenated in declaration order, `execution_spec` behind a compact
//! length prefix — the domain tag was written as literal bytes, the
//! integers were built by explicit little-endian construction, the
//! signatures come from an independent Ed25519, and the hashes from an
//! independent BLAKE3. The only machine input was the submitter test key,
//! resolved from `test-vectors/receipt/receipt-v1.json`.
//!
//! So this file is not the encoder checking its own output. It proves the
//! production envelope encoder, identity derivation and transaction
//! encoder agree with values derived without them. The TypeScript SDK
//! will consume the same file under Workstream D.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use mbongo_core::{
    Address, ComputeTask, Transaction, TransactionPayload, TransactionType, COMPUTE_TASK_VERSION,
    DOMAIN_TASK, MAX_COMPUTE_TASK_BYTES, MAX_EXECUTION_SPEC_BYTES, MAX_TASK_ID_PREIMAGE_BYTES,
};
use parity_scale_codec::{Decode, Encode};
use serde_json::Value;

const TASK_FIXTURE: &str = include_str!("../../../test-vectors/compute-task/compute-task-v1.json");
const RECEIPT_FIXTURE: &str = include_str!("../../../test-vectors/receipt/receipt-v1.json");

/// The fixture schema this test understands.
const SUPPORTED_FIXTURE_VERSION: u64 = 1;

fn doc(name: &str, raw: &str) -> Value {
    let v: Value =
        serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name}: invalid JSON: {e}"));
    let version = v["fixture_version"]
        .as_u64()
        .unwrap_or_else(|| panic!("{name}: fixture_version missing or not a number"));
    assert_eq!(
        version, SUPPORTED_FIXTURE_VERSION,
        "{name}: unsupported fixture schema version"
    );
    v
}

fn tasks_doc() -> Value {
    doc("compute-task fixture", TASK_FIXTURE)
}

fn receipts_doc() -> Value {
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

/// Expands the two specification patterns the fixture defines. Anything
/// else is a fixture error rather than something to guess at.
fn execution_spec(field: &str, v: &Value) -> Vec<u8> {
    let pattern = v["pattern"].as_str().unwrap_or_else(|| panic!("{field}: missing pattern"));
    match pattern {
        "repeat" => {
            let byte = hex_bytes(&format!("{field}.byte"), &v["byte"]);
            assert_eq!(byte.len(), 1, "{field}.byte: expected exactly one byte");
            let len = u64_at(&format!("{field}.length"), &v["length"]) as usize;
            vec![byte[0]; len]
        }
        "literal" => hex_bytes(&format!("{field}.hex"), &v["hex"]),
        other => panic!("{field}: unsupported execution_spec pattern {other:?}"),
    }
}

/// The TEST ONLY submitter key, resolved from the receipt fixture rather
/// than restated here. Public seed; never a production key.
fn submitter_key(rdoc: &Value) -> SigningKey {
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

fn task_from(field: &str, t: &Value) -> ComputeTask {
    ComputeTask {
        version: u64_at(&format!("{field}.version"), &t["version"]) as u8,
        submitter: Address(fixed(&format!("{field}.submitter"), &t["submitter"])),
        executor: Address(fixed(&format!("{field}.executor"), &t["executor"])),
        salt: fixed(&format!("{field}.salt"), &t["salt"]),
        input_commitment: fixed(&format!("{field}.input_commitment"), &t["input_commitment"]),
        execution_spec: execution_spec(&format!("{field}.execution_spec"), &t["execution_spec"]),
    }
}

fn named<'a>(field: &str, list: &'a Value, name: &str) -> &'a Value {
    let all = list.as_array().unwrap_or_else(|| panic!("{field} is not an array"));
    let matches: Vec<&Value> = all.iter().filter(|v| v["name"].as_str() == Some(name)).collect();
    assert_eq!(
        matches.len(),
        1,
        "{field}: expected exactly one vector named {name:?}, found {}",
        matches.len()
    );
    matches[0]
}

fn task_vectors(tdoc: &Value) -> Vec<Value> {
    tdoc["tasks"].as_array().expect("tasks is not an array").clone()
}

fn valid_vectors(tdoc: &Value) -> Vec<Value> {
    tdoc["valid"].as_array().expect("valid is not an array").clone()
}

/// Builds the transaction a valid vector describes. `sender` is derived
/// from the referenced task's submitter, which rule (o) requires.
fn transaction_from(entry: &Value, tdoc: &Value) -> (Transaction, ComputeTask) {
    let name = entry["task_vector"].as_str().expect("task_vector: expected a vector name");
    let task = task_from("task", &named("tasks", &tdoc["tasks"], name)["task"]);
    let t = &entry["transaction"];
    assert_eq!(
        t["sender"].as_str(),
        Some("<task.submitter>"),
        "a valid vector must derive its sender from the task"
    );
    let receiver = Address(fixed("transaction.receiver", &t["receiver"]));
    let amount = u128::from(u64_at("transaction.amount", &t["amount"]));
    assert_eq!(amount, 0, "a valid ComputeTask vector must carry amount 0");
    assert_eq!(
        receiver,
        Address::zero(),
        "a valid ComputeTask vector must carry the zero receiver"
    );
    let tx = Transaction {
        tx_type: TransactionType::ComputeTask,
        sender: task.submitter,
        receiver,
        amount,
        nonce: u64_at("transaction.nonce", &t["nonce"]),
        payload: TransactionPayload::ComputeTask(Box::new(task.clone())),
        signature: fixed(
            "expected.transaction_signature",
            &entry["expected"]["transaction_signature"],
        ),
    };
    (tx, task)
}

/// The transaction hash rule, mirrored from `compute_tx_hash` in
/// `crates/mbongo-node/src/backend.rs` (which is `pub(crate)`): BLAKE3 over
/// the full SCALE encoding, signature included.
fn transaction_hash(tx: &Transaction) -> [u8; 32] {
    *blake3::hash(&tx.encode()).as_bytes()
}

#[test]
fn fixture_pins_the_protocol_constants() {
    let tdoc = tasks_doc();
    assert_eq!(
        hex_bytes("domain_task.hex", &tdoc["domain_task"]["hex"]),
        DOMAIN_TASK.to_vec(),
        "the domain tag must be the literal bytes the fixture pins"
    );
    assert_eq!(
        tdoc["domain_task"]["ascii"].as_str(),
        Some("mbongo:compute-task:v1")
    );
    assert_eq!(
        u64_at("domain_task.length", &tdoc["domain_task"]["length"]),
        22
    );
    assert_eq!(
        u64_at(
            "execution_spec_max_bytes",
            &tdoc["execution_spec_max_bytes"]
        ) as usize,
        MAX_EXECUTION_SPEC_BYTES
    );
    assert_eq!(
        u64_at(
            "maximal_sizes.canonical_task",
            &tdoc["maximal_sizes"]["canonical_task"]
        ) as usize,
        MAX_COMPUTE_TASK_BYTES
    );
    assert_eq!(
        u64_at(
            "maximal_sizes.task_id_preimage",
            &tdoc["maximal_sizes"]["task_id_preimage"]
        ) as usize,
        MAX_TASK_ID_PREIMAGE_BYTES
    );
    assert_eq!(
        u64_at("envelope.version", &tdoc["envelope"]["version"]) as u8,
        COMPUTE_TASK_VERSION
    );
    assert_eq!(
        tdoc["envelope"]["task_id_is_not_a_field"].as_bool(),
        Some(true)
    );
    assert_eq!(
        tdoc["discriminants"]["TransactionType::ComputeTask"].as_str(),
        Some("01")
    );
    assert_eq!(
        tdoc["discriminants"]["TransactionPayload::ComputeTask"].as_str(),
        Some("02")
    );
    assert_eq!(TransactionType::ComputeTask.encode(), vec![0x01]);
    assert_eq!(
        u64_at(
            "signing_formula.fixed_bytes_before_task",
            &tdoc["signing_formula"]["fixed_bytes_before_task"]
        ),
        90,
        "the fixed prefix is 1 + 32 + 32 + 16 + 8 + 1 bytes"
    );
    // Exactly the four task vectors and three transaction vectors the RFC
    // §12 boundary list requires are present.
    assert_eq!(task_vectors(&tdoc).len(), 4, "task vector cardinality");
    assert_eq!(
        valid_vectors(&tdoc).len(),
        3,
        "transaction vector cardinality"
    );
    assert_eq!(
        tdoc["rejected"].as_array().map(Vec::len),
        Some(1),
        "rejected vector cardinality"
    );
}

#[test]
fn test_keys_resolve() {
    let (tdoc, rdoc) = (tasks_doc(), receipts_doc());
    let sk = submitter_key(&rdoc);
    let pinned: [u8; 32] = fixed(
        "test_keys.submitter.public_key",
        &tdoc["test_keys"]["submitter"]["public_key"],
    );
    assert_eq!(sk.verifying_key().to_bytes(), pinned);

    let seed: [u8; 32] = fixed(
        "test_keys.executor.ed25519_seed",
        &tdoc["test_keys"]["executor"]["ed25519_seed"],
    );
    let executor_pk: [u8; 32] = fixed(
        "test_keys.executor.public_key",
        &tdoc["test_keys"]["executor"]["public_key"],
    );
    assert_eq!(
        SigningKey::from_bytes(&seed).verifying_key().to_bytes(),
        executor_pk,
        "the executor seed must derive the pinned executor key"
    );
    assert_ne!(
        pinned, executor_pk,
        "submitter and executor are different parties"
    );
}

#[test]
fn canonical_task_bytes_and_task_id_match() {
    let tdoc = tasks_doc();
    for entry in task_vectors(&tdoc) {
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        let task = task_from("task", &entry["task"]);
        let expected = hex_bytes(
            "expected.canonical_task",
            &entry["expected"]["canonical_task"],
        );

        assert_eq!(
            task.encode(),
            expected,
            "{name}: production envelope encoding differs from the independently derived bytes"
        );
        assert_eq!(
            expected.len() as u64,
            u64_at(
                "expected.canonical_task_length",
                &entry["expected"]["canonical_task_length"]
            ),
            "{name}: pinned length disagrees with the pinned bytes"
        );
        // The compact prefix sits right after the four 32-byte fields.
        let prefix = hex_bytes(
            "expected.execution_spec_compact_prefix",
            &entry["expected"]["execution_spec_compact_prefix"],
        );
        assert_eq!(
            &expected[129..129 + prefix.len()],
            prefix.as_slice(),
            "{name}: prefix"
        );
        assert_eq!(
            &expected[129 + prefix.len()..],
            task.execution_spec.as_slice(),
            "{name}: the specification bytes are a contiguous suffix"
        );

        // Identity: raw tag, then exactly the canonical bytes, then BLAKE3.
        let preimage = task.task_id_preimage();
        assert_eq!(&preimage[..22], DOMAIN_TASK);
        assert_eq!(&preimage[22..], expected.as_slice());
        assert_eq!(
            preimage.len() as u64,
            u64_at(
                "expected.task_id_preimage_length",
                &entry["expected"]["task_id_preimage_length"]
            ),
            "{name}: preimage length"
        );
        let task_id: [u8; 32] = fixed("expected.task_id", &entry["expected"]["task_id"]);
        assert_eq!(
            task.task_id(),
            task_id,
            "{name}: task_id differs from the independently derived value"
        );

        // And the bytes decode back to the same envelope.
        assert_eq!(
            ComputeTask::decode(&mut &expected[..]).unwrap(),
            task,
            "{name}: decode"
        );
    }
}

#[test]
fn boundary_vectors_cover_the_bound() {
    let tdoc = tasks_doc();
    let empty = task_from(
        "task",
        &named("tasks", &tdoc["tasks"], "empty-spec")["task"],
    );
    assert!(empty.execution_spec.is_empty());
    assert_eq!(empty.encode().len(), 130);

    let widen = task_from("task", &named("tasks", &tdoc["tasks"], "spec-64")["task"]);
    assert_eq!(widen.execution_spec.len(), 64);
    assert_eq!(&widen.encode()[129..131], &[0x01, 0x01]);

    let maximal = task_from(
        "task",
        &named("tasks", &tdoc["tasks"], "spec-max-1024")["task"],
    );
    assert_eq!(maximal.execution_spec.len(), MAX_EXECUTION_SPEC_BYTES);
    assert_eq!(maximal.encode().len(), MAX_COMPUTE_TASK_BYTES);
    assert_eq!(maximal.task_id_preimage().len(), MAX_TASK_ID_PREIMAGE_BYTES);
    assert_eq!(&maximal.encode()[129..131], &[0x01, 0x10]);

    // 1025 encodes and hashes like anything else, and must be rejected by
    // rule (n) — the bound is consensus, not an encoding limit.
    let rejected = named("rejected", &tdoc["rejected"], "spec-1025");
    assert_eq!(rejected["consensus"]["valid"].as_bool(), Some(false));
    assert_eq!(rejected["consensus"]["rule"].as_str(), Some("n"));
    let over = task_from("task", &rejected["task"]);
    assert_eq!(over.execution_spec.len(), MAX_EXECUTION_SPEC_BYTES + 1);
    assert!(
        over.execution_spec.len() > MAX_EXECUTION_SPEC_BYTES,
        "the consensus bound must reject this envelope"
    );
    assert_eq!(
        over.encode(),
        hex_bytes(
            "rejected.canonical_task",
            &rejected["expected"]["canonical_task"]
        )
    );
    let over_id: [u8; 32] = fixed("rejected.task_id", &rejected["expected"]["task_id"]);
    assert_eq!(over.task_id(), over_id);
}

#[test]
fn identity_commits_to_every_field_and_not_the_nonce() {
    let tdoc = tasks_doc();
    let base_name = tdoc["identity"]["base_vector"].as_str().expect("identity.base_vector");
    let base_entry = named("tasks", &tdoc["tasks"], base_name);
    let base = task_from("task", &base_entry["task"]);
    let base_id: [u8; 32] = fixed("expected.task_id", &base_entry["expected"]["task_id"]);
    assert_eq!(base.task_id(), base_id);

    let variants = tdoc["identity"]["variants"].as_array().expect("identity.variants");
    assert_eq!(variants.len(), 5, "one variant per non-version field");
    let mut seen = std::collections::HashSet::new();
    for v in variants {
        let field = v["changed_field"].as_str().expect("changed_field");
        let task = task_from("variant.task", &v["task"]);
        let expected: [u8; 32] = fixed("variant.expected.task_id", &v["expected"]["task_id"]);

        // Exactly one field differs from the base.
        let differs = [
            ("salt", task.salt != base.salt),
            ("submitter", task.submitter != base.submitter),
            ("executor", task.executor != base.executor),
            (
                "input_commitment",
                task.input_commitment != base.input_commitment,
            ),
            ("execution_spec", task.execution_spec != base.execution_spec),
        ];
        let changed: Vec<&str> = differs.iter().filter(|(_, d)| *d).map(|(n, _)| *n).collect();
        assert_eq!(
            changed,
            vec![field],
            "variant {field}: exactly that field changes"
        );

        assert_eq!(task.task_id(), expected, "variant {field}: task_id");
        assert_ne!(
            task.task_id(),
            base_id,
            "variant {field}: identity must change"
        );
        assert!(
            seen.insert(expected),
            "variant {field}: collides with another variant"
        );
    }
    assert_eq!(
        tdoc["identity"]["same_envelope_same_id"].as_bool(),
        Some(true)
    );
    assert_eq!(
        tdoc["identity"]["nonce_changes_task_id"].as_bool(),
        Some(false)
    );

    // Two transaction vectors carry the base task under different nonces:
    // same task_id, different transaction hashes.
    let (tx_a, task_a) = transaction_from(named("valid", &tdoc["valid"], "canonical"), &tdoc);
    let (tx_b, task_b) = transaction_from(
        named("valid", &tdoc["valid"], "canonical-nonce-zero"),
        &tdoc,
    );
    assert_eq!(task_a, task_b);
    assert_ne!(tx_a.nonce, tx_b.nonce);
    assert_eq!(task_a.task_id(), task_b.task_id());
    assert_ne!(transaction_hash(&tx_a), transaction_hash(&tx_b));
}

#[test]
fn wrong_tagging_never_reproduces_the_task_id() {
    let tdoc = tasks_doc();
    let diag = &tdoc["wrong_tag_diagnostics"];
    let base = named(
        "tasks",
        &tdoc["tasks"],
        diag["base_vector"].as_str().expect("base_vector"),
    );
    let task = task_from("task", &base["task"]);
    let task_id = task.task_id();
    let body = task.encode();

    let cases: [(&str, Vec<u8>); 4] = [
        (
            "tag_scale_encoded",
            [&[0x58u8][..], DOMAIN_TASK, &body].concat(),
        ),
        (
            "tag_nul_terminated",
            [DOMAIN_TASK, &[0u8][..], &body].concat(),
        ),
        ("no_tag", body.clone()),
        (
            "hex_rendering_hashed",
            hex::encode([DOMAIN_TASK.as_slice(), &body].concat()).into_bytes(),
        ),
    ];
    for (name, preimage) in cases {
        let pinned: [u8; 32] = fixed(name, &diag[name]);
        // The diagnostic value is what that mistake produces...
        assert_eq!(
            *blake3::hash(&preimage).as_bytes(),
            pinned,
            "{name}: diagnostic"
        );
        // ...and production does not produce it.
        assert_ne!(task_id, pinned, "{name}: production must not tag this way");
    }
}

#[test]
fn transaction_signing_payload_signature_and_hash_match() {
    let (tdoc, rdoc) = (tasks_doc(), receipts_doc());
    let sk = submitter_key(&rdoc);
    for entry in valid_vectors(&tdoc) {
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        let (tx, task) = transaction_from(&entry, &tdoc);
        let expected_payload = hex_bytes(
            "expected.signing_payload",
            &entry["expected"]["signing_payload"],
        );

        assert_eq!(
            tx.signing_payload(),
            expected_payload,
            "{name}: production signing payload differs from the independently derived bytes"
        );
        assert_eq!(
            expected_payload.len() as u64,
            u64_at(
                "expected.signing_payload_length",
                &entry["expected"]["signing_payload_length"]
            )
        );
        // Everything before the task is fixed-width: the task bytes begin
        // at offset 90 whatever the specification length.
        let offset = u64_at("expected.task_offset", &entry["expected"]["task_offset"]) as usize;
        assert_eq!(offset, 90);
        assert_eq!(
            &expected_payload[offset..],
            task.encode().as_slice(),
            "{name}: task suffix"
        );
        assert_eq!(
            expected_payload[0], 0x01,
            "{name}: TransactionType::ComputeTask"
        );
        assert_eq!(
            expected_payload[89], 0x02,
            "{name}: TransactionPayload::ComputeTask"
        );
        let nonce_le = hex_bytes("expected.nonce_u64_le", &entry["expected"]["nonce_u64_le"]);
        assert_eq!(
            nonce_le,
            tx.nonce.to_le_bytes(),
            "{name}: nonce is little-endian"
        );
        assert_eq!(
            &expected_payload[81..89],
            nonce_le.as_slice(),
            "{name}: nonce offset"
        );

        // The pinned signature verifies, is what the test key produces,
        // and is over the raw payload (no prehash).
        assert!(
            tx.verify_signature(),
            "{name}: pinned signature must verify"
        );
        assert_eq!(
            sk.sign(&expected_payload).to_bytes(),
            tx.signature,
            "{name}: Ed25519 is deterministic here"
        );
        let vk: VerifyingKey = sk.verifying_key();
        let sig = ed25519_dalek::Signature::from_bytes(&tx.signature);
        assert!(
            vk.verify(blake3::hash(&expected_payload).as_bytes(), &sig).is_err(),
            "{name}: the signature must not verify over a prehash"
        );

        let full = hex_bytes(
            "expected.full_transaction",
            &entry["expected"]["full_transaction"],
        );
        assert_eq!(tx.encode(), full, "{name}: full SCALE encoding differs");
        assert_eq!(
            full.len() as u64,
            u64_at(
                "expected.full_transaction_length",
                &entry["expected"]["full_transaction_length"]
            )
        );
        assert_eq!(
            Transaction::decode(&mut &full[..]).unwrap(),
            tx,
            "{name}: decode"
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
fn canonical_diagnostic_nonce_is_asymmetric() {
    let tdoc = tasks_doc();
    let canonical = named("valid", &tdoc["valid"], "canonical");
    let bytes = hex_bytes("nonce_u64_le", &canonical["expected"]["nonce_u64_le"]);
    let mut reversed = bytes.clone();
    reversed.reverse();
    assert_ne!(
        bytes, reversed,
        "the canonical nonce is palindromic and cannot prove byte order"
    );
}

#[test]
fn serialized_compute_task_json_matches() {
    let tdoc = tasks_doc();
    let pinned = &tdoc["serialized_transaction"];
    let name = pinned["vector"].as_str().expect("serialized_transaction.vector missing");
    let (tx, _) = transaction_from(named("valid", &tdoc["valid"], name), &tdoc);
    let actual = serde_json::to_value(&tx).expect("transaction serialises");
    assert_eq!(
        actual, pinned["object"],
        "the serialised Transaction object differs from the pinned wire form"
    );
    // The mixed byte representation, asserted explicitly: Address fields
    // are hex, plain byte arrays are arrays of numbers — the same rule the
    // receipt follows.
    let task = &actual["payload"]["ComputeTask"];
    for field in ["submitter", "executor"] {
        assert!(
            task[field].as_str().is_some_and(|s| s.starts_with("0x")),
            "task.{field} should be an 0x hex string"
        );
    }
    for field in ["salt", "input_commitment", "execution_spec"] {
        assert!(
            task[field].is_array(),
            "task.{field} serialises as an array of numbers"
        );
    }
    assert_eq!(actual["tx_type"].as_str(), Some("ComputeTask"));
    assert!(
        actual["payload"].get("ComputeTask").is_some(),
        "externally tagged"
    );
    // And it deserialises back to the same transaction: submit_transaction
    // accepts exactly this object.
    let back: Transaction = serde_json::from_value(actual).expect("deserialises");
    assert_eq!(back, tx);
}

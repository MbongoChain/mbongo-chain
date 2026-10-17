# Inference contract vectors (`mbongo-infer:v1`)

Golden vectors for the MVP inference execution contract defined by
[`docs/product/MVP_INFERENCE_CONTRACT_v1.md`](../../docs/product/MVP_INFERENCE_CONTRACT_v1.md)
and implemented in `crates/mbongo-compute/src/inference/`. Nothing here is a
protocol vector: the chain sees only the 47 opaque `execution_spec` bytes of
RFC 0005 and never interprets them. The protocol vectors remain
`compute-task/`, `receipt/`, `rpc/` and `transaction/`.

| File | Pins |
|---|---|
| `kaayu-cv-extract-v1.json` | the frozen MVP job spec, its canonical bytes, `job_spec_digest = BLAKE3("mbongo:infer-job-spec:v1" ‖ canonical)`, the 47-byte `execution_spec`, and the SHA-256 of the prompt template and output schema. Byte-exact (test A20). |
| `execution-spec-negative-v1.json` | `execution_spec` values the parser refuses and the error code for each (A04, A05) |
| `job-spec-negative-v1.json` | job-spec documents and the result of `InferenceJobSpecV1::from_json` for each, including a valid spec with a different model artifact digest (A08, A11) |
| `input-example-v1.json` | a **synthetic** CV text, its canonical input object and `input_commitment` (A22). No real person, no name, no contact data. |
| `output-canonicalization-v1.json` | one schema-valid model output in two non-canonical forms, the single canonical byte form, its `output_commitment`, and outputs that yield no result and no receipt (A10, A13) |

The committed job-spec document itself is
`crates/mbongo-compute/profiles/kaayu-cv-extract-v1/job-spec.json`: exactly the
canonical bytes, no trailing newline. The three files under that directory
are marked `-text` in `.gitattributes` so no checkout can alter the bytes
their digests pin.

Consumers: the Rust test group `inference_contract` (CI step "Mbongo MVP
Inference Contract") today; MVP-B (worker) and MVP-C (client helpers) must
reproduce the same bytes.

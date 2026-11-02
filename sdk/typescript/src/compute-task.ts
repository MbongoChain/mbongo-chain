/**
 * `ComputeTask` primitives: the canonical envelope, its identity, the
 * transaction that commits it, and the receipt that answers it.
 *
 * Pure, synchronous and offline except for the two `submit*` helpers, which
 * compose the client's existing `submitTransaction`. Nothing here executes a
 * task, fetches its input, stores its result or talks to any service other
 * than a node's JSON-RPC.
 *
 * Authority:
 * - `docs/rfcs/0005-compute-task-commitment-v1.md` — the envelope (§2.1),
 *   `task_id` (§2.2), authority (§2.5), duplicates (§2.6), the payload
 *   variant (§2.7), field constraints (§2.8), the bound (§2.10), and the
 *   receipt binding rules (q)–(s) of §3
 * - `docs/specs/rpc_v0.3.md` — the JSON wire form (§4.4)
 * - `crates/mbongo-core/src/compute_task.rs` — the protocol type
 *
 * Correctness is checked against `test-vectors/compute-task/compute-task-v1.json`,
 * `test-vectors/compute-task/anchor-binding-v1.json` and
 * `test-vectors/rpc/compute-task-rpc-v1.json`, the shared fixtures Rust also
 * reads. Those files are the source of truth; no expected value is duplicated
 * here.
 *
 * ## What a task commits, and what it does not
 *
 * A task names one executor before it is committed, and consensus lets only
 * that executor answer it (rule s). It commits to an input by a 32-byte
 * commitment the chain never learns how to derive, and to an opaque
 * `executionSpec` the chain never interprets. The input itself, the result,
 * and everything about how the work is scheduled or performed stay
 * off-chain. An anchored receipt is a **bound claim** by the named executor;
 * neither this package nor the chain checks that the work was done correctly.
 */

import { blake3 } from "@noble/hashes/blake3.js";
import { ed25519 } from "@noble/curves/ed25519.js";

import type { MbongoClient } from "./client.js";
import {
  MbongoComputeTaskError,
  MbongoReceiptBindingError,
  MbongoRpcError,
  type ComputeTaskRejection,
} from "./errors.js";
import { normalizeU64 } from "./numeric.js";
import { receiptHash, type Receipt } from "./receipt.js";
import type { Hash, Transaction, WireComputeTask } from "./types.js";

/** The only envelope version RFC 0005 defines (§2.1); rule (m). */
export const COMPUTE_TASK_VERSION = 1;

/**
 * Maximum `executionSpec` length in bytes (RFC 0005 §2.10); rule (n).
 *
 * The only variable-length field, so this bounds the whole envelope. A task
 * above it cannot be committed, so it is rejected before anything
 * canonical-looking is produced for it.
 */
export const MAX_EXECUTION_SPEC_BYTES = 1024;

/**
 * The domain separator of `task_id` (RFC 0005 §2.2): these 22 ASCII bytes,
 * prepended **raw** to the canonical task bytes — no NUL terminator, no
 * length prefix, no hex rendering.
 */
export const COMPUTE_TASK_DOMAIN = "mbongo:compute-task:v1";

/** `TransactionType::ComputeTask`, from `#[codec(index = 1)]` — frozen at v0.3. */
const TX_TYPE_COMPUTE_TASK = 0x01;
/** `TransactionPayload::ComputeTask`, from `#[codec(index = 2)]` (RFC 0005 §2.7). */
const PAYLOAD_COMPUTE_TASK = 0x02;

const ADDRESS_BYTES = 32;
const HASH_BYTES = 32;
const SIGNATURE_BYTES = 64;
const SECRET_KEY_BYTES = 32;

/**
 * Bytes before the nested task in the signing payload:
 * `1 + 32 + 32 + 16 + 8 + 1`. Constant, because every field before the task is
 * fixed-width — the same offset a receipt has in an `AnchorReceipt`.
 */
export const COMPUTE_TASK_PAYLOAD_PREFIX_BYTES = 90;

/**
 * A compute task envelope, in its canonical byte form.
 *
 * Six fields, exactly RFC 0005 §2.1. `taskId` is **not** a field: it is
 * derived by {@link computeTaskId}, so it can never disagree with the bytes.
 * Fields are `Uint8Array` for the same reason receipt fields are: these bytes
 * are hashed and signed, and text invites signing the text.
 */
export interface ComputeTask {
  /** Protocol version. Must be {@link COMPUTE_TASK_VERSION}. */
  version: number;
  /** 32 bytes: the committing account's Ed25519 public key. Must equal the transaction sender (rule o). */
  submitter: Uint8Array;
  /** 32 bytes: the one executor authorised to answer. A different executor is a different task. */
  executor: Uint8Array;
  /** 32 bytes, client-chosen and opaque. Zero is legal. Not the nonce, and not input blinding. */
  salt: Uint8Array;
  /** 32 bytes: an opaque commitment to off-chain input. How it was derived is not the chain's business. */
  inputCommitment: Uint8Array;
  /** Opaque bytes, at most {@link MAX_EXECUTION_SPEC_BYTES}. Never interpreted by the chain, or by this package. */
  executionSpec: Uint8Array;
}

/**
 * A signed `ComputeTask` transaction, in its canonical byte form.
 *
 * The fields consensus pins are not caller-settable: `sender` is the task's
 * submitter (rule o), `receiver` is the zero address and `amount` is zero
 * (rule l). Only `nonce` is chosen by the caller.
 */
export interface ComputeTaskTransaction {
  /** Always `"ComputeTask"`. */
  readonly txType: "ComputeTask";
  /** 32 bytes, equal to `task.submitter`. */
  readonly sender: Uint8Array;
  /** 32 zero bytes. */
  readonly receiver: Uint8Array;
  /** Always `0n`. Committing a task transfers nothing. */
  readonly amount: 0n;
  /** The sender account's current nonce, supplied by the caller. Exact across the `u64` domain. */
  readonly nonce: bigint;
  /** The committed task. */
  readonly task: ComputeTask;
  /** 64 bytes: Ed25519 by the submitter over the raw signing payload. */
  readonly signature: Uint8Array;
}

function requireBytes(field: string, value: unknown, length: number): Uint8Array {
  if (!(value instanceof Uint8Array)) {
    throw new MbongoComputeTaskError(field, `expected a Uint8Array, got ${typeof value}`);
  }
  if (value.length !== length) {
    throw new MbongoComputeTaskError(
      field,
      `expected exactly ${length} bytes, got ${value.length}`,
    );
  }
  return value;
}

/**
 * Validates everything the canonical encoding depends on.
 *
 * Structural only. The rules that need chain state — uniqueness (p), and
 * whatever the node decides at admission — are not repeated here, because
 * the node is the authority and a local copy would only drift.
 */
function assertCanonical(task: ComputeTask): void {
  if (!Number.isInteger(task.version)) {
    throw new MbongoComputeTaskError("version", "must be an integer");
  }
  if (task.version < 0 || task.version > 0xff) {
    throw new MbongoComputeTaskError("version", "must fit in a u8");
  }
  if (task.version !== COMPUTE_TASK_VERSION) {
    // Fail closed. Hashing an unrecognised version would produce a
    // canonical-looking task_id for rules we do not know.
    throw new MbongoComputeTaskError(
      "version",
      `unsupported compute task version ${task.version}; this package implements version ${COMPUTE_TASK_VERSION}`,
    );
  }
  requireBytes("submitter", task.submitter, ADDRESS_BYTES);
  requireBytes("executor", task.executor, ADDRESS_BYTES);
  requireBytes("salt", task.salt, HASH_BYTES);
  requireBytes("inputCommitment", task.inputCommitment, HASH_BYTES);
  if (!(task.executionSpec instanceof Uint8Array)) {
    throw new MbongoComputeTaskError(
      "executionSpec",
      `expected a Uint8Array, got ${typeof task.executionSpec}`,
    );
  }
  if (task.executionSpec.length > MAX_EXECUTION_SPEC_BYTES) {
    throw new MbongoComputeTaskError(
      "executionSpec",
      `${task.executionSpec.length} bytes exceeds the ${MAX_EXECUTION_SPEC_BYTES}-byte consensus maximum; ` +
        "a task this large cannot be committed",
    );
  }
}

/**
 * SCALE compact encoding of a length. Only the modes reachable under the
 * `executionSpec` bound exist: one byte below 64, two bytes below 16384.
 * The width change at 64 is the interop mistake this encoder exists to
 * avoid; at the 1024-byte maximum the prefix is two bytes.
 */
function compactLength(n: number): Uint8Array {
  if (n < 64) {
    return new Uint8Array([n << 2]);
  }
  const encoded = (n << 2) | 0b01;
  return new Uint8Array([encoded & 0xff, (encoded >>> 8) & 0xff]);
}

function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/** SCALE fixed-width unsigned integer: little-endian, never compact. */
function unsignedLE(value: bigint, bytes: number): Uint8Array {
  let v = value;
  const out = new Uint8Array(bytes);
  for (let i = 0; i < bytes; i++) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  if (v !== 0n) {
    throw new MbongoComputeTaskError("encoding", `value does not fit in ${bytes} bytes`);
  }
  return out;
}

function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a[i]! ^ b[i]!;
  return diff === 0;
}

/**
 * The canonical SCALE encoding of a task (RFC 0005 §2.2): the six fields in
 * order, `executionSpec` behind a compact length prefix.
 *
 * ```
 * version[1] || submitter[32] || executor[32] || salt[32]
 *            || inputCommitment[32] || compact(len) || executionSpec
 * ```
 *
 * Never mutates the task or its arrays.
 *
 * @throws {MbongoComputeTaskError} the task is malformed, its version is
 * unsupported, or its `executionSpec` exceeds the consensus bound.
 */
export function encodeComputeTask(task: ComputeTask): Uint8Array {
  assertCanonical(task);
  return concat([
    Uint8Array.from([task.version]),
    task.submitter,
    task.executor,
    task.salt,
    task.inputCommitment,
    compactLength(task.executionSpec.length),
    task.executionSpec,
  ]);
}

/** The 22 raw ASCII bytes of {@link COMPUTE_TASK_DOMAIN}. */
function domainBytes(): Uint8Array {
  const out = new Uint8Array(COMPUTE_TASK_DOMAIN.length);
  for (let i = 0; i < COMPUTE_TASK_DOMAIN.length; i++) {
    out[i] = COMPUTE_TASK_DOMAIN.charCodeAt(i);
  }
  return out;
}

/**
 * The `task_id` preimage: the raw domain tag followed by the canonical task
 * bytes. Exposed so a caller can see exactly what is hashed.
 *
 * @throws {MbongoComputeTaskError} as {@link encodeComputeTask}.
 */
export function computeTaskIdPreimage(task: ComputeTask): Uint8Array {
  return concat([domainBytes(), encodeComputeTask(task)]);
}

/**
 * The task identity: `BLAKE3(DOMAIN_TASK || SCALE(task))` (RFC 0005 §2.2).
 *
 * Content-derived over all six fields, so changing any of them — the
 * executor included — yields a different task. The transaction nonce,
 * signature and type are not part of it: a resubmission after a nonce race
 * keeps its identity. This is the value a receipt's `taskId` must carry.
 *
 * @throws {MbongoComputeTaskError} as {@link encodeComputeTask}.
 */
export function computeTaskId(task: ComputeTask): Uint8Array {
  return blake3(computeTaskIdPreimage(task));
}

/**
 * The bytes a `ComputeTask` transaction is signed over.
 *
 * ```
 * 0x01 || sender[32] || receiver[32] || amount_u128_le[16]
 *      || nonce_u64_le[8] || 0x02 || <canonical task bytes>
 * ```
 *
 * `sender` is the task's submitter, `receiver` is zero and `amount` is zero,
 * because consensus pins all three (rules l and o). The task enters as a
 * nested SCALE struct, not a length-prefixed byte vector, so its canonical
 * bytes appear contiguously at offset {@link COMPUTE_TASK_PAYLOAD_PREFIX_BYTES}.
 *
 * @throws {MbongoComputeTaskError} the task cannot be canonically encoded.
 * @throws {MbongoNumericRangeError} `nonce` is not a safe non-negative integer.
 */
export function computeTaskSigningPayload(
  task: ComputeTask,
  nonce: number | bigint,
): Uint8Array {
  const exactNonce = normalizeU64("nonce", nonce);
  const taskBytes = encodeComputeTask(task);
  return concat([
    Uint8Array.from([TX_TYPE_COMPUTE_TASK]),
    task.submitter,
    new Uint8Array(ADDRESS_BYTES),
    unsignedLE(0n, 16),
    unsignedLE(exactNonce, 8),
    Uint8Array.from([PAYLOAD_COMPUTE_TASK]),
    taskBytes,
  ]);
}

function publicKeyOf(field: string, secretKey: Uint8Array): Uint8Array {
  requireBytes(field, secretKey, SECRET_KEY_BYTES);
  try {
    return ed25519.getPublicKey(secretKey);
  } catch {
    throw new MbongoComputeTaskError(field, "is not a usable Ed25519 seed");
  }
}

/**
 * Builds and signs a `ComputeTask` transaction.
 *
 * `secretKey` is the 32-byte Ed25519 seed of the **submitter**: consensus
 * requires `sender == task.submitter` (rule o), so a key that does not derive
 * the submitter is rejected here rather than producing a transaction the node
 * is guaranteed to refuse. The envelope itself carries no signature
 * (RFC 0005 §2.5); the transaction signature is the only authentication.
 *
 * The key is used and discarded; nothing is cached or stored, and the
 * caller's array is never mutated.
 *
 * @throws {MbongoComputeTaskError} the key is the wrong width, is unusable,
 * or does not derive `task.submitter`; or the task cannot be encoded.
 * @throws {MbongoNumericRangeError} `nonce` is not a safe non-negative integer.
 */
export function signComputeTaskTransaction(
  task: ComputeTask,
  nonce: number | bigint,
  secretKey: Uint8Array,
): ComputeTaskTransaction {
  const publicKey = publicKeyOf("secretKey", secretKey);
  const exactNonce = normalizeU64("nonce", nonce);
  const payload = computeTaskSigningPayload(task, exactNonce);
  if (!equalBytes(publicKey, task.submitter)) {
    throw new MbongoComputeTaskError(
      "secretKey",
      "does not derive the task submitter; consensus requires " +
        "sender == task.submitter, so this transaction could never be committed",
    );
  }
  // Signed over the RAW payload; no prehash.
  const signature = ed25519.sign(payload, secretKey);
  return {
    txType: "ComputeTask",
    sender: Uint8Array.from(task.submitter),
    receiver: new Uint8Array(ADDRESS_BYTES),
    amount: 0n,
    nonce: exactNonce,
    task,
    signature,
  };
}

/** Full canonical SCALE: the signing payload followed by the signature. */
function encodeComputeTaskTransaction(tx: ComputeTaskTransaction): Uint8Array {
  const payload = computeTaskSigningPayload(tx.task, tx.nonce);
  return concat([payload, requireBytes("signature", tx.signature, SIGNATURE_BYTES)]);
}

/**
 * The transaction hash: `BLAKE3` over the **full signed** encoding, signature
 * included — what `submit_transaction` returns. Distinct from
 * {@link computeTaskId}, which covers the task alone.
 */
export function computeTaskTransactionHash(tx: ComputeTaskTransaction): Uint8Array {
  return blake3(encodeComputeTaskTransaction(tx));
}

function toHex(bytes: Uint8Array): string {
  let s = "0x";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

/**
 * Converts a canonical task to the JSON object the node's serde expects
 * (`rpc_v0.3` §4.4): `submitter` and `executor` are `0x` hex, because the
 * Rust type is an `Address` with its own serializer; `salt`,
 * `inputCommitment` and `executionSpec` are plain byte arrays and cross the
 * wire as **arrays of numbers**. Every byte is preserved exactly; nothing is
 * decoded as text.
 *
 * @throws {MbongoComputeTaskError} the task cannot be canonically encoded.
 */
export function computeTaskToWire(task: ComputeTask): WireComputeTask {
  assertCanonical(task);
  return {
    version: task.version,
    submitter: toHex(task.submitter),
    executor: toHex(task.executor),
    salt: Array.from(task.salt),
    input_commitment: Array.from(task.inputCommitment),
    execution_spec: Array.from(task.executionSpec),
  };
}

/**
 * Converts the canonical transaction to the exact JSON object
 * `submit_transaction` accepts. The shape is pinned by
 * `test-vectors/rpc/compute-task-rpc-v1.json`, not inferred from prose.
 */
export function computeTaskTransactionToWire(tx: ComputeTaskTransaction): Transaction {
  return {
    tx_type: "ComputeTask",
    sender: toHex(tx.sender),
    receiver: toHex(tx.receiver),
    amount: 0n,
    nonce: tx.nonce,
    payload: { ComputeTask: computeTaskToWire(tx.task) },
    signature: toHex(tx.signature),
  };
}

function hexBytes(field: string, value: unknown, length: number): Uint8Array {
  if (typeof value !== "string") {
    throw new MbongoComputeTaskError(field, `expected a hex string, got ${typeof value}`);
  }
  if (!value.startsWith("0x")) {
    throw new MbongoComputeTaskError(field, "expected a 0x-prefixed hex string");
  }
  const body = value.slice(2);
  if (body.length !== length * 2) {
    throw new MbongoComputeTaskError(
      field,
      `expected exactly ${length} bytes (${length * 2} hex characters), got ${body.length / 2}`,
    );
  }
  if (!/^[0-9a-f]*$/.test(body)) {
    throw new MbongoComputeTaskError(field, "expected lowercase hexadecimal characters");
  }
  const out = new Uint8Array(length);
  for (let i = 0; i < length; i++) {
    out[i] = Number.parseInt(body.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function byteArray(field: string, value: unknown, exactLength?: number): Uint8Array {
  if (!Array.isArray(value)) {
    throw new MbongoComputeTaskError(field, `expected an array of byte values, got ${typeof value}`);
  }
  if (exactLength !== undefined && value.length !== exactLength) {
    throw new MbongoComputeTaskError(
      field,
      `expected exactly ${exactLength} bytes, got ${value.length}`,
    );
  }
  const out = new Uint8Array(value.length);
  for (let i = 0; i < value.length; i++) {
    const b: unknown = value[i];
    if (typeof b !== "number" || !Number.isInteger(b) || b < 0 || b > 255) {
      throw new MbongoComputeTaskError(
        `${field}[${i}]`,
        `expected an integer in 0..=255, got ${String(b)}`,
      );
    }
    out[i] = b;
  }
  return out;
}

/**
 * Converts a task from its JSON wire form to canonical bytes.
 *
 * Representation only: it decodes and does nothing else — no hashing, no
 * network, no judgement about the task. Fails closed on every field, and
 * every returned array is a fresh copy.
 *
 * @throws {MbongoComputeTaskError} the version is unsupported, a field has
 * the wrong width or type, a byte is out of range, the hex is malformed, or
 * `execution_spec` exceeds the consensus bound.
 */
export function wireComputeTaskToComputeTask(wire: WireComputeTask): ComputeTask {
  if (wire === null || typeof wire !== "object") {
    throw new MbongoComputeTaskError("task", `expected an object, got ${typeof wire}`);
  }
  if (!Number.isInteger(wire.version)) {
    throw new MbongoComputeTaskError("version", "must be an integer");
  }
  if (wire.version !== COMPUTE_TASK_VERSION) {
    throw new MbongoComputeTaskError(
      "version",
      `unsupported compute task version ${wire.version}; this package implements version ${COMPUTE_TASK_VERSION}`,
    );
  }
  const executionSpec = byteArray("executionSpec", wire.execution_spec);
  if (executionSpec.length > MAX_EXECUTION_SPEC_BYTES) {
    throw new MbongoComputeTaskError(
      "executionSpec",
      `${executionSpec.length} bytes exceeds the ${MAX_EXECUTION_SPEC_BYTES}-byte consensus maximum`,
    );
  }
  return {
    version: wire.version,
    submitter: hexBytes("submitter", wire.submitter, ADDRESS_BYTES),
    executor: hexBytes("executor", wire.executor, ADDRESS_BYTES),
    salt: byteArray("salt", wire.salt, HASH_BYTES),
    inputCommitment: byteArray("inputCommitment", wire.input_commitment, HASH_BYTES),
    executionSpec,
  };
}

/**
 * Maps a node rejection message onto a reason. The node answers `-32603`
 * with a message for every admission rule; `crates/mbongo-node/src/backend.rs`
 * is where these strings live.
 */
function classify(message: string): ComputeTaskRejection {
  const m = message.toLowerCase();
  if (m.includes("already registered")) return "duplicate-task";
  if (m.includes("compute task already pending")) return "task-pending";
  if (m.includes("unsupported compute task version")) return "unsupported-task-version";
  if (m.includes("execution_spec too large")) return "execution-spec-too-large";
  if (m.includes("sender must equal task submitter")) return "sender-submitter-mismatch";
  if (m.includes("invalid signature")) return "invalid-transaction-signature";
  if (m.includes("invalid nonce") || m.includes("duplicate sender nonce")) return "invalid-nonce";
  if (m.includes("amount 0 and zero receiver")) return "invalid-task-fields";
  if (m.includes("payload does not match")) return "invalid-task-fields";
  if (m.includes("insufficient balance")) return "sender-account-unusable";
  return "unknown";
}

/**
 * Submits a signed `ComputeTask` transaction and returns the transaction hash
 * the node reports.
 *
 * Composes the client's existing `submitTransaction`; no new RPC method
 * exists for tasks (RFC 0005 §7). A returned hash means the node **accepted
 * the transaction into its mempool** — not that it is in a block. To observe
 * the task, read the block that includes it with `getBlockByHeight` and
 * {@link computeTasksInBlock}; there is no lookup by `task_id`.
 *
 * @throws {MbongoComputeTaskError} the node rejected the task, with a `reason`.
 * @throws {MbongoRpcError} any other JSON-RPC error.
 * @throws {MbongoTransportError} no usable response.
 */
export async function submitComputeTask(
  client: MbongoClient,
  tx: ComputeTaskTransaction,
): Promise<Hash> {
  try {
    return await client.submitTransaction(computeTaskTransactionToWire(tx));
  } catch (err) {
    if (err instanceof MbongoRpcError && !err.isMethodUnavailable) {
      throw new MbongoComputeTaskError("submit", err.message, classify(err.message), err);
    }
    throw err;
  }
}

// ── The receipt that answers a task ──────────────────────────────────────

/**
 * The parts of a receipt the executor supplies. Everything else — `taskId`,
 * `inputCommitment`, `executor` — is bound to the task and is not a
 * parameter, so it cannot be supplied wrong.
 */
export interface BoundReceiptFields {
  /** 32 bytes: the executor's commitment to its output. Opaque to the chain. */
  outputCommitment: Uint8Array;
  /** Opaque bytes, at most 4096. Defaults to empty. */
  metadata?: Uint8Array;
}

/**
 * Builds and signs the receipt that answers `task`, as the executor the task
 * named.
 *
 * Three receipt fields are derived from the task rather than accepted:
 * `taskId` is {@link computeTaskId}`(task)`, `inputCommitment` is the task's,
 * and `executor` is `task.executor`. That is exactly what consensus checks
 * under RFC 0005 rules (q), (r) and (s), so a receipt built here cannot fail
 * them by construction. `executorSecretKey` must derive `task.executor`; a
 * key that does not — the submitter's, a relayer's, anyone else's — is
 * rejected, because the receipt it produced could never be anchored.
 *
 * The receipt is signed over the raw 32-byte receipt hash by that key. The
 * key is used and discarded.
 *
 * This says nothing about the work: the executor asserts an output
 * commitment, and neither this package nor the chain checks it.
 *
 * @throws {MbongoComputeTaskError} the task is malformed, or the key does not
 * derive `task.executor`.
 * @throws {MbongoReceiptError} the fields cannot form a canonical receipt.
 */
export function signBoundReceipt(
  task: ComputeTask,
  fields: BoundReceiptFields,
  executorSecretKey: Uint8Array,
): Receipt {
  assertCanonical(task);
  const publicKey = publicKeyOf("executorSecretKey", executorSecretKey);
  if (!equalBytes(publicKey, task.executor)) {
    throw new MbongoComputeTaskError(
      "executorSecretKey",
      "does not derive task.executor; only the executor the task named may " +
        "answer it (RFC 0005 rule s), so this receipt could never be anchored",
    );
  }
  const unsigned: Receipt = {
    version: 1,
    taskId: computeTaskId(task),
    inputCommitment: Uint8Array.from(task.inputCommitment),
    outputCommitment: fields.outputCommitment,
    executor: Uint8Array.from(task.executor),
    metadata: fields.metadata ?? new Uint8Array(0),
    signature: new Uint8Array(SIGNATURE_BYTES),
  };
  // receiptHash validates the receipt's own widths and bound.
  const signature = ed25519.sign(receiptHash(unsigned), executorSecretKey);
  return { ...unsigned, signature };
}

/**
 * Checks that `receipt` is bound to `task` the way RFC 0005 rules (q)–(s)
 * require: its `taskId` is the task's derived identity, its
 * `inputCommitment` equals the task's, and its `executor` is the executor
 * the task named. Three byte equalities, and nothing about chain state: the
 * node still decides whether the task is registered and whether the
 * `taskId` was already anchored.
 *
 * Use it on a receipt built elsewhere. A receipt from
 * {@link signBoundReceipt} passes by construction.
 *
 * @throws {MbongoReceiptBindingError} naming the first binding that fails.
 * @throws {MbongoComputeTaskError} the task cannot be canonically encoded.
 */
export function assertReceiptBoundToTask(receipt: Receipt, task: ComputeTask): void {
  const taskId = computeTaskId(task);
  if (!(receipt.taskId instanceof Uint8Array) || !equalBytes(receipt.taskId, taskId)) {
    throw new MbongoReceiptBindingError(
      "task-id",
      "receipt.taskId is not the identity derived from this task (rule q)",
    );
  }
  if (
    !(receipt.inputCommitment instanceof Uint8Array) ||
    !equalBytes(receipt.inputCommitment, task.inputCommitment)
  ) {
    throw new MbongoReceiptBindingError(
      "input-commitment",
      "receipt.inputCommitment does not equal the task's (rule r)",
    );
  }
  if (!(receipt.executor instanceof Uint8Array) || !equalBytes(receipt.executor, task.executor)) {
    throw new MbongoReceiptBindingError(
      "executor",
      "receipt.executor is not the executor the task named (rule s)",
    );
  }
}

/**
 * Returns the tasks committed in a block, in transaction order.
 *
 * Pure and offline, the counterpart of `receiptsInBlock`: give it a block you
 * already fetched. Transactions with any other payload are ignored; a
 * transaction that claims to carry a task but whose payload cannot be
 * decoded **throws**, because under-reporting a block's contents is worse
 * than failing.
 *
 * Observing a task in a block is how an executor learns it was asked
 * (RFC 0005 §6): there is no task lookup RPC, and none is proposed.
 *
 * @throws {MbongoComputeTaskError} the block shape is wrong, or a committed
 * task cannot be decoded.
 */
export function computeTasksInBlock(block: {
  body?: { transactions?: unknown };
}): ComputeTask[] {
  if (block === null || typeof block !== "object") {
    throw new MbongoComputeTaskError("block", `expected a block object, got ${typeof block}`);
  }
  const body: unknown = block.body;
  if (body === null || typeof body !== "object") {
    throw new MbongoComputeTaskError("block.body", "expected a block body object");
  }
  const transactions: unknown = (body as { transactions?: unknown }).transactions;
  if (!Array.isArray(transactions)) {
    throw new MbongoComputeTaskError("block.body.transactions", "expected an array");
  }
  const tasks: ComputeTask[] = [];
  for (let i = 0; i < transactions.length; i++) {
    const payload: unknown = (transactions[i] as { payload?: unknown } | undefined)?.payload;
    if (payload === null || typeof payload !== "object") continue;
    if (!("ComputeTask" in payload)) continue;
    try {
      tasks.push(wireComputeTaskToComputeTask((payload as { ComputeTask: WireComputeTask }).ComputeTask));
    } catch (err) {
      if (err instanceof MbongoComputeTaskError) {
        throw new MbongoComputeTaskError(
          `block.body.transactions[${i}].payload.ComputeTask.${err.field}`,
          err.message.slice(err.field.length + 2),
        );
      }
      throw err;
    }
  }
  return tasks;
}

/**
 * `AnchorReceipt` transaction construction, signing and submission.
 *
 * Anchoring puts a signed receipt inside a transaction that is itself signed.
 * The two signatures are **different**, and confusing them is the mistake this
 * module is shaped to prevent:
 *
 * | | key | message |
 * |---|---|---|
 * | `receipt.signature` | executor | the raw 32 bytes of `receiptHash` |
 * | transaction signature | sender | the **raw** transaction signing payload |
 *
 * Consensus requires `sender == receipt.executor`, so one key produces both.
 * The messages still differ, so the signatures differ. The transaction
 * signature has **no prehash**: it is over the raw payload bytes, never over a
 * digest of them.
 *
 * Correctness is checked against
 * `test-vectors/transaction/anchor-receipt-v1.json`, the shared fixture Rust
 * also reads. That file is the source of truth; no expected value is
 * duplicated here.
 *
 * Authority:
 * - `crates/mbongo-core/src/primitives.rs` — `Transaction`, `TransactionType`,
 *   `TransactionPayload`, `Transaction::signing_payload`
 * - `crates/mbongo-node/src/backend.rs` — admission and consensus rules
 * - `docs/rfcs/0002-receipt-anchoring-v0.3.md`; `PROTOCOL_LOCK_v0.3` (FROZEN)
 */

import { blake3 } from "@noble/hashes/blake3.js";
import { ed25519 } from "@noble/curves/ed25519.js";

import type { MbongoClient } from "./client.js";
import { MbongoAnchorError, MbongoRpcError, type AnchorRejection } from "./errors.js";
import { normalizeU64 } from "./numeric.js";
import { encodeReceipt, type Receipt } from "./receipt.js";
import type { Hash, Transaction, WireReceipt } from "./types.js";

/** `TransactionType::AnchorReceipt`, from `#[codec(index = 3)]`. */
const TX_TYPE_ANCHOR_RECEIPT = 0x03;
/** `TransactionPayload::AnchorReceipt`, from `#[codec(index = 1)]`. */
const PAYLOAD_ANCHOR_RECEIPT = 0x01;

const ADDRESS_BYTES = 32;
const SIGNATURE_BYTES = 64;
const SECRET_KEY_BYTES = 32;

/**
 * Bytes before the nested receipt in the signing payload:
 * `1 + 32 + 32 + 16 + 8 + 1`. Constant, because every field before the receipt
 * is fixed-width — so this offset does not depend on the metadata length.
 */
export const ANCHOR_RECEIPT_PAYLOAD_PREFIX_BYTES = 90;

/**
 * A signed `AnchorReceipt` transaction, in its canonical byte form.
 *
 * The fields consensus pins are not caller-settable: `sender` is the receipt's
 * executor, `receiver` is the zero address and `amount` is zero. Only `nonce`
 * is chosen by the caller.
 */
export interface AnchorReceiptTransaction {
  /** Always `"AnchorReceipt"`. */
  readonly txType: "AnchorReceipt";
  /** 32 bytes, equal to `receipt.executor`. */
  readonly sender: Uint8Array;
  /** 32 zero bytes. */
  readonly receiver: Uint8Array;
  /** Always `0n`. Anchoring transfers nothing. */
  readonly amount: 0n;
  /**
   * The sender account's current nonce, supplied by the caller.
   *
   * Exact across the whole `u64` domain. Builders accept a `bigint` or a
   * safe non-negative `number`; the canonical form keeps the `bigint`.
   */
  readonly nonce: bigint;
  /** The anchored receipt, with its own executor signature intact. */
  readonly receipt: Receipt;
  /** 64 bytes: Ed25519 over the raw signing payload. */
  readonly signature: Uint8Array;
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

/**
 * SCALE fixed-width unsigned integer: little-endian, fixed width, never
 * compact. Private on purpose — a public integer codec would invite callers to
 * encode values this package refuses to vouch for.
 */
function unsignedLE(value: bigint, bytes: number): Uint8Array {
  let v = value;
  const out = new Uint8Array(bytes);
  for (let i = 0; i < bytes; i++) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  if (v !== 0n) {
    throw new MbongoAnchorError("encoding", `value does not fit in ${bytes} bytes`);
  }
  return out;
}

function requireBytes(field: string, value: unknown, length: number): Uint8Array {
  if (!(value instanceof Uint8Array)) {
    throw new MbongoAnchorError(field, `expected a Uint8Array, got ${typeof value}`);
  }
  if (value.length !== length) {
    throw new MbongoAnchorError(
      field,
      `expected exactly ${length} bytes, got ${value.length}`,
    );
  }
  return value;
}

/**
 * The bytes an `AnchorReceipt` transaction is signed over.
 *
 * ```
 * 0x03 || sender[32] || receiver[32] || amount_u128_le[16]
 *      || nonce_u64_le[8] || 0x01 || <full canonical receipt bytes>
 * ```
 *
 * The receipt enters as a nested SCALE struct, **not** as a length-prefixed
 * byte vector, so its canonical bytes appear contiguously at offset
 * {@link ANCHOR_RECEIPT_PAYLOAD_PREFIX_BYTES}.
 *
 * Never mutates the receipt or its arrays.
 *
 * @throws {MbongoReceiptError} the receipt cannot be canonically encoded.
 * @throws {MbongoNumericRangeError} `nonce` is not a safe non-negative integer.
 */
export function anchorReceiptSigningPayload(
  receipt: Receipt,
  nonce: number | bigint,
): Uint8Array {
  // Before anything is encoded, and well before anything is signed.
  const exactNonce = normalizeU64("nonce", nonce);
  // encodeReceipt carries the receipt's own validation: version, field widths
  // and the 4096-byte metadata bound. Duplicating those rules here would let
  // the two drift apart.
  const receiptBytes = encodeReceipt(receipt);

  return concat([
    Uint8Array.from([TX_TYPE_ANCHOR_RECEIPT]),
    receipt.executor,
    new Uint8Array(ADDRESS_BYTES),
    unsignedLE(0n, 16),
    unsignedLE(exactNonce, 8),
    Uint8Array.from([PAYLOAD_ANCHOR_RECEIPT]),
    receiptBytes,
  ]);
}

/**
 * Builds and signs an `AnchorReceipt` transaction.
 *
 * `secretKey` is the 32-byte Ed25519 seed of the **executor**: consensus
 * requires `sender == receipt.executor`, so a key that does not derive the
 * receipt's executor is rejected here rather than producing a transaction the
 * node is guaranteed to refuse.
 *
 * The key is used and discarded. Nothing is cached, stored or derived from it
 * beyond the public key and this signature, and the caller's array is never
 * mutated.
 *
 * @throws {MbongoAnchorError} the key is the wrong width, or does not match
 * `receipt.executor`.
 * @throws {MbongoReceiptError} the receipt cannot be canonically encoded.
 * @throws {MbongoNumericRangeError} `nonce` is not a safe non-negative integer.
 */
export function signAnchorReceiptTransaction(
  receipt: Receipt,
  nonce: number | bigint,
  secretKey: Uint8Array,
): AnchorReceiptTransaction {
  requireBytes("secretKey", secretKey, SECRET_KEY_BYTES);
  const exactNonce = normalizeU64("nonce", nonce);
  const payload = anchorReceiptSigningPayload(receipt, exactNonce);

  let publicKey: Uint8Array;
  try {
    publicKey = ed25519.getPublicKey(secretKey);
  } catch {
    throw new MbongoAnchorError("secretKey", "is not a usable Ed25519 seed");
  }
  if (!equalBytes(publicKey, receipt.executor)) {
    throw new MbongoAnchorError(
      "secretKey",
      "does not derive the receipt executor; consensus requires " +
        "sender == receipt.executor, so this transaction could never be anchored",
    );
  }

  // Signed over the RAW payload. Hashing it first would be the receipt's
  // scheme, not this one, and the node would reject the result.
  const signature = ed25519.sign(payload, secretKey);

  return {
    txType: "AnchorReceipt",
    sender: Uint8Array.from(receipt.executor),
    receiver: new Uint8Array(ADDRESS_BYTES),
    amount: 0n,
    nonce: exactNonce,
    receipt,
    signature,
  };
}

/** Full canonical SCALE: the signing payload followed by the signature. */
function encodeAnchorReceiptTransaction(tx: AnchorReceiptTransaction): Uint8Array {
  const payload = anchorReceiptSigningPayload(tx.receipt, tx.nonce);
  return concat([payload, requireBytes("signature", tx.signature, SIGNATURE_BYTES)]);
}

/**
 * The transaction hash: `BLAKE3` over the **full signed** encoding, signature
 * included. This is what `submit_transaction` returns, so a caller can check
 * the node answered about the transaction they actually signed.
 *
 * Distinct from `receiptHash`, which covers only the receipt and excludes its
 * signature, and from the signing payload, which is raw bytes and never
 * hashed before signing.
 */
export function anchorReceiptTransactionHash(
  tx: AnchorReceiptTransaction,
): Uint8Array {
  return blake3(encodeAnchorReceiptTransaction(tx));
}

function toHex(bytes: Uint8Array): string {
  let s = "0x";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

/**
 * Converts the canonical transaction to the exact JSON object the node's serde
 * expects.
 *
 * This boundary is explicit because the two representations genuinely differ.
 * In canonical form every byte field is a `Uint8Array`; on the wire, three
 * representations coexist. Hex appears exactly where the Rust type has a
 * custom serializer — `Address` and the 64-byte signatures — while the
 * receipt's `task_id`, `input_commitment`, `output_commitment` and `metadata`
 * are plain byte arrays with no annotation and serialise as **arrays of
 * numbers**.
 *
 * The shape is pinned by the shared fixture, not inferred from prose.
 */
export function anchorReceiptTransactionToWire(
  tx: AnchorReceiptTransaction,
): Transaction {
  const r = tx.receipt;
  const receipt: WireReceipt = {
    version: r.version,
    task_id: Array.from(r.taskId),
    input_commitment: Array.from(r.inputCommitment),
    output_commitment: Array.from(r.outputCommitment),
    executor: toHex(r.executor),
    metadata: Array.from(r.metadata),
    signature: toHex(r.signature),
  };
  return {
    tx_type: "AnchorReceipt",
    sender: toHex(tx.sender),
    receiver: toHex(tx.receiver),
    amount: 0n,
    nonce: tx.nonce,
    payload: { AnchorReceipt: receipt },
    signature: toHex(tx.signature),
  };
}

/**
 * Maps a node rejection message onto a reason. The node answers `-32603` with
 * a message for every anchoring rule, so the message is the only signal
 * available; `crates/mbongo-node/src/backend.rs` is where these strings live,
 * and the devnet harness already classifies the same way.
 */
function classify(message: string): AnchorRejection {
  const m = message.toLowerCase();
  if (m.includes("already anchored")) return "duplicate-task-id";
  if (m.includes("already pending")) return "task-id-pending";
  if (m.includes("metadata too large")) return "metadata-too-large";
  if (m.includes("unsupported receipt version")) return "unsupported-receipt-version";
  if (m.includes("sender must equal receipt executor")) return "sender-executor-mismatch";
  if (m.includes("invalid receipt signature")) return "invalid-receipt-signature";
  if (m.includes("invalid signature")) return "invalid-transaction-signature";
  if (m.includes("invalid nonce") || m.includes("duplicate sender nonce")) return "invalid-nonce";
  if (m.includes("amount 0 and zero receiver")) return "invalid-anchor-fields";
  if (m.includes("payload does not match")) return "invalid-anchor-fields";
  if (m.includes("insufficient balance")) return "sender-account-unusable";
  return "unknown";
}

/**
 * Submits a signed `AnchorReceipt` transaction and returns the transaction
 * hash the node reports.
 *
 * Composes: it converts to the wire shape and calls the client's existing
 * `submitTransaction`. It opens no connection and speaks no JSON-RPC of its
 * own; the only thing it adds is turning an anchoring rejection into a typed
 * {@link MbongoAnchorError} carrying a `reason`.
 *
 * A returned hash means the node **accepted the transaction into its
 * mempool** — not that it is in a block, and not that the computation the
 * receipt describes was performed correctly.
 *
 * Re-submitting the identical signed transaction after a timeout is safe: the
 * node treats an unanchored duplicate as idempotent. Once the `task_id` is
 * anchored, any further submission is rejected as `duplicate-task-id`, and
 * that reason cannot distinguish "already anchored by me" from "anchored by
 * someone else" — no public query API exists to tell them apart.
 *
 * @throws {MbongoAnchorError} the node rejected the anchoring attempt.
 * @throws {MbongoRpcError} any other JSON-RPC error.
 * @throws {MbongoTransportError} no usable response.
 */
export async function submitAnchorReceipt(
  client: MbongoClient,
  tx: AnchorReceiptTransaction,
): Promise<Hash> {
  try {
    return await client.submitTransaction(anchorReceiptTransactionToWire(tx));
  } catch (err) {
    if (err instanceof MbongoRpcError && !err.isMethodUnavailable) {
      throw new MbongoAnchorError("submit", err.message, classify(err.message), err);
    }
    throw err;
  }
}

function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a[i]! ^ b[i]!;
  return diff === 0;
}

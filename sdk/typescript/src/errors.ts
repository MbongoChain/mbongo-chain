/**
 * Typed errors for the Mbongo Chain SDK.
 *
 * Transport failures and JSON-RPC error objects are kept apart: a node that
 * answers with `-32601` behaved correctly, and conflating that with an
 * unreachable host loses information a caller needs.
 */

import type { JSONRPCErrorObject } from "./types.js";

/** JSON-RPC code for a method the node does not serve. */
export const METHOD_NOT_FOUND = -32601;

/** JSON-RPC code for invalid method parameters. */
export const INVALID_PARAMS = -32602;

/**
 * The node returned a JSON-RPC error object. Code, message and data are
 * preserved rather than flattened into a string.
 */
export class MbongoRpcError extends Error {
  readonly code: number;
  readonly data?: unknown;

  constructor(error: JSONRPCErrorObject) {
    super(`RPC error ${error.code}: ${error.message}`);
    this.name = "MbongoRpcError";
    this.code = error.code;
    this.data = error.data;
  }

  /**
   * True when the node does not serve this method.
   *
   * `-32601` means **the method is unavailable** — it is not implemented, or
   * it is a reserved name awaiting activation. It never means that a
   * resource was not found. Callers must not translate it into "no such
   * block", "no such transaction" or any other domain-level absence.
   */
  get isMethodUnavailable(): boolean {
    return this.code === METHOD_NOT_FOUND;
  }

  /** True when the node rejected the parameters as invalid. */
  get isInvalidParams(): boolean {
    return this.code === INVALID_PARAMS;
  }
}

/**
 * The request never produced a usable JSON-RPC response: the host was
 * unreachable, the HTTP status was not successful, or the body was not a
 * well-formed JSON-RPC 2.0 response.
 */
export class MbongoTransportError extends Error {
  readonly status?: number;

  constructor(message: string, status?: number) {
    super(message);
    this.name = "MbongoTransportError";
    this.status = status;
  }
}

/**
 * A numeric value could not be represented losslessly as a JavaScript
 * number, so the SDK refused to send or return it.
 *
 * This is a **local SDK restriction**, raised before any network call on the
 * outbound path and before returning data on the inbound path. It is not an
 * RPC rule: `rpc_v0.2.md` represents these fields as JSON numbers, and the
 * Rust types behind them (`u128` for `amount`, `u64` elsewhere) accept a
 * larger domain than JavaScript can hold exactly. The SDK fails closed
 * rather than transmit or return a silently rounded value.
 */
export class MbongoNumericRangeError extends Error {
  /** Dotted path of the offending field, e.g. `transaction.amount`. */
  readonly field: string;
  /** The value as the SDK saw it — already rounded, if rounding occurred. */
  readonly value: unknown;

  constructor(field: string, value: unknown, reason: string) {
    super(`${field}: ${reason} (received ${String(value)})`);
    this.name = "MbongoNumericRangeError";
    this.field = field;
    this.value = value;
  }
}

/**
 * A receipt is not canonically encodable: a field has the wrong width, the
 * version is unsupported, or the metadata exceeds the consensus bound.
 *
 * This is a **structural** failure, raised before any encoding or hashing. A
 * well-formed receipt whose signature simply does not verify is not an error
 * — `verifyReceiptSignature` returns `false` for that.
 */
export class MbongoReceiptError extends Error {
  /** The offending field, e.g. `metadata` or `taskId`. */
  readonly field: string;

  constructor(field: string, reason: string) {
    super(`${field}: ${reason}`);
    this.name = "MbongoReceiptError";
    this.field = field;
  }
}

/**
 * Why the node refused to anchor a receipt, or why a transaction could not be
 * built locally.
 *
 * The node answers `-32603` with a message for every anchoring rule, so these
 * are derived from that message. `"unknown"` means the rejection did not match
 * any rule this package recognises — treat it as a rejection, not as success.
 */
export type AnchorRejection =
  | "duplicate-task-id"
  | "task-id-pending"
  | "metadata-too-large"
  | "unsupported-receipt-version"
  | "sender-executor-mismatch"
  | "invalid-receipt-signature"
  | "invalid-transaction-signature"
  | "invalid-nonce"
  | "invalid-anchor-fields"
  | "sender-account-unusable"
  /** RFC 0005 rule (q): no committed task carries this `task_id`. */
  | "task-not-registered"
  /** RFC 0005 rule (r): the receipt's `input_commitment` is not the task's. */
  | "input-commitment-mismatch"
  /** RFC 0005 rule (s): the receipt's executor is not the one the task named. */
  | "executor-not-authorised"
  | "unknown";

/**
 * An `AnchorReceipt` transaction could not be built, or the node refused it.
 *
 * `reason` is absent for local construction failures — a wrong key width, a
 * key that does not derive the receipt's executor — and present when the
 * failure came back from the node.
 */
export class MbongoAnchorError extends Error {
  /** The offending field, or `submit` when the node rejected the attempt. */
  readonly field: string;
  /** Set only when the node rejected the attempt. */
  readonly reason?: AnchorRejection;
  /** The underlying JSON-RPC error, when there was one. */
  readonly cause?: unknown;

  constructor(field: string, reason: string, rejection?: AnchorRejection, cause?: unknown) {
    super(`${field}: ${reason}`);
    this.name = "MbongoAnchorError";
    this.field = field;
    this.reason = rejection;
    this.cause = cause;
  }

  /** True when the task id was already anchored, by anyone. */
  get isDuplicateTaskId(): boolean {
    return this.reason === "duplicate-task-id";
  }
}

/**
 * Why the node refused to commit a compute task, or why a transaction could
 * not be built locally. Derived from the node's `-32603` message, as
 * {@link AnchorRejection} is. `"unknown"` is a rejection, never success.
 */
export type ComputeTaskRejection =
  | "duplicate-task"
  | "task-pending"
  | "unsupported-task-version"
  | "execution-spec-too-large"
  | "sender-submitter-mismatch"
  | "invalid-transaction-signature"
  | "invalid-nonce"
  | "invalid-task-fields"
  | "sender-account-unusable"
  | "unknown";

/**
 * A compute task is not canonically encodable, a `ComputeTask` transaction
 * could not be built, or the node refused it.
 *
 * `reason` is absent for local failures — a wrong field width, an
 * over-bound `executionSpec`, a key that does not derive the submitter or
 * the executor — and present when the failure came back from the node.
 */
export class MbongoComputeTaskError extends Error {
  /** The offending field, or `submit` when the node rejected the attempt. */
  readonly field: string;
  /** Set only when the node rejected the attempt. */
  readonly reason?: ComputeTaskRejection;
  /** The underlying JSON-RPC error, when there was one. */
  readonly cause?: unknown;

  constructor(field: string, reason: string, rejection?: ComputeTaskRejection, cause?: unknown) {
    super(`${field}: ${reason}`);
    this.name = "MbongoComputeTaskError";
    this.field = field;
    this.reason = rejection;
    this.cause = cause;
  }

  /** True when a task with this identity was already committed, by anyone. */
  get isDuplicateTask(): boolean {
    return this.reason === "duplicate-task";
  }
}

/** Which of RFC 0005's receipt-binding rules a receipt fails. */
export type ReceiptBinding = "task-id" | "input-commitment" | "executor";

/**
 * A receipt is not bound to the task it is meant to answer: its `taskId` is
 * not the task's derived identity (rule q), its `inputCommitment` is not the
 * task's (rule r), or its `executor` is not the one the task named (rule s).
 *
 * A **local** structural check over the two objects; it says nothing about
 * whether the task is committed on chain or the `taskId` already anchored,
 * which the node decides.
 */
export class MbongoReceiptBindingError extends Error {
  /** The binding that failed. */
  readonly binding: ReceiptBinding;

  constructor(binding: ReceiptBinding, reason: string) {
    super(`${binding}: ${reason}`);
    this.name = "MbongoReceiptBindingError";
    this.binding = binding;
  }
}

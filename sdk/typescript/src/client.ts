/**
 * Baseline JSON-RPC client for the Mbongo Chain node.
 *
 * Covers exactly the six methods specified by `docs/specs/rpc_v0.2.md`
 * (FROZEN). Nothing else is exposed: reserved compute methods are
 * unavailable on the node and are not wrapped here, and no receipt or
 * signing helpers exist in this package.
 *
 * ## Exact integers
 *
 * Requests and responses go through `json-exact.ts` rather than
 * `JSON.stringify` and `JSON.parse`. The wire is unchanged — integers remain
 * JSON numbers, method names and shapes are identical — but a `u64` no longer
 * loses digits crossing into JavaScript.
 *
 * The parser hands back every integer token as `bigint`. That is a transport
 * detail, not the public shape: {@link normalizeBlock} and its neighbours map
 * each field to the type its domain calls for, so `error.code` and
 * `receipt.version` stay numbers while `amount`, `nonce`, `height` and
 * `timestamp` stay exact.
 */

import { MbongoRpcError, MbongoTransportError } from "./errors.js";
import { MbongoJsonError, parseExact, stringifyExact } from "./json-exact.js";
import { normalizeAmount, normalizeU64 } from "./numeric.js";
import type {
  Block,
  BlockBody,
  BlockHeader,
  Hash,
  JSONRPCRequest,
  Transaction,
  TransactionInput,
  TransactionPayload,
  WireReceipt,
} from "./types.js";

/**
 * The exact JSON-RPC method strings served by the node. These are wire
 * values fixed by the frozen spec; the TypeScript method names below are
 * only ergonomics.
 */
export const RPC_METHODS = {
  ping: "ping",
  getBlockHeight: "get_block_height",
  submitTransaction: "submit_transaction",
  produceBlock: "produce_block",
  getLatestBlockHash: "get_latest_block_hash",
  getBlockByHeight: "get_block_by_height",
} as const;

/**
 * The response fields this client reads.
 *
 * Deliberately not the whole `Response` interface. Naming `Response` would
 * make a consumer's compilation depend on an ambient web-platform declaration
 * — a DOM lib, or `@types/node` — merely to describe a type this package
 * owns. The two members below are exactly the ones {@link MbongoClient}
 * touches.
 */
export interface MbongoFetchResponse {
  readonly status: number;
  text(): Promise<string>;
}

/**
 * The request options this client sends.
 *
 * Exactly the three fields the RPC call sets. `signal`, `credentials` and the
 * rest of `RequestInit` are absent because this client never sets them, and
 * declaring them would reintroduce the ambient dependency these types exist
 * to remove.
 */
export interface MbongoFetchInit {
  method: string;
  headers: Record<string, string>;
  body: string;
}

/**
 * The `fetch` contract {@link MbongoClient} requires.
 *
 * The platform `fetch` satisfies it structurally, so passing
 * `globalThis.fetch` stays valid. It is deliberately narrower than the
 * platform signature: `input` is a `string` because the client only ever
 * sends its own RPC URL, and `init` is required because the client always
 * supplies it — an optional one would force every custom implementation to
 * guard a field that is never missing.
 *
 * Being narrower is a real, accepted trade: code that reads the option back
 * out and requires the full `typeof globalThis.fetch` no longer compiles.
 */
export type MbongoFetch = (
  input: string,
  init: MbongoFetchInit,
) => Promise<MbongoFetchResponse>;

/** Client options. */
export interface MbongoClientOptions {
  /**
   * `fetch` implementation to use. Defaults to the global one. Provided so
   * tests can observe the exact request body without a network.
   */
  fetch?: MbongoFetch;
}

// ── Response normalisation ───────────────────────────────────────────────
//
// The exact parser returns integers as bigint. These functions decide what
// each field becomes in the public API, so parser types never leak.

function fail(path: string, detail: string): never {
  throw new MbongoTransportError(`${path} ${detail}`);
}

function expectObject(path: string, value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(path, `is not an object`);
  }
  return value as Record<string, unknown>;
}

function expectString(path: string, value: unknown): string {
  if (typeof value !== "string") fail(path, `is not a string`);
  return value;
}

/** An exact integer field: the parser produced a bigint, and it stays one. */
function expectBigint(path: string, value: unknown): bigint {
  if (typeof value !== "bigint") {
    fail(path, `is not an integer (got ${typeof value})`);
  }
  return value;
}

/**
 * A bounded integer field that belongs in `number`.
 *
 * Converted only after the bound is checked, so an out-of-range value is
 * reported rather than silently rounded.
 */
function expectBoundedNumber(path: string, value: unknown, max: bigint): number {
  const v = expectBigint(path, value);
  if (v < 0n || v > max) fail(path, `is outside 0..${max}`);
  return Number(v);
}

/**
 * A signed bounded integer field. JSON-RPC error codes are negative
 * (`-32603` and friends), so this is deliberately not the unsigned helper.
 */
function expectSignedNumber(path: string, value: unknown, min: bigint, max: bigint): number {
  const v = expectBigint(path, value);
  if (v < min || v > max) fail(path, `is outside ${min}..${max}`);
  return Number(v);
}

function expectByteArray(path: string, value: unknown): number[] {
  if (!Array.isArray(value)) fail(path, `is not an array`);
  return value.map((b, i) => expectBoundedNumber(`${path}[${i}]`, b, 255n));
}

function normalizeReceipt(path: string, value: unknown): WireReceipt {
  const r = expectObject(path, value);
  return {
    version: expectBoundedNumber(`${path}.version`, r.version, 255n),
    task_id: expectByteArray(`${path}.task_id`, r.task_id),
    input_commitment: expectByteArray(`${path}.input_commitment`, r.input_commitment),
    output_commitment: expectByteArray(`${path}.output_commitment`, r.output_commitment),
    executor: expectString(`${path}.executor`, r.executor),
    metadata: expectByteArray(`${path}.metadata`, r.metadata),
    signature: expectString(`${path}.signature`, r.signature),
  };
}

function normalizePayload(path: string, value: unknown): TransactionPayload {
  if (value === "None") return "None";
  const p = expectObject(path, value);
  if (!("AnchorReceipt" in p)) fail(path, `is not a known payload variant`);
  return { AnchorReceipt: normalizeReceipt(`${path}.AnchorReceipt`, p.AnchorReceipt) };
}

function normalizeTransaction(path: string, value: unknown): Transaction {
  const t = expectObject(path, value);
  return {
    tx_type: expectString(`${path}.tx_type`, t.tx_type) as Transaction["tx_type"],
    sender: expectString(`${path}.sender`, t.sender),
    receiver: expectString(`${path}.receiver`, t.receiver),
    amount: expectBigint(`${path}.amount`, t.amount),
    nonce: expectBigint(`${path}.nonce`, t.nonce),
    payload: normalizePayload(`${path}.payload`, t.payload),
    signature: expectString(`${path}.signature`, t.signature),
  };
}

function normalizeHeader(path: string, value: unknown): BlockHeader {
  const h = expectObject(path, value);
  return {
    parent_hash: expectString(`${path}.parent_hash`, h.parent_hash),
    state_root: expectString(`${path}.state_root`, h.state_root),
    transactions_root: expectString(`${path}.transactions_root`, h.transactions_root),
    timestamp: expectBigint(`${path}.timestamp`, h.timestamp),
    height: expectBigint(`${path}.height`, h.height),
  };
}

function normalizeBlock(path: string, value: unknown): Block {
  const b = expectObject(path, value);
  const body = expectObject(`${path}.body`, b.body);
  if (!Array.isArray(body.transactions)) {
    fail(`${path}.body.transactions`, `is not an array`);
  }
  const transactions = body.transactions.map((tx, i) =>
    normalizeTransaction(`${path}.body.transactions[${i}]`, tx),
  );
  const normalizedBody: BlockBody = { transactions };
  return { header: normalizeHeader(`${path}.header`, b.header), body: normalizedBody };
}

/** Converts a caller's transaction into the exact form that goes on the wire. */
function normalizeTransactionInput(tx: TransactionInput): Transaction {
  return {
    ...tx,
    amount: normalizeAmount("transaction.amount", tx.amount),
    nonce: normalizeU64("transaction.nonce", tx.nonce),
  };
}

export class MbongoClient {
  private requestId = 0;
  private readonly fetchImpl: MbongoFetch;

  constructor(
    private readonly rpcUrl: string,
    options: MbongoClientOptions = {},
  ) {
    // The same property off the same object as before; only the typing
    // differs. Reaching `globalThis.fetch` by name would need the ambient
    // declaration this package no longer depends on, so the global is read
    // through the contract it has to satisfy anyway. When the runtime has no
    // `fetch`, this is `undefined` and the first call throws — exactly as it
    // did before.
    this.fetchImpl =
      options.fetch ?? (globalThis as unknown as { fetch: MbongoFetch }).fetch;
  }

  /** Health check. Resolves to the string `"pong"`. */
  async ping(): Promise<string> {
    return expectString("ping result", await this.call(RPC_METHODS.ping));
  }

  /**
   * Current chain height.
   *
   * `u64` on the wire, returned as `bigint` so the whole domain is exact.
   * Always a `bigint`, including for small heights: a type that changed with
   * the value would need a check at every call site.
   */
  async getBlockHeight(): Promise<bigint> {
    return expectBigint(
      "get_block_height result",
      await this.call(RPC_METHODS.getBlockHeight),
    );
  }

  /**
   * Submits an already-signed transaction and resolves to its hex-encoded
   * hash.
   *
   * **This package does not sign.** The caller supplies a complete,
   * correctly signed `Transaction` object; the SDK serialises it as-is. The
   * historical `[signed_tx_hex]` form is not supported, because the node
   * does not accept it.
   *
   * `amount` and `nonce` accept a `bigint` or a safe non-negative `number`.
   * An unsafe number is refused before any network call: it was rounded
   * before this package saw it, and a rounded amount would be settled as a
   * different value than the caller meant.
   */
  async submitTransaction(transaction: TransactionInput): Promise<Hash> {
    const exact = normalizeTransactionInput(transaction);
    return expectString(
      "submit_transaction result",
      await this.call(RPC_METHODS.submitTransaction, exact),
    );
  }

  /**
   * Asks the node to produce a block, and resolves to its hex-encoded hash.
   *
   * Takes no parameters. The node bounds block size itself; that limit is
   * not part of the RPC contract and is not exposed here.
   */
  async produceBlock(): Promise<Hash> {
    return expectString(
      "produce_block result",
      await this.call(RPC_METHODS.produceBlock),
    );
  }

  /** Hex-encoded hash of the block at the current chain tip. */
  async getLatestBlockHash(): Promise<Hash> {
    return expectString(
      "get_latest_block_hash result",
      await this.call(RPC_METHODS.getLatestBlockHash),
    );
  }

  /**
   * Fetches the block at `height`.
   *
   * Accepts a `bigint` or a safe non-negative `number`, and sends the
   * canonical `{"height": N}` object with the height as an exact integer
   * token. The node also tolerates a bare number, but that is an
   * implementation detail of the current runtime rather than contract, so
   * this client never emits it.
   */
  async getBlockByHeight(height: number | bigint): Promise<Block> {
    const exact = normalizeU64("height", height);
    const block = await this.call(RPC_METHODS.getBlockByHeight, { height: exact });
    return normalizeBlock("block", block);
  }

  /**
   * Issues one JSON-RPC call and returns the exact-parsed `result`.
   *
   * `params` is omitted entirely when the method takes none, matching the
   * frozen spec rather than sending an empty array.
   *
   * @throws {MbongoTransportError} the host was unreachable, the HTTP status
   * was not successful, or the body was not a well-formed JSON-RPC response.
   * @throws {MbongoRpcError} the node returned a JSON-RPC error object.
   */
  private async call(method: string, params?: unknown): Promise<unknown> {
    const id = ++this.requestId;
    const request: JSONRPCRequest = { jsonrpc: "2.0", id, method };
    if (params !== undefined) {
      request.params = params;
    }

    let response: MbongoFetchResponse;
    try {
      response = await this.fetchImpl(this.rpcUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        // Not JSON.stringify: it cannot serialise a bigint at all.
        body: stringifyExact(request),
      });
    } catch (cause) {
      throw new MbongoTransportError(
        `request to ${this.rpcUrl} failed: ${String(cause)}`,
      );
    }

    let body: unknown;
    try {
      // Not response.json(): it would round every integer past 2^53 - 1
      // before this client could look at it, and the digits are gone by then.
      body = parseExact(await response.text());
    } catch (cause) {
      // A JSON-RPC error is still delivered with a non-2xx status, so the
      // status alone is not the failure; an unparsable body is.
      const detail = cause instanceof MbongoJsonError ? `: ${cause.message}` : "";
      throw new MbongoTransportError(
        `response from ${this.rpcUrl} was not valid JSON${detail}`,
        response.status,
      );
    }

    if (
      body === null ||
      typeof body !== "object" ||
      (body as Record<string, unknown>).jsonrpc !== "2.0"
    ) {
      throw new MbongoTransportError(
        "response is not a JSON-RPC 2.0 object",
        response.status,
      );
    }

    const rpc = body as Record<string, unknown>;
    if ("error" in rpc) {
      const err = expectObject("error", rpc.error);
      throw new MbongoRpcError({
        // i32 on the wire and compared against numeric constants by callers,
        // so it stays a number rather than inheriting the parser's bigint.
        code: expectSignedNumber("error.code", err.code, -2147483648n, 2147483647n),
        message: expectString("error.message", err.message),
        ...(err.data === undefined ? {} : { data: err.data }),
      });
    }
    if (!("result" in rpc)) {
      throw new MbongoTransportError(
        "JSON-RPC response has neither result nor error",
        response.status,
      );
    }
    return rpc.result;
  }
}


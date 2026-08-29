/**
 * Baseline JSON-RPC client for the Mbongo Chain node.
 *
 * Covers exactly the six methods specified by `docs/specs/rpc_v0.2.md`
 * (FROZEN). Nothing else is exposed: reserved compute methods are
 * unavailable on the node and are not wrapped here, and no receipt or
 * signing helpers exist in this package.
 */

import { MbongoRpcError, MbongoTransportError } from "./errors.js";
import type {
  Block,
  Hash,
  JSONRPCRequest,
  JSONRPCResponse,
  Transaction,
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

/** Client options. */
export interface MbongoClientOptions {
  /**
   * `fetch` implementation to use. Defaults to the global one. Provided so
   * tests can observe the exact request body without a network.
   */
  fetch?: typeof globalThis.fetch;
}

export class MbongoClient {
  private requestId = 0;
  private readonly fetchImpl: typeof globalThis.fetch;

  constructor(
    private readonly rpcUrl: string,
    options: MbongoClientOptions = {},
  ) {
    this.fetchImpl = options.fetch ?? globalThis.fetch;
  }

  /** Health check. Resolves to the string `"pong"`. */
  async ping(): Promise<string> {
    return this.call<string>(RPC_METHODS.ping);
  }

  /**
   * Current chain height.
   *
   * `u64` on the wire; see the precision note in `types.ts`.
   */
  async getBlockHeight(): Promise<number> {
    return this.call<number>(RPC_METHODS.getBlockHeight);
  }

  /**
   * Submits an already-signed transaction and resolves to its hex-encoded
   * hash.
   *
   * **This package does not sign.** The caller supplies a complete,
   * correctly signed `Transaction` object; the SDK serialises it as-is. The
   * historical `[signed_tx_hex]` form is not supported, because the node
   * does not accept it.
   */
  async submitTransaction(transaction: Transaction): Promise<Hash> {
    return this.call<Hash>(RPC_METHODS.submitTransaction, transaction);
  }

  /**
   * Asks the node to produce a block, and resolves to its hex-encoded hash.
   *
   * Takes no parameters. The node bounds block size itself; that limit is
   * not part of the RPC contract and is not exposed here.
   */
  async produceBlock(): Promise<Hash> {
    return this.call<Hash>(RPC_METHODS.produceBlock);
  }

  /** Hex-encoded hash of the block at the current chain tip. */
  async getLatestBlockHash(): Promise<Hash> {
    return this.call<Hash>(RPC_METHODS.getLatestBlockHash);
  }

  /**
   * Fetches the block at `height`.
   *
   * Sends the canonical `{"height": N}` object. The node also tolerates a
   * bare number, but that is an implementation detail of the current
   * runtime rather than contract, so this client never emits it.
   */
  async getBlockByHeight(height: number): Promise<Block> {
    return this.call<Block>(RPC_METHODS.getBlockByHeight, { height });
  }

  /**
   * Issues one JSON-RPC call.
   *
   * `params` is omitted entirely when the method takes none, matching the
   * frozen spec rather than sending an empty array.
   *
   * @throws {MbongoTransportError} the host was unreachable, the HTTP status
   * was not successful, or the body was not a well-formed JSON-RPC response.
   * @throws {MbongoRpcError} the node returned a JSON-RPC error object.
   */
  private async call<T>(method: string, params?: unknown): Promise<T> {
    const id = ++this.requestId;
    const request: JSONRPCRequest = { jsonrpc: "2.0", id, method };
    if (params !== undefined) {
      request.params = params;
    }

    let response: Response;
    try {
      response = await this.fetchImpl(this.rpcUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
      });
    } catch (cause) {
      throw new MbongoTransportError(
        `request to ${this.rpcUrl} failed: ${String(cause)}`,
      );
    }

    let body: unknown;
    try {
      body = await response.json();
    } catch {
      // A JSON-RPC error is still delivered with a non-2xx status, so the
      // status alone is not the failure; an unparseable body is.
      throw new MbongoTransportError(
        `response from ${this.rpcUrl} was not valid JSON`,
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

    const rpc = body as JSONRPCResponse<T>;
    if ("error" in rpc) {
      throw new MbongoRpcError(rpc.error);
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

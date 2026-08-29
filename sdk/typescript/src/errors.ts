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

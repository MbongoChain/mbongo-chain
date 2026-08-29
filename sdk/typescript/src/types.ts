/**
 * Wire types for the Mbongo Chain JSON-RPC surface.
 *
 * Derived from `docs/specs/rpc_v0.2.md` (FROZEN), cross-checked against the
 * Rust serde representations in `mbongo-core`.
 *
 * Field names are the wire names. They are deliberately left in snake_case
 * rather than converted to camelCase: these interfaces describe the actual
 * JSON objects sent and received, and a renaming layer would make them
 * describe something else.
 */

/** A `0x`-prefixed lowercase hex string. */
export type HexString = string;

/** Ed25519 public key, 32 bytes: `0x` + 64 hex characters. */
export type Address = HexString;

/** BLAKE3 digest, 32 bytes: `0x` + 64 hex characters. */
export type Hash = HexString;

/** Ed25519 signature, 64 bytes: `0x` + 128 hex characters. */
export type Signature = HexString;

/**
 * Transaction type discriminant, serialised as the variant name.
 *
 * `ComputeTask` and `Stake` exist in the enum but carry no validated
 * semantics in protocol v0.3.
 */
export type TransactionType =
  | "Transfer"
  | "ComputeTask"
  | "Stake"
  | "AnchorReceipt";

/**
 * Transaction payload.
 *
 * The unit variant serialises as the bare string `"None"`; the receipt
 * variant serialises as `{ "AnchorReceipt": <receipt> }`.
 *
 * The receipt body is {@link WireReceipt}: the exact JSON shape the node's
 * serde produces, pinned by
 * `test-vectors/transaction/anchor-receipt-v1.json`.
 */

/**
 * A receipt as it crosses the wire, inside an `AnchorReceipt` payload.
 *
 * Three byte representations coexist here, and that is the runtime's actual
 * serde output rather than a choice this package makes. Hex appears exactly
 * where the Rust type has a custom serializer: `Address` has its own
 * `impl Serialize`, and the 64-byte signature uses `serde_arr64`. The three
 * commitment fields and `metadata` are plain `[u8; 32]` and `Vec<u8>` with no
 * annotation, so they serialise as arrays of numbers.
 *
 * The general byte-encoding sentence in `rpc_v0.2.md` does not describe these
 * four fields; reconciling that wording is tracked separately.
 */
export interface WireReceipt {
  version: number;
  /** Array of 32 byte values, not hex. */
  task_id: number[];
  /** Array of 32 byte values, not hex. */
  input_commitment: number[];
  /** Array of 32 byte values, not hex. */
  output_commitment: number[];
  executor: Address;
  /** Array of byte values, not hex. */
  metadata: number[];
  signature: Signature;
}

export type TransactionPayload = "None" | { AnchorReceipt: WireReceipt };

/**
 * A transaction as it crosses the wire.
 *
 * `amount` is a `u128` and `nonce` a `u64` on the Rust side, and
 * `rpc_v0.2.md` §1 specifies both as JSON numbers. JavaScript numbers are
 * exact only up to `Number.MAX_SAFE_INTEGER` (2^53 − 1), so an `amount`
 * above that bound cannot round-trip through this type without losing
 * precision. That is a property of the frozen wire contract, not of this
 * package, and it is not worked around here: silently re-encoding the field
 * would make these types stop describing the actual JSON.
 */
export interface Transaction {
  tx_type: TransactionType;
  sender: Address;
  receiver: Address;
  /** `u128` on the wire. See the precision note on this interface. */
  amount: number;
  /** `u64` on the wire. */
  nonce: number;
  payload: TransactionPayload;
  signature: Signature;
}

/** Block header. `timestamp` and `height` are `u64` on the wire. */
export interface BlockHeader {
  parent_hash: Hash;
  state_root: Hash;
  transactions_root: Hash;
  timestamp: number;
  height: number;
}

/** Block body: the transactions included in the block, in order. */
export interface BlockBody {
  transactions: Transaction[];
}

/**
 * A block as returned by `get_block_by_height`: nested `{header, body}`,
 * not a flattened object.
 */
export interface Block {
  header: BlockHeader;
  body: BlockBody;
}

/** Parameters for `get_block_by_height`. */
export interface GetBlockByHeightParams {
  height: number;
}

/** JSON-RPC 2.0 request envelope. `params` is omitted for methods that take none. */
export interface JSONRPCRequest {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params?: unknown;
}

/** JSON-RPC 2.0 error object. */
export interface JSONRPCErrorObject {
  code: number;
  message: string;
  data?: unknown;
}

/** JSON-RPC 2.0 response, success or error. */
export type JSONRPCResponse<T> =
  | { jsonrpc: "2.0"; id: number | string | null; result: T }
  | { jsonrpc: "2.0"; id: number | string | null; error: JSONRPCErrorObject };

/**
 * TypeScript interfaces for Mbongo Chain JSON-RPC v0.1.
 * Matches docs/specs/jsonrpc_v0.1.md exactly.
 */

/** Spec section 4: ValidatorData */
export interface ValidatorData {
  stake: string;
  is_active: boolean;
  compute_score: number;
}

/** Spec section 4: Account */
export interface Account {
  address: string;
  balance: string;
  nonce: number;
  validator_data: ValidatorData | null;
}

/** Spec section 4: Block */
export interface Block {
  number: number;
  hash: string;
  parentHash: string;
  timestamp: number;
}

/** Spec section 3.5: TransactionStatus response */
export interface TransactionStatus {
  status: "pending" | "confirmed" | "failed";
  blockNumber: number | null;
  executionResult: { success: boolean; error?: string } | null;
}

/** JSON-RPC 2.0 request envelope */
export interface JSONRPCRequest<T> {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params: T;
}

/** JSON-RPC 2.0 success response */
export interface JSONRPCSuccess<T> {
  jsonrpc: "2.0";
  id: number;
  result: T;
}

/** JSON-RPC 2.0 error object */
export interface JSONRPCError {
  code: number;
  message: string;
  data?: string;
}

/** JSON-RPC 2.0 response (success or error) */
export type JSONRPCResponse<T> =
  | JSONRPCSuccess<T>
  | { jsonrpc: "2.0"; id: number; error: JSONRPCError };

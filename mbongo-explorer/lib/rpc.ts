const RPC_URL = process.env.NEXT_PUBLIC_RPC_URL ?? "http://localhost:9944";

interface JsonRpcResponse<T> {
  jsonrpc: "2.0";
  result?: T;
  error?: { code: number; message: string; data?: unknown };
  id: number;
}

let nextId = 1;

export class RpcError extends Error {
  constructor(
    public code: number,
    message: string,
  ) {
    super(message);
    this.name = "RpcError";
  }
}

export async function rpcCall<T>(
  method: string,
  params?: Record<string, unknown>,
): Promise<T> {
  const id = nextId++;
  const body = JSON.stringify({
    jsonrpc: "2.0",
    method,
    params: params ?? null,
    id,
  });

  const res = await fetch(`${RPC_URL}/rpc`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
    cache: "no-store",
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new RpcError(res.status, `HTTP ${res.status}: ${text}`);
  }

  const json: JsonRpcResponse<T> = await res.json();

  if (json.error) {
    throw new RpcError(json.error.code, json.error.message);
  }

  return json.result as T;
}

export async function getBlockHeight(): Promise<number> {
  return rpcCall<number>("get_block_height");
}

export async function getBlockByHeight(height: number): Promise<import("@/types/block").Block> {
  return rpcCall<import("@/types/block").Block>("get_block_by_height", { height });
}

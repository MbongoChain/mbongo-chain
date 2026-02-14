import type {
  Block,
  Account,
  TransactionStatus,
  ValidatorData,
  JSONRPCResponse,
} from "./types.js";

export class MbongoClient {
  private requestId = 0;

  constructor(private rpcUrl: string) {}

  async getBlockNumber(): Promise<number> {
    return this.rpcCall<number>("mbg_getBlockNumber", []);
  }

  async getBlockByNumber(blockNumber: number): Promise<Block> {
    return this.rpcCall<Block>("mbg_getBlockByNumber", [blockNumber]);
  }

  async getAccount(address: string): Promise<Account> {
    return this.rpcCall<Account>("mbg_getAccount", [address]);
  }

  async sendTransaction(signedTx: string): Promise<string> {
    const result = await this.rpcCall<{ txHash: string }>(
      "mbg_sendTransaction",
      [signedTx],
    );
    return result.txHash;
  }

  async getTransaction(txHash: string): Promise<TransactionStatus> {
    return this.rpcCall<TransactionStatus>("mbg_getTransaction", [txHash]);
  }

  async getValidatorSet(): Promise<ValidatorData[]> {
    return this.rpcCall<ValidatorData[]>("mbg_getValidatorSet", []);
  }

  private async rpcCall<T>(method: string, params: unknown[]): Promise<T> {
    const id = ++this.requestId;

    const response = await fetch(this.rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id,
        method,
        params,
      }),
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    let json: unknown;
    try {
      json = await response.json();
    } catch {
      throw new Error("Invalid JSON in response");
    }

    if (
      json === null ||
      typeof json !== "object" ||
      !("jsonrpc" in json) ||
      (json as Record<string, unknown>).jsonrpc !== "2.0"
    ) {
      throw new Error("Invalid JSON-RPC response: missing jsonrpc field");
    }

    const rpcResponse = json as JSONRPCResponse<T>;

    if ("error" in rpcResponse) {
      const { code, message, data } = rpcResponse.error;
      const detail = data != null ? `: ${data}` : "";
      throw new Error(`RPC error ${code}: ${message}${detail}`);
    }

    return rpcResponse.result;
  }
}

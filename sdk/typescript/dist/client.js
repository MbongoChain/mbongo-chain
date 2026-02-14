export class MbongoClient {
    rpcUrl;
    requestId = 0;
    constructor(rpcUrl) {
        this.rpcUrl = rpcUrl;
    }
    async getBlockNumber() {
        return this.rpcCall("mbg_getBlockNumber", []);
    }
    async getBlockByNumber(blockNumber) {
        return this.rpcCall("mbg_getBlockByNumber", [blockNumber]);
    }
    async getAccount(address) {
        return this.rpcCall("mbg_getAccount", [address]);
    }
    async sendTransaction(signedTx) {
        const result = await this.rpcCall("mbg_sendTransaction", [signedTx]);
        return result.txHash;
    }
    async getTransaction(txHash) {
        return this.rpcCall("mbg_getTransaction", [txHash]);
    }
    async getValidatorSet() {
        return this.rpcCall("mbg_getValidatorSet", []);
    }
    async rpcCall(method, params) {
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
        let json;
        try {
            json = await response.json();
        }
        catch {
            throw new Error("Invalid JSON in response");
        }
        if (json === null ||
            typeof json !== "object" ||
            !("jsonrpc" in json) ||
            json.jsonrpc !== "2.0") {
            throw new Error("Invalid JSON-RPC response: missing jsonrpc field");
        }
        const rpcResponse = json;
        if ("error" in rpcResponse) {
            const { code, message, data } = rpcResponse.error;
            const detail = data != null ? `: ${data}` : "";
            throw new Error(`RPC error ${code}: ${message}${detail}`);
        }
        return rpcResponse.result;
    }
}

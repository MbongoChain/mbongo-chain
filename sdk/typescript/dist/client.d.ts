import type { Block, Account, TransactionStatus, ValidatorData } from "./types.js";
export declare class MbongoClient {
    private rpcUrl;
    private requestId;
    constructor(rpcUrl: string);
    getBlockNumber(): Promise<number>;
    getBlockByNumber(blockNumber: number): Promise<Block>;
    getAccount(address: string): Promise<Account>;
    sendTransaction(signedTx: string): Promise<string>;
    getTransaction(txHash: string): Promise<TransactionStatus>;
    getValidatorSet(): Promise<ValidatorData[]>;
    private rpcCall;
}

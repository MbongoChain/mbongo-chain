export interface BlockHeader {
  parent_hash: string;
  state_root: string;
  transactions_root: string;
  timestamp: number;
  height: number;
}

export interface Block {
  header: BlockHeader;
  body: {
    transactions: unknown[];
  };
}

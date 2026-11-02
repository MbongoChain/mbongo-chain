//! The chain as the worker sees it: blocks to observe, a transaction to
//! submit, a nonce to read. Nothing more.
//!
//! The chain is authority for three facts and no others (E §16): a task
//! exists with its `executor` and `input_commitment`; a receipt for a
//! `task_id` is anchored; an account's nonce. Discovery is block
//! observation through the existing `get_block_by_height` (E §4); there is
//! no task lookup and none is added.

use std::future::Future;

use mbongo_core::{Address, Block, ComputeTask, Hash, Receipt, Transaction, TransactionPayload};
use parity_scale_codec::Encode;

/// Why a chain call failed.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ChainError {
    /// No usable answer: the request may or may not have been processed.
    #[error("transport: {0}")]
    Transport(String),
    /// The node answered and refused.
    #[error("rejected: {0}")]
    Rejected(String),
}

/// A chain client. Mirrors the six-method RPC contract of `rpc_v0.3` plus
/// the REST account read the node already serves.
pub trait ChainClient: Send + Sync {
    /// Current tip height.
    fn latest_height(&self) -> impl Future<Output = Result<u64, ChainError>> + Send;
    /// The block at `height`, if it exists.
    fn block_by_height(
        &self,
        height: u64,
    ) -> impl Future<Output = Result<Option<Block>, ChainError>> + Send;
    /// Submits a signed transaction; returns the hash the node reports.
    fn submit_transaction(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Hash, ChainError>> + Send;
    /// The account's current nonce; `0` when the account does not exist.
    fn account_nonce(
        &self,
        address: &Address,
    ) -> impl Future<Output = Result<u64, ChainError>> + Send;
}

/// The transaction hash rule: BLAKE3 over the full SCALE encoding.
pub fn transaction_hash(tx: &Transaction) -> Hash {
    Hash(mbongo_core::crypto::blake3_hash(&tx.encode()))
}

/// A task seen in a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTask {
    /// The block height that carries it.
    pub height: u64,
    /// The committed envelope.
    pub task: ComputeTask,
    /// Its derived identity.
    pub task_id: [u8; 32],
}

/// Every task committed in blocks `from..=to`, in chain order.
pub async fn scan_tasks<C: ChainClient>(
    chain: &C,
    from: u64,
    to: u64,
) -> Result<Vec<ObservedTask>, ChainError> {
    let mut out = Vec::new();
    for height in from..=to {
        let Some(block) = chain.block_by_height(height).await? else {
            continue;
        };
        for tx in &block.body.transactions {
            if let TransactionPayload::ComputeTask(task) = &tx.payload {
                out.push(ObservedTask {
                    height,
                    task_id: task.task_id(),
                    task: (**task).clone(),
                });
            }
        }
    }
    Ok(out)
}

/// The receipt anchored for `task_id` in blocks `from..=to`, if any.
pub async fn find_receipt<C: ChainClient>(
    chain: &C,
    task_id: [u8; 32],
    from: u64,
    to: u64,
) -> Result<Option<(u64, Receipt)>, ChainError> {
    for height in from..=to {
        let Some(block) = chain.block_by_height(height).await? else {
            continue;
        };
        for tx in &block.body.transactions {
            if let TransactionPayload::AnchorReceipt(receipt) = &tx.payload {
                if receipt.task_id == task_id {
                    return Ok(Some((height, (**receipt).clone())));
                }
            }
        }
    }
    Ok(None)
}

/// Every receipt anchored in blocks `from..=to`, with its height.
pub async fn scan_receipts<C: ChainClient>(
    chain: &C,
    from: u64,
    to: u64,
) -> Result<Vec<(u64, Receipt)>, ChainError> {
    let mut out = Vec::new();
    for height in from..=to {
        let Some(block) = chain.block_by_height(height).await? else {
            continue;
        };
        for tx in &block.body.transactions {
            if let TransactionPayload::AnchorReceipt(receipt) = &tx.payload {
                out.push((height, (**receipt).clone()));
            }
        }
    }
    Ok(out)
}

/// A chain client over a live node's JSON-RPC and REST endpoints.
#[derive(Debug, Clone)]
pub struct RpcChainClient {
    rpc_url: String,
    rest_url: String,
    http: reqwest::Client,
}

impl RpcChainClient {
    /// `rpc_url` is the `/rpc` endpoint; `rest_url` the REST base.
    pub fn new(rpc_url: impl Into<String>, rest_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            rest_url: rest_url.into(),
            http: reqwest::Client::new(),
        }
    }

    async fn call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ChainError> {
        let mut body = serde_json::json!({"jsonrpc": "2.0", "method": method, "id": 1});
        if let Some(p) = params {
            body["params"] = p;
        }
        let resp = self
            .http
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ChainError::Transport(e.to_string()))?;
        let v: serde_json::Value =
            resp.json().await.map_err(|e| ChainError::Transport(e.to_string()))?;
        if let Some(err) = v.get("error") {
            let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
            return Err(ChainError::Rejected(msg));
        }
        v.get("result")
            .cloned()
            .ok_or_else(|| ChainError::Transport("missing result".to_string()))
    }
}

impl ChainClient for RpcChainClient {
    async fn latest_height(&self) -> Result<u64, ChainError> {
        let v = self.call("get_block_height", None).await?;
        v.as_u64().ok_or_else(|| ChainError::Transport("height is not u64".to_string()))
    }

    async fn block_by_height(&self, height: u64) -> Result<Option<Block>, ChainError> {
        match self
            .call(
                "get_block_by_height",
                Some(serde_json::json!({"height": height})),
            )
            .await
        {
            Ok(v) => serde_json::from_value(v)
                .map(Some)
                .map_err(|e| ChainError::Transport(format!("block does not deserialise: {e}"))),
            Err(ChainError::Rejected(m)) if m.contains("not found") => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn submit_transaction(&self, tx: &Transaction) -> Result<Hash, ChainError> {
        let params = serde_json::to_value(tx).map_err(|e| ChainError::Transport(e.to_string()))?;
        let v = self.call("submit_transaction", Some(params)).await?;
        let s = v
            .as_str()
            .ok_or_else(|| ChainError::Transport("hash is not a string".to_string()))?;
        s.parse::<Hash>().map_err(ChainError::Transport)
    }

    async fn account_nonce(&self, address: &Address) -> Result<u64, ChainError> {
        let url = format!("{}/accounts/{address}", self.rest_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ChainError::Transport(e.to_string()))?;
        if resp.status().as_u16() == 404 {
            return Ok(0);
        }
        let v: serde_json::Value =
            resp.json().await.map_err(|e| ChainError::Transport(e.to_string()))?;
        v.get("nonce")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ChainError::Transport("nonce missing".to_string()))
    }
}

/// Test doubles. None of this is consensus: the fake chain records what it
/// is given and serves it back; it validates nothing. Rules (k)–(s) are
/// proven against the real node in `mbongo-node` and by the live harness.
pub mod testing {
    use std::sync::{Arc, Mutex};

    use mbongo_core::{
        compute_transactions_root, Address, Block, BlockBody, BlockHeader, Hash, Transaction,
    };

    use super::{transaction_hash, ChainClient, ChainError};

    #[derive(Default)]
    struct State {
        blocks: Vec<Block>,
        pending: Vec<Transaction>,
        nonces: std::collections::HashMap<Address, u64>,
        /// Accept the next submission but answer with a transport error.
        ambiguous_next: bool,
        /// Refuse the next submission with this message.
        reject_next: Option<String>,
        submissions: u32,
    }

    /// An in-memory chain that includes whatever it is given.
    #[derive(Clone, Default)]
    pub struct FakeChain {
        state: Arc<Mutex<State>>,
    }

    impl FakeChain {
        /// A chain with a genesis block.
        pub fn new() -> Self {
            let chain = Self::default();
            chain.produce_block();
            chain
        }

        /// Includes every pending transaction in a new block.
        pub fn produce_block(&self) -> u64 {
            let mut s = self.state.lock().unwrap();
            let txs: Vec<Transaction> = s.pending.drain(..).collect();
            for tx in &txs {
                let n = s.nonces.entry(tx.sender).or_insert(0);
                *n = (*n).max(tx.nonce + 1);
            }
            let height = s.blocks.len() as u64;
            s.blocks.push(Block {
                header: BlockHeader {
                    parent_hash: Hash::zero(),
                    state_root: Hash::zero(),
                    transactions_root: compute_transactions_root(&txs),
                    timestamp: 0,
                    height,
                },
                body: BlockBody { transactions: txs },
            });
            height
        }

        /// The next submission is accepted but its response is lost.
        pub fn lose_next_response(&self) {
            self.state.lock().unwrap().ambiguous_next = true;
        }

        /// The next submission is refused with `message`.
        pub fn reject_next(&self, message: &str) {
            self.state.lock().unwrap().reject_next = Some(message.to_string());
        }

        /// How many submissions reached the chain.
        pub fn submissions(&self) -> u32 {
            self.state.lock().unwrap().submissions
        }

        /// Transactions awaiting a block.
        pub fn pending(&self) -> Vec<Transaction> {
            self.state.lock().unwrap().pending.clone()
        }
    }

    impl ChainClient for FakeChain {
        async fn latest_height(&self) -> Result<u64, ChainError> {
            Ok(self.state.lock().unwrap().blocks.len() as u64 - 1)
        }

        async fn block_by_height(&self, height: u64) -> Result<Option<Block>, ChainError> {
            let s = self.state.lock().unwrap();
            Ok(usize::try_from(height).ok().and_then(|i| s.blocks.get(i).cloned()))
        }

        async fn submit_transaction(&self, tx: &Transaction) -> Result<Hash, ChainError> {
            let mut s = self.state.lock().unwrap();
            if let Some(msg) = s.reject_next.take() {
                return Err(ChainError::Rejected(msg));
            }
            let hash = transaction_hash(tx);
            // Idempotent while pending, as the node is.
            if !s.pending.iter().any(|p| transaction_hash(p) == hash) {
                s.pending.push(tx.clone());
            }
            s.submissions += 1;
            if s.ambiguous_next {
                s.ambiguous_next = false;
                return Err(ChainError::Transport("response lost".to_string()));
            }
            Ok(hash)
        }

        async fn account_nonce(&self, address: &Address) -> Result<u64, ChainError> {
            Ok(self.state.lock().unwrap().nonces.get(address).copied().unwrap_or(0))
        }
    }
}

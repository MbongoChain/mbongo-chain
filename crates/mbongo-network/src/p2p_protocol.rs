//! Block sync protocol messages for P2P communication.
//!
//! All messages are SCALE-encoded (`parity-scale-codec`). The protocol
//! uses libp2p request/response: a peer sends a [`SyncRequest`] and
//! receives a [`SyncResponse`].

use async_trait::async_trait;
use futures::prelude::*;
use libp2p::request_response;
use mbongo_core::{Block, Hash};
use parity_scale_codec::{Decode, Encode};

/// Maximum number of blocks that can be requested in a single `GetBlocks` range.
pub const MAX_RANGE: u64 = 256;

/// Protocol name used for libp2p request/response negotiation.
pub const SYNC_PROTOCOL: &str = "/mbongo-sync/1";

/// Protocol name for block announcement push notifications.
pub const BLOCK_NOTIFY_PROTOCOL: &str = "/mbongo/block_notify/0.1.0";

// ── Request ────────────────────────────────────────────────────────────

/// Inbound sync request from a peer.
#[derive(Debug, Clone, Encode, Decode)]
#[allow(clippy::cast_possible_truncation)]
pub enum SyncRequest {
    /// Ask the remote peer for its current block height.
    GetHeight,
    /// Request blocks in a half-open range `[start_height, end_height)`.
    ///
    /// The responder should clamp `end_height` to its own latest height + 1
    /// and enforce `end_height - start_height <= MAX_RANGE`.
    GetBlocks {
        /// First height to include (inclusive).
        start_height: u64,
        /// One past the last height to include (exclusive).
        end_height: u64,
    },
}

// ── Response ───────────────────────────────────────────────────────────

/// Outbound sync response to a peer.
#[derive(Debug, Clone, Encode, Decode)]
#[allow(clippy::cast_possible_truncation)]
pub enum SyncResponse {
    /// Current height of the responding peer.
    Height(u64),
    /// Ordered list of `(block_hash, block)` tuples covering the requested range.
    ///
    /// May be shorter than requested when the responder's chain is shorter.
    Blocks(Vec<(Hash, Block)>),
    /// The request was malformed or could not be served.
    Error(String),
}

// ── Block Notification ─────────────────────────────────────────────────

/// Push notification sent by a block producer to all connected peers.
#[derive(Debug, Clone, Encode, Decode)]
#[allow(clippy::cast_possible_truncation)]
pub enum SyncNotification {
    /// A newly produced block.
    NewBlock {
        /// The block that was just produced.
        block: Block,
    },
}

/// Empty acknowledgement for block notifications.
#[derive(Debug, Clone, Encode, Decode)]
pub struct BlockNotifyAck;

// ── Codec ──────────────────────────────────────────────────────────────

/// Length-delimited SCALE codec for libp2p request/response.
///
/// Messages are framed as `[u32 LE length][SCALE payload]`.
/// Maximum frame size is 16 MiB (generous upper bound for 256 full blocks).
#[derive(Debug, Clone, Default)]
pub struct SyncCodec;

/// Maximum frame size: 16 MiB.
const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

#[async_trait]
impl request_response::Codec for SyncCodec {
    type Protocol = &'static str;
    type Request = SyncRequest;
    type Response = SyncResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let buf = read_length_delimited(io).await?;
        SyncRequest::decode(&mut &buf[..]).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("SCALE decode error: {e}"),
            )
        })
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let buf = read_length_delimited(io).await?;
        SyncResponse::decode(&mut &buf[..]).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("SCALE decode error: {e}"),
            )
        })
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let encoded = req.encode();
        write_length_delimited(io, &encoded).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        resp: Self::Response,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let encoded = resp.encode();
        write_length_delimited(io, &encoded).await
    }
}

/// Length-delimited SCALE codec for block notification push messages.
///
/// Uses the same framing as [`SyncCodec`]: `[u32 LE length][SCALE payload]`.
#[derive(Debug, Clone, Default)]
pub struct BlockNotifyCodec;

#[async_trait]
impl request_response::Codec for BlockNotifyCodec {
    type Protocol = &'static str;
    type Request = SyncNotification;
    type Response = BlockNotifyAck;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let buf = read_length_delimited(io).await?;
        SyncNotification::decode(&mut &buf[..]).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("SCALE decode error: {e}"),
            )
        })
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let buf = read_length_delimited(io).await?;
        BlockNotifyAck::decode(&mut &buf[..]).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("SCALE decode error: {e}"),
            )
        })
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let encoded = req.encode();
        write_length_delimited(io, &encoded).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        resp: Self::Response,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let encoded = resp.encode();
        write_length_delimited(io, &encoded).await
    }
}

/// Read a length-delimited frame from an async reader.
async fn read_length_delimited<T: AsyncRead + Unpin>(io: &mut T) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame too large: {len} > {MAX_FRAME_SIZE}"),
        ));
    }
    let mut buf = vec![0u8; len];
    io.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Write a length-delimited frame to an async writer.
async fn write_length_delimited<T: AsyncWrite + Unpin>(
    io: &mut T,
    data: &[u8],
) -> std::io::Result<()> {
    #[allow(clippy::cast_possible_truncation)]
    let len = data.len() as u32;
    io.write_all(&len.to_le_bytes()).await?;
    io.write_all(data).await?;
    io.close().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbongo_core::{Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionType};
    use parity_scale_codec::{Decode, Encode};

    #[test]
    fn sync_request_get_height_roundtrip() {
        let req = SyncRequest::GetHeight;
        let encoded = req.encode();
        let decoded = SyncRequest::decode(&mut &encoded[..]).unwrap();
        assert!(matches!(decoded, SyncRequest::GetHeight));
    }

    #[test]
    fn sync_request_get_blocks_roundtrip() {
        let req = SyncRequest::GetBlocks {
            start_height: 10,
            end_height: 20,
        };
        let encoded = req.encode();
        let decoded = SyncRequest::decode(&mut &encoded[..]).unwrap();
        match decoded {
            SyncRequest::GetBlocks {
                start_height,
                end_height,
            } => {
                assert_eq!(start_height, 10);
                assert_eq!(end_height, 20);
            }
            _ => panic!("expected GetBlocks"),
        }
    }

    #[test]
    fn sync_response_height_roundtrip() {
        let resp = SyncResponse::Height(42);
        let encoded = resp.encode();
        let decoded = SyncResponse::decode(&mut &encoded[..]).unwrap();
        assert!(matches!(decoded, SyncResponse::Height(42)));
    }

    #[test]
    fn sync_response_blocks_roundtrip() {
        let block = Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: Hash::zero(),
                transactions_root: Hash::zero(),
                timestamp: 1_700_000_000,
                height: 5,
            },
            body: BlockBody {
                transactions: vec![Transaction {
                    tx_type: TransactionType::Transfer,
                    sender: Address([1u8; 32]),
                    receiver: Address([2u8; 32]),
                    amount: 100,
                    nonce: 0,
                    signature: [0u8; 64],
                }],
            },
        };
        let hash = Hash([99u8; 32]);
        let resp = SyncResponse::Blocks(vec![(hash, block.clone())]);
        let encoded = resp.encode();
        let decoded = SyncResponse::decode(&mut &encoded[..]).unwrap();
        match decoded {
            SyncResponse::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks[0].0, hash);
                assert_eq!(blocks[0].1.header.height, 5);
                assert_eq!(blocks[0].1.body.transactions.len(), 1);
                assert_eq!(blocks[0].1.body.transactions[0].amount, 100);
            }
            _ => panic!("expected Blocks"),
        }
    }

    #[test]
    fn sync_response_error_roundtrip() {
        let resp = SyncResponse::Error("something went wrong".to_string());
        let encoded = resp.encode();
        let decoded = SyncResponse::decode(&mut &encoded[..]).unwrap();
        match decoded {
            SyncResponse::Error(msg) => assert_eq!(msg, "something went wrong"),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn max_range_is_256() {
        assert_eq!(MAX_RANGE, 256);
    }

    #[test]
    fn sync_notification_new_block_roundtrip() {
        let block = Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: Hash::zero(),
                transactions_root: Hash::zero(),
                timestamp: 1_700_000_000,
                height: 3,
            },
            body: BlockBody {
                transactions: vec![],
            },
        };
        let notif = SyncNotification::NewBlock {
            block: block.clone(),
        };
        let encoded = notif.encode();
        let decoded = SyncNotification::decode(&mut &encoded[..]).unwrap();
        match decoded {
            SyncNotification::NewBlock {
                block: decoded_block,
            } => {
                assert_eq!(decoded_block.header.height, 3);
                assert!(decoded_block.body.transactions.is_empty());
            }
        }
    }

    #[test]
    fn block_notify_ack_roundtrip() {
        let ack = BlockNotifyAck;
        let encoded = ack.encode();
        let _decoded = BlockNotifyAck::decode(&mut &encoded[..]).unwrap();
    }
}

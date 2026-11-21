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
use std::sync::Arc;

use crate::frame_budget::{Category, FrameBudget, Reservation, SMALL_FRAME};

/// Maximum number of blocks that can be requested in a single `GetBlocks` range.
pub const MAX_RANGE: u64 = 256;

/// Protocol name used for libp2p request/response negotiation.
///
/// v0.3 (RFC 0002 Phase 4): bumped from `/mbongo-sync/1` so v0.2 and
/// v0.3 nodes fail cleanly at negotiation instead of exchanging
/// undecodable payloads. There is deliberately no v0.2 fallback.
pub const SYNC_PROTOCOL: &str = "/mbongo-sync/2";

/// Protocol name for block announcement push notifications.
///
/// v0.3 (RFC 0002 Phase 4): bumped from `/mbongo/block_notify/0.1.0`;
/// same rationale as [`SYNC_PROTOCOL`], no v0.2 fallback.
pub const BLOCK_NOTIFY_PROTOCOL: &str = "/mbongo/block_notify/0.2.0";

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

/// A block announcement as [`BlockNotifyCodec`] carries it (S3 A2.2d-1).
///
/// Read from a peer, it is [`Announcement::Decoded`]. Broadcast by this node,
/// it is [`Announcement::Encoded`]: the SCALE bytes of the
/// [`SyncNotification`], encoded once and shared by every peer's request
/// through an [`Arc`], instead of one clone of the block per peer and one
/// encoding per write. libp2p moves a request from `send_request` to
/// `write_request` without cloning it. Both forms put the same bytes on the
/// wire.
#[derive(Debug, Clone)]
pub enum Announcement {
    /// Decoded, as read from a peer.
    Decoded(SyncNotification),
    /// The SCALE encoding of a [`SyncNotification`], shared.
    Encoded(Arc<Vec<u8>>),
}

impl Announcement {
    /// Encodes `block` once as a [`SyncNotification::NewBlock`], without
    /// cloning it: the variant's index byte, then the block.
    #[must_use]
    pub fn new_block(block: &Block) -> Self {
        // `SyncNotification::NewBlock` is variant 0 of a derived SCALE enum;
        // a test checks these bytes against the derived encoding.
        // `encoded_size` is exact and allocates nothing; `size_hint` may
        // overestimate, which would over-allocate the shared buffer.
        let mut bytes = Vec::with_capacity(1 + block.encoded_size());
        bytes.push(0);
        block.encode_to(&mut bytes);
        Self::Encoded(Arc::new(bytes))
    }
}

impl From<SyncNotification> for Announcement {
    fn from(notification: SyncNotification) -> Self {
        Self::Decoded(notification)
    }
}

// ── Codec ──────────────────────────────────────────────────────────────

/// Length-delimited SCALE codec for libp2p request/response.
///
/// Messages are framed as `[u32 LE length][SCALE payload]`.
/// Maximum frame size is 16 MiB (generous upper bound for 256 full blocks).
#[derive(Debug, Clone, Default)]
pub struct SyncCodec {
    budget: Arc<FrameBudget>,
}

impl SyncCodec {
    /// A codec that reserves inbound frame bytes from `budget`, shared with
    /// every clone libp2p makes of it (S3 A2.2b-lite).
    #[must_use]
    pub fn with_budget(budget: Arc<FrameBudget>) -> Self {
        Self { budget }
    }
}

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
        read_length_delimited(io, &self.budget, Category::Unsolicited)
            .await?
            .decode::<SyncRequest>()
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_delimited(io, &self.budget, Category::Solicited)
            .await?
            .decode::<SyncResponse>()
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
        let encoded = encode_exact(&resp);
        // The decoded answer is no longer needed: free it before the write,
        // which may take up to the request timeout (S3 A2.2d-1).
        drop(resp);
        write_length_delimited(io, &encoded).await
    }
}

/// Length-delimited SCALE codec for block notification push messages.
///
/// Uses the same framing as [`SyncCodec`]: `[u32 LE length][SCALE payload]`.
#[derive(Debug, Clone, Default)]
pub struct BlockNotifyCodec {
    budget: Arc<FrameBudget>,
}

impl BlockNotifyCodec {
    /// A codec that reserves inbound frame bytes from `budget`, shared with
    /// every clone libp2p makes of it (S3 A2.2b-lite).
    #[must_use]
    pub fn with_budget(budget: Arc<FrameBudget>) -> Self {
        Self { budget }
    }
}

#[async_trait]
impl request_response::Codec for BlockNotifyCodec {
    type Protocol = &'static str;
    type Request = Announcement;
    type Response = BlockNotifyAck;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_delimited(io, &self.budget, Category::Unsolicited)
            .await?
            .decode::<SyncNotification>()
            .map(Announcement::Decoded)
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_delimited(io, &self.budget, Category::Solicited)
            .await?
            .decode::<BlockNotifyAck>()
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
        match req {
            // Shared bytes, encoded once for every peer: written as they are.
            Announcement::Encoded(bytes) => write_length_delimited(io, &bytes).await,
            Announcement::Decoded(notification) => {
                let encoded = encode_exact(&notification);
                drop(notification);
                write_length_delimited(io, &encoded).await
            }
        }
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

/// The size of each piece a frame body is read into.
const READ_CHUNK: usize = 64 * 1024;

/// A frame body as it arrived: pieces of [`READ_CHUNK`] bytes, the last one
/// possibly shorter. It is decoded where it lies, never copied into one buffer.
struct Frame {
    chunks: Vec<Vec<u8>>,
    len: usize,
    /// The bytes this frame holds, given back when the frame is dropped.
    _reservation: Reservation,
}

impl Frame {
    /// Decodes the body as SCALE, exactly as decoding it from one slice would:
    /// the same value, the same error, and trailing bytes ignored.
    fn decode<D: Decode>(&self) -> std::io::Result<D> {
        let mut input = FrameInput {
            chunks: &self.chunks,
            index: 0,
            offset: 0,
            remaining: self.len,
        };
        D::decode(&mut input).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("SCALE decode error: {e}"),
            )
        })
    }
}

/// A SCALE input over a [`Frame`]'s pieces, in order.
struct FrameInput<'a> {
    chunks: &'a [Vec<u8>],
    index: usize,
    offset: usize,
    remaining: usize,
}

impl parity_scale_codec::Input for FrameInput<'_> {
    fn remaining_len(&mut self) -> Result<Option<usize>, parity_scale_codec::Error> {
        Ok(Some(self.remaining))
    }

    fn read(&mut self, into: &mut [u8]) -> Result<(), parity_scale_codec::Error> {
        // The same refusal, in the same words, as the input for `&[u8]`.
        if into.len() > self.remaining {
            return Err("Not enough data to fill buffer".into());
        }
        let mut written = 0;
        while written < into.len() {
            let chunk = &self.chunks[self.index];
            let n = (chunk.len() - self.offset).min(into.len() - written);
            into[written..written + n].copy_from_slice(&chunk[self.offset..self.offset + n]);
            written += n;
            self.offset += n;
            if self.offset == chunk.len() {
                self.index += 1;
                self.offset = 0;
            }
        }
        self.remaining -= into.len();
        Ok(())
    }
}

/// Read a length-delimited frame from an async reader.
///
/// The frame format is unchanged: a `u32` LE length, at most
/// [`MAX_FRAME_SIZE`], then that many bytes. What changed (S3 A1) is memory:
/// the body is read piece by piece, so a frame holds what has arrived plus one
/// piece of [`READ_CHUNK`] bytes, never the length a peer merely claims, and
/// nothing is reallocated as it grows.
///
/// S3 A2.2b-lite: each piece is reserved from `budget` before it is allocated,
/// in `category`, or in the small pool for a frame of at most [`SMALL_FRAME`]
/// bytes. When the budget refuses, the read fails with
/// [`std::io::ErrorKind::OutOfMemory`]: a resource refusal, distinct from the
/// [`std::io::ErrorKind::InvalidData`] given to an invalid frame. The
/// reservation is given back when the returned frame is dropped, or on any
/// error or cancellation.
async fn read_length_delimited<T: AsyncRead + Unpin>(
    io: &mut T,
    budget: &Arc<FrameBudget>,
    category: Category,
) -> std::io::Result<Frame> {
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame too large: {len} > {MAX_FRAME_SIZE}"),
        ));
    }
    let category = if len <= SMALL_FRAME {
        Category::Small
    } else {
        category
    };
    let mut reservation = budget.reservation(category);
    let mut chunks = Vec::with_capacity(len.div_ceil(READ_CHUNK));
    let mut filled = 0;
    while filled < len {
        let size = (len - filled).min(READ_CHUNK);
        // Reserved before it is allocated, and only once its turn comes.
        reservation.grow(size)?;
        let mut chunk = vec![0u8; size];
        let mut at = 0;
        while at < chunk.len() {
            let n = io.read(&mut chunk[at..]).await?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            at += n;
        }
        filled += chunk.len();
        chunks.push(chunk);
    }
    Ok(Frame {
        chunks,
        len,
        _reservation: reservation,
    })
}

/// SCALE-encodes `value` into a buffer of exactly its size. `encode()` sizes
/// its buffer from `size_hint`, which can overestimate; this buffer is held
/// for a whole write (S3 A2.2d-1). The bytes are the same.
fn encode_exact<E: Encode>(value: &E) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(value.encoded_size());
    value.encode_to(&mut bytes);
    bytes
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
    use mbongo_core::{
        Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionPayload,
        TransactionType,
    };
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
            SyncRequest::GetHeight => panic!("expected GetBlocks"),
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
                    payload: TransactionPayload::None,
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
            SyncResponse::Height(_) | SyncResponse::Error(_) => panic!("expected Blocks"),
        }
    }

    #[test]
    fn sync_response_error_roundtrip() {
        let resp = SyncResponse::Error("something went wrong".to_string());
        let encoded = resp.encode();
        let decoded = SyncResponse::decode(&mut &encoded[..]).unwrap();
        match decoded {
            SyncResponse::Error(msg) => assert_eq!(msg, "something went wrong"),
            SyncResponse::Height(_) | SyncResponse::Blocks(_) => panic!("expected Error"),
        }
    }

    #[test]
    fn max_range_is_256() {
        assert_eq!(MAX_RANGE, 256);
    }

    #[test]
    fn sync_protocol_string_pinned() {
        // Locked surface (PROTOCOL_LOCK_v0.3): changing this string is a
        // protocol version bump and requires an RFC.
        assert_eq!(SYNC_PROTOCOL, "/mbongo-sync/2");
    }

    #[test]
    fn block_notify_protocol_string_pinned() {
        // Locked surface (PROTOCOL_LOCK_v0.3): changing this string is a
        // protocol version bump and requires an RFC.
        assert_eq!(BLOCK_NOTIFY_PROTOCOL, "/mbongo/block_notify/0.2.0");
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

    // ── Framing: S3 A1 changed how a frame is read, not what a frame is ──

    /// The reader on `dev` before S3 A1, kept verbatim as the reference the
    /// new reader must agree with.
    async fn reference_read<T: AsyncRead + Unpin>(io: &mut T) -> std::io::Result<Vec<u8>> {
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

    /// A reader that hands out at most `step` bytes per read.
    struct Trickle<'a> {
        data: &'a [u8],
        step: usize,
    }

    impl AsyncRead for Trickle<'_> {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            out: &mut [u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            let n = self.step.min(out.len()).min(self.data.len());
            out[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            std::task::Poll::Ready(Ok(n))
        }
    }

    /// A budget too large to refuse anything, for tests about framing alone.
    fn roomy() -> Arc<FrameBudget> {
        Arc::new(FrameBudget::new(
            usize::MAX / 4,
            usize::MAX / 4,
            usize::MAX / 4,
        ))
    }

    async fn read_roomy<T: AsyncRead + Unpin>(io: &mut T) -> std::io::Result<Frame> {
        read_length_delimited(io, &roomy(), Category::Unsolicited).await
    }

    fn frame(len: u32, body_len: usize) -> Vec<u8> {
        let mut bytes = len.to_le_bytes().to_vec();
        bytes.extend((0..body_len).map(|i| u8::try_from(i % 251).unwrap()));
        bytes
    }

    fn outcome(r: std::io::Result<Vec<u8>>) -> Result<Vec<u8>, std::io::ErrorKind> {
        r.map_err(|e| e.kind())
    }

    /// Every input, whole or cut short, valid or oversized, read in steps from
    /// one byte to a whole frame, gives the same bytes or the same error kind
    /// as the reference reader.
    #[test]
    fn the_reader_agrees_with_the_reference_on_every_case() {
        let max = u32::try_from(MAX_FRAME_SIZE).unwrap();
        let cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![1, 0],
            frame(0, 0),
            frame(1, 1),
            frame(17, 17),
            frame(17, 16),
            frame(17, 40),
            frame(70_000, 70_000),
            frame(70_000, 69_999),
            frame(200_000, 200_000),
            frame(max, 3),
            frame(max + 1, 3),
            frame(u32::MAX, 0),
        ];
        let mut compared = 0;
        for case in &cases {
            for step in [1, 3, 4096, READ_CHUNK + 1, usize::MAX] {
                let new =
                    futures::executor::block_on(read_roomy(&mut Trickle { data: case, step }));
                let old =
                    futures::executor::block_on(reference_read(&mut Trickle { data: case, step }));
                assert_eq!(
                    outcome(new.map(|f| f.chunks.concat())),
                    outcome(old),
                    "len {} step {step}",
                    case.len()
                );
                compared += 1;
            }
        }
        assert_eq!(compared, 13 * 5);
    }

    /// The writer is unchanged: a frame is the `u32` LE length then the SCALE
    /// bytes, and a written frame reads back to the same message.
    #[test]
    fn a_written_frame_is_length_then_scale_and_reads_back() {
        let request = SyncRequest::GetBlocks {
            start_height: 7,
            end_height: 9,
        };
        let mut wire = Vec::new();
        futures::executor::block_on(write_length_delimited(&mut wire, &request.encode())).unwrap();
        let mut expected = vec![17, 0, 0, 0, 1];
        expected.extend(7u64.to_le_bytes());
        expected.extend(9u64.to_le_bytes());
        assert_eq!(wire, expected);

        let body = futures::executor::block_on(read_roomy(&mut Trickle {
            data: &wire,
            step: 1,
        }))
        .unwrap();
        assert_eq!(body.chunks.concat(), request.encode());
        assert!(matches!(
            body.decode::<SyncRequest>().unwrap(),
            SyncRequest::GetBlocks {
                start_height: 7,
                end_height: 9
            }
        ));
    }

    /// A frame cut short is refused with the error kind the old reader gave.
    #[test]
    fn a_frame_cut_short_is_unexpected_eof() {
        let cut = frame(1_000_000, 999_999);
        let Err(err) = futures::executor::block_on(read_roomy(&mut Trickle {
            data: &cut,
            step: usize::MAX,
        })) else {
            panic!("a frame cut short was accepted");
        };
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    fn block_with_transfers(height: u64, count: usize) -> Block {
        Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: Hash::zero(),
                transactions_root: Hash::zero(),
                timestamp: 1_700_000_000,
                height,
            },
            body: BlockBody {
                transactions: (0..count)
                    .map(|i| Transaction {
                        tx_type: TransactionType::Transfer,
                        sender: Address([1u8; 32]),
                        receiver: Address([2u8; 32]),
                        amount: 100,
                        nonce: i as u64,
                        payload: TransactionPayload::None,
                        signature: [0u8; 64],
                    })
                    .collect(),
            },
        }
    }

    /// Decoding a body held in pieces gives what decoding one slice gives: the
    /// same value, or the same error message, for whole and cut-short bodies,
    /// including messages that span many pieces.
    #[test]
    fn decoding_a_frame_in_pieces_equals_decoding_one_slice() {
        let payloads = [
            SyncResponse::Height(9).encode(),
            SyncResponse::Error("x".repeat(3 * READ_CHUNK + 17)).encode(),
            SyncResponse::Blocks(
                (0..300).map(|h| (Hash([7u8; 32]), block_with_transfers(h, 6))).collect(),
            )
            .encode(),
        ];
        assert!(payloads[2].len() > 2 * READ_CHUNK, "spans pieces");
        let mut compared = 0;
        for payload in &payloads {
            for cut in [payload.len(), payload.len() / 2, 1, 0] {
                let body = &payload[..cut];
                let mut wire = u32::try_from(cut).unwrap().to_le_bytes().to_vec();
                wire.extend_from_slice(body);
                let frame = futures::executor::block_on(read_roomy(&mut Trickle {
                    data: &wire,
                    step: 1000,
                }))
                .unwrap();
                let pieces =
                    frame.decode::<SyncResponse>().map(|r| r.encode()).map_err(|e| e.to_string());
                let slice = SyncResponse::decode(&mut &body[..])
                    .map(|r| r.encode())
                    .map_err(|e| format!("SCALE decode error: {e}"));
                assert_eq!(pieces, slice, "payload {} cut {cut}", payload.len());
                compared += 1;
            }
        }
        assert_eq!(compared, 3 * 4);
    }

    // ── S3 A2.2b-lite: the inbound frame budget ──────────────────────────

    fn used(budget: &FrameBudget, category: Category) -> usize {
        budget.reading(category).used
    }

    fn tight(unsolicited: usize) -> Arc<FrameBudget> {
        Arc::new(FrameBudget::new(1 << 30, unsolicited, 1 << 20))
    }

    #[test]
    fn a_frame_beyond_the_budget_is_refused_as_a_resource_and_gives_everything_back() {
        let budget = tight(100 * 1024);
        let wire = frame(200 * 1024, 200 * 1024);
        let Err(err) = futures::executor::block_on(read_length_delimited(
            &mut Trickle {
                data: &wire,
                step: usize::MAX,
            },
            &budget,
            Category::Unsolicited,
        )) else {
            panic!("a frame beyond the budget was accepted");
        };
        assert_eq!(err.kind(), std::io::ErrorKind::OutOfMemory);
        let reading = budget.reading(Category::Unsolicited);
        assert_eq!((reading.used, reading.refusals), (0, 1));
        assert!(reading.peak <= 100 * 1024, "never above the budget");
    }

    #[test]
    fn a_frame_holds_its_bytes_until_dropped_even_when_it_does_not_decode() {
        let budget = tight(1 << 20);
        let wire = frame(5000, 5000);
        let frame = futures::executor::block_on(read_length_delimited(
            &mut Trickle {
                data: &wire,
                step: 777,
            },
            &budget,
            Category::Unsolicited,
        ))
        .unwrap();
        assert_eq!(used(&budget, Category::Unsolicited), 5000);
        assert!(frame.decode::<SyncNotification>().is_err(), "not a block");
        drop(frame);
        assert_eq!(used(&budget, Category::Unsolicited), 0);
    }

    #[test]
    fn a_frame_cut_short_gives_its_bytes_back() {
        let budget = tight(1 << 20);
        let wire = frame(300_000, 100_000);
        let Err(err) = futures::executor::block_on(read_length_delimited(
            &mut Trickle {
                data: &wire,
                step: 4096,
            },
            &budget,
            Category::Unsolicited,
        )) else {
            panic!("a cut-short frame was accepted");
        };
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(used(&budget, Category::Unsolicited), 0);
        assert!(budget.reading(Category::Unsolicited).peak > 0);
    }

    /// A reader that hands out what it has, then waits forever.
    struct Stalls<'a> {
        data: &'a [u8],
    }

    impl AsyncRead for Stalls<'_> {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            out: &mut [u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            if self.data.is_empty() {
                return std::task::Poll::Pending;
            }
            let n = out.len().min(self.data.len());
            out[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            std::task::Poll::Ready(Ok(n))
        }
    }

    #[test]
    fn a_cancelled_read_gives_its_bytes_back() {
        use std::future::Future;
        let budget = tight(1 << 20);
        let wire = frame(500_000, 150_000);
        let mut reader = Stalls { data: &wire };
        let mut read = Box::pin(read_length_delimited(
            &mut reader,
            &budget,
            Category::Unsolicited,
        ));
        let waker = futures::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        assert!(read.as_mut().poll(&mut cx).is_pending());
        let held = used(&budget, Category::Unsolicited);
        assert!(held >= 150_000, "what arrived is reserved: {held}");
        assert!(
            held <= 150_000 + READ_CHUNK,
            "plus one piece at most: {held}"
        );
        drop(read);
        assert_eq!(
            used(&budget, Category::Unsolicited),
            0,
            "a timeout or a closed stream drops the read"
        );
    }

    #[test]
    fn small_frames_use_the_small_pool_and_large_ones_their_category() {
        let budget = Arc::new(FrameBudget::default());
        let small = frame(17, 17);
        let held = futures::executor::block_on(read_length_delimited(
            &mut Trickle {
                data: &small,
                step: usize::MAX,
            },
            &budget,
            Category::Unsolicited,
        ))
        .unwrap();
        assert_eq!(used(&budget, Category::Small), 17);
        assert_eq!(used(&budget, Category::Unsolicited), 0);
        drop(held);
        let large = frame(2000, 2000);
        let held = futures::executor::block_on(read_length_delimited(
            &mut Trickle {
                data: &large,
                step: usize::MAX,
            },
            &budget,
            Category::Solicited,
        ))
        .unwrap();
        assert_eq!(used(&budget, Category::Solicited), 2000);
        drop(held);
        assert_eq!(budget.reading(Category::Small).peak, 17);
    }

    /// Requests and announcements draw on the unsolicited pool, answers on
    /// the solicited one, through the codecs libp2p clones.
    #[test]
    fn codecs_and_their_clones_share_one_budget_by_direction() {
        use libp2p::request_response::Codec;
        let budget = Arc::new(FrameBudget::default());
        let notify = BlockNotifyCodec::with_budget(Arc::clone(&budget));
        let sync = SyncCodec::with_budget(Arc::clone(&budget));

        let announcement = SyncNotification::NewBlock {
            block: block_with_transfers(5, 20),
        }
        .encode();
        assert!(announcement.len() > SMALL_FRAME);
        let mut wire = Vec::new();
        futures::executor::block_on(write_length_delimited(&mut wire, &announcement)).unwrap();
        let mut clone = notify.clone();
        futures::executor::block_on(clone.read_request(
            &BLOCK_NOTIFY_PROTOCOL,
            &mut Trickle {
                data: &wire,
                step: usize::MAX,
            },
        ))
        .unwrap();
        assert_eq!(
            budget.reading(Category::Unsolicited).peak,
            announcement.len()
        );
        assert_eq!(budget.reading(Category::Solicited).peak, 0);

        let answer =
            SyncResponse::Blocks(vec![(Hash([3u8; 32]), block_with_transfers(6, 20))]).encode();
        let mut wire = Vec::new();
        futures::executor::block_on(write_length_delimited(&mut wire, &answer)).unwrap();
        let mut clone = sync.clone();
        futures::executor::block_on(clone.read_response(
            &SYNC_PROTOCOL,
            &mut Trickle {
                data: &wire,
                step: usize::MAX,
            },
        ))
        .unwrap();
        assert_eq!(budget.reading(Category::Solicited).peak, answer.len());
        for category in [Category::Solicited, Category::Unsolicited, Category::Small] {
            assert_eq!(used(&budget, category), 0, "{category:?} returned");
        }
    }

    // ── S3 A2.2d-1: shared broadcast bytes, earlier release ─────────────

    fn write_announcement(announcement: Announcement) -> Vec<u8> {
        use libp2p::request_response::Codec;
        let mut wire = Vec::new();
        futures::executor::block_on(BlockNotifyCodec::default().write_request(
            &BLOCK_NOTIFY_PROTOCOL,
            &mut wire,
            announcement,
        ))
        .unwrap();
        wire
    }

    /// The encoded-once announcement is byte for byte the derived SCALE
    /// encoding of `SyncNotification::NewBlock`, for empty and full blocks.
    #[test]
    fn a_shared_announcement_is_the_derived_encoding_byte_for_byte() {
        let mut compared = 0;
        for block in [block_with_transfers(0, 0), block_with_transfers(9, 300)] {
            let derived = SyncNotification::NewBlock {
                block: block.clone(),
            }
            .encode();
            let Announcement::Encoded(shared) = Announcement::new_block(&block) else {
                panic!("new_block encodes");
            };
            assert_eq!(shared.as_slice(), derived.as_slice());
            compared += 1;
        }
        assert_eq!(compared, 2);
    }

    /// Both forms write the same frame, which reads back to the same block.
    #[test]
    fn both_forms_write_the_same_frame() {
        use libp2p::request_response::Codec;
        let block = block_with_transfers(4, 50);
        let shared = write_announcement(Announcement::new_block(&block));
        let decoded = write_announcement(
            SyncNotification::NewBlock {
                block: block.clone(),
            }
            .into(),
        );
        assert_eq!(shared, decoded);
        let read = futures::executor::block_on(BlockNotifyCodec::default().read_request(
            &BLOCK_NOTIFY_PROTOCOL,
            &mut Trickle {
                data: &shared,
                step: 1000,
            },
        ))
        .unwrap();
        let Announcement::Decoded(SyncNotification::NewBlock { block: back }) = read else {
            panic!("the codec reads decoded announcements");
        };
        assert_eq!(back.encode(), block.encode());
    }

    /// Concurrent writes of one shared announcement all produce the same
    /// frame, and the bytes are freed once every request is done.
    #[test]
    fn concurrent_broadcast_writes_share_one_buffer_and_release_it() {
        let block = block_with_transfers(2, 100);
        let announcement = Announcement::new_block(&block);
        let Announcement::Encoded(bytes) = &announcement else {
            panic!("encoded");
        };
        let weak = Arc::downgrade(bytes);
        let expected = write_announcement(announcement.clone());
        let outputs: Vec<Vec<u8>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let copy = announcement.clone();
                    scope.spawn(move || write_announcement(copy))
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(outputs.len(), 8);
        assert!(outputs.iter().all(|o| *o == expected));
        drop(announcement);
        assert!(
            weak.upgrade().is_none(),
            "no request holds the bytes any longer"
        );
    }

    /// A writer that takes the length prefix, then never more.
    struct StallsAfterHeader {
        taken: usize,
    }

    impl AsyncWrite for StallsAfterHeader {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            if self.taken >= 4 {
                return std::task::Poll::Pending;
            }
            let n = buf.len().min(4 - self.taken);
            self.taken += n;
            std::task::Poll::Ready(Ok(n))
        }
        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    /// A broadcast write cancelled mid-frame (timeout or closed connection)
    /// gives the shared bytes back.
    #[test]
    fn a_cancelled_broadcast_write_releases_the_shared_bytes() {
        use libp2p::request_response::Codec;
        let announcement = Announcement::new_block(&block_with_transfers(1, 100));
        let Announcement::Encoded(bytes) = &announcement else {
            panic!("encoded");
        };
        let weak = Arc::downgrade(bytes);
        let mut codec = BlockNotifyCodec::default();
        let mut writer = StallsAfterHeader { taken: 0 };
        let mut write = codec.write_request(&BLOCK_NOTIFY_PROTOCOL, &mut writer, announcement);
        let waker = futures::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        assert!(write.as_mut().poll(&mut cx).is_pending());
        assert!(weak.upgrade().is_some(), "held while the write is pending");
        drop(write);
        assert!(
            weak.upgrade().is_none(),
            "released when the write is dropped"
        );
    }
}

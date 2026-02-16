//! Storage-backed responder for inbound block-sync requests.
//!
//! Runs as a background task, reading [`InboundSyncRequest`] messages from
//! the P2P layer's mpsc channel and answering them from local storage.
//! Responses are sent back to the swarm via [`SyncCommand::SendResponse`].

use std::sync::Arc;

use log::{debug, warn};
use mbongo_network::{InboundSyncRequest, SyncCommand, SyncRequest, SyncResponse, MAX_RANGE};
use mbongo_storage::Storage;
use tokio::sync::mpsc;

use crate::backend::compute_block_hash;

/// Runs the sync service loop.
///
/// Reads inbound requests from `rx` and responds using data from `storage`.
/// Responses are delivered back to the P2P swarm via `cmd_tx` using
/// [`SyncCommand::SendResponse`], which the swarm event loop drains.
///
/// This function runs forever; spawn it on a tokio task.
pub async fn run_sync_service<S: Storage + Send + Sync + 'static>(
    storage: Arc<S>,
    mut rx: mpsc::UnboundedReceiver<InboundSyncRequest>,
    cmd_tx: mpsc::UnboundedSender<SyncCommand>,
) {
    while let Some(inbound) = rx.recv().await {
        let response = handle_request(storage.as_ref(), &inbound.request);
        debug!(
            "Sync response to {}: {:?}",
            inbound.peer,
            response_summary(&response)
        );
        let cmd = SyncCommand::SendResponse {
            channel: inbound.channel,
            response,
        };
        if cmd_tx.send(cmd).is_err() {
            warn!("Sync command channel closed; stopping sync service");
            break;
        }
    }
}

/// Handle a single sync request against local storage.
fn handle_request<S: Storage>(storage: &S, request: &SyncRequest) -> SyncResponse {
    match request {
        SyncRequest::GetHeight => match storage.get_latest_height() {
            Ok(h) => SyncResponse::Height(h),
            Err(e) => SyncResponse::Error(format!("storage error: {e}")),
        },
        SyncRequest::GetBlocks {
            start_height,
            end_height,
        } => {
            if end_height <= start_height {
                return SyncResponse::Error("end_height must be > start_height".to_string());
            }
            if end_height - start_height > MAX_RANGE {
                return SyncResponse::Error(format!(
                    "range too large: {} > {MAX_RANGE}",
                    end_height - start_height
                ));
            }

            let latest = match storage.get_latest_height() {
                Ok(h) => h,
                Err(e) => return SyncResponse::Error(format!("storage error: {e}")),
            };

            // Clamp end_height to our chain tip + 1.
            let clamped_end = (*end_height).min(latest + 1);

            let mut blocks = Vec::new();
            for h in *start_height..clamped_end {
                match storage.get_block_by_height(h) {
                    Ok(Some(block)) => {
                        let hash = compute_block_hash(&block);
                        blocks.push((hash, block));
                    }
                    Ok(None) => break, // Gap in chain; stop here.
                    Err(e) => return SyncResponse::Error(format!("storage error: {e}")),
                }
            }

            SyncResponse::Blocks(blocks)
        }
    }
}

/// Short summary for logging without dumping entire blocks.
fn response_summary(resp: &SyncResponse) -> String {
    match resp {
        SyncResponse::Height(h) => format!("Height({h})"),
        SyncResponse::Blocks(blocks) => format!("Blocks(count={})", blocks.len()),
        SyncResponse::Error(e) => format!("Error({e})"),
    }
}

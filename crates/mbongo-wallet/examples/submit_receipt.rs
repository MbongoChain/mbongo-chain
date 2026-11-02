//! Example: build a signed `AnchorReceipt` transaction for the devnet.
//!
//! Constructs a canonical version-1 [`Receipt`], signs its raw 32-byte
//! receipt hash with the devnet key, wraps it in a canonically formed
//! `AnchorReceipt` transaction (sender == executor, zero receiver,
//! amount 0), signs the transaction, and prints a structured JSON
//! object containing the exact JSON-RPC `submit_transaction` request
//! body plus useful metadata.
//!
//! # SECURITY
//!
//! This uses the code-baked PUBLIC devnet signing key (seed `0xAA` × 32),
//! the same account `ensure_genesis` pre-funds. It is intentionally
//! public and therefore UNSAFE anywhere outside a throwaway devnet.
//! Never use it for funds, production, or anything you care about.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p mbongo-wallet --example submit_receipt -- \
//!     --nonce 0 --task-id <64 hex chars> [--input-commitment <64 hex>] \
//!     [--output-commitment <64 hex>] [--metadata <hex bytes>]
//! ```

use clap::Parser;
use ed25519_dalek::{Signer, SigningKey};
use mbongo_core::{Address, Receipt, Transaction, TransactionPayload, TransactionType};
use serde_json::json;

/// Maximum metadata size accepted by v0.3 consensus (RFC 0002 §3).
const MAX_RECEIPT_METADATA_BYTES: usize = 4096;

#[derive(Parser, Debug)]
#[command(name = "submit_receipt")]
#[command(about = "Build a signed AnchorReceipt transaction for the Mbongo devnet")]
struct Args {
    /// Transaction nonce (must equal the dev account's current nonce).
    #[arg(long)]
    nonce: u64,

    /// Task id: exactly 64 hex characters (32 bytes), optionally 0x-prefixed.
    #[arg(long)]
    task_id: String,

    /// Input commitment: 64 hex characters. Opaque to the chain.
    #[arg(
        long,
        default_value = "0000000000000000000000000000000000000000000000000000000000000000"
    )]
    input_commitment: String,

    /// Output commitment: 64 hex characters. Opaque to the chain.
    #[arg(
        long,
        default_value = "0000000000000000000000000000000000000000000000000000000000000000"
    )]
    output_commitment: String,

    /// Metadata as hex bytes (even length, max 4096 bytes). Opaque.
    #[arg(long, default_value = "")]
    metadata: String,
}

/// Decodes a strict 32-byte hex field (64 chars, optional 0x prefix).
fn parse_hash32(label: &str, value: &str) -> Result<[u8; 32], String> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    if raw.len() != 64 {
        return Err(format!(
            "{label} must be exactly 64 hex characters, got {}",
            raw.len()
        ));
    }
    let bytes = hex::decode(raw).map_err(|e| format!("{label} is not valid hex: {e}"))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    let task_id = parse_hash32("task-id", &args.task_id)?;
    let input_commitment = parse_hash32("input-commitment", &args.input_commitment)?;
    let output_commitment = parse_hash32("output-commitment", &args.output_commitment)?;

    let metadata_hex = args.metadata.strip_prefix("0x").unwrap_or(&args.metadata);
    let metadata =
        hex::decode(metadata_hex).map_err(|e| format!("metadata is not valid hex: {e}"))?;
    if metadata.len() > MAX_RECEIPT_METADATA_BYTES {
        return Err(format!(
            "metadata is {} bytes; consensus maximum is {MAX_RECEIPT_METADATA_BYTES}",
            metadata.len()
        ));
    }

    // The public devnet key. The seed is a well-known constant; the
    // private key itself is never printed or logged.
    eprintln!(
        "WARNING: using the public devnet key (seed 0xAA..AA). \
         This key is UNSAFE outside a throwaway devnet."
    );
    let sk = SigningKey::from_bytes(&[0xAAu8; 32]);
    let sender = Address(sk.verifying_key().to_bytes());

    // Canonical version-1 receipt, executor == sender, signed over the
    // raw 32-byte receipt hash (never the hex display string).
    let mut receipt = Receipt {
        version: 1,
        task_id,
        input_commitment,
        output_commitment,
        executor: sender,
        metadata,
        signature: [0u8; 64],
    };
    let receipt_hash = receipt.receipt_hash();
    receipt.signature = sk.sign(&receipt_hash.0).to_bytes();

    // Canonically formed AnchorReceipt transaction (RFC 0002 §1):
    // sender == executor, zero receiver, amount 0.
    let mut tx = Transaction {
        tx_type: TransactionType::AnchorReceipt,
        sender,
        receiver: Address::zero(),
        amount: 0,
        nonce: args.nonce,
        payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
        signature: [0u8; 64],
    };
    tx.signature = sk.sign(&tx.signing_payload()).to_bytes();

    let output = json!({
        "rpc_request": {
            "jsonrpc": "2.0",
            "method": "submit_transaction",
            "params": tx,
            "id": 1
        },
        "metadata": {
            "task_id": format!("0x{}", hex::encode(task_id)),
            "sender": sender.to_string(),
            "receipt_hash": receipt_hash.to_string(),
            "nonce": args.nonce
        }
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|e| e.to_string())?
    );
    Ok(())
}

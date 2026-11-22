//! Example: generate a properly signed `submit_transaction` JSON-RPC request.
//!
//! Run with:
//! ```sh
//! cargo run -p mbongo-wallet --example sign_tx
//! ```

use ed25519_dalek::{Signer, SigningKey};
use mbongo_core::{Address, Transaction, TransactionPayload, TransactionType};
use serde_json::json;

fn main() {
    // Deterministic key for demo purposes.
    let signing_key = SigningKey::from_bytes(&[0xAA; 32]);
    let verifying_key = signing_key.verifying_key();
    let sender = Address(verifying_key.to_bytes());

    let receiver = Address([0x22u8; 32]);

    // Build the transaction (signature placeholder).
    let mut tx = Transaction {
        tx_type: TransactionType::Transfer,
        sender,
        receiver,
        amount: 100,
        nonce: 0,
        payload: TransactionPayload::None,
        signature: [0u8; 64],
    };

    // Sign for the network this example targets: the dev chain, the network
    // of a node started with --dev (RFC 0006 §2). A signature for it does
    // not verify on any other network.
    let chain_id = mbongo_core::Genesis::dev().chain_id;
    let sig = signing_key.sign(&tx.signing_payload(&chain_id));
    tx.signature = sig.to_bytes();

    // Sanity check.
    assert!(tx.verify_signature(&chain_id), "signature must be valid");

    // Wrap in a JSON-RPC 2.0 request.
    let request = json!({
        "jsonrpc": "2.0",
        "method": "submit_transaction",
        "params": tx,
        "id": 1
    });

    println!("{}", serde_json::to_string_pretty(&request).unwrap());
}

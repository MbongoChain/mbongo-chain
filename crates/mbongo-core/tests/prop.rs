use mbongo_core::{
    compute_transactions_root, Address, ComputeTask, Transaction, TransactionPayload,
    TransactionType, MAX_EXECUTION_SPEC_BYTES,
};
use parity_scale_codec::{Decode, Encode};
use proptest::prelude::*;

prop_compose! {
    fn arb_address()(bytes in proptest::array::uniform32(any::<u8>())) -> Address {
        Address(bytes)
    }
}

prop_compose! {
    fn arb_signature()(v in proptest::collection::vec(any::<u8>(), 64)) -> [u8;64] {
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&v);
        arr
    }
}

prop_compose! {
    fn arb_tx()(tx_type in prop_oneof![
            Just(TransactionType::Transfer),
            Just(TransactionType::ComputeTask),
            Just(TransactionType::Stake),
        ],
        sender in arb_address(),
        receiver in arb_address(),
        amount in any::<u128>(),
        nonce in any::<u64>(),
        signature in arb_signature(),
    ) -> Transaction {
        Transaction {
            tx_type,
            sender,
            receiver,
            amount,
            nonce,
            payload: TransactionPayload::None,
            signature,
        }
    }
}

prop_compose! {
    /// Any envelope within the RFC 0005 field shapes, including specs at
    /// and just past the consensus bound: encoding must round-trip for all
    /// of them, and identity must be a function of the bytes.
    fn arb_task()(
        version in any::<u8>(),
        submitter in arb_address(),
        executor in arb_address(),
        salt in proptest::array::uniform32(any::<u8>()),
        input_commitment in proptest::array::uniform32(any::<u8>()),
        execution_spec in proptest::collection::vec(any::<u8>(), 0..=MAX_EXECUTION_SPEC_BYTES + 1),
    ) -> ComputeTask {
        ComputeTask { version, submitter, executor, salt, input_commitment, execution_spec }
    }
}

prop_compose! {
    fn arb_task_tx()(
        task in arb_task(),
        nonce in any::<u64>(),
        signature in arb_signature(),
    ) -> Transaction {
        Transaction {
            tx_type: TransactionType::ComputeTask,
            sender: task.submitter,
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::ComputeTask(Box::new(task)),
            signature,
        }
    }
}

proptest! {
    // Every envelope encodes to bytes that decode back to itself.
    #[test]
    fn compute_task_scale_roundtrip(task in arb_task()) {
        let bytes = task.encode();
        prop_assert_eq!(ComputeTask::decode(&mut &bytes[..]).unwrap(), task);
    }

    // task_id is a pure function of the envelope, and the preimage is the
    // raw domain tag followed by exactly the canonical bytes.
    #[test]
    fn compute_task_identity_is_deterministic(task in arb_task()) {
        prop_assert_eq!(task.task_id(), task.clone().task_id());
        let preimage = task.task_id_preimage();
        let canonical = task.encode();
        prop_assert_eq!(&preimage[..22], &b"mbongo:compute-task:v1"[..]);
        prop_assert_eq!(&preimage[22..], canonical.as_slice());
    }

    // The salt alone separates otherwise identical tasks.
    #[test]
    fn compute_task_salt_changes_identity(task in arb_task(), other in proptest::array::uniform32(any::<u8>())) {
        prop_assume!(other != task.salt);
        let resalted = ComputeTask { salt: other, ..task.clone() };
        prop_assert_ne!(task.task_id(), resalted.task_id());
    }

    // A ComputeTask transaction round-trips through the full transaction
    // encoding, and its signing payload is the strict prefix before the
    // signature — same rule as every other transaction.
    #[test]
    fn compute_task_transaction_roundtrip(tx in arb_task_tx()) {
        let bytes = tx.encode();
        prop_assert_eq!(Transaction::decode(&mut &bytes[..]).unwrap(), tx.clone());
        let payload = tx.signing_payload();
        prop_assert_eq!(&bytes[..payload.len()], payload.as_slice());
        prop_assert_eq!(&bytes[payload.len()..], &tx.signature[..]);
        prop_assert_eq!(bytes[89], 0x02);
    }
}

proptest! {
    // Root changes when list changes (append one tx)
    #[test]
    fn root_changes_on_append(mut txs in proptest::collection::vec(arb_tx(), 0..10), extra in arb_tx()) {
        let r1 = compute_transactions_root(&txs);
        txs.push(extra);
        let r2 = compute_transactions_root(&txs);
        prop_assert_ne!(r1, r2);
    }

    // Identical lists yield identical roots
    #[test]
    fn same_list_same_root(txs in proptest::collection::vec(arb_tx(), 0..10)) {
        let r1 = compute_transactions_root(&txs);
        let r2 = compute_transactions_root(&txs);
        prop_assert_eq!(r1, r2);
    }

    // Different permutations likely produce different roots (not guaranteed, but extremely likely)
    #[test]
    fn permutation_changes_root(mut txs in proptest::collection::vec(arb_tx(), 3..8)) {
        let r1 = compute_transactions_root(&txs);
        let len = txs.len();
        txs.swap(0, len-1);
        let r2 = compute_transactions_root(&txs);
        prop_assert_ne!(r1, r2);
    }
}

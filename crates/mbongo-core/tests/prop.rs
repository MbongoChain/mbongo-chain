use mbongo_core::{compute_transactions_root, Address, Transaction, TransactionType};
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
        Transaction { tx_type, sender, receiver, amount, nonce, signature }
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

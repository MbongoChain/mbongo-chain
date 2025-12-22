use mbongo_core::storage::trie::MerklePatriciaTrie;

#[test]
fn integration_state_trie_basic() {
    let mut t = MerklePatriciaTrie::with_memory();
    t.insert(&[0u8, 1, 2], vec![9]);
    t.insert(&[0u8, 1, 3], vec![8]);
    assert_eq!(t.get(&[0u8, 1, 2]), Some(vec![9]));
    assert_eq!(t.get(&[0u8, 1, 3]), Some(vec![8]));
    assert_eq!(t.get(&[0u8, 1, 4]), None);
    let root1 = t.root();
    assert!(root1 != Default::default());

    // Update and ensure root changes
    t.insert(&[0u8, 1, 3], vec![7]);
    let root2 = t.root();
    assert_ne!(root1, root2);

    // Delete and verify absence
    assert!(t.delete(&[0u8, 1, 2]));
    assert_eq!(t.get(&[0u8, 1, 2]), None);
}

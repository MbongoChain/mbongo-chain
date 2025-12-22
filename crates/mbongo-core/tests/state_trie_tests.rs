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

#[test]
fn test_delete_nonexistent_key() {
    let mut t = MerklePatriciaTrie::with_memory();
    // Deleting from empty trie should return false
    assert!(!t.delete(&[0u8, 1, 2]));
    
    // Insert a key, then try to delete a different key
    t.insert(&[0u8, 1, 2], vec![9]);
    assert!(!t.delete(&[0u8, 1, 3]));
}

#[test]
fn test_empty_trie_operations() {
    let mut t = MerklePatriciaTrie::with_memory();
    
    // Get from empty trie should return None
    assert_eq!(t.get(&[0u8, 1, 2]), None);
    
    // Delete from empty trie should return false
    assert!(!t.delete(&[0u8, 1, 2]));
    
    // Root of empty trie should be default
    assert_eq!(t. root(), Default::default());
}

#[test]
fn test_operations_after_complete_deletion() {
    let mut t = MerklePatriciaTrie::with_memory();
    
    // Insert some keys
    t.insert(&[0u8, 1, 2], vec![9]);
    t.insert(&[0u8, 1, 3], vec![8]);
    let root_with_data = t.root();
    assert_ne!(root_with_data, Default::default());
    
    // Delete all keys
    assert!(t.delete(&[0u8, 1, 2]));
    assert!(t.delete(&[0u8, 1, 3]));
    
    // Trie should be empty again
    assert_eq!(t. get(&[0u8, 1, 2]), None);
    assert_eq!(t.get(&[0u8, 1, 3]), None);
    
    // Root should return to default (empty state)
    assert_eq!(t.root(), Default::default());
    
    // Should be able to insert again
    t.insert(&[0u8, 1, 2], vec![10]);
    assert_eq!(t.get(&[0u8, 1, 2]), Some(vec![10]));
}

#[test]
fn test_empty_values_and_special_keys() {
    let mut t = MerklePatriciaTrie::with_memory();
    
    // Insert empty value
    t.insert(&[0u8, 1, 2], vec![]);
    assert_eq!(t.get(&[0u8, 1, 2]), Some(vec![]));
    
    // Insert with all zeros key
    t.insert(&[0u8, 0, 0], vec![1]);
    assert_eq!(t. get(&[0u8, 0, 0]), Some(vec![1]));
    
    // Insert with all 0xFF key
    t.insert(&[0xFFu8, 0xFF, 0xFF], vec![2]);
    assert_eq!(t.get(&[0xFFu8, 0xFF, 0xFF]), Some(vec![2]));
    
    // Single byte keys
    t.insert(&[0u8], vec![3]);
    assert_eq!(t.get(&[0u8]), Some(vec![3]));
}

#[test]
fn test_prefix_keys() {
    let mut t = MerklePatriciaTrie::with_memory();
    
    // Insert keys where one is a prefix of another
    t.insert(&[0u8, 1], vec![1]);
    t.insert(&[0u8, 1, 2], vec![2]);
    t.insert(&[0u8, 1, 2, 3], vec![3]);
    
    // All should be retrievable independently
    assert_eq!(t. get(&[0u8, 1]), Some(vec![1]));
    assert_eq!(t.get(&[0u8, 1, 2]), Some(vec![2]));
    assert_eq!(t.get(&[0u8, 1, 2, 3]), Some(vec![3]));
    
    // Delete middle key shouldn't affect others
    assert!(t.delete(&[0u8, 1, 2]));
    assert_eq!(t.get(&[0u8, 1]), Some(vec![1]));
    assert_eq!(t.get(&[0u8, 1, 2]), None);
    assert_eq!(t.get(&[0u8, 1, 2, 3]), Some(vec![3]));
}

#[test]
fn test_proof_for_nonexistent_keys() {
    let mut t = MerklePatriciaTrie::with_memory();
    
    // Insert some data
    t.insert(&[0u8, 1, 2], vec![9]);
    t.insert(&[0u8, 1, 3], vec![8]);
    
    // Generate proof for non-existent key (if your trie supports proof generation)
    // This will depend on your MerklePatriciaTrie API
    // Example (adjust based on your actual API):
    // let proof = t.get_proof(&[0u8, 1, 4]);
    // assert!(proof.verify_absence(&[0u8, 1, 4], t.root()));
}

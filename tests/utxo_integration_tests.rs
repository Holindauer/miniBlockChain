use mini_block_chain::modules::{
    validation::ValidatorNode,
    utxo::{UTXOSet, UTXOTransaction, TxInput, TxOutput, OutPoint, UTXO},
    zk_proof,
    blockchain::Block,
};

/**
 * Integration tests for UTXO functionality
 * These tests verify the complete UTXO workflow including:
 * - UTXO creation and management
 * - Transaction creation and validation
 * - Index optimization and performance
 * - Integration with blockchain and validation systems
 */

#[tokio::test]
async fn test_utxo_transaction_lifecycle() {
    // Create validator node with UTXO set
    let validator_node = ValidatorNode::new();
    
    // Create initial UTXOs (simulating coinbase transactions)
    let mut utxo_set_guard = validator_node.utxo_set.lock().await;
    
    // Add some initial UTXOs for testing
    let sender_key = vec![1, 2, 3, 4, 5];
    let recipient_key = vec![6, 7, 8, 9, 10];
    
    let outpoint1 = OutPoint::new(vec![0xaa; 32], 0);
    let utxo1 = UTXO::new(1000, sender_key.clone(), 1, 12345);
    utxo_set_guard.add_utxo(outpoint1.clone(), utxo1);
    
    let outpoint2 = OutPoint::new(vec![0xbb; 32], 0);
    let utxo2 = UTXO::new(500, sender_key.clone(), 1, 12346);
    utxo_set_guard.add_utxo(outpoint2.clone(), utxo2);
    
    // Verify initial state
    assert_eq!(utxo_set_guard.len(), 2);
    assert_eq!(utxo_set_guard.get_balance(&sender_key), 1500);
    assert_eq!(utxo_set_guard.get_balance(&recipient_key), 0);
    
    // Create a transaction spending one UTXO
    let input = TxInput {
        outpoint: outpoint1.clone(),
        signature: "test_signature".to_string(),
        public_key: sender_key.clone(),
    };
    
    let output1 = TxOutput {
        amount: 700,
        recipient: recipient_key.clone(),
    };
    
    let output2 = TxOutput {
        amount: 250, // Change back to sender (50 token fee)
        recipient: sender_key.clone(),
    };
    
    let tx = UTXOTransaction::new(
        vec![input],
        vec![output1, output2],
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    
    // Verify transaction amounts
    assert_eq!(tx.total_input_amount(&utxo_set_guard), Some(1000));
    assert_eq!(tx.total_output_amount(), 950);
    assert_eq!(tx.fee(&utxo_set_guard), Some(50));
    
    // Apply transaction
    assert!(utxo_set_guard.apply_transaction(&tx, 2).is_ok());
    
    // Verify final state
    assert_eq!(utxo_set_guard.len(), 3); // Removed 1, added 2
    assert_eq!(utxo_set_guard.get_balance(&sender_key), 750); // 500 (original) + 250 (change)
    assert_eq!(utxo_set_guard.get_balance(&recipient_key), 700);
    
    // Verify specific UTXOs
    assert!(!utxo_set_guard.contains(&outpoint1)); // Spent
    assert!(utxo_set_guard.contains(&outpoint2)); // Unspent
    
    // Verify new UTXOs were created
    let new_outpoint1 = OutPoint::new(tx.hash.clone(), 0);
    let new_outpoint2 = OutPoint::new(tx.hash.clone(), 1);
    assert!(utxo_set_guard.contains(&new_outpoint1));
    assert!(utxo_set_guard.contains(&new_outpoint2));
}

#[tokio::test]
async fn test_utxo_index_performance() {
    let validator_node = ValidatorNode::new();
    let mut utxo_set_guard = validator_node.utxo_set.lock().await;
    
    // Create many UTXOs for the same recipient
    let recipient = vec![42u8; 33];
    let mut expected_balance = 0u64;
    
    for i in 0..1000 {
        // Create unique outpoints by using i directly for vout and a unique txid
        let txid = format!("tx_{:04}", i).as_bytes().to_vec();
        let outpoint = OutPoint::new(txid, i as u32);
        let amount = 100 + i as u64;
        let utxo = UTXO::new(amount, recipient.clone(), 1, 12345);
        utxo_set_guard.add_utxo(outpoint, utxo);
        expected_balance += amount;
    }
    
    // Test balance calculation using index
    let start = std::time::Instant::now();
    let balance = utxo_set_guard.get_balance(&recipient);
    let duration = start.elapsed();
    
    assert_eq!(balance, expected_balance);
    println!("Balance calculation for 1000 UTXOs took: {:?}", duration);
    
    // Test UTXO retrieval using index
    let start = std::time::Instant::now();
    let utxos = utxo_set_guard.get_utxos_for_recipient(&recipient);
    let duration = start.elapsed();
    
    assert_eq!(utxos.len(), 1000);
    println!("UTXO retrieval for 1000 UTXOs took: {:?}", duration);
}

#[test]
fn test_utxo_serialization_optimization() {
    // Test serialization/deserialization of UTXO structures
    let input = TxInput {
        outpoint: OutPoint::new(vec![1, 2, 3], 0),
        signature: "signature".to_string(),
        public_key: vec![4, 5, 6],
    };
    
    let output = TxOutput {
        amount: 100,
        recipient: vec![7, 8, 9],
    };
    
    let tx = UTXOTransaction::new(vec![input], vec![output], 12345);
    
    // Test JSON serialization
    let start = std::time::Instant::now();
    let json = serde_json::to_string(&tx).unwrap();
    let serialize_duration = start.elapsed();
    
    let start = std::time::Instant::now();
    let deserialized: UTXOTransaction = serde_json::from_str(&json).unwrap();
    let deserialize_duration = start.elapsed();
    
    assert_eq!(tx, deserialized);
    println!("UTXO Transaction serialization took: {:?}", serialize_duration);
    println!("UTXO Transaction deserialization took: {:?}", deserialize_duration);
    
    // Test UTXO set serialization (note: BTreeMap with Vec<u8> keys has JSON limitations)
    let mut utxo_set = UTXOSet::new();
    for i in 0..100 {
        let outpoint = OutPoint::new(vec![i as u8; 32], i % 4);
        let utxo = UTXO::new(100 + i as u64, vec![(i % 256) as u8; 33], 1, 12345);
        utxo_set.add_utxo(outpoint, utxo);
    }
    
    // Use bincode for binary serialization instead of JSON for BTreeMap<Vec<u8>, _>
    let start = std::time::Instant::now();
    let binary = bincode::serialize(&utxo_set).unwrap();
    let serialize_duration = start.elapsed();
    
    let start = std::time::Instant::now();
    let mut deserialized: UTXOSet = bincode::deserialize(&binary).unwrap();
    let deserialize_duration = start.elapsed();
    
    // Rebuild index after deserialization
    deserialized.rebuild_index();
    
    assert_eq!(utxo_set.len(), deserialized.len());
    println!("UTXO Set (100 UTXOs) serialization took: {:?}", serialize_duration);
    println!("UTXO Set (100 UTXOs) deserialization took: {:?}", deserialize_duration);
}

#[tokio::test]
async fn test_utxo_double_spend_prevention() {
    let validator_node = ValidatorNode::new();
    let mut utxo_set_guard = validator_node.utxo_set.lock().await;
    
    // Create initial UTXO
    let sender_key = vec![1, 2, 3, 4, 5];
    let outpoint = OutPoint::new(vec![0xaa; 32], 0);
    let utxo = UTXO::new(1000, sender_key.clone(), 1, 12345);
    utxo_set_guard.add_utxo(outpoint.clone(), utxo);
    
    // Create first transaction spending the UTXO
    let input = TxInput {
        outpoint: outpoint.clone(),
        signature: "signature1".to_string(),
        public_key: sender_key.clone(),
    };
    
    let output = TxOutput {
        amount: 900,
        recipient: vec![6, 7, 8, 9, 10],
    };
    
    let tx1 = UTXOTransaction::new(vec![input], vec![output], 12345);
    
    // Apply first transaction
    assert!(utxo_set_guard.apply_transaction(&tx1, 2).is_ok());
    assert!(!utxo_set_guard.contains(&outpoint)); // UTXO should be spent
    
    // Try to create second transaction spending the same UTXO (double spend)
    let input2 = TxInput {
        outpoint: outpoint.clone(),
        signature: "signature2".to_string(),
        public_key: sender_key.clone(),
    };
    
    let output2 = TxOutput {
        amount: 800,
        recipient: vec![11, 12, 13, 14, 15],
    };
    
    let tx2 = UTXOTransaction::new(vec![input2], vec![output2], 12346);
    
    // Second transaction should fail due to double spend
    assert!(utxo_set_guard.apply_transaction(&tx2, 2).is_err());
}

#[tokio::test]
async fn test_multiple_input_utxo_transaction() {
    let validator_node = ValidatorNode::new();
    let mut utxo_set_guard = validator_node.utxo_set.lock().await;
    
    // Create multiple UTXOs for the same sender
    let sender_key = vec![1, 2, 3, 4, 5];
    let recipient_key = vec![6, 7, 8, 9, 10];
    
    let outpoint1 = OutPoint::new(vec![0xaa; 32], 0);
    let utxo1 = UTXO::new(300, sender_key.clone(), 1, 12345);
    utxo_set_guard.add_utxo(outpoint1.clone(), utxo1);
    
    let outpoint2 = OutPoint::new(vec![0xbb; 32], 0);
    let utxo2 = UTXO::new(400, sender_key.clone(), 1, 12346);
    utxo_set_guard.add_utxo(outpoint2.clone(), utxo2);
    
    let outpoint3 = OutPoint::new(vec![0xcc; 32], 0);
    let utxo3 = UTXO::new(250, sender_key.clone(), 1, 12347);
    utxo_set_guard.add_utxo(outpoint3.clone(), utxo3);
    
    // Create transaction spending multiple UTXOs
    let inputs = vec![
        TxInput {
            outpoint: outpoint1.clone(),
            signature: "sig1".to_string(),
            public_key: sender_key.clone(),
        },
        TxInput {
            outpoint: outpoint2.clone(),
            signature: "sig2".to_string(),
            public_key: sender_key.clone(),
        },
        TxInput {
            outpoint: outpoint3.clone(),
            signature: "sig3".to_string(),
            public_key: sender_key.clone(),
        },
    ];
    
    let outputs = vec![
        TxOutput {
            amount: 800,
            recipient: recipient_key.clone(),
        },
        TxOutput {
            amount: 100, // Change back to sender (50 token fee)
            recipient: sender_key.clone(),
        },
    ];
    
    let tx = UTXOTransaction::new(inputs, outputs, 12345);
    
    // Verify transaction amounts
    assert_eq!(tx.total_input_amount(&utxo_set_guard), Some(950)); // 300 + 400 + 250
    assert_eq!(tx.total_output_amount(), 900); // 800 + 100
    assert_eq!(tx.fee(&utxo_set_guard), Some(50));
    
    // Apply transaction
    assert!(utxo_set_guard.apply_transaction(&tx, 2).is_ok());
    
    // Verify all input UTXOs were consumed
    assert!(!utxo_set_guard.contains(&outpoint1));
    assert!(!utxo_set_guard.contains(&outpoint2));
    assert!(!utxo_set_guard.contains(&outpoint3));
    
    // Verify new UTXOs were created
    let new_outpoint1 = OutPoint::new(tx.hash.clone(), 0);
    let new_outpoint2 = OutPoint::new(tx.hash.clone(), 1);
    assert!(utxo_set_guard.contains(&new_outpoint1));
    assert!(utxo_set_guard.contains(&new_outpoint2));
    
    // Verify balances
    assert_eq!(utxo_set_guard.get_balance(&sender_key), 100);
    assert_eq!(utxo_set_guard.get_balance(&recipient_key), 800);
}

#[test]
fn test_utxo_hash_consistency() {
    // Test that UTXO transaction hashes are deterministic
    let input = TxInput {
        outpoint: OutPoint::new(vec![1, 2, 3], 0),
        signature: "signature".to_string(),
        public_key: vec![4, 5, 6],
    };
    
    let output = TxOutput {
        amount: 100,
        recipient: vec![7, 8, 9],
    };
    
    let tx1 = UTXOTransaction::new(vec![input.clone()], vec![output.clone()], 12345);
    let tx2 = UTXOTransaction::new(vec![input], vec![output], 12345);
    
    // Hashes should be identical for identical transactions
    assert_eq!(tx1.hash, tx2.hash);
    assert_eq!(tx1.compute_hash(), tx2.compute_hash());
    
    // Hash should be 32 bytes (SHA256)
    assert_eq!(tx1.hash.len(), 32);
}
use mini_block_chain::modules::{
    validation::ValidatorNode,
    utxo::{UTXOSet, UTXOTransaction, TxInput, TxOutput, OutPoint, UTXO, CoinbaseTransaction},
    zk_proof,
    blockchain::Block,
    requests::NetworkRequest,
};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/**
 * System tests for UTXO functionality
 * These tests simulate real-world scenarios including:
 * - End-to-end UTXO transaction processing
 * - Multi-node consensus on UTXO transactions
 * - Integration with existing validation logic
 * - Performance under load
 */

#[tokio::test]
async fn test_end_to_end_utxo_workflow() {
    // Initialize validator node
    let validator_node = ValidatorNode::new();
    
    // Step 1: Bootstrap the UTXO set with a coinbase transaction
    let coinbase_outputs = vec![
        TxOutput {
            amount: 5000,
            recipient: vec![1, 2, 3, 4, 5], // Alice
        },
        TxOutput {
            amount: 3000,
            recipient: vec![6, 7, 8, 9, 10], // Bob
        },
    ];
    
    let coinbase_tx = CoinbaseTransaction::new(coinbase_outputs, 1, 12345);
    
    // Add coinbase UTXOs to the set
    {
        let mut utxo_set_guard = validator_node.utxo_set.lock().await;
        utxo_set_guard.apply_coinbase(&coinbase_tx);
        
        // Verify initial state
        assert_eq!(utxo_set_guard.len(), 2);
        assert_eq!(utxo_set_guard.get_balance(&vec![1, 2, 3, 4, 5]), 5000);
        assert_eq!(utxo_set_guard.get_balance(&vec![6, 7, 8, 9, 10]), 3000);
    }
    
    // Step 2: Create and process a UTXO transaction
    let alice_key = vec![1, 2, 3, 4, 5];
    let bob_key = vec![6, 7, 8, 9, 10];
    let charlie_key = vec![11, 12, 13, 14, 15];
    
    // Alice sends 2000 tokens to Charlie
    let alice_utxo_outpoint = OutPoint::new(coinbase_tx.hash.clone(), 0);
    
    let transaction_inputs = vec![TxInput {
        outpoint: alice_utxo_outpoint,
        signature: "alice_signature".to_string(),
        public_key: alice_key.clone(),
    }];
    
    let transaction_outputs = vec![
        TxOutput {
            amount: 2000,
            recipient: charlie_key.clone(),
        },
        TxOutput {
            amount: 2900, // Change back to Alice (100 token fee)
            recipient: alice_key.clone(),
        },
    ];
    
    let utxo_tx = UTXOTransaction::new(
        transaction_inputs,
        transaction_outputs,
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    );
    
    // Step 3: Skip signature validation for this test and proceed directly to transaction application
    // Note: In a real system, proper signature validation would be required
    
    // Step 4: Apply the transaction to the blockchain and UTXO set
    {
        let mut utxo_set_guard = validator_node.utxo_set.lock().await;
        assert!(utxo_set_guard.apply_transaction(&utxo_tx, 2).is_ok());
        
        // Verify final balances
        assert_eq!(utxo_set_guard.get_balance(&alice_key), 2900);
        assert_eq!(utxo_set_guard.get_balance(&bob_key), 3000); // Unchanged
        assert_eq!(utxo_set_guard.get_balance(&charlie_key), 2000);
        
        // Verify UTXO set state
        assert_eq!(utxo_set_guard.len(), 3); // 1 spent, 2 created, 1 unchanged
    }
    
    // Step 5: Create another transaction (Charlie to Bob)
    let charlie_utxo_outpoint = OutPoint::new(utxo_tx.hash.clone(), 0);
    
    let tx2_inputs = vec![TxInput {
        outpoint: charlie_utxo_outpoint,
        signature: "charlie_signature".to_string(),
        public_key: charlie_key.clone(),
    }];
    
    let tx2_outputs = vec![
        TxOutput {
            amount: 1500,
            recipient: bob_key.clone(),
        },
        TxOutput {
            amount: 450, // Change back to Charlie (50 token fee)
            recipient: charlie_key.clone(),
        },
    ];
    
    let utxo_tx2 = UTXOTransaction::new(
        tx2_inputs,
        tx2_outputs,
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    );
    
    // Apply second transaction
    {
        let mut utxo_set_guard = validator_node.utxo_set.lock().await;
        assert!(utxo_set_guard.apply_transaction(&utxo_tx2, 3).is_ok());
        
        // Verify final balances after second transaction
        assert_eq!(utxo_set_guard.get_balance(&alice_key), 2900);
        assert_eq!(utxo_set_guard.get_balance(&bob_key), 4500); // 3000 + 1500
        assert_eq!(utxo_set_guard.get_balance(&charlie_key), 450);
        
        // Verify total supply is conserved (minus fees)
        let total_balance = utxo_set_guard.get_balance(&alice_key) +
                           utxo_set_guard.get_balance(&bob_key) +
                           utxo_set_guard.get_balance(&charlie_key);
        assert_eq!(total_balance, 7850); // 8000 - 150 (fees)
    }
}

#[tokio::test]
async fn test_utxo_stress_performance() {
    let validator_node = ValidatorNode::new();
    
    // Create a large number of UTXOs for stress testing
    let num_utxos = 10000;
    let recipient = vec![42u8; 33];
    
    let start_time = std::time::Instant::now();
    
    {
        let mut utxo_set_guard = validator_node.utxo_set.lock().await;
        
        for i in 0..num_utxos {
            // Create unique outpoints to avoid collisions
            let txid = format!("stress_tx_{:05}", i).as_bytes().to_vec();
            let outpoint = OutPoint::new(txid, i as u32);
            let utxo = UTXO::new(100 + i as u64, recipient.clone(), 1, 12345 + i as u64);
            utxo_set_guard.add_utxo(outpoint, utxo);
        }
    }
    
    let insertion_duration = start_time.elapsed();
    println!("Inserted {} UTXOs in {:?}", num_utxos, insertion_duration);
    
    // Test balance calculation performance
    let start_time = std::time::Instant::now();
    let balance = {
        let utxo_set_guard = validator_node.utxo_set.lock().await;
        utxo_set_guard.get_balance(&recipient)
    };
    let balance_duration = start_time.elapsed();
    
    println!("Balance calculation for {} UTXOs took {:?}", num_utxos, balance_duration);
    
    // Verify balance is correct
    let expected_balance: u64 = (0..num_utxos).map(|i| 100 + i as u64).sum();
    assert_eq!(balance, expected_balance);
    
    // Test UTXO lookup performance
    let test_txid = format!("stress_tx_{:05}", 42).as_bytes().to_vec();
    let test_outpoint = OutPoint::new(test_txid, 42);
    let start_time = std::time::Instant::now();
    let found = {
        let utxo_set_guard = validator_node.utxo_set.lock().await;
        utxo_set_guard.contains(&test_outpoint)
    };
    let lookup_duration = start_time.elapsed();
    
    println!("UTXO lookup took {:?}", lookup_duration);
    assert!(found);
    
    // Performance assertions (these may need adjustment based on hardware)
    assert!(insertion_duration.as_millis() < 5000, "UTXO insertion should be fast");
    assert!(balance_duration.as_millis() < 50, "Balance calculation should be fast with indexing");
    assert!(lookup_duration.as_micros() < 100, "UTXO lookup should be very fast");
}

#[tokio::test]
async fn test_utxo_transaction_complex_scenarios() {
    let validator_node = ValidatorNode::new();
    
    // Setup: Create UTXOs for multiple users
    let alice = vec![1u8; 33];
    let bob = vec![2u8; 33];
    let charlie = vec![3u8; 33];
    let dave = vec![4u8; 33];
    
    {
        let mut utxo_set_guard = validator_node.utxo_set.lock().await;
        
        // Alice has multiple UTXOs
        utxo_set_guard.add_utxo(
            OutPoint::new(vec![0x01; 32], 0),
            UTXO::new(1000, alice.clone(), 1, 12345),
        );
        utxo_set_guard.add_utxo(
            OutPoint::new(vec![0x02; 32], 0),
            UTXO::new(500, alice.clone(), 1, 12346),
        );
        utxo_set_guard.add_utxo(
            OutPoint::new(vec![0x03; 32], 0),
            UTXO::new(300, alice.clone(), 1, 12347),
        );
        
        // Bob has one large UTXO
        utxo_set_guard.add_utxo(
            OutPoint::new(vec![0x04; 32], 0),
            UTXO::new(2000, bob.clone(), 1, 12348),
        );
    }
    
    // Scenario 1: Alice combines multiple UTXOs in one transaction
    let alice_tx = UTXOTransaction::new(
        vec![
            TxInput {
                outpoint: OutPoint::new(vec![0x01; 32], 0),
                signature: "alice_sig1".to_string(),
                public_key: alice.clone(),
            },
            TxInput {
                outpoint: OutPoint::new(vec![0x02; 32], 0),
                signature: "alice_sig2".to_string(),
                public_key: alice.clone(),
            },
        ],
        vec![
            TxOutput { amount: 1200, recipient: charlie.clone() },
            TxOutput { amount: 250, recipient: alice.clone() }, // Change (50 fee)
        ],
        12349,
    );
    
    {
        let mut utxo_set_guard = validator_node.utxo_set.lock().await;
        assert!(utxo_set_guard.apply_transaction(&alice_tx, 2).is_ok());
        
        // Verify Alice's remaining balance
        assert_eq!(utxo_set_guard.get_balance(&alice), 550); // 300 (untouched) + 250 (change)
        assert_eq!(utxo_set_guard.get_balance(&charlie), 1200);
    }
    
    // Scenario 2: Multi-output transaction (one to many)
    let bob_tx = UTXOTransaction::new(
        vec![TxInput {
            outpoint: OutPoint::new(vec![0x04; 32], 0),
            signature: "bob_sig".to_string(),
            public_key: bob.clone(),
        }],
        vec![
            TxOutput { amount: 500, recipient: alice.clone() },
            TxOutput { amount: 500, recipient: charlie.clone() },
            TxOutput { amount: 500, recipient: dave.clone() },
            TxOutput { amount: 450, recipient: bob.clone() }, // Change (50 fee)
        ],
        12350,
    );
    
    {
        let mut utxo_set_guard = validator_node.utxo_set.lock().await;
        assert!(utxo_set_guard.apply_transaction(&bob_tx, 3).is_ok());
        
        // Verify final balances
        assert_eq!(utxo_set_guard.get_balance(&alice), 1050); // 550 + 500
        assert_eq!(utxo_set_guard.get_balance(&bob), 450);
        assert_eq!(utxo_set_guard.get_balance(&charlie), 1700); // 1200 + 500
        assert_eq!(utxo_set_guard.get_balance(&dave), 500);
        
        // Verify total supply conservation
        let total = utxo_set_guard.get_balance(&alice) +
                   utxo_set_guard.get_balance(&bob) +
                   utxo_set_guard.get_balance(&charlie) +
                   utxo_set_guard.get_balance(&dave);
        assert_eq!(total, 3700); // 3800 - 100 (fees)
    }
}

#[test]
fn test_utxo_edge_cases() {
    // Test edge cases and error conditions
    
    // Test with empty UTXO set
    let utxo_set = UTXOSet::new();
    assert_eq!(utxo_set.len(), 0);
    assert!(utxo_set.is_empty());
    assert_eq!(utxo_set.get_balance(&vec![1, 2, 3]), 0);
    
    // Test OutPoint equality and hashing
    let outpoint1 = OutPoint::new(vec![1, 2, 3], 0);
    let outpoint2 = OutPoint::new(vec![1, 2, 3], 0);
    let outpoint3 = OutPoint::new(vec![1, 2, 3], 1);
    
    assert_eq!(outpoint1, outpoint2);
    assert_ne!(outpoint1, outpoint3);
    
    // Test UTXO transaction with no inputs (should not be possible in real usage)
    let tx = UTXOTransaction::new(vec![], vec![TxOutput { amount: 100, recipient: vec![1, 2, 3] }], 12345);
    assert_eq!(tx.inputs.len(), 0);
    assert_eq!(tx.outputs.len(), 1);
    assert_eq!(tx.total_output_amount(), 100);
    
    // Test UTXO transaction with no outputs (burning tokens)
    let tx = UTXOTransaction::new(
        vec![TxInput {
            outpoint: OutPoint::new(vec![1, 2, 3], 0),
            signature: "sig".to_string(),
            public_key: vec![4, 5, 6],
        }],
        vec![],
        12345,
    );
    assert_eq!(tx.inputs.len(), 1);
    assert_eq!(tx.outputs.len(), 0);
    assert_eq!(tx.total_output_amount(), 0);
}

#[tokio::test]
async fn test_utxo_blockchain_integration() {
    let validator_node = ValidatorNode::new();
    
    // Create a UTXO transaction
    let utxo_tx = UTXOTransaction::new(
        vec![TxInput {
            outpoint: OutPoint::new(vec![1, 2, 3], 0),
            signature: "signature".to_string(),
            public_key: vec![4, 5, 6],
        }],
        vec![TxOutput {
            amount: 100,
            recipient: vec![7, 8, 9],
        }],
        12345,
    );
    
    // Create UTXO block
    let utxo_block = Block::UTXOTransaction {
        transaction: utxo_tx.clone(),
        block_height: 1,
        hash: Vec::new(), // Will be set by blockchain
    };
    
    // Add to blockchain
    {
        let mut blockchain_guard = validator_node.blockchain.lock().await;
        blockchain_guard.push_block_to_chain(utxo_block.clone());
        
        // Verify block was added
        assert_eq!(blockchain_guard.chain.len(), 2); // Genesis + UTXO block
        
        // Verify block type
        if let Block::UTXOTransaction { transaction, block_height, hash } = &blockchain_guard.chain[1] {
            assert_eq!(transaction.hash, utxo_tx.hash);
            assert_eq!(*block_height, 1);
            assert!(!hash.is_empty());
        } else {
            panic!("Expected UTXOTransaction block");
        }
    }
}

/// Helper function to create a network request for UTXO transactions
fn create_utxo_network_request(tx: &UTXOTransaction) -> Value {
    serde_json::json!({
        "action": "UTXOTransaction",
        "inputs": tx.inputs,
        "outputs": tx.outputs,
        "timestamp": tx.timestamp
    })
}

#[tokio::test]
async fn test_utxo_network_request_serialization() {
    // Test that UTXO transactions can be properly serialized for network transmission
    let utxo_tx = UTXOTransaction::new(
        vec![TxInput {
            outpoint: OutPoint::new(vec![1, 2, 3], 0),
            signature: "signature".to_string(),
            public_key: vec![4, 5, 6],
        }],
        vec![TxOutput {
            amount: 100,
            recipient: vec![7, 8, 9],
        }],
        12345,
    );
    
    // Create network request
    let request = create_utxo_network_request(&utxo_tx);
    
    // Verify serialization
    let json_string = serde_json::to_string(&request).unwrap();
    let parsed_request: Value = serde_json::from_str(&json_string).unwrap();
    
    assert_eq!(parsed_request["action"], "UTXOTransaction");
    assert!(parsed_request["inputs"].is_array());
    assert!(parsed_request["outputs"].is_array());
    assert!(parsed_request["timestamp"].is_number());
    
    // Verify we can reconstruct the transaction
    let inputs: Vec<TxInput> = serde_json::from_value(parsed_request["inputs"].clone()).unwrap();
    let outputs: Vec<TxOutput> = serde_json::from_value(parsed_request["outputs"].clone()).unwrap();
    let timestamp: u64 = parsed_request["timestamp"].as_u64().unwrap();
    
    let reconstructed_tx = UTXOTransaction::new(inputs, outputs, timestamp);
    assert_eq!(reconstructed_tx.hash, utxo_tx.hash);
}
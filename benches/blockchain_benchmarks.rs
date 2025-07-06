use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mini_block_chain::modules::{
    blockchain::{BlockChain, Block},
    merkle_tree::{MerkleTree, Account},
    zk_proof,
    validation::ValidatorNode,
    utxo::{UTXOSet, UTXOTransaction, TxInput, TxOutput, OutPoint, UTXO},
};
use std::time::Duration;

fn benchmark_signature_generation(c: &mut Criterion) {
    c.bench_function("generate_signature", |b| {
        let (secret_key, public_key) = zk_proof::generate_keypair().unwrap();
        let private_key_hex = secret_key.to_string();
        let public_key_hex = hex::encode(public_key.serialize());
        let recipient_public_key = "04abcd1234567890abcdef";
        let amount = "100";
        let nonce = 1u64;
        
        b.iter(|| {
            zk_proof::sign_transaction(
                black_box(&private_key_hex),
                black_box(&public_key_hex),
                black_box(&recipient_public_key.to_string()),
                black_box(&amount.to_string()),
                black_box(nonce)
            ).unwrap()
        });
    });
}

fn benchmark_signature_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_verification");
    group.measurement_time(Duration::from_secs(10));
    
    // Setup
    let (secret_key, public_key) = zk_proof::generate_keypair().unwrap();
    let private_key_hex = secret_key.to_string();
    let public_key_hex = hex::encode(public_key.serialize());
    let recipient_public_key = "04abcd1234567890abcdef";
    let amount = "100";
    let nonce = 1u64;
    
    let signature = zk_proof::sign_transaction(
        &private_key_hex,
        &public_key_hex,
        &recipient_public_key.to_string(),
        &amount.to_string(),
        nonce
    ).unwrap();
    
    let validator_node = ValidatorNode::new();
    
    group.bench_function("verify", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            runtime.block_on(async {
                zk_proof::verify_transaction_signature(
                    black_box(&signature),
                    black_box(&public_key_hex),
                    black_box(recipient_public_key),
                    black_box(amount),
                    black_box(nonce),
                    black_box(validator_node.clone())
                ).await
            })
        });
    });
    
    group.finish();
}

fn benchmark_merkle_tree_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_tree");
    
    group.bench_function("insert_account", |b| {
        let mut tree = MerkleTree::new();
        let account = Account {
            public_key: vec![1, 2, 3, 4],
            public_key_hash: vec![5, 6, 7, 8],
            balance: 100,
            nonce: 0,
        };
        
        b.iter(|| {
            tree.insert_account(black_box(account.clone()));
        });
    });
    
    group.bench_function("get_balance", |b| {
        let mut tree = MerkleTree::new();
        for i in 0..1000 {
            let account = Account {
                public_key: vec![i as u8, (i + 1) as u8, (i + 2) as u8, (i + 3) as u8],
                public_key_hash: vec![5, 6, 7, 8],
                balance: 100 + i,
                nonce: 0,
            };
            tree.insert_account(account);
        }
        
        let test_key = vec![42, 43, 44, 45];
        
        b.iter(|| {
            tree.get_account_balance(black_box(&test_key))
        });
    });
    
    group.finish();
}

fn benchmark_blockchain_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("blockchain");
    
    group.bench_function("push_block", |b| {
        let mut blockchain = BlockChain::new();
        let block = Block::Transaction {
            sender: vec![1, 2, 3],
            sender_balance: 90,
            sender_nonce: 1,
            recipient: vec![4, 5, 6],
            recipient_balance: 110,
            amount: 10,
            time: 12345,
            hash: vec![],
        };
        
        b.iter(|| {
            blockchain.push_block_to_chain(black_box(block.clone()));
        });
    });
    
    group.bench_function("hash_blockchain", |b| {
        let mut blockchain = BlockChain::new();
        // Add some blocks to make it more realistic
        for i in 0..10 {
            let block = Block::Transaction {
                sender: vec![1, 2, 3],
                sender_balance: 90 - i,
                sender_nonce: i as u64,
                recipient: vec![4, 5, 6],
                recipient_balance: 110 + i,
                amount: 10,
                time: 12345 + i as u64,
                hash: vec![],
            };
            blockchain.push_block_to_chain(block);
        }
        
        b.iter(|| {
            blockchain.hash_blockchain()
        });
    });
    
    group.finish();
}

fn benchmark_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");
    
    let block = Block::Transaction {
        sender: vec![1, 2, 3],
        sender_balance: 90,
        sender_nonce: 1,
        recipient: vec![4, 5, 6],
        recipient_balance: 110,
        amount: 10,
        time: 12345,
        hash: vec![7, 8, 9],
    };
    
    group.bench_function("serialize_block", |b| {
        b.iter(|| {
            serde_json::to_string(black_box(&block)).unwrap()
        });
    });
    
    let json_str = serde_json::to_string(&block).unwrap();
    
    group.bench_function("deserialize_block", |b| {
        b.iter(|| {
            serde_json::from_str::<Block>(black_box(&json_str)).unwrap()
        });
    });
    
    group.finish();
}

fn benchmark_utxo_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_operations");
    
    // Setup test data
    let mut utxo_set = UTXOSet::new();
    let mut test_utxos = Vec::new();
    
    // Create 1000 test UTXOs
    for i in 0..1000 {
        let outpoint = OutPoint::new(vec![i as u8; 32], i % 4);
        let utxo = UTXO::new((100 + i) as u64, vec![(i % 256) as u8; 33], 1, 12345);
        utxo_set.add_utxo(outpoint.clone(), utxo.clone());
        test_utxos.push((outpoint, utxo));
    }
    
    group.bench_function("utxo_lookup", |b| {
        b.iter(|| {
            let index = black_box(42);
            utxo_set.get_utxo(&test_utxos[index].0)
        });
    });
    
    group.bench_function("utxo_contains_check", |b| {
        b.iter(|| {
            let index = black_box(42);
            utxo_set.contains(&test_utxos[index].0)
        });
    });
    
    group.bench_function("utxo_balance_calculation", |b| {
        let recipient = vec![42u8; 33];
        b.iter(|| {
            utxo_set.get_balance(black_box(&recipient))
        });
    });
    
    group.finish();
}

fn benchmark_utxo_transaction_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_transaction");
    
    group.bench_function("create_utxo_transaction", |b| {
        let input = TxInput {
            outpoint: OutPoint::new(vec![1, 2, 3], 0),
            signature: "signature".to_string(),
            public_key: vec![4, 5, 6],
        };
        let output = TxOutput {
            amount: 100,
            recipient: vec![7, 8, 9],
        };
        
        b.iter(|| {
            UTXOTransaction::new(
                black_box(vec![input.clone()]),
                black_box(vec![output.clone()]),
                black_box(12345)
            )
        });
    });
    
    group.bench_function("utxo_transaction_hash", |b| {
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
        
        b.iter(|| {
            black_box(&tx).compute_hash()
        });
    });
    
    group.finish();
}

fn benchmark_utxo_transaction_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_validation");
    group.measurement_time(Duration::from_secs(10));
    
    // Setup: Create UTXO set with test UTXOs
    let mut utxo_set = UTXOSet::new();
    let prev_outpoint = OutPoint::new(vec![1, 2, 3], 0);
    let prev_utxo = UTXO::new(1000, vec![4, 5, 6], 1, 12345);
    utxo_set.add_utxo(prev_outpoint.clone(), prev_utxo);
    
    // Create transaction spending that UTXO
    let input = TxInput {
        outpoint: prev_outpoint,
        signature: "signature".to_string(),
        public_key: vec![4, 5, 6],
    };
    let output = TxOutput {
        amount: 900,
        recipient: vec![7, 8, 9],
    };
    let tx = UTXOTransaction::new(vec![input], vec![output], 12345);
    
    group.bench_function("validate_utxo_amounts", |b| {
        b.iter(|| {
            let input_amount = black_box(&tx).total_input_amount(black_box(&utxo_set));
            let output_amount = black_box(&tx).total_output_amount();
            input_amount.unwrap_or(0) >= output_amount
        });
    });
    
    group.bench_function("apply_utxo_transaction", |b| {
        b.iter_batched(
            || {
                // Setup: Clone UTXO set for each iteration
                let set = utxo_set.clone();
                (set, tx.clone())
            },
            |(mut set, transaction)| {
                // Benchmark: Apply transaction
                set.apply_transaction(&transaction, 2).ok()
            },
            criterion::BatchSize::SmallInput
        );
    });
    
    group.finish();
}

fn benchmark_utxo_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_serialization");
    
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
    
    group.bench_function("serialize_utxo_transaction", |b| {
        b.iter(|| {
            serde_json::to_string(black_box(&tx)).unwrap()
        });
    });
    
    let json_str = serde_json::to_string(&tx).unwrap();
    
    group.bench_function("deserialize_utxo_transaction", |b| {
        b.iter(|| {
            serde_json::from_str::<UTXOTransaction>(black_box(&json_str)).unwrap()
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_signature_generation,
    benchmark_signature_verification,
    benchmark_merkle_tree_operations,
    benchmark_blockchain_operations,
    benchmark_json_serialization,
    benchmark_utxo_operations,
    benchmark_utxo_transaction_creation,
    benchmark_utxo_transaction_validation,
    benchmark_utxo_serialization
);
criterion_main!(benches);
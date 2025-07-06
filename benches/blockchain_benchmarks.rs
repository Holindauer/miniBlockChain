use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mini_block_chain::modules::{
    blockchain::{BlockChain, Block},
    merkle_tree::{MerkleTree, Account},
    zk_proof,
    validation::ValidatorNode,
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

criterion_group!(
    benches,
    benchmark_signature_generation,
    benchmark_signature_verification,
    benchmark_merkle_tree_operations,
    benchmark_blockchain_operations,
    benchmark_json_serialization
);
criterion_main!(benches);
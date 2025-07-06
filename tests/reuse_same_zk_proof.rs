use mini_block_chain::modules::{
    zk_proof,
    network,
    requests
};

use secp256k1::{PublicKey, SecretKey};
use std::io;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;
use serde_json::Value;
use std::{fs};
extern crate secp256k1;
extern crate rand;
extern crate hex;


/**
 * @test this rust code tests whether the nodes are correctly protecting against the same signature being used twice
 * This is important because if the same signature is used twice, a listener to the network could steal a valid signature
 * and use it to send a transaction on behalf of the original sender. There is a mechanism in place to prevent reuse
 * of the same signature that is tested here.
 * 
 * This rust code is called within the shell script test_scripts/proof_reuse_rejection_test.sh 
 * 
 * The script will verify that the signature is rejected if it is reused.
 * 
 */


 #[tokio::test]
async fn test_signature_reuse() {

    // Create two accounts, one for the sender and one for the recipient
    let sender_keypair: (SecretKey, PublicKey) = make_account().await;
    let recipient_keypair: (SecretKey, PublicKey) = make_account().await;

    // unpack the keypairs into strings
    let sender_private_key_str: String = sender_keypair.0.to_string();
    let sender_public_key_str: String = hex::encode(sender_keypair.1.serialize());

    let recipient_private_key_str: String = recipient_keypair.0.to_string();
    let recipient_public_key_str: String = hex::encode(recipient_keypair.1.serialize());

    // For simplicity, use nonce 0
    let nonce: u64 = 0;
    let amount = "0";

    // Sign the transaction
    let signature = zk_proof::sign_transaction(
        &sender_private_key_str,
        &sender_public_key_str,
        &recipient_public_key_str,
        &amount.to_string(),
        nonce
    ).expect("Failed to sign transaction");

    // Package the message
    let request = requests::NetworkRequest::Transaction {
        sender_public_key: sender_public_key_str,
        signature: signature,
        recipient_public_key: recipient_public_key_str,
        amount: amount.to_string(),
        nonce: nonce,
    };
    let request_json: String = serde_json::to_string(&request).unwrap();    

    // wait 2 seconds to ensure the account creation requests have been processed
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    // Send the transaction request to the network
    requests::send_json_request_to_all_ports(request_json.clone()).await; // ! NOTE: This one should pass, the signature is valid and has not been used before

    // wait 2 seconds to ensure the transaction request has been processed
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Transaction request 2
    requests::send_json_request_to_all_ports(request_json.clone()).await; // ! NOTE: This one should fail, the signature has been used before
}


/**
 * @helper that creates an account and sends the account creation request to the network and returns the keypair
 */
async fn make_account()-> (SecretKey, PublicKey) {
    // Generate a new keypair
    let (secret_key, public_key) = zk_proof::generate_keypair().unwrap();

    // Hash the public key for storage
    let public_key_hash: Vec<u8> = zk_proof::get_public_key_hash(&public_key);
    
    // Package account creation request
    let request = requests::NetworkRequest::AccountCreation {
        public_key: public_key.to_string(),
        public_key_hash: hex::encode(public_key_hash),
    };
    
    // Serialize request to JSON
    let request_json: String = serde_json::to_string(&request).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
    
    // Send the account creation request to the network
    requests::send_json_request_to_all_ports(request_json).await;

    (secret_key, public_key)
}
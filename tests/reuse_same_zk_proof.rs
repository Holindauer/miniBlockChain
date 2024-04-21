use mini_block_chain::modules::{
    zk_proof,
    network,
    requests
};

use curve25519_dalek::ristretto::RistrettoPoint;
use secp256k1::{PublicKey, SecretKey};
use std::io;
use base64;

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
 * @test this rust code tests whether the nodes are correctly protecting against the same sk_proof being used twice
 * This is important because if the same zk_proof is used twice, a listener to the network could steal valid proof
 * and use it to send a transaction on behalf of the original sender. There is a mechanism in place to prevent reuse
 * of the same proof that is tested here.
 * 
 * This rust code is called within the shell script test_scripts/proof_reuse_rejection_test.sh 
 * 
 * The script will verify that the zk_proof is rejected if it is reused.
 * 
 */



 #[tokio::test]
async fn test_zk_proof_reuse() {

    // Create two accounts, one for the sender and one for the recipient
    let sender_keypair: (SecretKey, PublicKey, RistrettoPoint) = make_account().await;
    let recipient_keypair: (SecretKey, PublicKey, RistrettoPoint) = make_account().await;

    // unpack the keypairs intp strings
    let sender_private_key_str: String = sender_keypair.0.to_string();
    let sender_public_key_str: String = sender_keypair.1.to_string();

    let recipient_private_key_str: String = recipient_keypair.0.to_string();
    let recipient_public_key_str: String = recipient_keypair.1.to_string();

    // retrieve the obscured private key for the sender
    let sender_encoded_private_key: RistrettoPoint = sender_keypair.2;

    // Proof Generation:

    // Convert the private key to two RistrettoPoints (elliptic curve points)
    let (point1, point2) = zk_proof::private_key_to_curve_points(&sender_private_key_str);

    // Assert that these points add to the sender encoded private key
    let sum_point: RistrettoPoint = point1 + point2;
    assert_eq!(sum_point, sender_encoded_private_key);

    // Base64 encode the points to send over the network
    let encoded_key_point_1: String = base64::encode(point1.compress().to_bytes());
    let encoded_key_point_2: String = base64::encode(point2.compress().to_bytes());

    // Package the message
    let request = requests::NetworkRequest::Transaction {
        sender_public_key: sender_public_key_str,
        encoded_key_curve_point_1: encoded_key_point_1,
        encoded_key_curve_point_2: encoded_key_point_2,
        recipient_public_key: recipient_public_key_str,
        amount: "0".to_string(),
    };
    let request_json: String = serde_json::to_string(&request).unwrap();    

    // wait 2 seconds to ensure the account creation requests have been processed
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    // Send the transaction request to the network
    requests::send_json_request_to_all_ports(request_json.clone()).await; // ! NOTE: This one should pass, the proof is valid and has not been used before

    // wait 2 seconds to ensure the transaction request has been processed
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Transaction request 2
    requests::send_json_request_to_all_ports(request_json.clone()).await; // ! NOTE: This one should fail, the proof has been used before
}


/**
 * @helper that creates an account and sends the account creation request to the network and returns the keypair
 */
async fn make_account()-> (SecretKey, PublicKey, RistrettoPoint) {
    // Generate a new keypair
    let (secret_key, public_key) = zk_proof::generate_keypair().unwrap();

    // Obfuscate the private key for zk-proof
    let obscured_private_key: RistrettoPoint = zk_proof::obfuscate_private_key(secret_key);
    let obfuscated_private_key_hash: Vec<u8> = zk_proof::hash_obfuscated_private_key(obscured_private_key);
    
    // Package account creation request
    let request = requests::NetworkRequest::AccountCreation {
        public_key: public_key.to_string(),
        obfuscated_private_key_hash: hex::encode(obfuscated_private_key_hash),
    };
    
    // Serialize request to JSON
    let request_json: String = serde_json::to_string(&request).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
    
    // Send the account creation request to the network
    requests::send_json_request_to_all_ports(request_json).await;

    (secret_key, public_key, obscured_private_key)
 }


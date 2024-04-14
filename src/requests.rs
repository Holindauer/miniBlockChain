use curve25519_dalek::ristretto::RistrettoPoint;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;
use std::{io, fs};
extern crate secp256k1;
use secp256k1::{SecretKey, PublicKey};
extern crate rand;
use crate::constants::{INTEGRATION_TEST, VERBOSE_STACK};
extern crate hex;
use base64;
use crate::zk_proof;
use crate::network::NetworkConfig;

/**
 * @notice requests.rs contains functions for sending different types of requests to the blockchain network. The three 
 * basic types of request are: account creation, transaction, and faucet requests. Upon recieving one of thee requests,
 * the peer to peer network of validator nodes will process, validate, and reach a consensus over how to handle the 
 * request. The network will then update the blockchain and merkle tree accordingly. 
 */


/**
 * @notice AccountCreationRequest encapsulate the details of a request to create a new account on the blockchain
 *        network. This includes the public key of the account, the obfuscated elliptic curve private key hash.
*/
#[derive(Serialize, Deserialize)]
pub struct AccountCreationRequest {
    pub action: String,
    pub public_key: String,
    pub obfuscated_private_key_hash: String,
}

/**
 * @notice KeyPair encapsulate a new private and public key generated for a new 
 * blockchain account for the purpose of sending to other nodes in the network.
*/
  #[derive(Serialize, Deserialize)]
  pub struct TransactionRequest {
      pub action: String,
      pub sender_public_key: String,     
      pub sender_obfuscated_private_key_part1: String,
      pub sender_obfuscated_private_key_part2: String,
      pub recipient_public_key: String,
      pub amount: String,
 }
/**
 * @notice FaucetRequest encapsulate the details of a request to use the faucet
*/
 #[derive(Serialize, Deserialize)]
 pub struct FaucetRequest {
     pub action: String,
     pub public_key: String,
}

/**
 * @notice NewAccountDetailsTestOutput encapsulate the details of a new account created on the blockchain 
 * for the purpose of printing these outputs to terminal during testing for validation/use in other tests
 */
#[derive(Serialize, Deserialize)]
struct NewAccountDetailsTestOutput {
    secret_key: String,
    public_key: String,
}
   
/**
 * @notice send_account_creation_msg() asynchonously creates a new private/public keypair, creates the 
 * obfuscated private key hash, and sends the account creation request to the network as a json object.
 */
pub async fn send_account_creation_request(){
    if VERBOSE_STACK { println!("account_creation::send_account_creation_request() : Sending account creation request...") };

    // Generate a new keypair
    let (secret_key, public_key) = zk_proof::generate_keypair().unwrap();

    // Obfuscate the private key for zk-proof
    let obscured_private_key: RistrettoPoint = zk_proof::obfuscate_private_key(secret_key);
    let obfuscated_private_key_hash: Vec<u8> = zk_proof::hash_obfuscated_private_key(obscured_private_key);

    // Package account creation request
    let request: AccountCreationRequest = AccountCreationRequest {
        action: "make".to_string(),
        public_key: public_key.to_string(),
        obfuscated_private_key_hash: hex::encode(obfuscated_private_key_hash),
    };

    // Serialize request to JSON
    let request_json = serde_json::to_string(&request).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();

    // Send the account creation request to the network
    send_json_request(request_json).await;

    // print hunman readable account details
    if VERBOSE_STACK { print_human_readable_account_details(&secret_key, &public_key); }
    if INTEGRATION_TEST { save_new_account_details_json(&secret_key.to_string(), &public_key.to_string()); }
}


 /**
 * @notice send_transcation_request() asynchonously packages a transaction request and sends it to the network.
 * @dev The sender's private key is split into two parts, each multiplied by the generator point of the curve25519
 *      elliptic curve. The points are base64 encoded and sent to the network along w/ other transaction details.
 */
pub async fn send_transaction_request(sender_private_key: String, recipient_public_key: String, amount: String ) {
    if VERBOSE_STACK { println!("send_transaction::send_transaction_request() : Sending transaction request...") };

    // derive the public key from the private key
    let sender_public_key: String = zk_proof::derive_public_key_from_private_key(&sender_private_key);

    // Convert the private key to two RistrettoPoints (elliptic curve points)
    let (point1, point2) = zk_proof::private_key_to_curve_points(&sender_private_key);

    // Base64 encode the points to send over the network
    let encoded_key_point_1: String = base64::encode(point1.compress().to_bytes());
    let encoded_key_point_2: String = base64::encode(point2.compress().to_bytes());

    // Package the message
    let request: TransactionRequest = TransactionRequest {
        action: "transaction".to_string(),
        sender_public_key,
        sender_obfuscated_private_key_part1: encoded_key_point_1,
        sender_obfuscated_private_key_part2: encoded_key_point_2,
        recipient_public_key,
        amount,
    };
    let request_json: String = serde_json::to_string(&request).unwrap();    

    // Send the transaction request to the network
    send_json_request(request_json).await;
}


/**
 * @notice send_faucet_request() sends a request to the network to provide a given public key with a small amount of tokens
 */
pub async fn send_faucet_request(public_key: String)  {
    if VERBOSE_STACK { println!("faucet::send_faucet_request() : Sending faucet request..."); }

    // Package the message for network transmission
    let request: FaucetRequest = FaucetRequest {
        action: "faucet".to_string(),
        public_key: public_key.to_string(),
    };
    let request_json: String = serde_json::to_string(&request).unwrap();

    // Send the faucet request to the network
    send_json_request(request_json).await;
}

/**
 * @notice send_json_request() sends a json request to the network
 */
async fn send_json_request(request_json: String) {

    // Load accepted ports configuration
    let config_data: String = fs::read_to_string("accepted_ports.json").map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
    let config: NetworkConfig = serde_json::from_str(&config_data).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
      
    // Send account creation request to all accepted po
    for node in config.nodes.iter() {
        let addr = format!("{}:{}", node.address, node.port);
        if let Ok(mut stream) = TcpStream::connect(&addr).await {
            let _ = stream.write_all(request_json.as_bytes()).await;
        }
    }
}




//------------------------------------ Helper Functions ------------------------------------//

/**
 * @notice print_human_readable_account_details() prints the details of a new account created on the blockchain
 * network in human readable format.
 */
fn print_human_readable_account_details(secret_key: &SecretKey, public_key: &PublicKey) {
    println!("Account details sucessfully created: ");
    println!("Secret Key: {:?}", secret_key.to_string());
    println!("Public Key: {:?}", public_key.to_string());
}

/**
 * @notice save_new_account_details_json() saves a json string of the details of a new account created on the blockchain
 * network to the terminal. This is used during integration testing to save the output of the account creation process.
 */
fn save_new_account_details_json(private_key: &String, public_key: &String) {

    // Package the message into a NewAccountDetailsTestOutput struct
    let message: NewAccountDetailsTestOutput = NewAccountDetailsTestOutput {
        secret_key: private_key.to_string(),
        public_key: public_key.to_string(),
    };

    // Save the account details to a json file
    let message_json: String = serde_json::to_string(&message).unwrap();
    std::fs::write("new_account_details.json", message_json).unwrap();
}



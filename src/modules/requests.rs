use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;
use serde_json::Value;
use std::{io, fs};
extern crate secp256k1;
use secp256k1::{SecretKey, PublicKey};
extern crate rand;
use crate::modules::constants::INTEGRATION_TEST;
extern crate hex;

use crate::modules::zk_proof;
use crate::modules::network::NetworkConfig;
use crate::modules::network;
use crate::modules::validation::ValidatorNode;


/**
 * @notice requests.rs contains functions for sending different types of requests to the blockchain network. Upon 
 * recieving as request, the peer-to-peer network of validator nodes will process, validate, and reach a consensus 
 * over how to handle the request. The network will then update the blockchain and merkle tree accordingly. 
 */


 /**
 * @notice NetworkRequest is an enum that encapsulates the different types of requests that can be sent to the network.
 * The enum is serialized and deserialized to JSON for transmission over the network. The different types of requests
 * include AccountCreation, Transaction, Faucet, ConsensusRequest, HeartBeat, and PeerLedgerRequest.
 * @dev the 'action' tag is used to specify the type of request based on the 'action' field. This is used by the
 * network::master_event_handler() to filter the recieved, serialized version of this struct into the correct variant
 * event handler.    
*/
 #[derive(Serialize, Deserialize, Clone, Debug)]
 #[serde(tag = "action")] // Adding a tag to specify the type of request based on the 'action' field
 pub enum NetworkRequest {
     AccountCreation {
         public_key: String,
         public_key_hash: String,
     },
     Transaction {
         sender_public_key: String,
         signature: String,
         recipient_public_key: String,
         amount: String,
         nonce: u64,
     },
     Faucet {
         public_key: String,
     },
     ConsensusRequest{ 
        request_hash: Vec<u8>,
        response_port: String,
    },
    HeartBeat{
        port_address: String,
    },
    PeerLedgerRequest{
        response_port: String,
    }
 }


/**
 * @notice NewAccountDetailsTestOutput encapsulate the details of a new account created on the blockchain 
 * for integration testing. The struct is serialized and deserialized to JSON for saving the account details.
 */
#[derive(Serialize, Deserialize)]
struct NewAccountDetailsTestOutput {
    secret_key: String,
    public_key: String,
}
   
/**
 * @notice send_account_creation_msg() asynchonously creates a new private/public keypair, obfuscates the private key 
 * using elliptic curve cryptography, and sends the public key and obfuscated private key hash to the network.
 */
pub async fn send_account_creation_request(){
    println!("Sending Account Creation Request...");

    // Generate a new keypair
    let (secret_key, public_key) = zk_proof::generate_keypair().unwrap();

    // Hash the public key for storage
    let public_key_hash: Vec<u8> = zk_proof::get_public_key_hash(&public_key);

    // Package account creation request
    let request = NetworkRequest::AccountCreation {
        public_key: public_key.to_string(),
        public_key_hash: hex::encode(public_key_hash),
    };

    // Serialize request to JSON
    let request_json = serde_json::to_string(&request).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();

    // Send the account creation request to the network
    send_json_request_to_all_ports(request_json).await;

    // print hunman readable account details
    print_human_readable_account_details(&secret_key, &public_key);
    if INTEGRATION_TEST { 
        save_new_account_details_json(&secret_key.to_string(), &public_key.to_string()); 
    }
}


 /**
 * @notice send_transcation_request() sends a request to the network to transfer a given amount of tokens from one account to another.
 * The request includes the public key of the sender, the public key of the recipient, and the amount of tokens to transfer.
 * @dev The sender's private key is used to derive the sender's public key and sign the transaction.
 * The signature proves ownership of the private key without revealing it.
 */
pub async fn send_transaction_request(sender_private_key: String, recipient_public_key: String, amount: String ) {
    println!("Sending Transaction Request...");

    // derive the public key from the private key
    let sender_public_key: String = zk_proof::derive_public_key_from_private_key(&sender_private_key);

    // For simplicity, use nonce 0. In production, this should be fetched from the network
    let nonce: u64 = 0;

    // Sign the transaction
    let signature = zk_proof::sign_transaction(
        &sender_private_key,
        &sender_public_key,
        &recipient_public_key,
        &amount,
        nonce
    ).expect("Failed to sign transaction");

    // Package the message
    let request = NetworkRequest::Transaction {
        sender_public_key,
        signature,
        recipient_public_key,
        amount,
        nonce,
    };
    let request_json: String = serde_json::to_string(&request).unwrap();    

    // Send the transaction request to the network
    send_json_request_to_all_ports(request_json).await;
}

/**
 * @notice send_faucet_request() sends a request to the network to provide a given public key with a small amount of tokens
 */
pub async fn send_faucet_request(public_key: String)  {
    println!("Sending Faucet Request...");

    // Package the message for network transmission
    let request = NetworkRequest::Faucet { public_key: public_key.to_string(), };
    let request_json: String = serde_json::to_string(&request).unwrap();

    // Send the faucet request to the network
    send_json_request_to_all_ports(request_json).await;
}

/**
 * @notice send_block_consensus_request() asynchronously sends a request to all other validator nodes for their decision 
 * on whether or not to accept a new block into the blockchain. The function uses the hash of the request recieved by the 
 * client as a unique identifier in the request sent to other nodes. Recieved responces will be handled by the master event 
 * handler in the network module. Once all are in, the client will proceed with determining a majority decision.
*/
pub async fn send_consensus_request( request: Value, validator_node: ValidatorNode )  {
    println!("Sending request to peers for their independent decisions...");

    // extract the port number form the validator node
    let client_port: String = validator_node.client_port_address.clone();

    // get hash of request recieved by client, (used as key)
    let client_request_hash: Vec<u8> = network::hash_network_request(request.clone()).await;

    // Package peer request in struct and serialize to JSON
    let consensus_request = NetworkRequest::ConsensusRequest {
        request_hash: client_request_hash.clone(),
        response_port: client_port.clone()
    };

    // Serialize request to JSON
    let request_json: String = serde_json::to_string(&consensus_request).unwrap();

    // Send request to all outbound ports
    send_json_request_to_other_nodes(request_json, validator_node).await;
}


/**
 * @notice send_peer_ledger_request() sends a request to all currently active nodes for a copy of their local ledger state.
 * @dev This function is called when a new node joins the network and needs to sync its local ledger state with the rest of the network.
 */
pub async fn send_peer_ledger_request(validator_node: ValidatorNode){

    // get client port form validator node
    let client_port: String = validator_node.client_port_address.clone();

    // Package peer request in struct and serialize to JSON
    let peer_ledger_request = NetworkRequest::PeerLedgerRequest {
        response_port: client_port.clone()
    }; 

    // Serialize request to JSON
    let request_json: String = serde_json::to_string(&peer_ledger_request).unwrap();

    // Send request to all outbound ports
    send_json_request_to_other_nodes(request_json, validator_node.clone()).await;
}


/**
 * @notice send_heartbeat() is an asynchronous process that is blocked by start_listening() after the succesfull connection of a listener 
 * to the network. A heartbeat signal is sent every constants::HEARTBEAT_PERIOD seconds to the network to indicate that the node is still 
 * active and responces should be expected from the port_address in the HeartBeat msg folllowing a consensus request. 
 */
pub async fn send_heartbeat_request(validator_node: ValidatorNode) {
    println!("\nSending HeartBeat...");

    // get client port and outbound ports
    let client_port: String = validator_node.client_port_address.clone();

    // package and serialize the heartbeat signal
    let heartbeat = NetworkRequest::HeartBeat { port_address: client_port.clone() };
    let heartbeat_json: String = serde_json::to_string(&heartbeat).unwrap();

    // Send the heartbeat signal to all outbound ports
    send_json_request_to_other_nodes(heartbeat_json, validator_node).await
}

//------------------------------------ Helper Functions ------------------------------------//

/**
 * @notice send_json_request() sends a json request to all accepted ports on the network
 */
pub async fn send_json_request_to_all_ports(request_json: String) {

    // Load accepted ports configuration
    let config_data: String = fs::read_to_string("accepted_ports.json").map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
    let config: NetworkConfig = serde_json::from_str(&config_data).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
      
    // Send account creation request to all accepted po
    for node in config.nodes.iter() {
        let addr: String = format!("{}:{}", node.address, node.port);
            if let Ok(mut stream) = TcpStream::connect(&addr).await {
                let _ = stream.write_all(request_json.as_bytes()).await;
            }
    }
}

/**
 * @notice send_json_request() sends a json request to all accepted ports on 
 * the network that are not the client port stored in the validator node
 */
pub async fn send_json_request_to_other_nodes(request_json: String, validator_node: ValidatorNode) {

    // Retrieve the client port address
    let client_port: String = validator_node.client_port_address.clone();

    // Load accepted ports configuration
    let config_data: String = fs::read_to_string("accepted_ports.json").map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
    let config: NetworkConfig = serde_json::from_str(&config_data).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();

    // Send account creation request to all accepted po
    for node in config.nodes.iter() {
        let addr: String = format!("{}:{}", node.address, node.port);

        // Only send the request to other nodes
        if client_port != addr {
            if let Ok(mut stream) = TcpStream::connect(&addr).await {
                let _ = stream.write_all(request_json.as_bytes()).await;
            }
        }
    }
}

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



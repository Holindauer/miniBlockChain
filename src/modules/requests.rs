use curve25519_dalek::ristretto::RistrettoPoint;
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
use base64;

use crate::modules::zk_proof;
use crate::modules::network::NetworkConfig;
use crate::modules::network;
use crate::modules::validation::ValidatorNode;
/**
 * @notice requests.rs contains functions for sending different types of requests to the blockchain network. The three 
 * basic types of request are: account creation, transaction, and faucet requests. Upon recieving one of thee requests,
 * the peer to peer network of validator nodes will process, validate, and reach a consensus over how to handle the 
 * request. The network will then update the blockchain and merkle tree accordingly. 
 */


 #[derive(Serialize, Deserialize, Clone, Debug)]
 #[serde(tag = "action")] // Adding a tag to specify the type of request based on the 'action' field
 pub enum NetworkRequest {
     AccountCreation {
         public_key: String,
         obfuscated_private_key_hash: String,
     },
     Transaction {
         sender_public_key: String,
         encoded_key_curve_point_1: String,
         encoded_key_curve_point_2: String,
         recipient_public_key: String,
         amount: String,
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
    println!("Sending Account Creation Request...");

    // Generate a new keypair
    let (secret_key, public_key) = zk_proof::generate_keypair().unwrap();

    // Obfuscate the private key for zk-proof
    let obscured_private_key: RistrettoPoint = zk_proof::obfuscate_private_key(secret_key);
    let obfuscated_private_key_hash: Vec<u8> = zk_proof::hash_obfuscated_private_key(obscured_private_key);

    // Package account creation request
    let request = NetworkRequest::AccountCreation {
        public_key: public_key.to_string(),
        obfuscated_private_key_hash: hex::encode(obfuscated_private_key_hash),
    };

    // Serialize request to JSON
    let request_json = serde_json::to_string(&request).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();

    // Send the account creation request to the network
    send_json_request_to_all_ports(request_json).await;

    // print hunman readable account details
    print_human_readable_account_details(&secret_key, &public_key);
    if INTEGRATION_TEST { save_new_account_details_json(&secret_key.to_string(), &public_key.to_string()); }
}


 /**
 * @notice send_transcation_request() asynchonously packages a transaction request and sends it to the network.
 * @dev The sender's private key is split into two parts, each multiplied by the generator point of the curve25519
 *      elliptic curve. The points are base64 encoded and sent to the network along w/ other transaction details.
 */
pub async fn send_transaction_request(sender_private_key: String, recipient_public_key: String, amount: String ) {
    println!("Sending Transaction Request...");

    // derive the public key from the private key
    let sender_public_key: String = zk_proof::derive_public_key_from_private_key(&sender_private_key);

    // Convert the private key to two RistrettoPoints (elliptic curve points)
    let (point1, point2) = zk_proof::private_key_to_curve_points(&sender_private_key);

    // Base64 encode the points to send over the network
    let encoded_key_point_1: String = base64::encode(point1.compress().to_bytes());
    let encoded_key_point_2: String = base64::encode(point2.compress().to_bytes());

    // Package the message
    let request = NetworkRequest::Transaction {
        sender_public_key,
        encoded_key_curve_point_1: encoded_key_point_1,
        encoded_key_curve_point_2: encoded_key_point_2,
        recipient_public_key,
        amount,
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
 * @notice send_block_consensus_request() asynchronously sends a request to all other validator nodes for their decision on whether or not to 
 * accept a new block into the blockchain. The function uses the hash of the request recieved by the client as a unique identifier in the request
 * sent to other nodes. Recieved responces will be handled by the main listener loop in validation module. Which will collect the responces and
 * stored them in the peer_consensus_decisions arc mutex hash map. These responces will accessed by the determine_majority() function to determine
 * the majority decision of the network.
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
 * @notice send_peer_ledger_request() sends a request to all currently 
 * active nodes for a copy of their local ledger state.
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
async fn send_json_request_to_all_ports(request_json: String) {

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


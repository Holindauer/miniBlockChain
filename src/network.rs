use std::io::Error as IoError;
use std::fs;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;
use tokio::io::Error as TokioIoError; 
use tokio::sync::Mutex;
use tokio::time;

use serde::{Serialize, Deserialize};
use serde_json::Value;

use sha2::{Digest, Sha256};

use crate::validation;
use crate::validation::ValidatorNode;
use crate::constants::{INTEGRATION_TEST, HEARTBEAT_PERIOD, HEARTBEAT_TIMEOUT};
use crate::consensus;
use crate::blockchain::{save_most_recent_block_json, print_chain_human_readable};
use crate::requests;


/**
 * @notice network.rs contains functions related to generali configuration of the network
 */

 /**
 * @notice the following structs are used to load in the accepted_ports.json file which contains a llist
 * of accepted ports for the network. When a node is booted up, if the port cannot connnect to the network,
 * an excpetion will be thrown and handled by attempting to connect to the next port in the list.
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
   pub nodes: Vec<PortConfig>,
}

/**
 * @notice PortConfig is within the process of deserializing the accepted_ports.json file into a Vec<NodeConfig> struct.
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct PortConfig {
   pub id: String,
   pub address: String,
   pub port: u16,
}


 /**
 * @notice try_bind_to_ports() is an asynchronous function that attempts to bind to the ports specified in the
 * accepted_ports.json file. If the function is successful, it will return a TcpListener that is bound to the
 * first available port. If the function is unsuccessful, it will return an IoError.
*/
pub async fn try_bind_to_ports() -> Result<(TcpListener, String), IoError> {
    println!("network::try_bind_to_ports() : Attempting to bind to ports specified in accepted_ports.json...");

    // Load the accepted ports configuration file
    let config_data = match fs::read_to_string("accepted_ports.json") {
        Ok(data) => data,
        Err(e) => {return Err(IoError::new(std::io::ErrorKind::Other, e)); }  // Error here
    };

    // Parse the configuration file into a Config struct
    let config: NetworkConfig = match serde_json::from_str(&config_data) {
        Ok(config) => config,
        Err(e) => { return Err(IoError::new(std::io::ErrorKind::Other, e)); }
    };

    // error is not bad in this case, and are expected for ports w/ no listners
    let mut last_error = None;

    // Attempt to bind to each port in the configuration
    for node in &config.nodes {
        println!("validation::try_bind_to_ports() : Attempting to bind to port {}...", node.port); 

        // format the address and port into a string
        let port_address: String = format!("{}:{}", node.address, node.port);

        // Attempt to bind to the address and port
        match TcpListener::bind(port_address.clone()).await {
            
            Ok(listener) => return Ok((listener, port_address.clone())), // return the listener if successful
            Err(e) => last_error = Some(e),
        }
    }

    Err(last_error.unwrap_or_else(|| IoError::new(std::io::ErrorKind::Other, "No ports available")))
}

/**
 * @notice listen_for_connections() asynchronously listens for incoming connections on the specified address. It will spawn 
 * new tasks to handle each incoming connection. Messages to the network are passed of to handle_incoming_message() for processing.
 */
pub async fn start_listening(validator_node: ValidatorNode) {


    // Attempt to bind to one of the ports specified in the accepted_ports.json config file
    let (listener, client_port_address) = match try_bind_to_ports().await {

        Ok(result) => { println!("validation::start_listening() : Listening on `{}...`", result.1); result },
        Err(e) => { eprintln!("Refused to bind to any configured port: {}", e); return; }
    };       

    // set the client port address in the validator node master struct 
    let mut validator_node = validator_node;
    validator_node.client_port_address = client_port_address.clone();

    // Start a separate task for sending heartbeats
    let validator_node_clone = validator_node.clone();
    tokio::spawn(async move {
        send_heartbeat_periodically(validator_node_clone).await;
    });

    // Listen for incoming connections
    while let Ok((mut socket, _)) = listener.accept().await {

        // Spawn a new task to handle the incoming message
        let validator_node_clone = validator_node.clone();
        tokio::spawn(async move {
            
            // Read the incoming message into a buffer and pass into the master event handler
            let mut buffer = Vec::new();
            if socket.read_to_end(&mut buffer).await.is_ok() && !buffer.is_empty() {
                handle_incoming_message(&buffer, validator_node_clone).await;
            }
        });
    }
}


/**
 * @notice send_heartbeat_periodically() is an asynchronous function that 
 * sends a heartbeat signal to the network every HEARTBEAT_PERIOD seconds.
 */
async fn send_heartbeat_periodically(validator_node: ValidatorNode) {
    let mut interval = time::interval(HEARTBEAT_PERIOD);
    loop {
        interval.tick().await;
        requests::send_heartbeat_request(validator_node.clone()).await;
    }
}


/**
 * @notice handle_incoming_message() asynchronously accepts a msg buffer and the current state of the merkle tree 
 * and blockchain. The buffer is parsed and the next step for the request is determined from the msg contents. 
 */
async fn handle_incoming_message( buffer: &[u8], validator_node: ValidatorNode ) {
    println!("\nvalidation::handle_incoming_message()...");

    // convert the buffer to a string 
    let msg = String::from_utf8_lossy(&buffer[..buffer.len()]);

    // After parsing to JSON determine what to do with the msg
    if let Ok(request) = serde_json::from_str::<Value>(&msg) {
        let request_action: Option<&str> = request["action"].as_str(); // Extract request action from JSON        
        
        // Determine the action to take based on the request
        match request_action {

            Some("AccountCreation") => { // Handle Request to Make New Account
                
                match validation::handle_account_creation_request( request, validator_node.clone() ).await {  

                    Ok(public_key) => { // upon succesfull account creation, print blockchain state, save most recent block for integration testing
                        print_chain_human_readable(validator_node.blockchain.clone()).await;
                        if INTEGRATION_TEST { save_most_recent_block_json(validator_node.blockchain.clone()).await; } // TODO move these functions out of the validation module and into the blockchain module
                    },
                    Err(e) => {eprintln!("Account creation Invalid: {}", e);}
                }
            },
            Some("Transaction") => { // Handle Request to Make New Transaction
                
                match validation::handle_transaction_request(request, validator_node.clone()).await {
                    Ok(success) => {
    
                        // upon succesfull transaction, print blockchain state or indicate transaction refusall
                        if success { print_chain_human_readable(validator_node.blockchain.clone()).await;}
                        else { eprintln!("Transaction failed to verify"); }

                        // if doing an integration test, save the most recent block as a json file
                        if INTEGRATION_TEST { 
                            save_most_recent_block_json(validator_node.blockchain.clone()).await;
                            if !success { validation::save_failed_transaction_json().await; }
                        } 
                    },
                    Err(e) => {eprintln!("Transaction Validation Error: {}", e);}
                }
            },
            Some("Faucet") => { // Handle Request to Use Faucet
                                    
                match validation::handle_faucet_request(request, validator_node.clone()).await {
                    Ok(_) => { 

                        // upon succesfull faucet request, print blockchain state
                        print_chain_human_readable(validator_node.blockchain.clone()).await;
                        if INTEGRATION_TEST { save_most_recent_block_json(validator_node.blockchain.clone()).await; } // save latest block for integration testing
                    },
                    Err(e) => { eprintln!("Faucet request failed: {}", e); }
                }

            },
            Some("ConsensusRequest") => { // Handle New Block Decision Request

                println!("Block Consensus Request Recieved...");
                consensus::handle_consensus_request( request, validator_node.clone()).await 
            },
            Some("ConsensusResponse") => { // Handle New Block Decision Request

                println!("Block Consensus Response Recieved...");
                consensus::handle_consensus_response( request, validator_node.clone()).await 
            },
            Some("HeartBeat") => { // Handle Heartbeat Request

                println!("Heartbeat Request Recieved...");
                match validation::handle_heartbeat( request, validator_node.clone()).await {
                    Ok(_) => { println!("Heartbeat Request Handled..."); },
                    Err(e) => { eprintln!("Heartbeat Request Failed: {}", e); }
                }
            },
            _ => eprintln!("Unrecognized action: {:?}", request_action),
        }
    } else {eprintln!("Failed to parse message: {}", msg);}
}


/**
 * @notice hash_network_request() uses Sha256 to hash a serde_json::Value that contains that contains network request information
 */
pub async fn hash_network_request(request_struct_json: Value) -> Vec<u8> {
    println!("network::hash_network_request()...");

    // use SHA256 to hash the request
    let mut hasher = Sha256::new();
    hasher.update(request_struct_json.to_string());

    // return finalized Vec<u8> hash
    hasher.finalize().to_vec()
}

/**
 * @notice collect_outbound_ports() is an asynchronous function that reads the configuration file containing the accepted 
 * ports of the network. All ports that are not the port of the client are collected and returned as a vector of strings.
 */
pub async fn collect_outbound_ports(self_port: String) -> Result<Vec<String>, TokioIoError> {
    println!("network::collect_outbound_ports()...");

    // Asynchronously load the accepted ports configuration file
    let config_data = tokio::fs::read_to_string("accepted_ports.json").await?;

    // Parse the configuration file into a Config struct
    let config: NetworkConfig = serde_json::from_str(&config_data)
        .map_err(|e| TokioIoError::new(std::io::ErrorKind::Other, format!("Failed to parse configuration file: {}", e)))?;

    // Collect all outbound ports
    let outbound_ports: Vec<String> = config.nodes.iter()
        .map(|port| format!("{}:{}", port.address, port.port))
        .filter(|port_address| port_address != &self_port)
        .collect();

    Ok(outbound_ports)
}


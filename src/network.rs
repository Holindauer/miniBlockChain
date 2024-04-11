use std::io::Error as IoError;
use tokio::net::TcpListener;
use std::fs;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::io::AsyncReadExt;

use crate::validation;
use crate::validation::ValidatorNode;
use crate::constants::{VERBOSE_STACK, INTEGRATION_TEST};
use crate::block_consensus;

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
    if VERBOSE_STACK { println!("network::try_bind_to_ports() : Attempting to bind to ports specified in accepted_ports.json..."); }

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
        if VERBOSE_STACK { println!("validation::try_bind_to_ports() : Attempting to bind to port {}...", node.port); }

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
pub fn start_listening(validator_node: ValidatorNode) {

    // Establish a new tokio runtime
    let rt = Runtime::new().unwrap();

    // Spawn a new task to listen for incoming connections 
    rt.block_on(async {

        // Attempt to bind to one of the ports specified in the accepted_ports.json config file
        let (listener, client_port_address) = match try_bind_to_ports().await {

            Ok(result) => { // indicate success and return the listener and port address in a tuple
                if VERBOSE_STACK { println!("validation::start_listening() : Listening for incoming connection on`{}...`", result.1); } result },

            Err(e) => { eprintln!("Failed to bind to any configured port: {}", e); return; }
        };       

        // set the client port address in the validator node master struct 
        let mut validator_node = validator_node;
        validator_node.client_port_address = client_port_address.clone();

        // Listen for incoming connections
        while let Ok((mut socket, _)) = listener.accept().await {

            // Clone the validator node for use in the new task
            let validator_node_clone = validator_node.clone();

            // Spawn a new task to handle the incoming connection
            tokio::spawn(async move {

            
                // Read the incoming message from the socket
                let mut buffer: Vec<u8> = Vec::new();
                if socket.read_to_end(&mut buffer).await.is_ok() && !buffer.is_empty() {

                    // Handle the incoming message
                    handle_incoming_message( &buffer, validator_node_clone).await; 
                }
            });
        }
    });
}

/**
 * @notice handle_incoming_message() asynchronously accepts a msg buffer and the current state of the merkle tree 
 * and blockchain. The buffer is parsed and the next step for the request is determined from the msg contents. 
 */
async fn handle_incoming_message( buffer: &[u8], validator_node: ValidatorNode ) {
    if VERBOSE_STACK { println!("\nvalidation::handle_incoming_message() : Handling incoming message...") };

    // convert the buffer to a string 
    let msg = String::from_utf8_lossy(&buffer[..buffer.len()]);

    // After parsing to JSON determine what to do with the msg
    if let Ok(request) = serde_json::from_str::<Value>(&msg) {
        let request_action: Option<&str> = request["action"].as_str(); // Extract request action from JSON        
        
        // Determine the action to take based on the request
        match request_action {

            Some("make") => { // Handle Request to Make New Account
                
                match validation::handle_account_creation_request( request, validator_node.clone() ).await {  

                    Ok(public_key) => { // upon succesfull account creation, print blockchain state, save most recent block for integration testing
                        if VERBOSE_STACK { validation::print_chain_human_readable(validator_node.blockchain.clone()).await;}  
                        if INTEGRATION_TEST { validation::save_most_recent_block_json(validator_node.blockchain.clone()).await; } // TODO move these functions out of the validation module and into the blockchain module
                    },
                    Err(e) => {eprintln!("Account creation Invalid: {}", e);}
                }
            },
            Some("transaction") => { // Handle Request to Make New Transaction
                
                match validation::handle_transaction_request(request, validator_node.clone()).await {
                    Ok(success) => {
    
                        // upon succesfull transaction, print blockchain state or indicate transaction refusall
                        if VERBOSE_STACK {
                            if success { validation::print_chain_human_readable(validator_node.blockchain.clone()).await;}
                            else { eprintln!("Transaction failed to verify"); }
                        }                       
    
                        // if doing an integration test, save the most recent block as a json file
                        if INTEGRATION_TEST { 
                            validation::save_most_recent_block_json(validator_node.blockchain.clone()).await;
                            if !success { validation::save_failed_transaction_json().await; }
                        } 
                    },
                    Err(e) => {eprintln!("Transaction Validation Error: {}", e);}
                }
            },
            Some("faucet") => { // Handle Request to Use Faucet
                                    
                match validation::handle_faucet_request(request, validator_node.clone()).await {
                    Ok(_) => { 

                        // upon succesfull faucet request, print blockchain state
                        if VERBOSE_STACK { validation::print_chain_human_readable(validator_node.blockchain.clone()).await;} 
                        if INTEGRATION_TEST { validation::save_most_recent_block_json(validator_node.blockchain.clone()).await; } // save latest block for integration testing
                    },
                    Err(e) => { eprintln!("Faucet request failed: {}", e); }
                }

            },
            Some("block_consensus") => { // Handle New Block Decision Request

                println!("Block Consensus Request Recieved...");
                match block_consensus::handle_block_consensus_request( request, validator_node.clone()).await {
                    Ok(_) => { println!("Block Consensus Request Handled..."); },
                    Err(e) => { eprintln!("Block Consensus Request Failed: {}", e); }
                }
            },
            _ => eprintln!("Unrecognized action: {:?}", request_action),
        }
    } else {eprintln!("Failed to parse message: {}", msg);}
}
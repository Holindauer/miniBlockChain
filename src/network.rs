use std::io::Error as IoError;
use tokio::net::TcpListener;
use std::fs;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, MutexGuard};
use tokio::net::TcpStream;
use tokio::io::Error as TokioIoError; 
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use std::sync::Arc;
use sha2::{Digest, Sha256};

use crate::validation;
use crate::validation::ValidatorNode;
use crate::constants::{VERBOSE_STACK, INTEGRATION_TEST, HEARTBEAT_PERIOD, HEARTBEAT_TIMEOUT};
use crate::block_consensus;
use crate::blockchain::{save_most_recent_block_json, print_chain_human_readable};


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

            Err(e) => { eprintln!("Refused to bind to any configured port: {}", e); return; }
        };       

        // init time of last heartbeat signal
        let mut last_heartbeat = std::time::Instant::now();

        // set the client port address in the validator node master struct 
        let mut validator_node = validator_node;
        validator_node.client_port_address = client_port_address.clone();

        // Listen for incoming connections
        while let Ok((mut socket, _)) = listener.accept().await {

            // Clone the validator node for use in the new task
            let validator_node_clone = validator_node.clone();

            // Spawn a new task to handle the incoming connection
            tokio::spawn(async move {

                // Read the incoming message from the socket and sender to master handler
                let mut buffer: Vec<u8> = Vec::new();
                if socket.read_to_end(&mut buffer).await.is_ok() && !buffer.is_empty() {
                    handle_incoming_message( &buffer, validator_node_clone).await; 
                }
            });

            // If the time since the last heartbeat signal is greater than HEARTBEAT_PERIOD, send a new heartbeat signal
            if last_heartbeat.elapsed() > HEARTBEAT_PERIOD {
                send_heartbeat(validator_node.clone()).await;
                last_heartbeat = std::time::Instant::now(); // 
            }
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
                        if VERBOSE_STACK { print_chain_human_readable(validator_node.blockchain.clone()).await;}  
                        if INTEGRATION_TEST { save_most_recent_block_json(validator_node.blockchain.clone()).await; } // TODO move these functions out of the validation module and into the blockchain module
                    },
                    Err(e) => {eprintln!("Account creation Invalid: {}", e);}
                }
            },
            Some("transaction") => { // Handle Request to Make New Transaction
                
                match validation::handle_transaction_request(request, validator_node.clone()).await {
                    Ok(success) => {
    
                        // upon succesfull transaction, print blockchain state or indicate transaction refusall
                        if VERBOSE_STACK {
                            if success { print_chain_human_readable(validator_node.blockchain.clone()).await;}
                            else { eprintln!("Transaction failed to verify"); }
                        }                       
    
                        // if doing an integration test, save the most recent block as a json file
                        if INTEGRATION_TEST { 
                            save_most_recent_block_json(validator_node.blockchain.clone()).await;
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
                        if VERBOSE_STACK { print_chain_human_readable(validator_node.blockchain.clone()).await;} 
                        if INTEGRATION_TEST { save_most_recent_block_json(validator_node.blockchain.clone()).await; } // save latest block for integration testing
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
            Some("heartbeat") => { // Handle Heartbeat Request

                println!("Heartbeat Request Recieved...");
                match handle_heartbeat( request, validator_node.clone()).await {
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



// @struct HeartBeat packages the information needed to send a heartbeat signal to the network that the node is still active.
#[derive(Debug, Serialize, Deserialize)]
struct HeartBeat{ action: String, port_address: String, }

/**
 * @notice send_heartbeat() is an asynchronous process that is blocked by start_listening() after the succesfull connection of a listener 
 * to the network. A heartbeat signal is sent every constants::HEARTBEAT_PERIOD seconds to the network to indicate that the node is still 
 * active and responces should be expected from the port_address in the HeartBeat msg folllowing a consensus request. 
 */
async fn send_heartbeat(validator_node: ValidatorNode){
    if VERBOSE_STACK { println!("network::send_heartbeat() : Sending heartbeat signals to the network..."); }

    // Establish a new tokio runtime
    let rt = Runtime::new().unwrap();

    // get client port
    let client_port: String = validator_node.client_port_address.clone();

    // Collect all outbound ports to send message to
    let outbound_ports: Vec<String> = collect_outbound_ports(client_port.clone()).await.unwrap();

    // send heartbeats to all ports not the client port
    for port in outbound_ports.iter() {
        if port != &client_port {

                        
            // Spawn a new task to send a heartbeat signal to the network
            rt.block_on(async {

                // Create a new HeartBeat struct and serialize to JSON
                let heartbeat = HeartBeat{ action: "heartbeat".to_string(), port_address: client_port.clone() };
                let heartbeat_json: String = serde_json::to_string(&heartbeat).unwrap();

                // Create a new TcpStream to send the heartbeat signal
                let mut stream = match TcpStream::connect(port).await {
                    Ok(stream) => stream, Err(e) => { eprintln!("Failed to connect to port: {}", e); return; }
                };

                // Send the heartbeat signal
                if let Err(e) = stream.write_all(heartbeat_json.as_bytes()).await {
                    eprintln!("Failed to send heartbeat to port: {} -- There may not be a listener", port);
                }
            });
         }
     }
}

/**
 * @notice handle_heartbeat_request() is an asynchronous function that handles incoming heartbeat requests from other nodes on the network.
 */
async fn handle_heartbeat(request: Value, validator_node: ValidatorNode) -> Result<(), String> {
    if VERBOSE_STACK { println!("network::handle_heartbeat_request() : Handling incoming heartbeat request..."); }

    // Extract the port address from the request
    let port_address: String = request["port_address"].as_str()
        .ok_or_else(|| "Failed to extract port address from heartbeat request".to_string())?
        .to_string();

    // Retrieve and lock the active_peers vector
    let active_peers: Arc<Mutex<Vec<(String, u64)>>> = validator_node.active_peers.clone();
    let mut active_peers = active_peers.lock().await;

    // Get the current time
    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Failed to get current time".to_string())?
        .as_secs();

    // Update the timestamp for the peer that sent the heartbeat
    for peer in &mut *active_peers {
        if peer.0 == port_address {
            peer.1 = current_time;
        }
    }

    // Remove peers that have not sent a heartbeat within the HEARTBEAT_TIMEOUT
    active_peers.retain(|peer| current_time - peer.1 < HEARTBEAT_TIMEOUT.as_secs());

    Ok(())
}
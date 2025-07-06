use std::io::Error as IoError;
use std::fs;
use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;
use tokio::time;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use crate::modules::validation;
use crate::modules::validation::ValidatorNode;
use crate::modules::constants::HEARTBEAT_PERIOD;
use crate::modules::consensus;
use crate::modules::blockchain::{print_chain, save_chain_json};
use crate::modules::requests;
use crate::modules::adopt_network_state;


/**
 * @notice network.rs contains the main logic for the network listenening, as well as the master event 
 * handler for all incoming traffic into the network. The network will listen for incoming transactions,
 * account creations, and consensus requests. The network will also send heartbeats to the network every
 * HEARTBEAT_PERIOD seconds.
 */


 /**
 * @notice the following structs are used to load in the accepted_ports.json file which contains a list
 * of accepted ports for the network. When a node is booted up, if the port cannot connnect to the network,
 * an excpetion will be thrown and handled by attempting to connect to the next port in the list.
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkConfig { pub nodes: Vec<PortConfig>,}

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

        // format the address and port into a string
        let port_address: String = format!("{}:{}", node.address, node.port);

        // Attempt to bind to the address and port
        match TcpListener::bind(port_address.clone()).await {
            
            Ok(listener) => {
                println!("\nBinding to port {}...", node.port); 
                return Ok((listener, port_address.clone())) // return the listener if successful
            },
            Err(e) => last_error = Some(e),
        }
    }

    Err(last_error.unwrap_or_else(|| IoError::new(std::io::ErrorKind::Other, "No ports available")))
}

/**
 * @notice listen_for_connections() asynchronously listens for incoming connections on the specified 
 * address. It will spawn new tasks to handle each incoming connection. Messages to the network are 
 * passed of to handle_incoming_message() for processing.
 */
pub async fn start_listening(validator_node: ValidatorNode) {

    // Attempt to bind to one of the ports specified in the accepted_ports.json config file
    let (listener, client_port_address) = match try_bind_to_ports().await {

        Ok(result) => { println!("Listening on `{}...`", result.1); result },
        Err(e) => { eprintln!("Refused to bind to any configured port: {}", e); return; }
    };       

    // In a mutable validator node set the client port address returned from try_bind_to_ports()
    let mut validator_node: ValidatorNode = validator_node;
    validator_node.client_port_address = client_port_address.clone();

    // clone mutable validator node structs for use in the 3 below spawned tasks
    let validator_node_clone_1: ValidatorNode = validator_node.clone();
    let validator_node_clone_2: ValidatorNode = validator_node.clone();
    let validator_node_clone_3: ValidatorNode = validator_node.clone();

    // Spawn a task for sending heartbeat signals 
    tokio::spawn(async move {
        send_heartbeat_periodically(validator_node_clone_1).await;
    });

    // Spawn a delayed request for peer ledger states so to adopt the majority of the network
    tokio::spawn(async move {

        // wait a moment before sending request to adopt network state
        time::sleep(time::Duration::from_secs(5)).await;

        // Prepare for responses by updating the count of active peers
        validator_node.update_active_peer_count().await;

        // send request to peers to update to network majority blockchain state.    
        adopt_network_state::adopt_network_state(validator_node_clone_2).await; 
    });

    // Listen for incoming connections
    while let Ok((mut socket, _)) = listener.accept().await {

        // For each incoming connection, clone the validator node for use in the spawned task
        let validator_node_clone: ValidatorNode = validator_node_clone_3.clone();

        // Spawn a new task to handle the incoming message
        tokio::spawn(async move {
            
            // Read the incoming message into a buffer and pass into the master event handler
            let mut buffer: Vec<u8> = Vec::new();
            if socket.read_to_end(&mut buffer).await.is_ok() && !buffer.is_empty() {
                master_events_handler(&buffer, validator_node_clone).await;
            }
        });
    }
    
}

/**
 * @notice master_events_handler() asynchronously accepts a recieved msg buffer recieved by the network. This buffer contains a
 * request() that is parsed into a JSON object. Based on the 'action' field, the function will filter the request into the 
 * appropritate event handler. The event handlers are responsible for validating the request and updating the blockchain state.
 */
async fn master_events_handler( buffer: &[u8], validator_node: ValidatorNode ) {
    println!("\nNew Message Recieved...");

    // convert the buffer to a string 
    let msg = String::from_utf8_lossy(&buffer[..buffer.len()]);

    // After parsing to JSON determine what to do with the msg based on the action field
    if let Ok(request) = serde_json::from_str::<Value>(&msg) {
        
        let request_action: Option<&str> = request["action"].as_str();     
        
        // Determine the action to take based on the request
        match request_action {

            Some("AccountCreation") => { 
                match validation::handle_account_creation_request( request, validator_node.clone() ).await {  
                    Ok(_) => { println!("Account Creation Validated..."); },
                    Err(e) => {eprintln!("Account creation Invalid: {}", e);}
                }
            },
            Some("Transaction") => { 
                match validation::handle_transaction_request(request, validator_node.clone()).await {
                    Ok(success) => { 
                        if success { println!("Transaction Validated..."); } 
                        else {validation::save_failed_transaction_json().await; } // indicate a failed transaction for integration tests
                    },
                    Err(e) => { eprintln!("Transaction Validation Error: {}", e); }
                }
            },
            Some("Faucet") => { 
                match validation::handle_faucet_request(request, validator_node.clone()).await {
                    Ok(_) => { println!("Faucet Request Validated..."); },
                    Err(e) => {eprintln!("Faucet Request Invalid: {}", e);}
                };
            },
            Some("ConsensusRequest") => { // Handle Request From Peer For Independent Decision About New Block
                match consensus::handle_consensus_request( request, validator_node.clone()).await {
                    Ok(_) => { println!("Consensus Request Handled..."); },
                    Err(e) => {eprintln!("Consensus Request Invalid {}", e);}
                };
            },
            Some("ConsensusResponse") => { // Respond to Peer Request For Independent Decision
                match consensus::handle_consensus_response( request, validator_node.clone()).await{
                    Ok(_) => { println!("Consensus Response Handled..."); },
                    Err(e) => {eprintln!("Consensus Response Invalid: {}", e);}
                };  
            },
            Some("HeartBeat") => { // Handle Heartbeat Signal Sent From Peer
                match validation::handle_heartbeat( request, validator_node.clone()).await {
                    Ok(_) => { println!("Heartbeat Request Handled..."); },
                    Err(e) => { eprintln!("Heartbeat Request Failed: {}", e); }
                }
            },
            Some("PeerLedgerRequest") => { // Handle Peer Request for This Node's Ledger State
                match adopt_network_state::handle_peer_ledger_request(request, validator_node.clone()).await {
                    Ok(_) => { println!("Peer Ledger Request Handled..."); },
                    Err(e) => { eprintln!("Peer Ledger Request Failed: {}", e); }
                }
            }
            Some("PeerLedgerResponse") => { // Handle Peer Resppnse for their Ledger State
                match adopt_network_state::handle_peer_ledger_response(request, validator_node.clone()).await {
                    Ok(_) => { println!("Peer Ledger Response Handled..."); },
                    Err(e) => { eprintln!("Peer Ledger Response Failed {}", e); }
                }

            }
            _ => eprintln!("Unrecognized action: {:?}", request_action),
        }

        // print and save state of the blockchain
        print_chain(validator_node.blockchain.clone()).await;
        save_chain_json(validator_node.clone()).await;

    } else {eprintln!("Failed to parse message: {}", msg);}
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
 * @notice hash_network_request() uses Sha256 to hash a serde_json::Value 
 * that contains that contains network request information
 */
pub async fn hash_network_request(request_struct_json: Value) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(request_struct_json.to_string());
    hasher.finalize().to_vec() 
}

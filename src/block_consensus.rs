use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::Value;
use serde_json; 
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::time::Duration;
use tokio::time;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::fs;
use std::io as IoError;
use tokio::io::Error as TokioIoError; // For handling async I/O errors
use std::collections::HashMap;


use crate::constants::{BLOCK_CONSENSUS_LISTENING, PORT_NUMBER, VERBOSE_STACK};
use crate::network::NetworkConfig;
use crate::validation;


/**
 * Protocol for Peer to Peer Consensus: (needs to be implemented)
 * 
 * This is the protocol for how peer nodes will come to a consensus over the decision to write a block to the
 * blockchain by validator nodes. This protocol focuses specifically on how network communication needs to be 
 * set up in order for peer to peer communication to be possible for this purpose. 
 * 
 * A configuration file containing accepted ports for the network will be used to determine which ports to connect 
 * to when booting up a validator node for the first time. Having multiple ports also makes development of the 
 * network easier, preventing 'address in use' errors. When a validator node boots up, it will send a request to 
 * all ports that are not its own. This message indicates that a new validator node is connecting. This request will 
 * include the port number being connected on, as well as a request for the current state of the network (which will 
 * be implemented later).
 * 
 * When a validator receives a transaction or account creation request, they will first perform an independent 
 * validation of the request they recieved based on the state of their node and the request details that we sent.
 * Then, upon coming to an independent decision, they will send a network request to all accepted ports that are
 * not the same as the one running the current node instance. This request will contain information identifying 
 * the request and asking for thier decision of how to handle the request. Peers will send back their responce, 
 * yay or nay, and the majority decision will be chosen. 
 * 
 * In order to keep track of tentative client devisions, a hash map containing of request-identigier-->decision
 * will be used to store the responce that will be sent back when block consensus is requested from another 
 * validator node. 
 */


// ! TODO There needs to be some way to verify that a validator node is truly a validator node. This could involve keeping a 
// ! simple ledger of obfuscated private keys of validator nodes, collected when the boot up and sent to all other nodes. Then 
// ! when a request is made, the obfuscated private key can be sent along with the request to verify the node is a validator.

/**
 * @notice block_consensus.rs contains the logic for peer validator nodes to reach a consensus over whether or not to accept a new block
 * into the blockchain.
 * 
 * Each time a request for a transaction/account creation is made, validator nodes will independently check the validity of the request.
 * Then, upon their independent decision, they will send a request to all other validator nodes for their decision. The majority decision 
 * will be accepted by the network regardless of the individual validator node's decision.
 */


#[derive(Debug, Clone, Serialize, Deserialize)]
 struct BlockConsensusRequest{ 
    action: String, 
    request_hash: Vec<u8>,
    resonse_port: String,
}

 #[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockConsensusResponse {
    action: String,
    request_hash: Vec<u8>,
    decision: bool,
}

 

/**
 * @notice send_block_consensus_request() is an asynchronous function that sends a request to all other validator nodes for their decision
 * on whether or not to accept a new block into the blockchain. This function is used within the validation module following the independent
 * decision of the validator node for the type of request being processed.
 * 
 * @dev the message sent will include a hash of the request to ensure the same request is being processed by all validator nodes. The responce
 * to this request will be a boolean value indicating the majority decision of the network, along with the hash of the request to ensure that 
 * the correct block consensus decision in being considered.
*/
pub async fn send_block_consensus_request( request: Value, validator_node: validation::ValidatorNode ) -> bool {

    if VERBOSE_STACK { println!("block_consensus::send_block_consensus_request() : Preparing block consensus request..."); }


    // extract the port number form the validator node
    let self_port: String = validator_node.client_port_address.clone();
    let peer_consensus_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>>> = validator_node.peer_consensus_decisions.clone();
    let client_block_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>> = validator_node.client_block_decisions.clone();



    // Hash request recieved by client. This will be used to ensure te same right 
    // request is processed upon validator nodes recieving this request.
    let mut hasher = Sha256::new();
    hasher.update(request.to_string());
    let client_request_hash: Vec<u8> = hasher.finalize().to_vec();

    // Package request in struct and serialize to JSON
    let consensus_request = BlockConsensusRequest {
        action: "block_consensus".to_string(),
        request_hash: client_request_hash.clone(),
        resonse_port: self_port.clone()
    };
    let json_msg: String = serde_json::to_string(&consensus_request).unwrap();

    // Collect all outbound ports
    let outbound_ports: Vec<String> = collect_outbound_ports(self_port.clone()).await.unwrap();

    for port in outbound_ports.iter() {
        println!("Port: {}", port);
    }

    // Connect to port and send msg to validator nodes
    for port in outbound_ports.iter() {
        if VERBOSE_STACK { println!("send_block_consensus_request() : Sending block consensus request to: {}", port); } 

        // Only Send Messages to other ports
        if port != &self_port {

            // Connect to port and send message  
            match TcpStream::connect(port).await {

                // Send message to port if connection is successful
                Ok(mut stream) => {
                    if let Err(e) = stream.write_all(json_msg.as_bytes()).await { eprintln!("Failed to send message to {}: {}", port, e); }
                    if VERBOSE_STACK { println!("send_block_consensus_request() : Sending block consensus request to: {}", port); } 
                },

                // Print error message if connection fails
                Err(_) => { println!("block_consensus::send_block_consensus_request() : Failed to connect to {}, There may not be a listener...", port); }
            }
        }
    }   
    
    // Determine if the client's decision is the majority decision
    let majority_decision: bool = determine_majority(
        peer_consensus_decisions, 
        client_block_decisions,
        client_request_hash
    ).await;

    majority_decision
}


/**
 * @notice determine_majority() is an asynchronous function that determines the majority decision of the network based on the responses
 * recieved from other validator nodes. This function is used within the send_block_consensus_request() function.
 */
async fn determine_majority(
    peer_consensus_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>>>,
    client_block_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>,
    client_request_hash: Vec<u8>
) -> bool {

    // Lock mutex when accessing responses
    let peer_responces_guard = peer_consensus_decisions.lock().await;

    // Get counts for true and false  peerdecisions
    let mut true_count: u32 = peer_responces_guard.get(&client_request_hash).unwrap().0;
    let mut false_count: u32 = peer_responces_guard.get(&client_request_hash).unwrap().1;

    // get client decision from locked guard
    let client_decision_guard = client_block_decisions.lock().await;
    let client_decision: bool = client_decision_guard.get(&client_request_hash).unwrap().clone();

    // Add client decision to count
    if client_decision { true_count += 1; }
    else { false_count += 1; }

    // return the decision 
    if true_count > false_count { true } else { false }
}


/**
 * @notice collect_outbound_ports() is an asynchronous function that reads the configuration file containing the accepted 
 * ports of the network. All ports that are not the port of the client are collected and returned as a vector of strings.
 */
async fn collect_outbound_ports(self_port: String) -> Result<Vec<String>, TokioIoError> {

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




/**
 * @notice handle_block_consensus_request() is an asynchronous function that handles a block consensus request from another validator node.
 * This function will retrieve the client decision from the request, package the response, and send the response back to the requesting node.
 * The funnction is called within the validator module.
 */
pub async fn handle_block_consensus_request(request: Value, validator_node: validation::ValidatorNode) -> Result<(), std::io::Error> {
    

    // extract the port number and client block decisions from the validator node
    let self_port: String = validator_node.client_port_address.clone();
    let client_block_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>= validator_node.client_block_decisions.clone();

    // retrieve hash of request from request
    let request_hash: Vec<u8> = request["request_hash"].as_str().unwrap().as_bytes().to_vec();

    // Lock mutex when accessing responses
    let mut client_block_decisions_guard = client_block_decisions.lock().await;

    // retrieve client decision from request
    let client_decision: bool = client_block_decisions_guard.get(&request_hash).unwrap().clone();

    // Send response to client
    respond_to_block_consensus_request(request_hash, client_decision, self_port).await;

    Ok(())    
}

/**
 * @notice respond_to_block_consensus_request() is an asynchronous function that sends a response to a block consensus request
 * containing the client decision of whether or not to accept a new block into the blockchain. This function is used within the
 * handle_block_consensus_request() function.
 */
async fn respond_to_block_consensus_request(
    client_request_hash: Vec<u8>,
    client_decision: bool,
    self_port: String
){
    // Package responce in struct and serialize to JSON
    let consensus_responce = BlockConsensusResponse {
        action: "block_consensus".to_string(),
        request_hash: client_request_hash.clone(),
        decision: client_decision
    };

    // Serialize responce to JSON
    let json_msg: String = serde_json::to_string(&consensus_responce).unwrap();

    // Collect all outbound ports
    let outbound_ports: Vec<String> = collect_outbound_ports(self_port.clone()).await.unwrap();

    for port in outbound_ports.iter() {
        println!("Port: {}", port);
    }

    // Connect to port and send msg to validator nodes
    for port in outbound_ports.iter() {
        if VERBOSE_STACK { println!("respond_to_block_consensus_request() : Sending block consensus request to: {}", port); } 

        // Only Send Messages to other ports
        if port != &self_port {

            // Connect to port and send message  
            match TcpStream::connect(port).await {

                // Send message to port if connection is successful
                Ok(mut stream) => {
                    if let Err(e) = stream.write_all(json_msg.as_bytes()).await { eprintln!("Failed to send message to {}: {}", port, e); }
                    if VERBOSE_STACK { println!("respond_to_block_consensus_request() : Sending block consensus request to: {}", port); } 
                },

                // Print error message if connection fails
                Err(_) => { println!("block_consensus::respond_to_block_consensus_request() : Failed to connect to {}, There may not be a listener...", port); }
            }
        }
    }   
}


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

use crate::constants::{BLOCK_CONSENSUS_LISTENING, PORT_NUMBER, VERBOSE_STACK};
use crate::validation::NetworkConfig;


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
    self_port: String,
}

 #[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockConsensusResponse {
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
pub async fn send_block_consensus_request(request: Value, client_decision: bool, self_port: String) -> bool {
    if VERBOSE_STACK { println!("block_consensus::send_block_consensus_request() : Preparing block consensus request..."); }

    // Hash request recieved by client. This will be used to ensure te same right 
    // request is processed upon validator nodes recieving this request.
    let mut hasher = Sha256::new();
    hasher.update(request.to_string());
    let request_hash: Vec<u8> = hasher.finalize().to_vec();

    // Package request in struct and serialize to JSON
    let consensus_request = BlockConsensusRequest {
        action: "block_consensus".to_string(),
        request_hash: request_hash.clone(),
        self_port: self_port.clone()
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
                Err(_) => { eprintln!("Failed to connect to {}, There may not be a listener...", port); }
            }
        }
    }   
    
    if VERBOSE_STACK { println!("attemping to connect to listener..."); }

    // Establish listener on the client's port. Resonses willl be directed here.
    let listener = TcpListener::bind(self_port.clone()).await.unwrap(); // TODO binding to same port again

    if VERBOSE_STACK { println!("Listener established on port: {}", self_port); }

    // Establish a vector to store responses
    let responses:  Arc<Mutex<Vec<(Vec<u8>, bool)>>> = Arc::new(Mutex::new(Vec::new()));
    let listener_future = listen_for_responses(listener, responses.clone());


    // Determine if the client's decision is the majority decision
    let majority_decision: bool = determine_majority(
        responses.clone(), 
        client_decision,
        request_hash
    ).await;

    majority_decision
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
 * @notice listen_for_responses() is an asynchronous function that listens for responses from other validator nodes regarding their decision
 * on whether or not to accept a new block into the blockchain. This function is used within the send_block_consensus_request() function.
*/
async fn listen_for_responses(listener: TcpListener, responses: Arc<Mutex<Vec<(Vec<u8>, bool)>>>) {

    // Collect responses from validator nodes
    let collector = async move {

        // Listen for responses for a duration
        let end_time = time::Instant::now() + BLOCK_CONSENSUS_LISTENING;

        // Loop until timeout
        loop {
            
            // Establish loop break condition
            let timeout_duration: Duration = end_time.saturating_duration_since(time::Instant::now());
            if timeout_duration.is_zero() { break; }

            // Listen for responses
            let accept_future = listener.accept();
            match time::timeout(timeout_duration, accept_future).await {

                // If a response is recieved, spawn a new task to process the response
                Ok(Ok((mut socket, _))) => {
                    let responses = responses.clone();
                    tokio::spawn(async move {

                        // Read response from socket
                        let mut buffer = Vec::new();
                        if let Ok(_) = socket.read_to_end(&mut buffer).await {

                            // Convert responce to BlockConsensusResponce struct
                            if let Ok(response) = serde_json::from_slice::<BlockConsensusResponse>(&buffer) {

                                // Push responses to the locked Mutex guard
                                let mut responses_guard = responses.lock().await;
                                responses_guard.push((response.request_hash, response.decision));
                            }
                        }
                    });
                },
                _ => break, // Break on timeout or error
            }
        }
    };

    collector.await;
}

/**
 * @notice determine_majority() is an asynchronous function that determines the majority decision of the network based on the responses
 * recieved from other validator nodes. This function is used within the send_block_consensus_request() function.
 */
async fn determine_majority(
    responses: Arc<Mutex<Vec<(Vec<u8>, bool)>>>, 
    client_decision: bool, 
    client_request_hash: Vec<u8>
) -> bool {

    // Create counts for true and false decisions
    let mut true_count: u32 = 0;
    let mut false_count: u32 = 0;

    // Add client decision to count
    if client_decision { true_count += 1; }
    else { false_count += 1; }

    // Lock mutex when accessing responses
    let responces_guard = responses.lock().await;

    // Iterate through responses
    for (request_hash, decision) in responces_guard.iter() {

        // ensure we are working on the same request
        if request_hash == &client_request_hash {

            // Add decision to count depending on value
            if *decision { true_count += 1; }
            else { false_count += 1; }
        }
    }
    
    // Determine majority decision
    if true_count > false_count { return true; }
    else { return false; }
}

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::Value;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use std::time::Duration;
use tokio::time;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::net::SocketAddr;

use crate::constants::{PORT_NUMBER, BLOCK_CONSENSUS_LISTENING};




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
pub async fn send_block_consensus_request(request: Value, client_decision: bool) -> bool {

    // Hash request recieved by client. This will be used to ensure te same right 
    // request is processed upon validator nodes recieving this request.
    let mut hasher = Sha256::new();
    hasher.update(request.to_string());
    let request_hash: Vec<u8> = hasher.finalize().to_vec();

    // Clone request hash for use in determining majority decision prior to moving
    let request_hash_clone = request_hash.clone();

    // Package request in struct and serialize to JSON
    let consensus_request = BlockConsensusRequest {
        action: "block_consensus".to_string(),
        request_hash,
    };
    let json_msg: String = serde_json::to_string(&consensus_request).unwrap();

    // Connect to port and send msg to validator nodes
    let mut stream = TcpStream::connect(PORT_NUMBER).await.unwrap();
    stream.write_all(json_msg.as_bytes()).await.unwrap();

    // Listen for responses
    let listener = TcpListener::bind(PORT_NUMBER).await.unwrap();
    let responses:  Arc<Mutex<Vec<(Vec<u8>, bool)>>> = Arc::new(Mutex::new(Vec::new()));
    let listener_future = listen_for_responses(listener, responses.clone());


    // Determine if the client's decision is the majority decision
    let majority_decision: bool = determine_majority(
        responses.clone(), 
        client_decision,
        request_hash_clone
    ).await;

    majority_decision
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

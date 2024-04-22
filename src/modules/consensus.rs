use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;

use serde_json::Value;
use serde_json; 
use serde::{Serialize, Deserialize};

use tokio::sync::{Mutex, Notify};

use std::sync::Arc;
use std::collections::HashMap;


use crate::modules::validation;
use crate::modules::network;


/**
 * Protocol for Peer to Peer Consensus: 
 * 
 * consensus.rs contains the protocol for how peer nodes reach a consensus over whether or not to accept a new block 
 * into the blockchain.
 * 
 * First, a validator node will independently check the validity of a request. Then, upon their independent decision, 
 * they will send a request to all other validator nodes for their decision. The majority decision will be accepted by 
 * the network regardless of the individual validator node's decision. Then, each validator node will send a response 
 * back to the requesting node with their decision. The requesting node will then determine the majority decision based 
 * on the responses received from the other validator nodes.
 */


/**
 * @notice BlockConsensusRequest struct is a serializable struct that is used 
 * to package a block consensus request to be sent to other validator nodes.
 * @param action: String - the action to be taken by the receiving node
 * @param request_hash: Vec<u8> - the hash of the request to be validated
 * @param response_port: String - the port to send the response to
 */
 #[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockConsensusResponse {
    action: String,     
    request_hash: Vec<u8>,
    decision: bool,
}

/**
 * @notice handle_block_consensus_request() is an asynchronous function that handles a block consensus request from 
 * another validator node. This function will retrieve the client decision from the request, package the response, 
 * and send the response back to the requesting node. The funnction is called within the validator module.
 */
pub async fn handle_consensus_request(request: Value, validator_node: validation::ValidatorNode) -> Result<(), Box<dyn std::error::Error>> {
    println!("Handling request from peer for consensus..."); 

    // retrieve request hash from request as a vector of u8
    let request_hash: Vec<u8> = request["request_hash"].as_array().unwrap()
                                     .iter()
                                     .map(|x| x.as_u64().unwrap() as u8)
                                     .collect();

    // lock mutex and get client decision from validator node
    let client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>= validator_node.client_decisions.clone();
    let client_decisions_guard = client_decisions.lock().await;
    let client_decision: bool = client_decisions_guard.get(&request_hash).unwrap().clone();   

    // Package responce in struct and serialize to JSON
    let consensus_responce = BlockConsensusResponse {
        action: "ConsensusResponse".to_string(), 
        request_hash, 
        decision: client_decision
    };
    let json_msg: String = serde_json::to_string(&consensus_responce).unwrap();

    // retrieve response port from request
    let response_port: String = request["response_port"].as_str().unwrap().to_string();

   // Connect to port and send message  
    match TcpStream::connect(response_port.clone()).await {

        // Send message to port if connection is successful
        Ok(mut stream) => {

            // Write to the Stream
            if let Err(e) = stream.write_all(json_msg.as_bytes()).await { 
                eprintln!("Failed to send message to {}: {}", response_port, e); 
                return Ok(());
            }
            println!("Sent repsonse to conensus request to: {}", response_port); 
         },

        // Print error message if connection fails
        Err(_) => { println!("Failed to connect to {}, There may not be a listener...", response_port); }
    }

    Ok(())
}   

/**
 * @notice handle_block_consensus_response() is an asynchronous function that handles a block consensus response from 
 * another validator node. This function will retrieve the client decision from the response, update the peer decisions 
 * hash map, and trigger the notify to wake up the main thread. The funnction is called within the validator module.
 */
pub async fn handle_consensus_response(request: Value, validator_node: validation::ValidatorNode) -> Result<(), Box<dyn std::error::Error>> { 
    println!("Handling consensus request reponse from peer..."); 

    // get request hash from request
    let request_hash: Vec<u8> = request["request_hash"].as_array().unwrap()
                                     .iter()
                                     .map(|x| x.as_u64().unwrap() as u8)
                                     .collect();

    // get client decision from request
    let decision: bool = request["decision"].as_bool().unwrap();

    // get peer decisions from validator node
    let peer_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>>> = validator_node.peer_decisions.clone();
    let mut peer_decisions_guard = peer_decisions.lock().await;

    // get counts from peer decisions
    let mut true_count: u32 = 0; let mut false_count: u32 = 0;
    if peer_decisions_guard.contains_key(&request_hash) {
        true_count = peer_decisions_guard.get(&request_hash).unwrap().0;
        false_count = peer_decisions_guard.get(&request_hash).unwrap().1;
    }

    // add client decision to counts
    if decision { true_count += 1; } else { false_count += 1; }

    // update peer decisions
    peer_decisions_guard.insert(request_hash.clone(), (true_count, false_count));

    // trigger the notify to wake up the main thread
    let notify_consensus: Arc<Notify> = validator_node.notify_consensus.clone();
    notify_consensus.notify_one();

    println!("Current peer votes to accept transaction: {}, votes to reject: {}", true_count, false_count);
    Ok(())
}


/**
 * @notice determine_majority() is an asynchronous function that determines the majority decision of the network based on the 
 * responses recieved from other validator nodes. Pre collected responces from the peer_consensus_decisions arc mutex hash map.
 */
pub async fn determine_majority(request: Value, validator_node: validation::ValidatorNode) -> bool {
    println!("Determining majority decision by peers..."); 

    // get block decision from validator node
    let peer_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>>> = validator_node.peer_decisions.clone();
    let client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>> = validator_node.client_decisions.clone();

    // Hash request recieved by client. This will be used to ensure te same right
    let client_request_hash: Vec<u8> = network::hash_network_request(request).await;

    // get client decision from locked guard
    let client_decision_guard = client_decisions.lock().await;
    let client_decision: bool = client_decision_guard.get(&client_request_hash).unwrap().clone();

    // Lock mutex when accessing responses
    let mut true_count: u32 = 0; let mut false_count: u32 = 0;

    // print client decision
    println!("Client Decision: {}", client_decision);

    // Add client decision to count
    if client_decision { true_count += 1; } else { false_count += 1; }

    // Lock mutex and check if there are peer responces, get the counts for true and false decisions
    let peer_decisions_guard = peer_decisions.lock().await;
    if peer_decisions_guard.contains_key(&client_request_hash) {

        // if there are peer respencesget counts from locked guard
        true_count += peer_decisions_guard.get(&client_request_hash).unwrap().0;
        false_count += peer_decisions_guard.get(&client_request_hash).unwrap().1;
    } 

    // return the decision 
    let peer_decision: bool = true_count > false_count;
    println!("Final votes to accept: {}, votes to reject: {}", true_count, false_count);

    peer_decision
    
}


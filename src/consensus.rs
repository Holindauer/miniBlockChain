use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use serde_json::Value;
use serde_json; 
use serde::{Serialize, Deserialize};
use tokio::sync::{Mutex, Notify};
use std::sync::Arc;
use std::collections::HashMap;


use crate::validation;
use crate::network;


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
struct BlockConsensusResponse {
    action: String,
    request_hash: Vec<u8>,
    decision: bool,
}


/**
 * @notice handle_block_consensus_request() is an asynchronous function that handles a block consensus request from another validator node.
 * This function will retrieve the client decision from the request, package the response, and send the response back to the requesting node.
 * The funnction is called within the validator module.
 */
pub async fn handle_consensus_request(request: Value, validator_node: validation::ValidatorNode) {
    println!("block_consensus::handle_block_consensus_request() : Handling block consensus request..."); 

    // print the request json
    println!("Request: {}", request);

    // retrieve request hash from request 
    // retrieve request hash from request as a vector of u8
    let request_hash: Vec<u8> = request["request_hash"].as_array().unwrap()
                                     .iter()
                                     .map(|x| x.as_u64().unwrap() as u8)
                                     .collect();

    // print the request hash
    println!("Request Hash: {:?}", request_hash);

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
            if let Err(e) = stream.write_all(json_msg.as_bytes()).await { eprintln!("Failed to send message to {}: {}", response_port, e); }
            println!("respond_to_block_consensus_request() : Sending block consensus response to: {}", response_port); 
         },

        // Print error message if connection fails
        Err(_) => { println!("block_consensus::respond_to_block_consensus_request() : Failed to connect to {}, There may not be a listener...", response_port); }
    }
}   

pub async fn handle_consensus_response(request: Value, validator_node: validation::ValidatorNode) {
    println!("block_consensus::handle_block_consensus_response() : Handling block consensus response..."); 

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
    let notify: Arc<Notify> = validator_node.notify.clone();
    notify.notify_one();

    println!("Peer Votes for True: {} Votes for False: {}", true_count, false_count);

}


/**
 * @notice determine_majority() is an asynchronous function that determines the majority decision of the network based on the 
 * responses recieved from other validator nodes. Pre collected responces from the peer_consensus_decisions arc mutex hash map.
 */
pub async fn determine_majority(request: Value, validator_node: validation::ValidatorNode) -> bool {
    println!("consensus::determine_majority() : Determining majority decision by peers..."); 

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
    if true_count > false_count { true } else { false }
    
}


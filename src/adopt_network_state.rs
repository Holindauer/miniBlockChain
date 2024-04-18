use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{Mutex, Notify};
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};

use serde_json::Value;

use crate::validation::ValidatorNode;
use crate::blockchain::{BlockChain, Block};
use crate::merkle_tree::{MerkleTree, Account};
use crate::requests;
use crate::constants::PEER_STATE_RECEPTION_DURATION;





/**
 * 

Secret Key: "669815890583d2be695c2b5de3fd57cf0d69ba31ade6fe91000628348a19eebb"
Public Key: "02e83d256e1cb8b999207261defc66f912740952b164de44b6cf4557cdb3af0571"
 */


/**
 * @notice chain_consensus.rs contains the logic for updating the local blockchain and merkle tree of a validator node
 * that is booting up to the majority state of the network. This is done by sending a request to all other validators
 * to send their current blockchain state. The node will then determine the majority state of the network and update
 * its local blockchain to reflect the majority.
*/

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerLedgerResponse {
    action: String,
    blockchain: Vec<Block>,
    accounts_vec: Vec<Account>,
    accounts_map: HashMap<Vec<u8>, u64>,
}

/**
 * @notice adopt_network_state() is an asynchronous function that fascililtates the process of updating 
 * the local blockchain and merkle tree to the majority state of the network. This function is called by
 * validation::run_validation() when booting up a new validator node.
*/
pub async fn adopt_network_state(validator_node: ValidatorNode) {
    println!("\nSending request to peers for majority networtk state...");   

    // Send request to all peers for their blockchain and merkle tree data
    requests::send_peer_ledger_request(validator_node.clone()).await;

    // Wait for all active peers to respond with their ledgers
    validator_node.await_all_peer_ledger_states_received().await;

    // Determine the majority state of the network and 
    // update the local blockchain and merkle tree
    adopt_majority(validator_node.clone()).await;
 }

/**
 * @notice handle_peer_ledger_request() reciecves a serde_json::Value struct containing a request
 * for this node's copy of its's blockchain and 
 */
 pub async fn handle_peer_ledger_request(request: Value, validator_node: ValidatorNode)-> Result<(), Box<dyn std::error::Error>> {

    // lock blockchain
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let blockchain_guard = blockchain.lock().await;
    
    // lock merkle tree
    let merkle_tree : Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let merkle_tree_guard = merkle_tree.lock().await;

    // retrieve blockchain and merkle tree data
    let chain: Vec<Block> = blockchain_guard.chain.clone();
    let accounts_vec: Vec<Account> = merkle_tree_guard.accounts_vec.clone();
    let accounts_map: HashMap<Vec<u8>, u64> = merkle_tree_guard.accounts_map.clone();

    // Package data into a PeerLedgerResponse struct
    let response: PeerLedgerResponse = PeerLedgerResponse {
        action: "PeerLedgerResponse".to_string(),
        blockchain: chain,
        accounts_vec: accounts_vec,
        accounts_map: accounts_map,
    };

    // serialize the PeerLedgerResponse struct into a JSON string
    let ledger_json: String = serde_json::to_string(&response)?;

    // Retrieve response port from request
    let response_port: String = request["response_port"].as_str().unwrap().to_string();

    // Connect to port and send message  
    match TcpStream::connect(response_port.clone()).await {

        // Send message to port if connection is successful
        Ok(mut stream) => {

            // Write to the Stream
            if let Err(e) = stream.write_all(ledger_json.as_bytes()).await { 
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
  * @handle_peer_ledger_response() is a function that takes a serde_json::Value struct containing a response
  * from a peer node that contains the blockchain and merkle tree data of the peer node. This function will
  * store the data in the validator node's peer_ledger_state field for majority consensus determination once 
  * the listening period has ended.
  */
 pub async fn handle_peer_ledger_response(response: Value, validator_node: ValidatorNode)-> Result<(), Box<dyn std::error::Error>> {

    // Extract the blockchain data from the response
    let blockchain: Vec<Block> = response["blockchain"].as_array().unwrap()
        .iter()
        .map(|block| serde_json::from_value(block.clone()).unwrap())
        .collect();

    // Extract the accounts_vec data from the response
    let accounts_vec: Vec<Account> = response["accounts_vec"].as_array().unwrap()
        .iter()
        .map(|account| serde_json::from_value(account.clone()).unwrap())
        .collect();

    // Extract the accounts_map data from the response
    let accounts_map: HashMap<Vec<u8>, u64> = response["accounts_map"].as_object().unwrap()
        .iter()
        .map(|(key, value)| {
            let key_vec: Vec<u8> = key.as_bytes().to_vec();
            let value_u64: u64 = value.as_u64().unwrap();
            (key_vec, value_u64)
        })
        .collect();
        
    // Package the data into a PeerLedgerResponse struct
    let peer_ledger_response: PeerLedgerResponse = PeerLedgerResponse {
        action: "PeerLedgerResponse".to_string(),
        blockchain: blockchain,
        accounts_vec: accounts_vec,
        accounts_map: accounts_map,
    };

    // Lock the peer_ledger_state mutex
    let peer_ledger_state: Arc<Mutex<Vec<PeerLedgerResponse>>> = validator_node.peer_ledger_states.clone();
    let mut peer_ledger_state_guard = peer_ledger_state.lock().await;

    // Store the PeerLedgerResponse struct in the peer_ledger_state vector
    peer_ledger_state_guard.push(peer_ledger_response);

    // Notify the main thread that a new response is in. This will trigger an updated check of 
    // whether all peers have responded. (See the validator_node impl in validation.rs)
    let notify_all_ledgers_received: Arc<Notify> = validator_node.notify_all_ledgers_received.clone();
    notify_all_ledgers_received.notify_one();
    
    Ok(())
 }



 /**
  * @notice adopt_majority() is an asynchronous function that will determine the majority state of the collected
  * peer_ledger_states and update the local blockchain and merkle tree of the validator node to reflect the majority
  * state of the network. The majority is determine by hashing the blockchain and merkle tree data of each peer_ledger_state
    * and counting the number of occurences of each hash using a hash map. The hash with the most occurences is considered 
    the majority state.
  */
async fn adopt_majority(validator_node: ValidatorNode){

    // Lock the peer_ledger_states mutex
    let peer_ledger_states: Arc<Mutex<Vec<PeerLedgerResponse>>> = validator_node.peer_ledger_states.clone();
    let peer_ledger_states_guard = peer_ledger_states.lock().await;

    // Return if there are no peer_ledger_states (first node of the network)
    if peer_ledger_states_guard.len() == 0 { 
        println!("No peer ledger states to adopt...");  return; 
    }

    // Create a hash map to store the hash of each peer_ledger_state
    let mut ledger_hash_map: HashMap<Vec<u8>, u32> = HashMap::new();

    // Iterate through each peer_ledger_state and hash the blockchain and merkle tree data
    for peer_ledger_state in peer_ledger_states_guard.iter() {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_string(&peer_ledger_state.blockchain).unwrap());
        hasher.update(serde_json::to_string(&peer_ledger_state.accounts_vec).unwrap());
        hasher.update(serde_json::to_string(&peer_ledger_state.accounts_map).unwrap());
        let hash: Vec<u8> = hasher.finalize().to_vec();

        // Count the number of occurences of each hash
        let count = ledger_hash_map.entry(hash).or_insert(0);
        *count += 1;
    }

    // Find the hash with the most occurences
    let mut majority_hash: Vec<u8> = Vec::new();
    let mut majority_count: u32 = 0;
    for (hash, count) in ledger_hash_map.iter() {
        if *count > majority_count {
            majority_hash = hash.clone();
            majority_count = *count;
        }
    }

    // Find the peer_ledger_state that corresponds to the majority hash
    let majority_peer_ledger_state: &PeerLedgerResponse = peer_ledger_states_guard.iter()
        .find(|peer_ledger_state| {
            let mut hasher = Sha256::new();
            hasher.update(serde_json::to_string(&peer_ledger_state.blockchain).unwrap());
            hasher.update(serde_json::to_string(&peer_ledger_state.accounts_vec).unwrap());
            hasher.update(serde_json::to_string(&peer_ledger_state.accounts_map).unwrap());
            let hash: Vec<u8> = hasher.finalize().to_vec();
            hash == majority_hash
        })
        .unwrap();

    // lock blockchain
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let mut blockchain_guard = blockchain.lock().await;

    // lock merkle tree
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let mut merkle_tree_guard = merkle_tree.lock().await;

    // Update local ledger to majority state
    blockchain_guard.chain = majority_peer_ledger_state.blockchain.clone();
    merkle_tree_guard.accounts_vec = majority_peer_ledger_state.accounts_vec.clone();
    merkle_tree_guard.accounts_map = majority_peer_ledger_state.accounts_map.clone();

    println!("\n\n--- Adopted majority network state ---\n\n");   
}



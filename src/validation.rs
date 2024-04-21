use tokio::sync::{Mutex, MutexGuard, Notify};
use serde_json::Value;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use std::collections::HashMap;

use crate::blockchain::{BlockChain, Block};
use crate::merkle_tree::{MerkleTree, Account};
use crate::constants::{FAUCET_AMOUNT, HEARTBEAT_TIMEOUT};
use crate::consensus;
use crate::zk_proof;
use crate::network;
use crate::requests;
use crate::adopt_network_state;
use crate::adopt_network_state::PeerLedgerResponse;

/**
 * @protocol validation.rs contains the data structures and event handler logic for running a validator node. 
 * The network.rs module is the first to recieve requests into the network, to which it is filtered and passed 
 * to the event handlers in this module. 
 * 
 * For each event handler, there is a general protocol followed: First, the client comes to an independent 
 * decision on the validity of the request. This decision is stored. Then, the client sends a request to all 
 * currently active peers (as tracked by the heartbeat mechanism) for their own independent decisions. The 
 * client waits until the number of responses recieved matches the number of active peers. Once all responses
 * are recieved, the client determines the majority decision and either writes coordinates the writing of the
 * request to the blockchain or rejects it.
 */


/**
 * @struct The ValidatorNode struct contains the local ledger state of this validator node. A well as other
 * datastructures used to fascilitate the validation process. All datastructures are wrapperd in Arc<Mutex<>> 
 * to allow for concurrent access by multiple tasks while avoiding race conditions.
 * 
 * @param blockchain: Arc<Mutex<BlockChain>> - The BlockChain struct is the ledger of all transactions that 
 * have occured on the network. It stores a linked list of Block enum structs (see blockchain.rs for details 
 * on this implementation). As new transactions are verified, they are added to the blockchain.
 * 
 * @param merkle_tree: Arc<Mutex<MerkleTree>> - The merkle tree is a binary tree built built upwards from an 
 * iterable of account addresses that stores the account balances of all users on the network. As the tree is 
 * assembled, nodes are hashed into each other to produce a single root hash that is used as unique identifier 
 * for the state of the stored accounts network at a given time.
 * 
 * @param client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>> - This hashmap stores the decisions made by the 
 * client regarding the validity of a given request. The key is the hash of the request and the value is a 
 * boolean representing the decision made by the client. This datastructure is updated following the result of 
 * the independent validation of a request by the client.
 * 
 * @param peer_consensus_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>> - This hashmap stores the decisions 
 * made by each peer validator node on the network regarding the validity of a given request. The key is the hash 
 * of the request and the value is a tuple of u32 integers representing the number of yays vs nays for the request 
 * collected by peers. Decisions within this datastructure are updated following the independent validation by the 
 * client. After this point, the client will send a request to the network for consensus on the request. Responces 
 * recieved from the network will be updated within this structure.
 * 
 * @param client_port_address: String - The port address that the client is listening on for incoming connections. 
 * This is used to establish a connection with the client from the network.
 * 
 * @param used_zk_proofs: Arc<Mutex<Vec<u8>, String>> - A hashmap that stores the zk-proofs that hahses of the 
 * zk-proofs have been used by a requester to verify transactions. The key is the address of the client and the 
 * value is a hash of the zk-proof. This datastructure is used to prevent replay attacks with zk-proofs. This 
 * is because the same curve points will always add to the third curve point (the obscured private key) so
 * allowing the resuse of the same zk-proof would enable a listener to replay the same transaction multiple times.
 * 
 * @param active_peers: Arc<Mutex<Vec<(String, u64)>>> - A vector of (String, u64) tuples containing the addresses 
 * of all active peers (as represented by their port address) on the network and the timestamp of the last recieved 
 * heartbeat from this peer. This datastructure is maintained by the validator node via a blocked on heartbeat 
 * protocol that listens for the periodic heartbeat of other nodes on the network. Nodes that fail to send a 
 * heartbeat within a given time frame are removed from the active peers list.
 * 
 * @param total_peers: Arc<Mutex<usize>> - The total number of active peers on the network. This value is updated
 * when the active_peers datastructure is updated. It is used to determine when all responses have been recieved
 * for a given request.
 * 
 * @param peer_ledger_states: Arc<Vec<PeerLedgerResponse>> - A vector of PeerLedgerResponse structs that contain
 * the blockchain and merkle tree state of each peer on the network. This datastructure is used to store the state
 * of peer nodes and determine the majority state of the network when updating the local ledger state of the 
 * validator node.
 * 
 * @param notify: Arc<Notify> - A tokio sync Notify struct that is used to notify the validator node when all
 * responses have been recieved for a given request. This is used to break out of the loop that waits for responses
 * from the network.
 */
#[derive(Clone)]
pub struct ValidatorNode {

    // Local Ledger State
    pub blockchain: Arc<Mutex<BlockChain>>,
    pub merkle_tree: Arc<Mutex<MerkleTree>>,

    // Datastructures for Validation
    pub peer_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>>>, 
    pub client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>,
    pub client_port_address: String,    
    pub used_zk_proofs: Arc<Mutex<HashMap<Vec<u8>, Vec<String>>>>, // address -> vec of hashes of zk-proofs
    pub active_peers: Arc<Mutex<Vec<(String, u64)>>>, 
    pub total_peers: Arc<Mutex<usize>>, 
    pub peer_ledger_states: Arc<Mutex<Vec<PeerLedgerResponse>>>,
    pub notify_consensus: Arc<Notify>, 
    pub notify_all_ledgers_received: Arc<Notify>,
}

impl ValidatorNode { // initializes datastructures
    pub fn new() -> ValidatorNode {
        ValidatorNode { 
            blockchain: Arc::new(Mutex::new(BlockChain::new())),
            merkle_tree: Arc::new(Mutex::new(MerkleTree::new())),
            peer_decisions: Arc::new(Mutex::new(HashMap::new())),
            client_decisions: Arc::new(Mutex::new(HashMap::new())),
            client_port_address: String::new(),
            used_zk_proofs: Arc::new(Mutex::new(HashMap::new())),
            active_peers: Arc::new(Mutex::new(Vec::new())),
            total_peers: Arc::new(Mutex::new(0)), // Init to zero, will be set when peers are know
            peer_ledger_states: Arc::new(Mutex::new(Vec::new())),
            notify_consensus: Arc::new(Notify::new()),
            notify_all_ledgers_received: Arc::new(Notify::new()),
        }
    }

    // Updates the number of active peers in preparation to wait for their responses
    pub async fn update_active_peer_count(&self) {

        // lock active peers and total peers
        let active_peers = self.active_peers.lock().await;
        let mut total_peers = self.total_peers.lock().await;

        // update the total number of active peers
        *total_peers = active_peers.len();
    }

    // Checks if all responses have been received for a particular request
    pub async fn check_all_block_decisions_received(&self, request_hash: &Vec<u8>) -> bool {

        // lock peer decisions map
        let peer_decisions_guard = self.peer_decisions.lock().await;

        // Get the number of true and false responses for the request, handling a None case w/ default values
        let request_decisions: &(u32, u32) = peer_decisions_guard.get(request_hash).unwrap_or(&(0, 0));
        let (true_count, false_count) = request_decisions;

        // get total number of peers
        let total_peers: u32 = *self.total_peers.lock().await as u32;

        // return true if all expectec responses have been recieved
        true_count + false_count == total_peers
    }

    // Awaits until all responses have been received using a notify struct
    pub async fn await_all_block_decisions(&self, request_hash: &Vec<u8>) {
        println!("Waiting for all peers to respond with their decision...");

        // retrieve active peers
        let total_peers: usize = *self.total_peers.lock().await;

        // if there are no active peers, return 
        if total_peers == 0 { return; }
        
        // notify the validator node that all responses have been recieved
        while !self.check_all_block_decisions_received(request_hash).await {
            self.notify_consensus.notified().await;
        }

        // Once we break out of the loop, it means all responses are in
        let peer_decisions_guard = self.peer_decisions.lock().await;
        let (true_count, false_count): &(u32, u32) = peer_decisions_guard.get(request_hash).unwrap();
        println!("Received all responses for request: {} yays, {} nays", true_count, false_count);
    }

    // awaits until all peer ledger states recieved from peers
    pub async fn await_all_peer_ledger_states_received(&self) {
        println!("Waiting for all peers to respond with their ledger states...");

        // get total number of peers
        let total_peers: usize = *self.total_peers.lock().await as usize;

        // if there are no active peers, return 
        if total_peers == 0 { return; }

        // notify the validator node that all responses have been recieved
        while self.peer_ledger_states.lock().await.len() != total_peers {
            self.notify_all_ledgers_received.notified().await;
        }

        println!("All local ledger states from peers...");
    }


}

/**
 * @notice run_validation() is a wrapper called within main.rs that instigates the process of initializing the data structures in 
 * the ValidatorNode struct, sending a request to active peer node for the majority state of the networks and connecting a TCP 
 * listener to the network to start listening for incomring requests.
 */
pub async fn run_validation(private_key: &String) {
    println!("\nBooting up validator node..."); 

    // init validator node struct w/ empty blockchain and merkle tree
    let validator_node: ValidatorNode = ValidatorNode::new();
    
    // listen for and process incoming request
    network::start_listening(validator_node.clone()).await;
} 

// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Account Creation Verification Logic

/**
 * @notice handle_account_creation_request() is an asynchronous event handler that is called by network::handle_incoming_message() following a
 * recieved request for a new account creation. This function is responsible for fascilitating the independent validation of the request, sending
 * a request to the network for consensus, and adding the account to the ledger if the network reaches a consensus that the request is valid. 
 */
pub async fn handle_account_creation_request( request: Value, validator_node: ValidatorNode) -> Result<(), String> { 
    println!("Handling account creation request...");

    // perform independent vallidation and store decision in validator node struct
    verify_account_creation_independently(request.clone(), validator_node.clone()).await;

    // Prepare for responses by updating the count of active peers
    validator_node.update_active_peer_count().await;

    // send for network consensus on the request
    requests::send_consensus_request( request.clone(), validator_node.clone() ).await;

    // await responses from all peers (checks that num peers matches num responses)
    validator_node.await_all_block_decisions(
        &network::hash_network_request(request.clone()).await
    ).await;

    // Determine if the client's decision is the majority decision
    let peer_majority_decision: bool = consensus::determine_majority(request.clone(), validator_node.clone()).await;
    println!("Majority Decision: {}", peer_majority_decision);

    // return error if network consensus not reached
    if (peer_majority_decision == false) { return Err("invalid request".to_string());}

    // add the account to the ledger
    add_account_creation_to_ledger(request.clone() ,validator_node.clone()).await;

    Ok(())
}


/**
 * @notice verify_account_creation_independently() performs the independent verification of an account creation request recieved by a validator node.
 * First, the function checks that the account does not already exist in the merkle tree. If the account does not exist, the function will update the
 * client block decisions hashmap with the decision to accept the request.
 */
async fn verify_account_creation_independently( request: Value, validator_node: ValidatorNode) {
    println!("Performing Independent Validation of Account Creation Request...");

    // get public key from request
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // Lock the merkle tree 
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // make decision upone whether the account already exist in the tree
    let decision: bool;
    if merkle_tree_guard.account_exists(public_key.clone()) { decision = false; }else { decision = true; }
    println!("Client decision: {}", decision);

    // use SHA256 to hash the request
    let client_request_hash: Vec<u8> = network::hash_network_request(request).await;

    // lock the client decisions map
    let client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>> = validator_node.client_decisions.clone();
    let mut client_decisions_guard: MutexGuard<HashMap<Vec<u8>, bool>> = client_decisions.lock().await;

    // insert the decision in the client decision map
    client_decisions_guard.insert(client_request_hash.clone(), decision); 
}

/**
 * @notice add_account_creation_to_ledger() is an asynchronous function that adds a new account to the ledger after it has been
 * verified by the entire network. This function is called by handle_account_creation_request() after the account creation request
 * has been verified.
 */
async fn add_account_creation_to_ledger( request: Value, validator_node: ValidatorNode ) {
    println!("Adding Account Creation to Local Ledger...");

    // Retrieve public key and obfuscated private key hash from request
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let obfuscated_private_key_hash: Vec<u8> = hex::decode(request["obfuscated_private_key_hash"].as_str().unwrap_or_default()).unwrap();

    // Lock merkle tree for writing
    let merkel_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let mut merkel_tree_guard: MutexGuard<MerkleTree> = merkel_tree.lock().await;

    // Package account details in merkle_tree::Account struct and insert into merkle tree
    let account = Account { public_key: public_key.clone(), obfuscated_private_key_hash,  balance: 0, nonce: 0,};

    // Insert the account into the merkle tree
    merkel_tree_guard.insert_account(account);
    assert!(merkel_tree_guard.account_exists(public_key.clone()));

    // Get time of account creation
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Package request details in Request enum 
    let new_account_request = Block::NewAccount { 
        address: public_key, 
        account_balance: 0,
        time: time, 
        hash: Vec::new(),
    };

    // Lock blockchain for writing
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();    
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // Write the  acount creation in the blockchain
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_block_to_chain(new_account_request);   

}

// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Transaction Verification Logic

/**
 * @notice handle_incoming_transaction_request() is an asynchronous function that verifies a transaction request on the blockchain network.
 * This function is called by handle_incoming_message() when a new transaction request is received.
 * @dev The function will verify the validity of the transaction request, update the sender and recipient balances in the
 * merkle tree, and store the request in the blockchain.
 */
pub async fn handle_transaction_request(request: Value, validator_node: ValidatorNode) -> Result<bool, String> { 
    println!("vHandling transaction request..."); 

    // verify the transaction independently 
    verify_transaction_independently(request.clone(), validator_node.clone()).await;

    // Prepare for responses by updating the count of active peers
    validator_node.update_active_peer_count().await;

    // send for network consensus on the request
    requests::send_consensus_request( request.clone(), validator_node.clone() ).await;

    // await responses from all peers (checks that num peers matches num responses)
    validator_node.await_all_block_decisions(
        &network::hash_network_request(request.clone()).await
    ).await;

    // Determine if the client's decision is the majority decision
    let peer_majority_decision: bool = consensus::determine_majority(request.clone(), validator_node.clone()).await;

    // print peer majority decision
    println!("Majority Decision: {}", peer_majority_decision);

    // return false if network consensus not reached
    if (peer_majority_decision == false) { return Ok(false);}

    // add the transaction to the ledger
    add_transaction_to_ledger(request.clone(), validator_node.clone()).await;

    Ok(true) 
}

/**
 * @notice verify_transaction_independently() is an asynchronous function that performs the independent verification of a transaction
 * request recieved by a validator node. The decision of whether to accept the transaction is based on the information that was recieved by 
 * this particular node in isolation. The resulting decision will be sent to all other validator nodes to determine a majority decision. 
 * @dev the checks this function performs include: verifying the sender's private key (using zk_proof module), ensuruing 
 * the sender and recipient accounts both exist in the merkle tree, and that the sender has sufficient balance to send the
 * transaction.
 */
async fn verify_transaction_independently(request: Value, validator_node: ValidatorNode)-> bool {
    println!("Performing Independent Validation of Transaction Request...");
  
    // retrieve request information
    let sender_address: Vec<u8> = request["sender_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let recipient_address: Vec<u8> = request["recipient_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let transaction_amount: u64 = request["amount"].as_str().unwrap_or_default().parse().unwrap_or_default();
 
    // lock client decisions map
    let client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>> = validator_node.client_decisions.clone();
    let mut client_decisions_guard: MutexGuard<HashMap<Vec<u8>, bool>> = client_decisions.lock().await;

    // Lock the merkle tree while accessing sender account info
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // declare decision
    let decision: bool;

    // Reject decision if the sender account does not exist in the merkle tree
    if  merkle_tree_guard.account_exists(sender_address.clone()) != true { 
        decision = false;
    }
     // Reject decision if the recipient account does not exist in the merkle tree
    else if  merkle_tree_guard.account_exists(recipient_address.clone()) != true { 
        decision = false;
    }
    // Reject decision if the zk-proof fails
    else if zk_proof::verify_points_sum_hash(
        &request["encoded_key_curve_point_1"].as_str().unwrap_or_default().to_string(),
        &request["encoded_key_curve_point_2"].as_str().unwrap_or_default().to_string(),
        merkle_tree_guard.get_private_key_hash(sender_address.clone()).unwrap(),
        sender_address.clone(),
        validator_node.clone()
    ).await != true { 
        decision = false; 
    }
    // Reject decision if the sender does not have enough balance to send the transaction
    else if transaction_amount > merkle_tree_guard.get_account_balance(sender_address.clone()).unwrap(){
        decision = false;
    }
    // Accept decision if all checks pass
    else { decision = true; }
        
    // insert the decision in the client decision map 
    client_decisions_guard.insert(
        network::hash_network_request(request.clone()).await, decision
    );

    // return true if all checks pass
    decision
} // TODO issue #7 to be implemented here


/**
 * @notice add_transaction_to_ledger() is an asynchronous function that adds a transaction after it has been verified by the 
 * entire network to both the merkle tree and the blockchain. 
*/
async fn add_transaction_to_ledger(request: Value, validator_node: ValidatorNode) {
    println!("Adding transaction to ledger...");

    // get the sender and recipient addresses from the request
    let sender_address: Vec<u8> = request["sender_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let recipient_address: Vec<u8> = request["recipient_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    
    // get the transaction amount from the request
    let amount: u64 = request["amount"].as_str().unwrap_or_default().parse().unwrap_or_default();

    // retrieve and lock the merkle tree 
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // retrieve the sender and recipient balances from the merkle tree
    let mut sender_balance: u64 = merkle_tree_guard.get_account_balance(sender_address.clone()).unwrap();
    let mut recipient_balance: u64 = merkle_tree_guard.get_account_balance(recipient_address.clone()).unwrap();

    // retrieve the sender's nonce and the current time
    let sender_nonce: u64 = merkle_tree_guard.get_nonce(sender_address.clone()).unwrap();
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // determine new account balances
    sender_balance -= amount; recipient_balance += amount;

    // Change account balances in the merkle tree to the ones updated
    merkle_tree_guard.change_balance(sender_address.clone(), sender_balance);
    merkle_tree_guard.increment_nonce(sender_address.clone());
    merkle_tree_guard.change_balance(recipient_address.clone(), recipient_balance);
    
    // Package request details in Request enum 
    let new_account_request = Block::Transaction {  // TODO this could probably be replaced by the block struct itself
        sender: sender_address,  
        sender_balance,
        sender_nonce, 
        recipient: recipient_address, 
        recipient_balance,
        amount, 
        time, 
        hash: Vec::new(),
    };

    // Retrieve and lock the blockchain
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // Write a new block to the blockchain
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_block_to_chain(new_account_request);   

}

/**
 * @notice save_failed_transaction_json() is an asynchronous function that saves a json file to the local directory that contains the number 1.
 * If a failed transaction is detected in network.rs, this function will save the indcation to the root dir, used for integration testing.
 */
pub async fn save_failed_transaction_json(){

    // save a simple json file that just contains the number 1 for failed transaction
    let message_json = serde_json::to_string(&1).unwrap();
    std::fs::write("failed_transaction.json", message_json).unwrap();
}

// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Faucet Verification Logic

/**
 * @notice verify_faucet_request() is an asynchronous function that verifies a faucet request on the blockchain network.
 * This results in an account balance increase of FAUCET_AMOUNT for the provided public key. This function is called by 
 * handle_incoming_message() when a new faucet request is received.
 */
pub async fn handle_faucet_request(request: Value, validator_node: ValidatorNode)  -> Result<(), Box<dyn std::error::Error>> {
    println!("Handling request to use faucet...");

    // verify the faucet request independently
    if verify_faucet_request_independently(
        request.clone(), validator_node.clone()

    ).await != true {  return Ok(()); }

    // Prepare for responses by updating the count of active peers
    validator_node.update_active_peer_count().await;

    // send for network consensus on the request
    requests::send_consensus_request( request.clone(), validator_node.clone() ).await;

    // await responses from all peers (checks that num peers matches num responses)
    validator_node.await_all_block_decisions(
        &network::hash_network_request(request.clone()).await
    ).await;

    // Determine if the client's decision is the majority decision
    let peer_majority_decision: bool = consensus::determine_majority(
        request.clone(), validator_node.clone()
    ).await;

    // return error if network consensus not reached
    if (peer_majority_decision == false) { return Ok(()); }

    // add the faucet request to the ledger
    add_faucet_request_to_ledger(request.clone(), validator_node.clone()).await;

    Ok(())
}

/**
 * @notice verify_faucet_request_independently() is an asynchronous function that verifies a faucet request on the blockchain network
 * based on the information that was recieved by this particular node in isolation. The resulting decision will be sent to all other
 * validator nodes to determine a majority decision that will be accepted by the network regardless of the individual validator node's decision.
 */
async fn verify_faucet_request_independently(request: Value, validator_node: ValidatorNode) -> bool {
    println!("Performing independent validation of faucet request..");

    // Lock the merkle tree while accessing sender account info
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // Get the public key requesting the faucet
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // Check that the account doesnt already exist in the tree
    let decision: bool;
    if !merkle_tree_guard.account_exists(public_key.clone()) { decision = false } else { decision = true; }
    
    // use SHA256 to hash the request
    let client_request_hash: Vec<u8> = network::hash_network_request(request).await;

    // lock the client decisions map
    let client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>= validator_node.client_decisions.clone();
    let mut client_decisions_guard: MutexGuard<HashMap<Vec<u8>, bool>> = client_decisions.lock().await;

    // insert the decision in the client decision map
    client_decisions_guard.insert(
        client_request_hash.clone(), decision
    );

    decision
}

/**
 * @notice add_faucet_request_to_ledger() is an asynchronous function that adds a faucet request to the ledger after it has been
 * verified by the entire network. This function is called by handle_faucet_request() after the faucet request has been verified.
 */
async fn add_faucet_request_to_ledger(request: Value, validator_node: ValidatorNode) {
    println!("Adding faucet request to ledger...");

    // Lock the merkle tree for writing
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // get the public key from the request
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // get the account balance and update it
    let account_balance: u64 = merkle_tree_guard.get_account_balance(public_key.clone()).unwrap();
    let new_balance: u64 = account_balance + FAUCET_AMOUNT;

    // update the account balance
    merkle_tree_guard.change_balance(public_key.clone(), new_balance);

    // Update the blockchain with the faucet request
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Package processed request details in a Block
    let new_account_request = Block::Faucet { 
        address: public_key, 
        account_balance: new_balance,
        time: time, 
        hash: Vec::new(),
    };
    
    // Lock blockchain for writing
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // store and validate the request
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_block_to_chain(new_account_request);
}



// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Heartbeat Update Logic

/**
 * @notice handle_heartbeat_request() is an asynchronous function that handles incoming heartbeat requests from other nodes on the network.
 */
pub async fn handle_heartbeat(request: Value, validator_node: ValidatorNode) -> Result<(), String> {
    println!("Handling heartbeat recieved...");

    // Extract the port address from the request
    let port_address: String = request["port_address"].as_str()
        .ok_or_else(|| "Failed to extract port address from heartbeat request".to_string())?
        .to_string();

    // Retrieve and lock the active_peers vector
    let active_peers: Arc<Mutex<Vec<(String, u64)>>> = validator_node.active_peers.clone();
    let mut active_peers_guard = active_peers.lock().await;

    // Get the current time
    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Failed to get current time".to_string())?
        .as_secs();

    // add the peer to the active peers list if it is not already present
    if !active_peers_guard.iter().any(|peer| peer.0 == port_address) {
        active_peers_guard.push((port_address.clone(), current_time));        
    }else{
        // update the timestamp of the peer
        for peer in active_peers_guard.iter_mut() {
            if peer.0 == port_address {
                peer.1 = current_time;
            }
        }
    }

    // Remove peers that have not sent a heartbeat within the HEARTBEAT_TIMEOUT
    active_peers_guard.retain(|peer| current_time - peer.1 < HEARTBEAT_TIMEOUT.as_secs());


    // Print all active peers
    println!("Active Peers: {:?}", active_peers_guard);

    Ok(())
}


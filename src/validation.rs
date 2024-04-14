use tokio::sync::{Mutex, MutexGuard};
use tokio::runtime::Runtime;
use serde_json::Value;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use std::collections::HashMap;

use crate::blockchain;
use crate::blockchain::{BlockChain, Request};
use crate::merkle_tree::{MerkleTree, Account};
use crate::constants::{VERBOSE_STACK, FAUCET_AMOUNT};
use crate::chain_consensus;
use crate::consensus;
use crate::zk_proof;
use crate::network;
use crate::requests;

/**
 * @module validation.rs contains the necessary data structures for running a validator node, as well as the event handler logic for determining 
 * the validity of incoming requests recieved by the validator node. The validator node is responsible for verifying the correctness on the client 
 * side using the information known to the node. Then, once an indepdent decision is made, the node reaches out to peers for consensus on the 
 * request. If the network reaches a consensus that the request is valid, the request will be added to the blockchain and merkle tree. Otherwise,
 * the request will be rejected. Upon the  validation of a request, the update of the blockchain and merkle tree will also be performed within this 
 * module.
 */
/**
 * @struct The ValidatorNode struct contains the local ledger state of this validator node. A well as other datastructures used to fascilitate
 * the validation process. All datastructures are wrapperd in Arc<Mutex<>> to allow for concurrent access by multiple tasks while avoiding race
 * conditions.
 * 
 * @param blockchain: Arc<Mutex<BlockChain>> - The BlockChain struct is the ledger of all transactions that have occured on the network. It stores 
 * a linked list of Block enum structs (see blockchain.rs for details on this implementation). As new transactions are verified, they are added to
 * the blockchain.
 * 
 * @param merkle_tree: Arc<Mutex<MerkleTree>> - The merkle tree is a binary tree built built upwards from an iterable of account addresses that 
 * stores the account balances of all users on the network. As the tree is assembled, nodes are hashed into each other to produce a single root hash
 * that is used as unique identifier for the state of the stored accounts network at a given time.
 * 
 * @param client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>> - This hashmap stores the decisions made by the client regarding the validity of
 * a given request. The key is the hash of the request and the value is a boolean representing the decision made by the client. This datastructure
 * is updated following the result of the independent validation of a request by the client.
 * 
 * @param peer_consensus_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>> - This hashmap stores the decisions made by each peer validator node
 * on the network regarding the validity of a given request. The key is the hash of the request and the value is a tuple of u32 integers representing
 * the number of yays vs nays for the request collected by peers. Decisions within this datastructure are updated following the independent validation
 * by the client. After this point, the client will send a request to the network for consensus on the request. Responces recieved from the network
 * will be updated within this structure.
 * 
 * @param client_port_address: String - The port address that the client is listening on for incoming connections. This is used to establish a
 * connection with the client from the network.
 * 
 * @param active_peers: Arc<Mutex<Vec<(String, u64)>>> - A vector of (String, u64) tuples containing the addresses of all active peers (as represented 
 * by their port address) on the network and the timestamp of the last recieved heartbeat from this peer. This datastructure is maintained by the 
 * validator node via a blocked on heartbeat protocol that listens for the periodic heartbeat of other nodes on the network. Nodes that fail to send a 
 * heartbeat within a given time frame are removed from the active peers list.
 */
#[derive(Clone)]
pub struct ValidatorNode {
    pub blockchain: Arc<Mutex<BlockChain>>,
    pub merkle_tree: Arc<Mutex<MerkleTree>>,
    pub peer_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>>>, 
    pub client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>,
    pub client_port_address: String,    
    pub active_peers: Arc<Mutex<Vec<(String, u64)>>>, 
}

impl ValidatorNode { // initializes datastructures
    pub fn new() -> ValidatorNode {
        ValidatorNode { 
            blockchain: Arc::new(Mutex::new(BlockChain::new())),
            merkle_tree: Arc::new(Mutex::new(MerkleTree::new())),
            peer_decisions: Arc::new(Mutex::new(HashMap::new())),
            client_decisions: Arc::new(Mutex::new(HashMap::new())),
            client_port_address: String::new(),
            active_peers: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/**
 * @notice run_validation() is a wrapper called within main.rs that instigates the process of initializing the data structures in 
 * the ValidatorNode struct, sending a request to active peer node for the majority state of the networks and connecting a TCP 
 * listener to the network to start listening for incomring requests.
 */
pub async fn run_validation(private_key: &String) { // TODO implemnt private key/staking idea. Private key to send tokens to
    if VERBOSE_STACK { println!("\nvalidation::run_validation() : Booting up validator node..."); }

    // init validator node struct w/ empty blockchain and merkle tree
    let validator_node: ValidatorNode = ValidatorNode::new();

    // send request to peers to update to network majority blockchain state.    // TODO focus on this component once new block consensns is implemented
    // chain_consensus::update_local_blockchain(validator_node.clone()).await;  // TODO modify this to also update the merkle tree at bootup

    // listen for and process incoming request
    network::start_listening(validator_node.clone()).await;
} 

// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Account Creation Verification Logic

/**
 * @notice handle_account_creation_request() is an asynchronous event handler that is called by network::handle_incoming_message() following a
 * recieved request for a new account creation. This function is responsible for fascilitating the independent validation of the request, sending
 * a request to the network for consensus, and adding the account to the ledger if the network reaches a consensus that the request is valid. 
 */
pub async fn handle_account_creation_request( request: Value, validator_node: ValidatorNode) -> Result<String, String> { 
    if VERBOSE_STACK { println!("validation::handle_account_creation_request() : Handling account creation...") };

    // perform independent vallidation and store decision in validator node struct
    verify_account_creation_independently(request.clone(), validator_node.clone()).await;

    // send for network consensus on the request
    requests::send_consensus_request( request.clone(), validator_node.clone() ).await;

    // TODO MAKE SURE BLOCK CONSENSUS RESPONCE IS ACTUALLY BEING SENT 

    // TODO Heartbeat mechanism (to be implemented) should be checked here for who a response is expected from

    // Determine if the client's decision is the majority decision
    let peer_majority_decision: bool = consensus::determine_majority(request.clone(), validator_node.clone()).await;

    // return error if network consensus not reached
    if (peer_majority_decision == false) { return Err("Network agreed the request was invalid".to_string());}

    // add the account to the ledger
    add_account_creation_to_ledger(request.clone() ,validator_node.clone()).await;

    // Return validated public key as a string
    Ok(request["public_key"].as_str().unwrap_or_default().to_string())
}


/**
 * @notice verify_account_creation_independently() performs the independent verification of an account creation request recieved by a validator node.
 * First, the function checks that the account does not already exist in the merkle tree. If the account does not exist, the function will update the
 * client block decisions hashmap with the decision to accept the request.
 */
async fn verify_account_creation_independently( request: Value, validator_node: ValidatorNode) {

    // get public key from request
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // clone the merkle tree and client block decisions from the validator node struct
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let client_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>> = validator_node.client_decisions.clone();

    // init client decision to false
    let decision: bool;

    // Lock the merkle tree while checking that the account doesnt already exist in the tree
    let merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;
    if merkle_tree_guard.account_exists(public_key.clone()) { decision = false; }else { decision = true;}

    // use SHA256 to hash the request
    let client_request_hash: Vec<u8> = network::hash_network_request(request).await;

    // otherwise lock the client block decisions and update the client decision
    let mut client_decisions_guard: MutexGuard<HashMap<Vec<u8>, bool>> = client_decisions.lock().await;
    client_decisions_guard.insert(client_request_hash.clone(), decision);
}

/**
 * @notice add_account_creation_to_ledger() is an asynchronous function that adds a new account to the ledger after it has been
 * verified by the entire network. This function is called by handle_account_creation_request() after the account creation request
 * has been verified.
 */
async fn add_account_creation_to_ledger( request: Value, validator_node: ValidatorNode ) {

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
    let new_account_request = blockchain::Request::NewAccount { new_address: public_key, time: time, };

    // Lock blockchain for writing
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();    
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // Write the  acount creation in the blockchain
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_request_to_chain(new_account_request);   

}

// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Transaction Verification Logic

/**
 * @notice handle_incoming_transaction_request() is an asynchronous function that verifies a transaction request on the blockchain network.
 * This function is called by handle_incoming_message() when a new transaction request is received.
 * @dev The function will verify the validity of the transaction request, update the sender and recipient balances in the
 * merkle tree, and store the request in the blockchain.
 */
pub async fn handle_transaction_request(request: Value, validator_node: ValidatorNode) -> Result<bool, String> { // TODO Simplify/decompose this function
    if VERBOSE_STACK { println!("validation::verify_transaction() : Transaction verification master function...") };

    // verify the transaction independently // TODO modify this to adjust the client decision map instead of returning an Option
    if verify_transaction_independently(request.clone(), validator_node.clone()).await != true {return Ok(false); }

    // TODO implement network consensus here

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
async fn verify_transaction_independently(request: Value, validator_node: ValidatorNode) -> bool {

    // retrieve sender and recipient addresses from request
    let sender_address: Vec<u8> = request["sender_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let recipient_address: Vec<u8> = request["recipient_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // retrieve transaction amount from request
    let transaction_amount: u64 = request["amount"].as_str().unwrap_or_default().parse().unwrap_or_default();
 
    // retrieve sender obfuscated private key parts from the request
    let curve_point_1: String = request["sender_obfuscated_private_key_part1"].as_str().unwrap_or_default().to_string(); 
    let curve_point_2: String = request["sender_obfuscated_private_key_part2"].as_str().unwrap_or_default().to_string();

    // Lock the merkle tree while accessing sender account info
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // Check that the both accounts already exist
    if merkle_tree_guard.account_exists(sender_address.clone()) != true { return false; }
    if merkle_tree_guard.account_exists(recipient_address.clone()) != true { return false; }

    // Verify the sender's private key using the zk_proof module 
    let sender_private_key_hash: Vec<u8> = merkle_tree_guard.get_private_key_hash(sender_address.clone()).unwrap();
    if zk_proof::verify_points_sum_hash(&curve_point_1, &curve_point_2, sender_private_key_hash) != true {  return false; }
        
    // get sender and recipient balances    
    let sender_balance: u64 = merkle_tree_guard.get_account_balance(sender_address.clone()).unwrap();
    if sender_balance < transaction_amount { return false; }

    true // return true if all checks pass
} // TODO issue #7 to be implemented here


/**
 * @notice add_transaction_to_ledger() is an asynchronous function that adds a transaction after it has been verified by the 
 * entire network to both the merkle tree and the blockchain. 
*/
async fn add_transaction_to_ledger(request: Value, validator_node: ValidatorNode) {

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
    let new_account_request = blockchain::Request::Transaction {  // TODO this could probably just be replaced by the request object
        sender_address, sender_nonce, recipient_address, amount, time, 
    };

    // Retrieve and lock the blockchain
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // Write a new block to the blockchain
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_request_to_chain(new_account_request);   

}

// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Faucet Verification Logic

/**
 * @notice verify_faucet_request() is an asynchronous function that verifies a faucet request on the blockchain network.
 * This results in an account balance increase of FAUCET_AMOUNT for the provided public key. This function is called by 
 * handle_incoming_message() when a new faucet request is received.
 */
pub async fn handle_faucet_request(request: Value, validator_node: ValidatorNode) -> Result<(), String> {
    if VERBOSE_STACK { println!("validation::verify_faucet_request() : Verifying faucet request...") };

    // verify the faucet request independently
    if verify_faucet_request_independently(request.clone(), validator_node.clone()).await != true { 
        return Err("Account doesn't exists".to_string()); 
    }

    //TODO implement network consensus here

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

    // Lock the merkle tree while accessing sender account info
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // Get the public key requesting the faucet
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // Check that the account doesnt already exist in the tree
    if !merkle_tree_guard.account_exists(public_key.clone()) { return false; }

    true // TODO eventually this will need to be replaced with an update to the client_decision_mapS
}

/**
 * @notice add_faucet_request_to_ledger() is an asynchronous function that adds a faucet request to the ledger after it has been
 * verified by the entire network. This function is called by handle_faucet_request() after the faucet request has been verified.
 */
async fn add_faucet_request_to_ledger(request: Value, validator_node: ValidatorNode) {


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
    let new_account_request: Request = Request::Faucet { address: public_key, time: time, };
    
    // Lock blockchain for writing
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // store and validate the request
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_request_to_chain(new_account_request);
}






/**
 * @noticd save_failed_transaction_json() is an async function that saves the most recent failed transaction as a
 * JSON file. This function is used to save the most recent failed transaction during integration testing.
 */
pub async fn save_failed_transaction_json(){

    // save a simple json file that just contains the number 1 for failed transaction
    let message_json = serde_json::to_string(&1).unwrap();
    std::fs::write("failed_transaction.json", message_json).unwrap();
}
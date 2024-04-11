use tokio::sync::{Mutex, MutexGuard};
use tokio::runtime::Runtime;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use std::collections::HashMap;

// Import the necessary libraries
use crate::blockchain;
use crate::blockchain::{BlockChain, Request, Block};
use crate::merkle_tree::{MerkleTree, Account};
use crate::constants::{VERBOSE_STACK, INTEGRATION_TEST, FAUCET_AMOUNT};
use crate::chain_consensus;
use crate::block_consensus;
use crate::zk_proof;
use crate::network;
use sha2::{Digest, Sha256};

/**
 * @notice validation.rs contains the logic for running a validator node. This involves setup and validation steps.
 * 
 * Setup:
 *    When a new validator node starts up, it must retrieve the current majority state of the blockchain and merkle tree and 
 *    store it locally. There are scenarios:
 * 
 *       1.) The node is starting a new blockchain from scratch. In this case, the node will create the genesis block and an
 *           empty merkle tree.
 * 
 *       2.) The node is joining an existing network. In this case, the node will send a request for the latest blockchain 
 *           and merkle tree state from all current peers in the network. The node will hash each blockchain and determine 
 *           the majority consensus of the network state based on the most common hash. The node will then update its local 
 *           blockchain to the majority chain and merkle tree.
 * 
 *    After the blockchain and merkle tree are up to date, the node will start listening for incoming connections from into 
 *    the network.
 * 
 * Validation:
 *    Once the node is listening for incoming connections on the specified port, It will spawn new tasks to handle each incoming 
 *    connection. Such connects could include requests for:
 * 
 *       - Account Creation 
 *       - Transaction
 *       - View Account Balance
 *       - Request for latest Blockchain and Merkle Tree
 * 
 *    TODO eventually, risc0 will be used to validate the correct execution of validator nodes. As well, staking/slashing and 
 *    TODO validator rewards will be need to be implemented at some point.
 */



/**
 * @notice ValidatorNode contains the local copies of the blockchain and merkle tree data structures that 
 * are maintained by independent validator nodes in the network.
 * @dev The blockchain and merkle tree are wrapped in Arc<Mutex> to allow for safe concurrent access between tasks.
 */
#[derive(Clone)]
pub struct ValidatorNode {
    pub blockchain: Arc<Mutex<BlockChain>>,
    pub merkle_tree: Arc<Mutex<MerkleTree>>,
    pub peer_consensus_decisions: Arc<Mutex<HashMap<Vec<u8>, (u32, u32)>>>, // request hash -> (decisions yay, decisions nay)
    pub client_block_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>, // request hash -> decision
    pub client_port_address: String,    
}

impl ValidatorNode {

    // construct chain with empty block and empty merkle tree
    pub fn new() -> ValidatorNode {
        ValidatorNode { 
            blockchain: Arc::new(Mutex::new(BlockChain::new())),
            merkle_tree: Arc::new(Mutex::new(MerkleTree::new())),
            peer_consensus_decisions: Arc::new(Mutex::new(HashMap::new())),
            client_block_decisions: Arc::new(Mutex::new(HashMap::new())),
            client_port_address: String::new(),
        }
    }
}

/**
 * @notice run_validation() is a wrapper called within main.rs that instigates the process of accessing
 * the network from the client side for running a validator node.
 */
pub fn run_validation(private_key: &String) { // TODO implemnt private key/staking idea. Private key to send tokens to
    if VERBOSE_STACK { println!("\nvalidation::run_validation() : Booting up validator node..."); }

    // init validator node struct w/ empty blockchain and merkle tree
    let validator_node: ValidatorNode = ValidatorNode::new();
    let validator_node_clone = validator_node.clone();

    // Establish a new tokio runtime
    let rt = Runtime::new().unwrap(); 

    // send request to peers to update to network majority blockchain state. 
    rt.block_on(async move { chain_consensus::update_local_blockchain(validator_node_clone).await; }); // TODO modify this to also update the merkle tree at bootup

    // listen for and process incoming request
    network::start_listening(validator_node.clone());
} 



// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Account Creation Verification Logic

/**
 * @notice verify_account_creation() is an asynchronous function that verifies the creation of a new account on the blockchain
 * network. This function is called by handle_incoming_message() when a new account creation request is received. 
 * @dev The function will verify the validity of the account creation request, insert the new account into the merkle tree, and 
 * store the request in the blockchain.
 */
pub async fn handle_account_creation_request( request: Value, validator_node: ValidatorNode) -> Result<String, String> { 
    if VERBOSE_STACK { println!("validation::verify_account_creation() : Verifying account creation...") };

    // retrieve new public key sent with request as Vec<u8> UTF-8 encoded
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let obfuscated_private_key_hash: Vec<u8> = hex::decode(request["obfuscated_private_key_hash"].as_str().unwrap_or_default()).unwrap();

    // get independent decision of request validity from the client node
    verify_account_creation_independently(
        public_key.clone(), 
        request.clone(),
        validator_node.merkle_tree.clone(),
        validator_node.client_block_decisions.clone(),
    ).await;

    // get network consensus on the request
    let peer_decision: bool = block_consensus::send_block_consensus_request(
        request.clone(), validator_node.clone()
    ).await;

    // return error if network consensus not reached
    if (peer_decision == false) { return Err("Network agreed the request was invalid".to_string());}

    // add the account to the ledger
    add_account_creation_to_ledger(
        public_key.clone(), 
        obfuscated_private_key_hash.clone(), 
        validator_node.merkle_tree.clone(), 
        validator_node.blockchain.clone()
    ).await;

    // Return validated public key as a string
    Ok(request["public_key"].as_str().unwrap_or_default().to_string())
}


/**
 * @notice verify_account_creation_independently() is an asynchronous function that verifies the creation of a new account on the blockchain
 * based on the information that was recieved by this particular node in isolation. The resulting decision will be sent to all other validator
 * nodes to determine a majority decision that will be accepted by the network regardless of the individual validator node's decision. 
 */
async fn verify_account_creation_independently(
    public_key: Vec<u8>, 
    request: Value,
    merkle_tree: Arc<Mutex<MerkleTree>>, 
    client_block_decisions: Arc<Mutex<HashMap<Vec<u8>, bool>>>
) {

    let mut decision: bool = false;

    // Lock the merkle tree while accessing sender account info
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // Check that the account doesnt already exist in the tree
    if merkle_tree_guard.account_exists(public_key.clone()) { return; }

    // use SHA256 to hash the request
    let mut hasher = Sha256::new();
    hasher.update(request.to_string());
    let client_request_hash: Vec<u8> = hasher.finalize().to_vec();

    // otherwise lock the client block decisions and update the decision
    let mut client_block_decisions_guard: MutexGuard<HashMap<Vec<u8>, bool>> = client_block_decisions.lock().await;
    client_block_decisions_guard.insert(client_request_hash.clone(), decision);
}

/**
 * @notice add_account_creation_to_ledger() is an asynchronous function that adds a new account to the ledger after it has been
 * verified by the entire network. This function is called by handle_account_creation_request() after the account creation request
 * has been verified.
 */
async fn add_account_creation_to_ledger(
    public_key: Vec<u8>, 
    obfuscated_private_key_hash: Vec<u8>, 
    merkle_tree: Arc<Mutex<MerkleTree>>,
    blockchain: Arc<Mutex<BlockChain>>
) {

    // Lock the merkle tree and blockchain while updating account balances and writing blocks
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;


    // Package account details in Account struct and insert into merkle tree
    let account = Account { 
        public_key: public_key.clone(), 
        obfuscated_private_key_hash: obfuscated_private_key_hash.clone(), 
        balance: 0, 
        nonce: 0, 
    };

    // Insert the account into the merkle tree
    merkle_tree_guard.insert_account(account);
    assert!(merkle_tree_guard.account_exists(public_key.clone()));

    // Get time of account creation
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Package request details in Request enum 
    let new_account_request = blockchain::Request::NewAccount { new_address: public_key, time: time, };

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

    // clone the blockchain and merkle tree from the validator node struct
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();

    // retrieve transaction details from request
    let sender_address: Vec<u8> = request["sender_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let recipient_address: Vec<u8> = request["recipient_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let amount: u64 = request["amount"].as_str().unwrap_or_default().parse().unwrap_or_default();

    // retrieve sender obfuscated private key parts
    let curve_point1: String = request["sender_obfuscated_private_key_part1"].as_str().unwrap_or_default().to_string(); 
    let curve_point2: String = request["sender_obfuscated_private_key_part2"].as_str().unwrap_or_default().to_string();

    // verify the transaction independently 
    if verify_transaction_independently(
        sender_address.clone(), 
        recipient_address.clone(), 
        amount,
        curve_point1,
        curve_point2,
        merkle_tree.clone()
    ).await != true { return Ok(false); }

    // TODO implement network consensus here

    // add the transaction to the ledger
    add_transaction_to_ledger(
        sender_address, 
        recipient_address, 
        amount, 
        merkle_tree, 
        blockchain
    ).await;


    Ok(true) 
}

/**
 * @notice verify_transaction_independently() is an asynchronous function that performs the verification of a transaction
 * request recieved by a validator node. This function determines the decision of whether or not to accept the transaction
 * based on the information that was recieved by this particular node in isolation. The resulting decision will be sent to 
 * all other validator nodes to determine a majority decision. 
 * @dev the checks this function performs include: verifying the sender's private key (using zk_proof module), ensuruing 
 * the sender and recipient accounts both exist in the merkle tree, and that the sender has sufficient balance to send the
 * transaction.
 */
async fn verify_transaction_independently(
    sender_address: Vec<u8>,  // public keys
    recipient_address: Vec<u8>, 
    transaction_amount: u64, 
    curve_point_1: String,  // obfuscated private key parts for zk proof scheme
    curve_point_2: String, 
    merkle_tree: Arc<Mutex<MerkleTree>>
) -> bool {

    // Lock the merkle tree while accessing sender account info
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
async fn add_transaction_to_ledger(
    sender_address: Vec<u8>, 
    recipient_address: Vec<u8>, 
    amount: u64, 
    merkle_tree: Arc<Mutex<MerkleTree>>, 
    blockchain: Arc<Mutex<BlockChain>>
){

    // Lock the merkle tree and blockchain while updating account balances and writing blocks
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // retrieve account detials from the merkle tree relavant to the tranasaction
    let mut sender_balance: u64 = merkle_tree_guard.get_account_balance(sender_address.clone()).unwrap();
    let mut recipient_balance: u64 = merkle_tree_guard.get_account_balance(recipient_address.clone()).unwrap();
    let sender_nonce: u64 = merkle_tree_guard.get_nonce(sender_address.clone()).unwrap();
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // determine new account balances
    sender_balance -= amount; recipient_balance += amount;

    // Change account balancges in the merkle tree 
    merkle_tree_guard.change_balance(sender_address.clone(), sender_balance);
    merkle_tree_guard.increment_nonce(sender_address.clone());
    merkle_tree_guard.change_balance(recipient_address.clone(), recipient_balance);
    
    // Package request details in Request enum 
    let new_account_request = blockchain::Request::Transaction { 
        sender_address, sender_nonce, recipient_address, amount, time, 
    };

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

    // Check that the account exist
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // verify the faucet request independently
    if verify_faucet_request_independently(
        public_key.clone(), 
        validator_node.merkle_tree.clone()
    ).await != true { return Err("Account already exists".to_string()); }

    //TODO implement network consensus here

    // add the faucet request to the ledger
    add_faucet_request_to_ledger(
        public_key.clone(), 
        validator_node.merkle_tree.clone(), 
        validator_node.blockchain.clone()
    ).await;

    Ok(())
}

/**
 * @notice verify_faucet_request_independently() is an asynchronous function that verifies a faucet request on the blockchain network
 * based on the information that was recieved by this particular node in isolation. The resulting decision will be sent to all other
 * validator nodes to determine a majority decision that will be accepted by the network regardless of the individual validator node's decision.
 */
async fn verify_faucet_request_independently(public_key: Vec<u8>, merkle_tree: Arc<Mutex<MerkleTree>>) -> bool {

    // Lock the merkle tree while accessing sender account info
    let merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // Check that the account doesnt already exist in the tree
    if !merkle_tree_guard.account_exists(public_key.clone()) { return false; }

    true
}

/**
 * @notice add_faucet_request_to_ledger() is an asynchronous function that adds a faucet request to the ledger after it has been
 * verified by the entire network. This function is called by handle_faucet_request() after the faucet request has been verified.
 */
async fn add_faucet_request_to_ledger(public_key: Vec<u8>, merkle_tree: Arc<Mutex<MerkleTree>>, blockchain: Arc<Mutex<BlockChain>>) {
    // Lock the merkle tree and blockchain while updating account balances and writing blocks
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;

    // get the account balance and update it
    let account_balance: u64 = merkle_tree_guard.get_account_balance(public_key.clone()).unwrap();
    let new_balance: u64 = account_balance + FAUCET_AMOUNT;

    // update the account balance
    merkle_tree_guard.change_balance(public_key.clone(), new_balance);

    // Update the blockchain with the faucet request
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let new_account_request: Request = Request::Faucet { address: public_key, time: time, };
    
    // store and validate the request
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_request_to_chain(new_account_request);
}

// ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ // Helper and Integration Testing Related Functions

/**
 * @notice print_chain() is an asynchronous function that prints the current state of the blockchain as maintained on the 
 * client side. This function is called by verify_account_creation() and verify_transaction() after storing the request in the 
 * blockchain.
 */
pub async fn print_chain_human_readable(blockchain: Arc<Mutex<BlockChain>>) { 

    // lock blockchain mutex for printing
    let blockchain_guard: MutexGuard<'_, BlockChain> = blockchain.lock().await; 

    println!("\nCurrent State of Blockchain as Maintained on Client Side:");
    for (i, block) in blockchain_guard.chain.iter().enumerate() {
        match block {
            Block::NewAccount { address, time, hash } => {
                
                // Directly use address as it's already a UTF-8 encoded hex string
                let hash_hex = hex::encode(hash); // Assuming hash is a Vec<u8> needing encoding
                let address = String::from_utf8(address.clone()).unwrap();
                println!("\nBlock {}: \n\tNew Account: {}\n\tTime: {}\n\tHash: {}", i, address, time, hash_hex);
            },
            Block::Transaction { sender, sender_nonce, recipient, amount, time, hash } => {

                // Directly use sender and recipient as they're already UTF-8 encoded hex strings
                let hash_hex = hex::encode(hash); // Assuming hash is a Vec<u8> needing encoding
                let sender = String::from_utf8(sender.clone()).unwrap();
                let recipient = String::from_utf8(recipient.clone()).unwrap();

                println!("\nBlock {}: \n\tSender: {}\n\tSender Nonce: {}\n\tRecipient: {}\n\tAmount: {}\n\tTime: {:}\n\tHash: {}", i, sender, sender_nonce, recipient, amount, time, hash_hex);
            },
            Block::Genesis { time } => {
                println!("\nBlock {}: \n\tGenesis Block\n\tTime: {:?}", i, time);
            },
            Block::Faucet { address, time, hash } => {
                
                // Directly use address as it's already a UTF-8 encoded hex string
                let hash_hex = hex::encode(hash); // Assuming hash is a Vec<u8> needing encoding
                let address = String::from_utf8(address.clone()).unwrap();
                println!("\nBlock {}: \n\tFaucet Request: {}\n\tTime: {}\n\tHash: {}", i, address, time, hash_hex);
            },
        }
    }
}


/**
 * @notice save_most_recent_block_json() is an asynchronous function that saves the most recent block in the 
 * blockchain as a JSON file. This function is used to save the most recent block during integration testing.
 */
#[derive(Serialize)]
#[serde(untagged)]
enum BlockJson {
    Genesis {
        time: u64,
    },
    Transaction {
        sender: String,
        recipient: String,
        amount: u64,
        time: u64,
        sender_nonce: u64,
        hash: String,
    },
    NewAccount {
        address: String,
        time: u64,
        hash: String,
    },
}

/**
 * @notice save_most_recent_block_json() is an asynchronous function that saves the most recent block in the
 * blockchain as a JSON file. This function is used to save the most recent block during integration testing.
 */
pub async fn save_most_recent_block_json(blockchain: Arc<Mutex<BlockChain>>) {
    let blockchain_guard: MutexGuard<'_, BlockChain> = blockchain.lock().await;

    if let Some(most_recent_block) = blockchain_guard.chain.last() {
        let block_json = match most_recent_block {
            Block::Genesis { time } => BlockJson::Genesis { time: *time },
            Block::Transaction { sender, recipient, amount, time, sender_nonce, hash } => BlockJson::Transaction {
                sender: String::from_utf8(sender.clone()).unwrap_or_default(),
                recipient: String::from_utf8(recipient.clone()).unwrap_or_default(),
                amount: *amount,
                time: *time,
                sender_nonce: *sender_nonce,
                hash: hex::encode(hash),
            },
            Block::NewAccount { address, time, hash } => BlockJson::NewAccount {
                address: String::from_utf8(address.clone()).unwrap_or_default(),
                time: *time,
                hash: hex::encode(hash),
            },
            Block::Faucet { address, time, hash } => BlockJson::NewAccount {
                address: String::from_utf8(address.clone()).unwrap_or_default(),
                time: *time,
                hash: hex::encode(hash),
            },
        };
        let message_json = serde_json::to_string(&block_json).unwrap();
        std::fs::write("most_recent_block.json", message_json).unwrap();
    } else {
        eprintln!("Blockchain is empty.");
    }
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
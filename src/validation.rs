use tokio::sync::{Mutex, MutexGuard};
use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;
use tokio::runtime::Runtime;
use serde_json::Value;
use serde::Serialize;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use hex;

// Import the necessary libraries
use crate::blockchain::{BlockChain, Request, Block};
use crate::merkle_tree::{MerkleTree, Account};
use crate::zk_proof::verify_points_sum_hash;
use crate::constants::{PORT_NUMBER, VERBOSE_STACK, INTEGRATION_TEST};
use crate::get_consensus::update_local_blockchain;

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
    blockchain: Arc<Mutex<BlockChain>>,
    merkle_tree: Arc<Mutex<MerkleTree>>,
}

impl ValidatorNode {

    // construct chain with empty block and empty merkle tree
    pub fn new() -> ValidatorNode {
        ValidatorNode { 
            blockchain: Arc::new(Mutex::new(BlockChain::new())),
            merkle_tree: Arc::new(Mutex::new(MerkleTree::new())),
        }
    }
}

/**
 * @notice run_validation() is a wrapper called within main.rs that instigates the process of accessing
 * the network from the client side for running a validator node.
 */
pub fn run_validation(private_key: &String) { // TODO implemnt private key/staking idea. Private key to send tokens to
    if VERBOSE_STACK { println!("validation::run_validation() : Booting up validator node..."); }

    // init validator node struct w/ empty blockchain and merkle tree
    let validator_node: ValidatorNode = ValidatorNode::new();

    // establish a new tokio runtime
    let rt = Runtime::new().unwrap(); 

    // clone the blockchain and merkle tree Arc<Mutex> values
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();

    // send request to peers to update to network majority blockchain state. 
    rt.block_on(async move { update_local_blockchain(blockchain).await; });

    // listen for and process incoming request
    start_listening(validator_node);
} 

/**
 * @notice listen_for_connections() asynchronously listens for incoming connections on the specified address. It will spawn 
 * new tasks to handle each incoming connection. Messages to the network are passed of to handle_incoming_message() for processing.
 */
fn start_listening(validator_node: ValidatorNode) {
    if VERBOSE_STACK { println!("validation::start_listening() : Listening for incoming connections on port {}...", PORT_NUMBER); }

    let rt = Runtime::new().unwrap(); // new tokio runtime
    
    // block_on async listening
    rt.block_on(async move {
        let listener = TcpListener::bind(PORT_NUMBER).await.unwrap(); //  connect to port

        // loop msg acceptance --> msg handling process
        while let Ok((mut socket, _)) = listener.accept().await {

            // clone Arc<Mutex> values before passing them to handle_incoming_message
            let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
            let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();

            // spawn a new task to handle each incoming msg
            tokio::spawn(async move {
                let mut buffer: Vec<u8> = Vec::new();
                if socket.read_to_end(&mut buffer).await.is_ok() && !buffer.is_empty() {
                    
                    // handle the incoming message
                    handle_incoming_message(&buffer, blockchain, merkle_tree).await;
                }
            });
        }
    });
 }

/**
 * @notice handle_incoming_message() asynchronously accepts a msg buffer and the current state of the merkle tree 
 * and blockchain. The buffer is parsed and the next step for the request is determined from the msg contents. 
 */
async fn handle_incoming_message(buffer: &[u8], blockchain: Arc<Mutex<BlockChain>>, merkle_tree: Arc<Mutex<MerkleTree>>) {
    if VERBOSE_STACK { println!("validation::handle_incoming_message() : Handling incoming message...") };

    // convert the buffer to a string and print
    let msg = String::from_utf8_lossy(&buffer[..buffer.len()]);

    // After parsing to JSON determine what to do with the msg
    if let Ok(request) = serde_json::from_str::<Value>(&msg) {
            
        // Handle Request to Make New Account
        if request["action"] == "make" { 
            
            // verify the account creation
            match verify_account_creation(request, merkle_tree, blockchain.clone()).await {
                Ok(public_key) => {
                    
                    // upon succesfull account creation, print blockchain state
                    if VERBOSE_STACK { print_chain_human_readable(blockchain.clone()).await;}

                    // if doing an integration test, save the most recent block as a json file
                    if INTEGRATION_TEST { save_most_recent_block_json(blockchain.clone()).await; }  
                },
                Err(e) => {eprintln!("Account creation Invalid: {}", e);}
            }
        } 

        // Handle Request to Make New Transaction
        else if request["action"] == "transaction" { 

            // verify the transaction
            match verify_transaction(request, merkle_tree, blockchain.clone()).await {
                Ok(success) => {

                    if VERBOSE_STACK {
                        if success { print_chain_human_readable(blockchain.clone()).await;}
                        else { eprintln!("Transaction failed to verify"); }
                    }                       

                    // 
                    if INTEGRATION_TEST { 
                        save_most_recent_block_json(blockchain.clone()).await;
                        if !success { save_failed_transaction_json().await; }
                     } 
                },
                Err(e) => {eprintln!("Transaction Validation Error: {}", e);}
            }
        } 

    else { eprintln!("Unrecognized action: {}", request["action"]);}
    } else {eprintln!("Failed to parse message: {}", msg);}
}


/**
 * @notice verify_account_creation() is an asynchronous function that verifies the creation of a new account on the blockchain
 * network. This function is called by handle_incoming_message() when a new account creation request is received. 
 * @dev The function will verify the validity of the account creation request, insert the new account into the merkle tree, and 
 * store the request in the blockchain.
 */
async fn verify_account_creation(request: Value, merkle_tree: Arc<Mutex<MerkleTree>>, blockchain: Arc<Mutex<BlockChain>>) -> Result<String, String> { // TODO Simplify/decompose this function
    if VERBOSE_STACK { println!("validation::verify_account_creation() : Verifying account creation...") };

    // retrieve new public key sent with request as Vec<u8> UTF-8 encoded
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let obfuscated_private_key_hash: Vec<u8> = hex::decode(request["obfuscated_private_key_hash"].as_str().unwrap_or_default()).unwrap();

    // Lock the merkle tree
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // Check that the account doesnt already exist in the tree
    if merkle_tree_guard.account_exists(public_key.clone()) { return Err("Account already exists".to_string());} 

    // Package account details in Account struct and insert into merkle tree
    let account = Account { public_key: public_key.clone(), obfuscated_private_key_hash: obfuscated_private_key_hash.clone(), balance: 0, nonce: 0, };

    // Insert the account into the merkle tree
    merkle_tree_guard.insert_account(account);
    assert!(merkle_tree_guard.account_exists(public_key.clone()));

    // Get request details
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();

    // Package request details in Request enum and return
    let new_account_request: Request = Request::NewAccount { new_address: public_key, time: time, };

    // store and validate the request
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_request_to_chain(new_account_request);   

    // Return validated public key as a string
    Ok(request["public_key"].as_str().unwrap_or_default().to_string())
}


/**
 * @notice verify_transaction() is an asynchronous function that verifies a transaction request on the blockchain network.
 * This function is called by handle_incoming_message() when a new transaction request is received.
 * @dev The function will verify the validity of the transaction request, update the sender and recipient balances in the
 * merkle tree, and store the request in the blockchain.
 */
async fn verify_transaction(request: Value, merkle_tree: Arc<Mutex<MerkleTree>>, blockchain: Arc<Mutex<BlockChain>>) -> Result<bool, String> { // TODO Simplify/decompose this function
    if VERBOSE_STACK { println!("validation::verify_transaction() : Verifying transaction...") };

    // retrieve transaction details from request
    let sender_address: Vec<u8> = request["sender_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let recipient_address: Vec<u8> = request["recipient_public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let amount: u64 = request["amount"].as_str().unwrap_or_default().parse().unwrap_or_default();

    // retrieve sender obfuscated private key parts
    let curve_point1: String = request["sender_obfuscated_private_key_part1"].as_str().unwrap_or_default().to_string(); 
    let curve_point2: String = request["sender_obfuscated_private_key_part2"].as_str().unwrap_or_default().to_string();

    // Lock the merkle tree while accessing sender accountinfo
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // retrieve sender's account
    let sender_account: Account = merkle_tree_guard.get_account(sender_address.clone()).unwrap();

    // retrieve sender's private key hash
    let sender_private_key_hash: Vec<u8> = sender_account.obfuscated_private_key_hash.clone();

    // decompress the curve points
    if verify_points_sum_hash(&curve_point1, &curve_point2, sender_private_key_hash) != true { 
        return Ok(false); 
    }

    // Check that the account doesnt already exist in the tree
    if merkle_tree_guard.account_exists(sender_address.clone()) != true { return Ok(false); }
    if merkle_tree_guard.account_exists(recipient_address.clone()) != true { return Ok(false); }
        
    // get sender and recipient balances    
    let mut sender_balance: u64 = merkle_tree_guard.get_account_balance(sender_address.clone()).unwrap();
    let mut recipient_balance: u64 = merkle_tree_guard.get_account_balance(recipient_address.clone()).unwrap();

    // Check that the sender has sufficient balance
    if sender_balance < amount { return Ok(false);}

    // update balances and sender nonce
    sender_balance -= amount; recipient_balance += amount;
    merkle_tree_guard.change_balance(sender_address.clone(), sender_balance);
    merkle_tree_guard.increment_nonce(sender_address.clone());
    merkle_tree_guard.change_balance(recipient_address.clone(), recipient_balance);
    
    // retrieve other Request details
    let sender_nonce: u64 = merkle_tree_guard.get_nonce(sender_address.clone()).unwrap();
    let time: u64 = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    // Package request details in Request enum 
    let new_account_request: Request = Request::Transaction { 
        sender_address, sender_nonce, recipient_address, amount, time, 
    };

    // store and validate the request
    let mut blockchain_guard: MutexGuard<BlockChain> = blockchain.lock().await;
    blockchain_guard.store_incoming_requests(&new_account_request);
    blockchain_guard.push_request_to_chain(new_account_request);   

    Ok(true) 
}


/**
 * @notice print_chain() is an asynchronous function that prints the current state of the blockchain as maintained on the 
 * client side. This function is called by verify_account_creation() and verify_transaction() after storing the request in the 
 * blockchain.
 */
async fn print_chain_human_readable(blockchain: Arc<Mutex<BlockChain>>) { 

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
async fn save_most_recent_block_json(blockchain: Arc<Mutex<BlockChain>>) {
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
async fn save_failed_transaction_json(){

    // save a simple json file that just contains the number 1 for failed transaction
    let message_json = serde_json::to_string(&1).unwrap();
    std::fs::write("failed_transaction.json", message_json).unwrap();
}
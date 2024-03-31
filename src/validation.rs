use tokio::sync::{Mutex, MutexGuard, mpsc};
use tokio::time;
use tokio::net::{TcpListener};
use tokio::io::{AsyncReadExt};
use tokio::runtime::Runtime;
use serde_json::{Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use std::error::Error;
use hex;

// Import the necessary libraries
use crate::blockchain::{BlockChain, Request, Block};
use crate::merkle_tree::{MerkleTree, Account};

/**
 * @notice validation.rs contains the logic for running the validation process on the client side of the node software.
 * This process can roughly be split up into two sections: setup and validation.
 * 
 * Setup:
 *  When a new validator node starts up, it will need to retireve the current majority state of the blockchain and merkle tree 
 *  into local memory (will need to be stored on disk for larger networks, but for now memory is fine). There are two cases that
 *  need to be handled here:
 * 
 *    - The node is starting a new blockchain from scratch. In this case, the node will create a single genesis block and an
 *      empty merkle tree.
 *    - The node is joining an existing network. In this case, the node will need to request the latest blockchain and merkle
 *      tree from all current peers in the network. The node will hash each retireved blockchain and determine the majority 
 *      chain based on the hash. The node will then update its local blockchain to the majority chain and merkle tree.
 * 
 *  After the blockchain and merkle tree have been set up, then the node will start listening for incoming connections from
 *  other nodes in the network.
 * 
 * Validation:
 *  The validation process is an asynchronous task that listens for incoming connections on the specified address. It will spawn
 *  new tasks to handle each incoming connection. There will be in total 4 types of incoming messages that a node will respond 
 *  to:
 * 
 *   - Account Creation
 *   - Transaction
 *   - View Account Balance
 *   - Request for latest Blockchain and Merkle Tree
 */


const PORT_NUMBER: &str = "127.0.0.1:8080";

// durtion to listen for blockchain records from peers when booting up
const DURATION_GET_PEER_CHAINS: Duration = Duration::from_secs(1);  

/**
 * @notice ValidatorNode is a struct that contains the blockchain and merkle tree data structures.
 * @dev blockchain and merkle tree are the local copies of network state that is maintained and 
 * distributed by individual nodes to other nodes in the network.
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
 * the network from the client side for running the validation process. 
 */
pub fn run_validation(private_key: &String) { // ! TDOO implemnt private key/staking idea. Private key to send tokens to
    println!("Booting Up Validator Node...\n");

    // init mutable validator node struct and run validation
    let mut validator_node: ValidatorNode = ValidatorNode::new();

    let rt = Runtime::new().unwrap(); // new Tokio runtime

    // Send request to peers to update to network majority blockchain state. 
    rt.block_on(async { update_local_blockchain(validator_node.blockchain.clone()).await; });

    // listen for and process incoming request
    start_listening(validator_node);
}

/**
 * @notice update_local_blockchain() 
*/
async fn update_local_blockchain(local_chain: Arc<Mutex<BlockChain>>) {
    println!("Accepting blockchain records from peers for {} seconds...\n", DURATION_GET_PEER_CHAINS.as_secs());
 
     // Get majority network chain state if available
     match collect_blockchain_records(DURATION_GET_PEER_CHAINS).await {
 
         Ok(peer_blockchains) => {
 
             // Determine the majority blockchain consensus
             let majority_blockchain: Option<BlockChain> = determine_majority_blockchain(peer_blockchains);
     
             // If >50% of peers agree on a blockchain, update the local blockchain
             if let Some(majority_chain) = majority_blockchain {
 
                 // Lock the mutex to safely update the blockchain
                 let mut local_blockchain_guard = local_chain.lock().await;
                 *local_blockchain_guard = majority_chain;
 
                 println!("Blockchain updated to majority state of peer validator nodes.");
             }
         },
         Err(e) => println!("Error collecting blockchain records: {}", e),
     }
 }
 
 /**
  * @notice collect_blockchain_records() is an asynchronous function that listens for incoming connections on the specified
  * port and collects blockchain records from peer validation nodes. This function is called by update_local_blockchain() and
  * is not intended to be called directly.
  */
 async fn collect_blockchain_records(duration: Duration) -> Result<Vec<BlockChain>, Box<dyn Error>> {
 
     // MPSC (multi-producer, single-consumer) channel to collect blockchain records between tasks
     let (tx, mut rx) = mpsc::channel(32); 
 
     // Create a new listener on the specified port
     let listener = TcpListener::bind(PORT_NUMBER).await?;
     let end_time = time::Instant::now() + duration; // end time for listening
 
     // Spawn a new task to listen for incoming connections
     tokio::spawn(async move {
         while time::Instant::now() < end_time { // loop until time is up
 
             tokio::select! {
 
                 // Break if time is up
                 _ = time::sleep_until(end_time) => { break; }
 
                 // Accept incoming connections
                 Ok((mut socket, _)) = listener.accept() => {
                         
                     // Clone the sender to send the blockchain records to the channel
                     let tx_clone = tx.clone();
                     tokio::spawn(async move {
 
                         // Read blockchain data into buffer, convert to blockchain struct, and send to channel  // ! TODO  simplify this
                         let mut buffer = Vec::new();
                         if socket.read_to_end(&mut buffer).await.is_ok() {
                             if let Ok(blockchain) = serde_json::from_slice::<BlockChain>(&buffer) {  // deserialize into blockhain struct
                                 let _ = tx_clone.send(blockchain).await; // Handle error as needed
                             }
                         }
                     });
                 }
             }
         }
     });

    // print that none were collected if none were collected
    if rx.recv().await.is_none() {
        println!("No blockchain records collected from peers...\n");
    }else{
        println!("Blockchain records collected from peers...\n");   
    }
 
     // Collect records from the channel
     let mut records: Vec<BlockChain> = Vec::new();
     while let Some(blockchain) = rx.recv().await {
        records.push(blockchain);
     }
 
     Ok(records)
 }
 
 /**
  * @notice determine_majority_blockchain() is a function that takes a vector of blockchain records, takes
  * the hash of each blockchain, and determines the majority blockchain based on the hash. 
  * @dev This function is called directly after requesting blockchain records from peers when updating the local 
  * blockchain to the majority state within update_local_blockchain().
  */
 fn determine_majority_blockchain(blockchains: Vec<BlockChain>) -> Option<BlockChain> {
 
     // HashMap to store the hash votes
     let mut hash_votes: HashMap<Vec<u8>, i32> = HashMap::new();
 
     // Count the votes for each blockchain hash
     for blockchain in &blockchains {
 
         // hash chain and either insert or increment vote count
         let hash: Vec<u8> = blockchain.hash_blockchain();
         *hash_votes.entry(hash).or_insert(0) += 1; 
     }
 
     // Find the hash with the most votes
     let majority_hash: Option<Vec<u8>> = hash_votes.into_iter()
         .max_by_key(|&(_, count)| count)
         .map(|(hash, _)| hash);
 
     // If there's a majority hash, find and return the corresponding blockchain
     majority_hash.and_then(|hash|
         blockchains.into_iter().find(|blockchain| blockchain.hash_blockchain() == hash)
     )
 }
 
 

/**
 * @notice listen_for_connections()S is an asynchronous function that listens for incoming connections on the
 * specified address. It will spawn new tasks to handle each incoming connection. This function serves as the 
 * fascilitator of the validation process on the client side for the node software.
 * @dev This function is called by run_validation() and is not intended to be called directly.
 */
fn start_listening(validator_node: ValidatorNode) {
    let rt = Runtime::new().unwrap(); // new tokio runtime
    
    // block_on async listening
    rt.block_on(async move {
        let listener = TcpListener::bind(PORT_NUMBER).await.unwrap(); //  connect to port
        println!("Validator node listening on {}\n", PORT_NUMBER);

        // loop msg acceptance --> msg handling process
        while let Ok((mut socket, _)) = listener.accept().await {

            // clone Arc<Mutex> values before passing them to handle_incoming_message
            let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
            let merkle_tree: Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();

            // spawn a new task to handle each incoming msg
            tokio::spawn(async move {
                let mut buffer: Vec<u8> = Vec::new();
                if socket.read_to_end(&mut buffer).await.is_ok() && !buffer.is_empty() {
                    println!("Request Recieved...\n");

                    handle_incoming_message(&buffer, blockchain, merkle_tree).await;
                }
            });
        }
    });
 }

/**
 * @notice handle_incoming_message() is an asynchronous function that takes a buffer and size, converts the buffer
 * to a string, parses to JSON and then determines which validation function to send the message to. This function
 * is called by listen_for_connections() and is not intended to be called directly.
 */
async fn handle_incoming_message(buffer: &[u8], blockchain: Arc<Mutex<BlockChain>>, merkle_tree: Arc<Mutex<MerkleTree>>) {
    println!("Handling incoming message...\n");

    // convert the buffer to a string and print
    let msg = String::from_utf8_lossy(&buffer[..buffer.len()]);
        
    println!("Message: {}\n", msg);

    // After parsing to JSON determine what to do with the msg
    if let Ok(request) = serde_json::from_str::<Value>(&msg) {
            
        // Handle Request to Make New Account
        if request["action"] == "make" { 

            match verify_account_creation(request, merkle_tree, blockchain.clone()).await {
                Ok(public_key) => {
                    println!("Account creation verified for public key: {}", public_key);
                    print_chain(blockchain).await; // Pass the original blockchain variable
                },
                Err(e) => {eprintln!("Account creation Invalid: {}", e);}
            }
        } 

        // Handle Request to Make New Transaction
        else if request["action"] == "transaction" { 

            match verify_transaction(request, merkle_tree, blockchain.clone()).await {
                Ok(success) => {
                    
                    // print success status
                    if success {println!("Transaction verified!"); print_chain(blockchain).await; } 
                    else { eprintln!("Transaction failed to verify"); }
                },
                Err(e) => {eprintln!("Transaction Validation Error: {}", e);}
            }
        } 

    else { eprintln!("Unrecognized action: {}", request["action"]);}
    } else {eprintln!("Failed to parse message: {}", msg);}
}


/**
 * 
 */
async fn verify_account_creation(request: Value, merkle_tree: Arc<Mutex<MerkleTree>>, blockchain: Arc<Mutex<BlockChain>>) -> Result<String, String> {
    println!("Verifying account creation...\n");

    // retrieve new public key sent with request as Vec<u8> UTF-8 encoded
    let public_key: Vec<u8> = request["public_key"].as_str().unwrap_or_default().as_bytes().to_vec();
    let public_key_hex_str: String = request["public_key"].as_str().unwrap_or_default().to_string();
    let obfuscated_private_key_hash: Vec<u8> = hex::decode(request["obfuscated_private_key_hash"].as_str().unwrap_or_default()).unwrap();

    // Lock the merkle tree
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

    // Check that the account doesnt already exist in the tree
    if merkle_tree_guard.account_exists(public_key.clone()) { return Err("Account already exists".to_string());} 
    else { println!("Account does not exist in merkle tree...\n"); }

    // Package account details in Account struct and insert into merkle tree
    let account = Account { public_key: public_key.clone(), obfuscated_private_key_hash: obfuscated_private_key_hash.clone(), balance: 0, nonce: 0, };

    // Insert the account into the merkle tree
    merkle_tree_guard.insert_account(account);
    assert!(merkle_tree_guard.account_exists(public_key.clone()));
    println!("Account successfully inserted into merkle tree...\n");

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



async fn verify_transaction(request: Value, merkle_tree: Arc<Mutex<MerkleTree>>, blockchain: Arc<Mutex<BlockChain>>) -> Result<bool, String> {
    println!("Verifying transaction...\n");


    // ! TODO: Implement the client side zk proof idea for transaction verification

    // retrieve transaction details from request
    let sender_address: Vec<u8> = request["sender"].as_str().unwrap_or_default().as_bytes().to_vec();
    let recipient_address: Vec<u8> = request["recipient"].as_str().unwrap_or_default().as_bytes().to_vec();
    let amount: u64 = request["amount"].as_str().unwrap_or_default().parse().unwrap_or_default();

    // Lock the merkle tree while checking sender and recipient accounts
    let mut merkle_tree_guard: MutexGuard<MerkleTree> = merkle_tree.lock().await;

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


// Helper for printing the chain
async fn print_chain(blockchain: Arc<Mutex<BlockChain>>) {
    let blockchain_guard: MutexGuard<'_, BlockChain> = blockchain.lock().await; // lock blockchain for printing

    println!("\nCurrent State of Blockchain as Maintained on Client Side:");
    for (i, block) in blockchain_guard.chain.iter().enumerate() {
        match block {
            Block::NewAccount { address, time, hash } => {
                // Directly use address as it's already a UTF-8 encoded hex string
                let hash_hex = hex::encode(hash); // Assuming hash is a Vec<u8> needing encoding
                let address = String::from_utf8(address.clone()).unwrap();
                println!("\nBlock {}: \n\tNew Account: {:?}\n\tTime: {}\n\tHash: {}", i, address, time, hash_hex);
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
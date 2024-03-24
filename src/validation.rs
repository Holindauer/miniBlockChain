use tokio::sync::{Mutex, MutexGuard};
use tokio::time;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// Import the necessary libraries
use crate::accounts::AccountCreationRequest;
use crate::block::BlockChain;



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


// validation.rs
const PORT_NUMBER: &str = "127.0.0.1:8080";

/**
 * @notice run_validation() is a wrapper called within main.rs that instigates the process of accessing
 * the network from the client side for running the validation process. 
 */
pub fn run_validation(private_key: &String) {

    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Instatiating a new blockchain 
    let blockchain = Arc::new(Mutex::new(BlockChain::new()));

    // Send request to peers to get the latest blockchain.
    rt.block_on(async {
        let blockchain_clone = blockchain.clone();
        update_local_blockchain(blockchain_clone).await;
    });


    // Use block_on to start a new asynchronous task that listens for incoming connections
    rt.block_on(async {
        match listen_for_connections(PORT_NUMBER).await {
            Ok(_) => println!("Validation listener terminated."),
            Err(e) => eprintln!("Validation listener encountered an error: {}", e),
        }
    });
}


/**
 * @notice update_local_blockchain() is an asynchronous function that updates the local blockchain to the majority state
 * as determined by the received blockchain records. This function is called by run_validation() and is not intended to be
 */
async fn update_local_blockchain(blockchain: Arc<Mutex<BlockChain>>) {

    // Collect blockchain records from peer validation nodes
    let collected_blockchains: Vec<BlockChain> = collect_blockchain_records(Duration::from_secs(60)).await;

    // Determine the majority blockchain based on the collected records (could be None if no other nodes are online)
    let majority_blockchain: Option<BlockChain> = determine_majority_blockchain(collected_blockchains);

    // Only update the local new instantiaon of the blockchain 
    // if a majority blockchain was found to replace it
    if let Some(majority) = majority_blockchain {

        // await async tokio mutex lock
        let mut bc: MutexGuard<'_, BlockChain> = blockchain.lock().await;

        // update the blockchain once the mutex is locked
        *bc = majority;
        println!("Blockchain updated to majority state.");
    }
}


//  ! Placeholder
async fn collect_blockchain_records(duration: Duration) -> Vec<BlockChain> {

    // Vector to store received blockchain records
    let mut records: Vec<BlockChain> = Vec::new();

    // TODO: Currently a Placeholder: Needs to Populate `records` with received blockchains
    // TODO: from listening on a TCP socket. Records should be received as JSON, and deserialized
    // TODO: into BlockChain structs and returned for the next step, determine_majority_blockchain()
    // TODO: which is called within update_local_blockchain()

    // Simulate waiting for incoming records for a fixed duration
    time::sleep(duration).await;

    records
}

// ! Placeholder function to hash blockchains and determine the majority
fn determine_majority_blockchain(blockchains: Vec<BlockChain>) -> Option<BlockChain> {

    // HashMap to store the hash votes for each blockchain state  
    let mut hash_votes: HashMap<String, i32> = HashMap::new();

    // Iterate over the received blockchains and hash each one
    for blockchain in blockchains {

        // Placeholder: Hash the blockchain.  // TODO: add this logic into the blockah=chain struct methods
        let hash = hash_blockchain(&blockchain); // Placeholder for hashing logic

        // Either increment the vote count for the hash or add it to the map
        *hash_votes.entry(hash).or_insert(0) += 1;
    }

    // ! Placeholder: Find the hash with the majority voten

    None // ! Placeholder return value
}

// ! Placeholder for a function to hash a blockchain
// This would implement the actual logic to generate a hash for a blockchain
fn hash_blockchain(blockchain: &BlockChain) -> String {
    "hash_placeholder".to_string()
}


/**
 * @notice listen_for_connections()S is an asynchronous function that listens for incoming connections on the
 * specified address. It will spawn new tasks to handle each incoming connection. This function serves as the 
 * fascilitator of the validation process on the client side for the node software.
 * @dev This function is called by run_validation() and is not intended to be called directly.
 */
pub async fn listen_for_connections(address: &str) -> tokio::io::Result<()> {

    // create a new listener on the specified address
    let listener = TcpListener::bind(address).await?;
    println!("Validation server listening on {}\n", address);

    // loop to accept incoming connections
    loop {
        // accept a new connection. socket == stream
        let (mut socket, _) = listener.accept().await?;

        // spawn a new task to handle the incoming connection
        tokio::spawn(async move {

            // buffer to hold incoming data
            let mut buf: [u8; 1024] = [0; 1024];
            
            // read from the socket
            match socket.read(&mut buf).await {
                Ok(size) => {

                    // convert the buffer to a string and print it
                    let msg = String::from_utf8_lossy(&buf[..size]);
                    println!("Received: {}", msg);
                    
                    // Parse the JSON message and determine what to do with it
                    if let Ok(request) = serde_json::from_str::<Value>(&msg) {
                        
                        // Handle Request to Make New Account
                        if request["action"] == "make" { 
                            println!("Received account creation request. Validating...");

                            match verify_account_creation(request).await {
                                Ok(_) => {println!("Account creation validated.");},
                                Err(e) => {eprintln!("Account creation Invalid: {}", e);}
                            }
                        } 
                        // Handle Request to Make New Transaction
                        else if request["action"] == "transaction" { 
                            println!("TODO implement transaction validation");
                        }
                        else { eprintln!("Unrecognized action: {}", request["action"]);}
                    } else {eprintln!("Failed to parse message: {}", msg);}
                },
                Err(e) => eprintln!("Failed to read from socket: {}", e),
            }
        });
    }
}

// ! Placeholder: Implement your actual verification logic here
async fn verify_account_creation(request: Value) -> Result<(), String> {
    

    println!("Account creation verified for public key: {}", request["public_key"].as_str().unwrap_or_default());
    Ok(())
}




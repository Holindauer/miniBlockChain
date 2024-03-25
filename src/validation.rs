use tokio::sync::{Mutex, MutexGuard, mpsc};
use tokio::time;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::error::Error;

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
    println!("Booting Up Validator Node...\n");

    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Instatiating a new blockchain 
    let blockchain: Arc<Mutex<BlockChain>> = Arc::new(Mutex::new(BlockChain::new()));

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
    println!("Updating BlockChain State...\n");

    // Vector to store the collected blockchains
    let mut peer_blockchains: Vec<BlockChain> = Vec::new();

    // duration of listening for blockchain records
    let duration = Duration::from_secs(15); 
    println!("Accepting blockchain records from peers for {} seconds...\n", duration.as_secs());

    // Collect blockchain records from peer validation nodes
    match collect_blockchain_records(duration).await {
        Ok(peer_blockchains) => {

            // If successful, determine the majority blockchain consensus
            let majority_blockchain: Option<BlockChain> = determine_majority_blockchain(peer_blockchains);
            if let Some(majority) = majority_blockchain {

                // lock the blockchain and update it to the majority state
                let mut bc: MutexGuard<'_, BlockChain> = blockchain.lock().await; 
                *bc = majority;
                println!("Blockchain updated to majority state.");
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

                        // Read blockchain data into buffer, convert to blockchain struct, and send to channel
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
        let hash: Vec<u8> = blockchain.hash();
        *hash_votes.entry(hash).or_insert(0) += 1; 
    }

    // Find the hash with the most votes
    let majority_hash = hash_votes.into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(hash, _)| hash);

    // If there's a majority hash, find and return the corresponding blockchain
    majority_hash.and_then(|hash|
        blockchains.into_iter().find(|blockchain| blockchain.hash() == hash)
    )
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




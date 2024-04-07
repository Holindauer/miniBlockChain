
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener};
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use serde_json;

use crate::blockchain::BlockChain;
use crate::constants::{DURATION_GET_PEER_CHAINS, PORT_NUMBER, VERBOSE_STACK};


/**
 * @notice chain_consensus.rs contains the logic for updating the local blockchain and merkle tree of a validator node
 * that is booting up to the majority state of the network. This is done by sending a request to all other validators
 * to send their current blockchain state. The node will then determine the majority state of the network and update
 * its local blockchain to reflect the majority.
*/

/**
 * @notice update_local_blockchain() is an asynchronous function that fascililtates the process of updating the local 
 * blockchain and merkle tree to the majority state of the network. This function is called by validation::run_validation() 
 * when booting up a new validator node.
*/
pub async fn update_local_blockchain(local_chain: Arc<Mutex<BlockChain>>) {
    if VERBOSE_STACK { println!("validation::update_local_blockchain() : Accepting blockchain records from peers for {} seconds...", DURATION_GET_PEER_CHAINS.as_secs()); }  // TODO eventually move the blockchain update funcs into a seperate file. validation.rs is getting too big
 
     // Get majority network chain state if available
     match collect_blockchain_records(DURATION_GET_PEER_CHAINS).await {
         Ok(peer_blockchains) => {
 
             // Determine the majority blockchain consensus
             let majority_blockchain: Option<BlockChain> = determine_majority_blockchain(peer_blockchains);
     
             // If >50% of peers did agree on a blockchain, update the local blockchain
             if let Some(majority_chain) = majority_blockchain {
 
                 // Lock the mutex to safely update the blockchain
                 let mut local_blockchain_guard = local_chain.lock().await;
                 *local_blockchain_guard = majority_chain;
 
                if VERBOSE_STACK { println!("Blockchain updated to majority state of peer validator nodes."); }
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
 async fn collect_blockchain_records(duration: Duration) -> Result<Vec<BlockChain>, Box<dyn Error>> { // ! TODO Does this ever request the blockchain from the peers? 
    if VERBOSE_STACK { println!("validation::collect_blockchain_records() : Collecting blockchain records from peers..."); }
 
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
        if VERBOSE_STACK { println!("validation::collect_blockchain_records() : No blockchain records collected from peers..."); }
    }else{
        if VERBOSE_STACK { println!("validation::collect_blockchain_records() : Blockchain records collected from peers..."); }   
    }
 
     // Collect records from the channel
     let mut records: Vec<BlockChain> = Vec::new();
     while let Some(blockchain) = rx.recv().await {
        records.push(blockchain);
     }
 
     Ok(records)
 }
 
 /**
  * @notice determine_majority_blockchain() is a function that takes a vector of blockchain records, hashes each record, 
  * and determines the majority blockchain based on the most common hash. 
  * @dev This function is called directly after requesting blockchain records from peers when updating the local blockchain 
  * to the majority state within update_local_blockchain().
  */
 fn determine_majority_blockchain(blockchains: Vec<BlockChain>) -> Option<BlockChain> {
    if VERBOSE_STACK { println!("validation::determine_majority_blockchain() : Determining majority blockchain..."); }
 
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
 
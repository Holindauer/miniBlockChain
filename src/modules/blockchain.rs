use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use serde::{Serialize, Deserialize};
use tokio::sync::{Mutex, MutexGuard};
use std::sync::Arc;

use crate::modules::validation::ValidatorNode;

/**
 * @notice blockchain.rs contains the structs and methods for creating and manipulating blocks in the blockchain.
 * There are three types of blocks in the blockchain: Genesis, Transaction, and Account Creation.
 * 
 * Genesis Block: 
 *    The first block in the blockchain. It is hardcoded into the blockchain and contains no data other than creation 
 *    time.
 * 
 * Transaction Block:
 *    Tranasaction blocks contain the information revalavant to a single transaction of value between two users. 
 *    This includes the public key of the sender, the public key of the recipient, the amount being transacted,
 *    the timestamp of the transaction, and the hash of all this block data. 
 * 
 * Account Creation Block:
 *    Account creation blocks contain the information relevant to the creation of a new account. This includes 
 *    the public key of the new account, the timestamp of the account creation, and the hash of this block data.
 */



 /**
  * @notice Block is an enum that represents the different types of blocks that can be added to the blockchain.
  * @dev The Block enum is used to store the data of the block and differentiate between the different types of blocks.
  * @dev All addresses are stored as UTF-8 encoded byte vectors.
*/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)] 
pub enum Block {
    Genesis { 
        time : u64
    },
    Transaction { 
        sender: Vec<u8>, 
        sender_balance: u64,
        recipient: Vec<u8>, 
        recipient_balance: u64,
        amount: u64, 
        time : u64, 
        sender_nonce: u64, 
        hash: Vec<u8>
    },
    NewAccount { 
        address: Vec<u8>, 
        account_balance: u64,
        time: u64, 
        hash: Vec<u8>
    },
    Faucet { 
        address: Vec<u8>, 
        account_balance: u64,
        time: u64, 
        hash: Vec<u8>
    }
}


/**
 * @notice the Blockchain struct links Blocks in a linked list.
 * @param chain - a vector of Blocks that have been added to the blockchain.
 * @param pending_transactions_queue - a queue that stores the public keys of users who have sent transactions to be 
 * added to the blockchain in the order they were recieved into the node. Public keys will be used to retrieve the 
 * transactions from the joint_transactions_map.
 * @param joint_transactions_map - a hashmap that stores transactions that have not yet been added to the blockchain yet.
 * The keys are the addresses of the senders and the values are all transaction requests that have been made by that sender.
 * These transactions will be sorted in terms of lowest to highest nonce and validated processed in that order.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockChain {
    pub chain: Vec<Block>,                                         
    pending_request_queue: VecDeque<Vec<u8>>,          // queue of public keys
    joint_request_map: HashMap<Vec<u8>, Vec<Block>>, // map of public keys to transactions
}

/**
 * @notice the Blockchain struct contains the following methods for creating and manipulating blocks.
 * @dev These methods are used within validation.rs to push validated transactions to the blockchain.
 */
impl BlockChain {

    // Initialize a new blockchain with a genesis block
    pub fn new() -> Self {
        println!("Creating new BlockChain struct..."); 

        // Create a new blockchain 
        let mut blockchain: BlockChain = BlockChain {
            chain: Vec::new(),
            pending_request_queue: VecDeque::new(),
            joint_request_map: HashMap::new(),
        };

        // Create a genesis block and return the blockchain
        blockchain.create_genesis_block(); blockchain
    }

    /**
     * @notice create_genesis_block() is a method that creates the first block in the blockchain.
     * @dev the genesis block contains only the timestamp of the block creation.
     */
    fn create_genesis_block(&mut self) {

        let time: u64  = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let genesis_block = Block::Genesis {time: time};
        self.chain.push(genesis_block);
    }

    /**
     * @notice store_incoming_transaction() recieves data for a new transaction contained within the 
     * Transaction struct. Stored transactions are validated using the validate_transaction() method.
     * @dev the sender's address is pushed to the pending_transactions_queue and the transaction is
     * added to the joint_transactions_map, using the sender's address as the key.
     */
    pub fn store_incoming_requests(&mut self, new_block_request: &Block) {

        // Retrieve and clone relavant address from the request
        let address: Vec<u8> = match &new_block_request {
            Block::Transaction { sender, .. } => sender,
            Block::NewAccount { address, .. } => address,
            Block::Faucet { address, .. } => address,
            Block::Genesis { .. } => panic!("Invalid request type"),
        }.clone();
    
        // Push the address to the pending request queue
        self.pending_request_queue.push_back(address.clone());
    
        // Insert the request into the joint_request_map, creating a new entry if necessary
        self.joint_request_map.entry(address)
            .or_insert_with(Vec::new)
            .push(new_block_request.clone());
    }

   // Method to create a new block from a request and add it to the blockchain
    pub fn push_block_to_chain(&mut self, new_block: Block) {
        
        // get address of sender
        let address: Vec<u8> = match &new_block {
            Block::Transaction { sender, .. } => sender,
            Block::NewAccount { address, .. } => address,
            Block::Faucet { address, .. } => address,
            _ => panic!("Invalid request type"),
        }.clone();

        // Set the hash of the block
        let mut new_block = new_block.clone();
        self.hash_block_data(&mut new_block);

        // Push the new block to the blockchain
        self.chain.push(new_block.clone());    
        self.pending_request_queue.pop_front(); // Remove leading address from the queue
    
        // retrieve mutable vector of all requests from the sender
        if let Some(requests) = self.joint_request_map.get_mut(&address) {             

            // Remove the request from requests Vec that matches the one added to the blockchain
            if let Some(index) = requests.iter().position(|r| *r == new_block) { requests.remove(index); }
        }
    }

    // Sets the hash of a block based on its data
    fn hash_block_data(&mut self, block: &mut Block) {

        let mut hasher = Sha256::new(); // new SHA256 hasher
    
        match block { // Contribute block to hasher based on its type
            Block::Genesis { time, .. } => {
                hasher.update(time.to_string().as_bytes());
            }
            Block::Transaction { sender, sender_balance, recipient, recipient_balance, amount, time, sender_nonce, .. } => {
                hasher.update(sender);
                hasher.update(&sender_balance.to_be_bytes());
                hasher.update(recipient);
                hasher.update(&recipient_balance.to_be_bytes());
                hasher.update(&amount.to_be_bytes());
                hasher.update(time.to_string().as_bytes());
                hasher.update(sender_nonce.to_string().as_bytes());
            }
            Block::NewAccount { address, account_balance, time, .. } => {
                hasher.update(address);
                hasher.update(&account_balance.to_string().as_bytes());
                hasher.update(time.to_string().as_bytes());
            }
            Block::Faucet { address, account_balance, time, .. } => {
                hasher.update(address);
                hasher.update(account_balance.to_string().as_bytes());
                hasher.update(time.to_string().as_bytes());
            }
        }
        
        // Finalize the hash and return it as Vec<u8>
        let hash = hasher.finalize().to_vec();
        
        // Set the hash in the block
        match block {
            Block::Transaction { hash: block_hash, .. } | Block::NewAccount { hash: block_hash, .. } => {
                *block_hash = hash.clone();
            }
            _ => (),
        }    
    }
    
    /// Hashes the entire blockchain using SHA-256.
    pub fn hash_blockchain(&self) -> Vec<u8> {

        // Create a new SHA256 hasher
        let mut hasher = Sha256::new();

        // Push each block's data to the hasher
        for block in &self.chain {

            // Contribute block to hasher based on its type
            match block {
                Block::Genesis { time } => {
                    hasher.update(time.to_string().as_bytes());
                }
                Block::Transaction { sender, sender_balance, recipient, recipient_balance, amount, time, sender_nonce, hash  } => {
                    hasher.update(sender);
                    hasher.update(&sender_balance.to_be_bytes());
                    hasher.update(recipient);
                    hasher.update(&recipient_balance.to_be_bytes());
                    hasher.update(&amount.to_be_bytes());
                    hasher.update(time.to_string().as_bytes());
                    hasher.update(sender_nonce.to_string().as_bytes());
                    hasher.update(hash);
                }
                Block::NewAccount { address, account_balance, time, hash } => {
                    hasher.update(address);
                    hasher.update(&account_balance.to_string().as_bytes());
                    hasher.update(time.to_string().as_bytes());
                    hasher.update(hash);
                }
                Block::Faucet { address, account_balance, time, hash } => {
                    hasher.update(address);
                    hasher.update(account_balance.to_string().as_bytes());
                    hasher.update(time.to_string().as_bytes());
                    hasher.update(hash);
                }
            }
        }

        // Finalize the hash and return it
        hasher.finalize().to_vec()
    }
}

/**
 * @notice BlockJson is a version of the Block enum that is used to serialize the blockchain to JSON. 
 * @dev The only difference is that the addresses and hashes are stored as strings instead of byte vectors.
 * for easier serialization to JSON and comparision within integration tests. 
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)] 
pub enum BlockJson {
    Genesis { 
        time : u64
    },
    Transaction { 
        sender: String, 
        sender_balance: u64,
        recipient: String, 
        recipient_balance: u64,
        amount: u64, 
        time : u64, 
        sender_nonce: u64, 
        hash: String
    },
    NewAccount { 
        address: String, 
        account_balance: u64,
        time: u64, 
        hash: String
    },
    Faucet { 
        address: String, 
        account_balance: u64,
        time: u64, 
        hash: String
    }
}



pub async fn save_chain_json(validator_node: ValidatorNode){

    // Lock blockchain for saving
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let blockchain_guard: MutexGuard<'_, BlockChain> = blockchain.lock().await;

    // Retrieve the chain from the blockchain
    let chain: Vec<Block> = blockchain_guard.chain.clone();

    // Convert the Vec<Block> to Vec<BlockJson> for serialization
    let vec_blockjson: Vec<BlockJson> = convert_vec_block_to_vec_blockjson(chain).await;

    // Get port number for node
    let port: String = validator_node.client_port_address.clone();

    // Format directory name based on port number 
    let dir: String = format!("Node_{}", port);
    let path: String = format!("Node_{}/blockchain.json", port);

    // creat directory 
    std::fs::create_dir_all(dir.clone()).unwrap();

    // Serialize the blockchain to JSON
    let file = File::create(path).unwrap();
    serde_json::to_writer_pretty(file, &vec_blockjson).unwrap();
}


/**
 * @notice load_chain_json() is an asynchronous function that loads the blockchain from a JSON file.
 * @dev This function is used to load the blockchain from a JSON file after the node has been restarted.
 * @dev The blockchain is loaded from the JSON file and stored in the blockchain struct.
 */
async fn convert_vec_block_to_vec_blockjson(vec_block: Vec<Block>) -> Vec<BlockJson> {

    let mut vec_blockjson: Vec<BlockJson> = Vec::new();

    // convert all blocks to blockjson and push to vec_blockjson
    for block in vec_block {
        let block_json: BlockJson = convert_block_to_blockjson(block).await;
        vec_blockjson.push(block_json);
    }

    vec_blockjson
}

/**
 * @notice convert_block_to_blockjson() is an asynchronous function that converts a Block enum to a BlockJson enum.
 * @dev This function is used to convert the blockchain to JSON for saving and sending to other nodes.
 */
async fn convert_block_to_blockjson(block: Block) -> BlockJson {

    let block_json: BlockJson;

    match block {
        Block::Genesis { time } => {

            // package genesis block data into BlockJson
            block_json = BlockJson::Genesis { time };
        },
        Block::Transaction { sender, sender_balance, recipient, recipient_balance, amount, time, sender_nonce, hash } => {

            // decode sender and recipient to strings
            let sender = String::from_utf8(sender).unwrap();
            let recipient = String::from_utf8(recipient).unwrap();
            let hash = hex::encode(hash);

            // package transaction block data into BlockJson
            block_json = BlockJson::Transaction { sender, sender_balance, recipient, recipient_balance, amount, time, sender_nonce, hash };
        },
        Block::NewAccount { address, account_balance, time, hash } => {

            // decode address and hash to strings
            let address = String::from_utf8(address).unwrap();
            let hash = hex::encode(hash);

            // package new account block data into BlockJson
            block_json = BlockJson::NewAccount { address, account_balance, time, hash };
        },
        Block::Faucet { address, account_balance, time, hash } => {
            let address = String::from_utf8(address).unwrap();
            let hash = hex::encode(hash);

            // package faucet block data into BlockJson
            block_json = BlockJson::Faucet { address, account_balance, time, hash };
        },
    }

    block_json
}





/**
 * @notice print_chain() is an asynchronous function that prints the current state of the blockchain as maintained on the 
 * client side. This function is called by verify_account_creation() and verify_transaction() after storing the request in the 
 * blockchain.
 */
pub async fn print_chain(blockchain: Arc<Mutex<BlockChain>>) { 

    // lock blockchain mutex for printing
    let blockchain_guard: MutexGuard<'_, BlockChain> = blockchain.lock().await; 

    println!("\nCurrent State of Blockchain as Maintained on Client Side:");
    for (i, block) in blockchain_guard.chain.iter().enumerate() {
        match block {
            Block::NewAccount { address, account_balance, time, hash } => {
                
                // Directly use address as it's already a UTF-8 encoded hex string
                let hash_hex = hex::encode(hash); // Assuming hash is a Vec<u8> needing encoding
                let address = String::from_utf8(address.clone()).unwrap();
                println!(
                    "\nBlock {}: \n\tNew Account: {}\n\tAccount Balance: {}\n\tTime: {}\n\tHash: {}", 
                    i, address, account_balance, time, hash_hex
                );
            },
            Block::Transaction {sender, sender_balance, recipient, recipient_balance, amount, time, sender_nonce, hash} => {

                // Directly use sender and recipient as they're already UTF-8 encoded hex strings
                let hash_hex = hex::encode(hash); // Assuming hash is a Vec<u8> needing encoding
                let sender = String::from_utf8(sender.clone()).unwrap();
                let recipient = String::from_utf8(recipient.clone()).unwrap();

                println!(
                    "\nBlock {}: \n\tSender: {}\n\tSender Balance: {}\n\tSender Nonce: {}\n\tRecipient: {}\n\tRecipient Balance: {}\n\tAmount: {}\n\tTime: {:}\n\tHash: {}", 
                    i, sender, sender_balance, sender_nonce, recipient, recipient_balance, amount, time, hash_hex);
            },
            Block::Genesis { time } => {
                println!("\nBlock {}: \n\tGenesis Block\n\tTime: {:?}", i, time);
            },
            Block::Faucet { address, account_balance, time, hash } => {
                
                // Directly use address as it's already a UTF-8 encoded hex string
                let hash_hex = hex::encode(hash); // Assuming hash is a Vec<u8> needing encoding
                let address = String::from_utf8(address.clone()).unwrap();
                println!(
                    "\nBlock {}: \n\tFaucet Used By: {}\n\tAccount Balance: {}\n\tTime: {}\n\tHash: {}", 
                    i, address, account_balance, time, hash_hex
                );
            },
        }
    }
}


/**
 * @test the following tests are used to verify the functionality of the blockchain struct.
 */
#[cfg(test)]mod tests {
    use super::*;

    #[test]
    fn test_genesis_block_creation() {
        // Initialize a new blockchain and verify it starts with a Genesis block
        let blockchain: BlockChain = BlockChain::new();
        assert_eq!(blockchain.chain.len(), 1, "Blockchain should have 1 block (Genesis) after creation");
        match &blockchain.chain[0] {
            Block::Genesis { time: _ } => (), // Success if Genesis
            _ => panic!("First block should be a Genesis block"),
        }
    }

    #[test]
    fn test_account_creation_block_addition() {

        // Initialize a new blockchain
        let mut blockchain = BlockChain::new();

        // Simulate account creation request
        let new_address = vec![0u8; 20]; // Dummy address for testing
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let request = Block::NewAccount {
            address: new_address.clone(),
            account_balance: 0,
            time,
            hash: Vec::new(), // Placeholder hash
        };

        // Assume validation is successful and directly push the request to the chain
        blockchain.push_block_to_chain(request);

        // Verify that a new NewAccount block has been added
        assert_eq!(blockchain.chain.len(), 2, "Blockchain should have 2 blocks after account creation");

        match &blockchain.chain[1] {
            Block::NewAccount { address, account_balance, time: _, hash: _ } => {
                assert_eq!(&address[..], &new_address[..], "The new account address should match the request");
            },
            _ => panic!("Second block should be an Account Creation block"),
        }
    }
}
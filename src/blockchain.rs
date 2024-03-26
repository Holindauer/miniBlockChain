use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, VecDeque};
use std::{fs::File, io::{self, Read}, path::Path};
use serde::{Serialize, Deserialize};

use crate::merkle_tree::{MerkleTree, Account};


/**
 * @notice blockchain.rs contains the structs and methods for creating and manipulating blocks in the blockchain.
 * There are three types of blocks in the blockchain: Genesis, Transaction, and Account Creation.
 * 
 * Genesis Block: 
 *   The first block in the blockchain. It is hardcoded into the blockchain and contains no data.
 * 
 * Transaction Block:
 *   Tranasaction blocks contain the information revalavant to a single transaction of value between two users. 
 *   This includes the public key of the sender, the public key of the recipient, the amount being transacted,
 *   the timestamp of the transaction, and the hash of all this block data. 
 * 
 * Account Creation Block:
 *   Account creation blocks contain the information relevant to the creation of a new account. This includes 
 *   the public key of the new account, the timestamp of the account creation, and the hash of this block data.
 */

 /**
  * @notice Block is an enum that represents the different types of blocks that can be added to the blockchain.
  * @dev The Block enum is used to store the data of the block and differentiate between the different types of blocks.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
enum Block {
    Genesis { time : u64},
    Transaction { sender: [u8; 20], recipient: [u8; 20], amount: u64, time : u64, hash: Vec<u8>},
    AccountCreation { address: [u8; 20], time: u64, hash: Vec<u8>},
}

/**
 * @notice TransactionRequest structs contain information about a single transpassing information 
 * to the blockchain.
 * @dev account addresses are stored as arrays of 20 bytes.
 * @param senderAdress - the Blockchainaddress of the sender.
 * @param senderNonce - the nonce of the sender. (num transactions sender has made).
 * @param recipientAdress - the address of the recipient.
 * @param amount - the amount of the transaction.
*/  
#[derive(Debug, Clone, Serialize, Deserialize)]
enum Request {
    Transaction {
        sender_address: Vec<u8>,
        sender_nonce: u64,
        recipient_address: Vec<u8>,
        amount: f32,
        time: u64,
    }, 
    AccountCreation {
        new_address: Vec<u8>,
        time: u64,
    }
}


/**
 * @notice the Blockchain struct links Blocks int a linked list.
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
    chain: Vec<Block>,                                         
    pending_request_queue: VecDeque<Vec<u8>>,                     // queue of public keys
    joint_request_map: HashMap<Vec<u8>, Vec<Request>>, // map of public keys to transactions
}

/**
 * @notice the Blockchain struct contains the following methods for creating and manipulating blocks.
 * @dev These methods are used within validation.rs to push validated transactions to the blockchain.
 */
impl BlockChain {

    // Initialize a new blockchain with a genesis block
    pub fn new() -> Self {
        println!("Intializing Empty BlockChain...\n");

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

    // Enqueue an incoming transaction     

    /**
     * @notice store_incoming_transaction() recieves data for a new transaction contained within the 
     * Transaction struct. Stored transactions are validated using the validate_transaction() method.
     * @dev the sender's address is pushed to the pending_transactions_queue and the transaction is
     * added to the joint_transactions_map, using the sender's address as the key.
     */
    pub fn store_incoming_requests(&mut self, request: Request) {

        // Retrieve and clone relavant address from the request
        let address: Vec<u8> = match &request {
            Request::Transaction { sender_address, .. } => sender_address,
            Request::AccountCreation { new_address, .. } => new_address,
        }.clone();
    
        // Push the address to the pending request queue
        self.pending_request_queue.push_back(address.clone());
    
        // Insert the request into the joint_request_map, creating a new entry if necessary
        self.joint_request_map.entry(address)
            .or_insert_with(Vec::new)
            .push(request);
    }

    /**
     * @notice validate_account_creation() validates a single account creation request within the blockchain 
     * and adds the account to the merkle tree
     */
    pub fn validate_account_creation(&mut self, request: Request, merkle_tree: &mut MerkleTree) {

        // retrieve public key
        let public_key: Vec<u8> = match &request {
            Request::AccountCreation { new_address, .. } => new_address.clone(),
            _ => panic!("Invalid request type for account creation validation."),
        };

        // check if account already exists in merkle tree
        if merkle_tree.account_exists(public_key.clone()){
            println!("Invalid! Account already exists in the Merkle Tree.");
            return;
        }

        // Package account data in Account struct
        let new_account: Account = Account {
            public_key: public_key.clone(),
            balance: 0,
            nonce: 0,
        };

        // Insert the new account into the merkle tree and push the request to the chain
        merkle_tree.insert_account(new_account);
        self.push_request_to_chain(request);
        


    }


    // Validates all transactions related to a single sender
    pub fn validate_transaction(&mut self, request: Request, merkle_tree: &mut MerkleTree) {    
        println!("Function Stub for validate_transaction() called.");
    }

    // Method to create a new block from a request and add it to the blockchain
    fn push_request_to_chain(&mut self, request: Request) {

        // Create Block from request
        let block: Block = match request {

            // package transaction block
            Request::Transaction { sender_address, recipient_address, amount,  time, sender_nonce} => {
                Block::Transaction { 
                    sender: sender_address.try_into().unwrap(), 
                    recipient: recipient_address.try_into().unwrap(), 
                    amount: amount as u64, 
                    time: time, 
                    hash: Vec::new()
                }
            }

            // package account creation block
            Request::AccountCreation { new_address, time } => {
                Block::AccountCreation { 
                    address: new_address.try_into().unwrap(), 
                    time: time, 
                    hash: Vec::new()
                }
            }
        };

        // Push the new block to the blockchain
        self.chain.push(block.clone());
    }

    // Populates empty block data hash vector w/ the hash of all data in a block
    fn hash_block_data(&mut self, block: &mut Block) {  

        // Create a new SHA256 hasher
        let mut hasher = Sha256::new();
        
        // Add the block's data to the hasher based on its type
        match block {
            Block::Genesis { time, .. } => {
                hasher.update(time.to_string().as_bytes());
            }
            Block::Transaction { sender, recipient, amount, time, .. } => {
                hasher.update(sender);
                hasher.update(recipient);
                hasher.update(&amount.to_be_bytes());
                hasher.update(time.to_string().as_bytes());
            }
            Block::AccountCreation { address, time, .. } => {
                hasher.update(address);
                hasher.update(time.to_string().as_bytes());
            }
        }
    
        // Update the block's hash field with the finalized hash
        match block {
            Block::Transaction { hash, .. } => { *hash = hasher.finalize().to_vec().clone(); }
            Block::AccountCreation { hash, .. } => { *hash = hasher.finalize().to_vec().clone(); }
            _ => {}
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
                Block::Transaction { sender, recipient, amount, time, hash } => {
                    hasher.update(sender);
                    hasher.update(recipient);
                    hasher.update(&amount.to_be_bytes());
                    hasher.update(time.to_string().as_bytes());
                    hasher.update(hash);
                }
                Block::AccountCreation { address, time, hash } => {
                    hasher.update(address);
                    hasher.update(time.to_string().as_bytes());
                    hasher.update(hash);
                }
            }
        }

        // Finalize the hash and return it
        hasher.finalize().to_vec()
    }

    // Loading the blockchain from a JSON file
    pub fn load_json(&mut self) -> io::Result<()> {
        // TODO - this function will need to load in an up to date blockchain for the node. This  
        // TODO - will eventually require a network request to a peer to get the latest blockchain.

        // Check if the BlockChain.json file exists
        let path: &Path = Path::new("BlockChain.json");
        if path.exists() {

            // Open the file and read its contents
            let mut file = File::open(path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let loaded_chain: Vec<Block> = serde_json::from_str(&contents)?;

            // Check if the loaded_chain only contains the genesis block 
            // replace with the loaded chain to restore chain locally
            if self.chain.len() == 1 && loaded_chain.len() > 1 {
                self.chain = loaded_chain;
            }
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "BlockChain.json not found"))
        }
    }

    // Saving the blockchain to a JSON file
    pub fn save_json(&self) -> io::Result<()> {
        let file = File::create("BlockChain.json")?;
        serde_json::to_writer_pretty(file, &self.chain)?;
        Ok(())
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
        let request = Request::AccountCreation {
            new_address: new_address.clone(),
            time,
        };

        // Assume validation is successful and directly push the request to the chain
        blockchain.push_request_to_chain(request);

        // Verify that a new AccountCreation block has been added
        assert_eq!(blockchain.chain.len(), 2, "Blockchain should have 2 blocks after account creation");

        match &blockchain.chain[1] {
            Block::AccountCreation { address, time: _, hash: _ } => {
                assert_eq!(&address[..], &new_address[..], "The new account address should match the request");
            },
            _ => panic!("Second block should be an Account Creation block"),
        }
    }
}
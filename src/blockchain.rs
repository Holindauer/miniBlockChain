use sha2::{Sha256, Digest};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs::File, io::{self, Read}, path::Path};
use serde::{Serialize, Deserialize};

// block.rs contains structs and functions for fascilitating the creation and 
// manipulation of blocks in the blockchain.

/**
 * @notice Block structs are what are linked together to form the blockchain.
 * @param  timestamp - time the block was created (in Unix time).
 * @param hash - hash of this block as an array of 32 bytes.
 * @param prev_hash - hash of previous block as an array of 32 bytes.
 * @param data - data stored in the block as a dynamic byte vector
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Block {
    timestamp: u64,
    hash: [u8; 32],
    prev_hash: [u8; 32],
    data: Vec<u8>, 
}

/**
 * @notice Transaction structs are used to store information about transactions.
 * @dev account addresses are stored as arrays of 20 bytes.
 * @param senderAdress - the Blockchainaddress of the sender.
 * @param recipientAdress - the address of the recipient.
 * @param amount - the amount of the transaction.
 * @param signature - the signature of the transaction represented as a vector of bytes.
*/  
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Transaction {
    sender_address: [u8; 20],
    recipient_address: [u8; 20],
    amount: f32,
    signature: Vec<u8>,
}


/**
 * @notice the Blockchain struct links Blocks to form the blockchain.
 * @param chain - a vector of Blocks that have been added to the blockchain.
 * @param transaction_queue - a queue of transactions that have not yet been added to a block.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockChain {
    chain: Vec<Block>,
    transaction_queue: VecDeque<Transaction>,
}

/**
 * @notice the Blockchain struct containsv the following  methods for creating and manipulating blocks.
 * @dev These methods are used within validation.rs to push validated transactions to the blockchain.
 */
impl BlockChain {

    // Initialize a new blockchain with a genesis block
    pub fn new() -> Self {
        println!("Intializing Empty BlockChain...\n");

        // Create a new blockchain 
        let mut blockchain: BlockChain = BlockChain {
            chain: Vec::new(),
            transaction_queue: VecDeque::new(),
        };

        // Create a genesis block
        blockchain.create_genesis_block();
        blockchain
    }

    // Method to create a genesis block (first block in the chain)
    fn create_genesis_block(&mut self) {
        let genesis_block = Block {
            timestamp: 0,
            hash: [0; 32],
            prev_hash: [0; 32],
            data: b"Genesis Block".to_vec(), // byte vec
        };
        self.chain.push(genesis_block);
    }

    // Method to enqueue an incoming transaction
    fn add_transaction(&mut self, transaction: Transaction) {
        self.transaction_queue.push_back(transaction);
    }

    // Method to create a new block from pending transactions
    fn create_block(&mut self, data: Vec<u8>) -> Box<Block> {

        // retrieve the hash of the last block in the chain
        let prev_hash: [u8; 32] = self.chain.last().unwrap().hash;

        // create new block 
        let block: Block = Block {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            hash: [0; 32], // <-- placeholder, hash set in set_block_hash() below
            prev_hash,
            data,
        };
        self.set_block_hash(Box::new(block))
    }

    // Method to set the hash of a block
    fn set_block_hash(&self, mut block: Box<Block>) -> Box<Block> {
        
        // create new Sha256 hasher
        let mut hasher = Sha256::new();

        // feed block data to the hasher
        hasher.update(block.timestamp.to_string().as_bytes());
        hasher.update(&block.prev_hash);
        hasher.update(&block.data);

        // finalize the hash and copy it to the block
        let result = hasher.finalize().into_iter().collect::<Vec<u8>>();
        block.hash.copy_from_slice(&result[..32]);
        block
    }

    // function for adding a block to the chain
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

    // Method for saving the blockchain to a JSON file
    pub fn save_json(&self) -> io::Result<()> {
        let file = File::create("BlockChain.json")?;
        serde_json::to_writer_pretty(file, &self.chain)?;
        Ok(())
    }

    /// Hashes the entire blockchain using SHA-256.
    pub fn hash(&self) -> Vec<u8> {

        // Create a new SHA256 hasher
        let mut hasher = Sha256::new();

        // Push each block's data to the hasher
        for block in &self.chain {
            hasher.update(block.timestamp.to_string().as_bytes());
            hasher.update(&block.prev_hash);
            hasher.update(&block.data);
        }

        // Finalize the hash and return it
        hasher.finalize().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_creation() {

        // create a new blockchain w/ just the genesis block
        let mut blockchain: BlockChain = BlockChain::new();
        assert_eq!(blockchain.chain.len(), 1, "Blockchain should have 1 block after creation");
        assert_eq!(blockchain.chain[0].data, b"Genesis Block", "Genesis block should have correct data");

        // Create a new block with some data
        let new_block_data: Vec<u8> = b"Some transactions".to_vec();
        let new_block: Box<Block> = blockchain.create_block(new_block_data.clone());
        
        // Validate the new block's data
        assert_eq!(new_block.data, new_block_data);

        // Validate the hash is not the default valuepending_transactions
        assert_ne!(new_block.hash, [0; 32], "Block hash should not be default value after creation");
    }
}
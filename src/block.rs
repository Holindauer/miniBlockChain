use sha2::{Sha256, Digest};

// block.rs contains structs and functions for fascilitating the creation and 
// manipulation of blocks in the blockchain.

/**
 * @notice Block structs are what are linked together to form the blockchain.
 * @param  timestamp - time the block was created (in Unix time).
 * @param hash - hash of this block as an array of 32 bytes.
 * @param prev_hash - hash of previous block as an array of 32 bytes.
 * @param data - data stored in the block as a dynamic byte vector
 */
struct Block {
    timestamp: u64,
    hash: [u8; 32],
    prev_hash: [u8; 32],
    data: Vec<u8>, 
}

/**
 * @notice Transaction structs are used to store information about transactions.
 * @dev account addresses are stored as arrays of 20 bytes.
 * @param senderAdress - the address of the sender.
 * @param recipientAdress - the address of the recipient.
 * @param amount - the amount of the transaction.
 * @param signature - the signature of the transaction represented as a vector of bytes.
*/  
struct Transaction {
    sender_adress: [u8; 20],
    recipient_adress: [u8; 20],
    amount: f32,
    signature: Vec<u8>,
}

/**
 * @notice the Blockchain struct links Blocks to form the blockchain.
 * @param chain - a vector of Blocks that have been added to the blockchain.
 * @param pending_transactions - a vector of Transactions that must be validated
 *        before being added to the blockchain.
*/
struct Blockchain {
    chain: Vec<Block>,
    pending_transactions: Vec<Transaction>,
}


/**
 * @notice - This function takes a block and sets the hash of the block to the
 * hash of the block's data.
 * @param block - the block to set the hash of.
 * @return the block with the hash set.
*/
fn set_block_hash(mut block: Box<Block>) -> Box<Block> {
    
    // Create a new SHA256 object
    let mut hasher = Sha256::new();

    // Convert the block to a string and input data into hasher
    hasher.update(block.timestamp.to_string().as_bytes());
    hasher.update(&block.prev_hash);
    hasher.update(&block.data);

    // Finalize the hash and store it in the block
    let result = hasher.finalize().into_iter().collect::<Vec<u8>>();
    block.hash.copy_from_slice(&result[..32]);

    block
}

// unit test for set_block_hash
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Sha256, Digest};

    #[test]
    fn test_set_block_hash() {

        // Create a test block
        let data: Vec<u8> = b"test data".to_vec();
        let prev_hash: [u8; 32] = [0u8; 32]; // Example previous hash
        let block: Block = Block {
            timestamp: 123456789,
            hash: [0; 32],
            prev_hash,
            data: data.clone(),
        };
        
        // Create the expected hash manually
        let mut hasher = Sha256::new();
        hasher.update(block.timestamp.to_string().as_bytes());
        hasher.update(&block.prev_hash);
        hasher.update(&data);
        let expected_hash = hasher.finalize();

        // Box the block to comply with set_block_hash signature
        let boxed_block = Box::new(block);
        let result_block = set_block_hash(boxed_block);

        assert_eq!(result_block.hash, *expected_hash);
    }
}

/**
 * @notice - This function creates a new block with the given data and previous
 * hash. It then sets the hash of the block and returns the block.
 * @param data - the data to store in the block.
 * @param prev_hash - the hash of the previous block.
 * @return the new block with the hash set.
*/
fn new_block(data: Vec<u8>, prev_hash: [u8; 32]) -> Box<Block> {
    let block: Block = Block {
        timestamp: 0, // You might want to capture the actual timestamp here
        hash: [0; 32],
        prev_hash,
        data,
    };

    let boxed_block = Box::new(block);

    // Assuming setBlockHash is modified to take a Box<Block>
    set_block_hash(boxed_block)
}

#[test]
fn test_new_block() {
    let data: Vec<u8> = b"block data".to_vec();
    let prev_hash: [u8; 32] = [1u8; 32]; // Example previous hash for differentiation

    let block: Box<Block> = new_block(data.clone(), prev_hash);

    // Since `new_block` uses `set_block_hash`, we assume the hash is set correctly and just validate the input and existence
    assert_eq!(block.prev_hash, prev_hash);
    assert_eq!(block.data, data);

    // Validate the hash is not the default value, implying it was set
    assert_ne!(block.hash, [0; 32], "Block hash should not be default value after creation");
}

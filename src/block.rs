use sha2::{Sha256, Digest};

/**
 * @notice - Block structs are what are linked together to form the blockchain.
 * @param timestamp - time the block was created (in Unix time).
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
 * @notice - Transaction structs are used to store information about transactions.
 * @dev - account addresses are stored as arrays of 20 bytes.
 * @param senderAdress - the address of the sender.
 * @param recipientAdress - the address of the recipient.
 * @param amount - the amount of the transaction.
 * @param signature - the signature of the transaction represented as a vector of bytes.
*/
struct Transaction {
    senderAdress: [u8; 20],
    recipientAdress: [u8; 20],
    amount: f32,
    signature: Vec<u8>,
}

/**
 * @notice - the Blockchain struct links Blocks to form the blockchain.
 * @param chain - a vector of Blocks that have been added to the blockchain.
 * @param pending_transactions - a vector of Transactions that must be validated
 *        before being added to the blockchain.
*/
struct Blockchain {
    chain: Vec<Block>,
    pending_transactions: Vec<Transaction>,
}


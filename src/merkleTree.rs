use std::collections::HashMap;
use sha2::{Sha256, Digest};
use hex_literal::hex;

/**
 * @notice merkleTree.rs contains an implementation of a merkle tree for the purpose of retrieval of 
 * account balances form public keys. As well as for the validation of transactions and new blocks 
 * being written to the blockchain.
 *          
 * This implementation splits the merkle tree into two parts. The first being a simple hash map that 
 * allows for efficient retrieval of account balances from public keys. The second part is the actual
 * merkle tree structure, which hashes together balance-key pairs until a root hash is formed. This
 * root hash is required during the validation process.
 */


/**
 * @notice Account struct
 */
#[derive(Debug, Clone)]
struct Account {
    public_key: Vec<u8>,
    balance: u64,
}

/**
 * @notice the MerkleNode enum represents a single node leaf/branch in the merkle tree. 
 */
#[derive(Debug, Clone)]
enum MerkleNode {
    Leaf { hash: Vec<u8> },
    Branch { hash: Vec<u8>, left: Box<MerkleNode>, right: Box<MerkleNode> },
}

/**
 * @notice The MerkleTree struct contains two members: a root node and a hash map of MerkleNode structs.
 * @dev the hash map of accounts form the base of the tree, and the root node is the top of the tree.
 * @dev Option is used to allow for the root node to be set to None initially.
 */
#[derive(Debug, Clone)]
struct MerkleTree {
    root: Option<MerkleNode>,
    accounts: HashMap<Vec<u8>, u64>,
}

impl MerkleTree {

    // constructor for MerkleTree
    fn new() -> Self {
        MerkleTree {
            root: None,
            accounts: HashMap::new(),
        }
    }

    // Function to add an account to the tree
    // This is a simplified approach; in a full implementation, you would rebuild or adjust the tree here
    fn add_account(&mut self, account: Account) {

        // Hash the account
        let account_hash = Self::hash_account(&account);

        // Add the account to the accounts HashMap
        self.accounts.insert(account.public_key.clone(), account.balance);
    }

    // Hash function for accounts
    fn hash_account(account: &Account) -> Vec<u8> {

        // new SHA256 hasher
        let mut hasher = Sha256::new();

        // combine bytes and hash
        hasher.update(&account.public_key);
        hasher.update(account.balance.to_le_bytes());
        hasher.finalize().to_vec()
    }

    // Placeholder function for generating the Merkle root; to be implemented
    fn generate_merkle_root(&mut self) {
        // This function would iterate over the account hashes, pair them, and build the tree upwards
        // until a single root hash is produced.
    }

    // Method to save the Merkle tree to a JSON file
    pub fn save_to_json(&self, final_hash: &str) -> std::io::Result<()> {

        // Convert the accounts HashMap to a JSON array
        let accounts_json: Vec<serde_json::Value> = self.accounts.iter()
            .map(|(key, &balance)| json!({"public_key": hex::encode(key), "balance": balance}))
            .collect();

        // Create a JSON object with the final hash and accounts
        let merkle_tree_json = json!({
            "final_hash": final_hash,
            "accounts": accounts_json
        });

        // Serialize the JSON object and write it to a file
        let serialized = serde_json::to_string_pretty(&merkle_tree_json).expect("Failed to serialize Merkle tree");
        let mut file = File::create("merkleTree.json")?;
        file.write_all(serialized.as_bytes())?;

        Ok(())
    }

    // Method to clear the Merkle tree 
    fn clear(&mut self) {
        self.root = None;
        self.accounts.clear();
    }

    // Method to load the Merkle tree from a JSON file
    pub fn load_from_json(&mut self) -> std::io::Result<()> {
        let mut file = File::open("merkleTree.json")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // Parse the JSON string
        let parsed: Value = serde_json::from_str(&contents)?;

        // Clear existing data
        self.clear();

        // Example assumes `accounts` is directly under root and public_key is hex encoded
        if let Some(accounts) = parsed["accounts"].as_array() {
            for account in accounts {
                let public_key_hex = account["public_key"].as_str().unwrap_or_default();
                let balance = account["balance"].as_u64().unwrap_or_default();

                // Decode the hex public key
                if let Ok(public_key) = hex_decode(public_key_hex) {
                    self.accounts.insert(public_key, balance);
                }
            }
        }

        Ok(())
    }
}
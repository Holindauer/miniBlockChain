use sha2::{Sha256, Digest};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use hex;


/**
 * @notice merkleTree.rs contains an implementation of a merkle tree for the purpose of retrieval of 
 * account balances from public keys. As well as for the validation of transactions and new blocks 
 * being written to the blockchain.
 *          
 * This implementation splits the merkle tree into two parts. The first being a simple hash map that 
 * allows for efficient retrieval of account balances from public keys. The second part is the actual
 * merkle tree structure, which hashes together (account balance, pub key) pairs until a root hash is 
 * formed. This root hash is required during the consensus process to validate new blocks being written
 * to the blockchain.
 */

/**
 * @notice Account struct represents a single account in the blockchain.
 * @param public_key - the public key of the account as a vector of bytes.
 * @param balance - the balance of the account.
 * @param nonce - the nonce of the account (amount of transactions sent from this account).
 */
#[derive(Debug, Clone)]
struct Account {
    public_key: Vec<u8>,
    balance: u64,
    nonce: u64,
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
 * the hash map of accounts form the base of the tree, and the root node is the top of the tree.
 * @param root - the root node of the merkle tree.
 * @param accounts - a vector of all accounts in the blockchain. Stored in Account structs.
 * @param accountsMap - a hash map of account balances indexed by public key.
 */
#[derive(Debug, Clone)]
struct MerkleTree {
    root: Option<MerkleNode>,
    accountsVec: Vec<Account>,
    accountsMap: HashMap<Vec<u8>, u64>,
}

impl MerkleTree {

    // constructor 
    fn new() -> Self {
        MerkleTree {
            root: None,
            accountsVec: Vec::new(),
            accountsMap: HashMap::new(),
        }
    }

    // Hashes an Account struct and inserts into accountsMap HashMap
    fn add_account(&mut self, account: Account) {
        self.accountsVec.push(account.clone());    // accou
        self.accountsMap.insert(account.public_key.clone(), account.balance); // Use public key as key and balance as value
    }

    // Retrieves account balance from accountsMap public key
    fn retrieve_account_balance(&self, public_key: Vec<u8>) -> Option<u64> {
        self.accountsMap.get(&public_key).cloned()
    }
    
    /**
     * @notice generate_merkle_root() is a method that generates the root hash of the merkle tree. It is called during the consensus 
     * process to validate new blocks being written to the blockchain. The method transforms accounts into leaf nodes and hashes them 
     * together in pairs until only one node remains. The final node is set as the root node of the merkle tree.
     */
    fn generate_merkle_root(&mut self) {

        // Transform accounts into leaf nodes w/ .map()
        let mut nodes = self.accountsVec.iter()
            .map(|account| MerkleNode::Leaf { hash: MerkleTree::hash_account(account) })
            .collect::<Vec<_>>();
        
        // Loop in chunks of 2, hashing nodes together until only 1 remains
        while nodes.len() > 1 {
            nodes = nodes.chunks(2)
                .map(|chunk| {
                    match chunk {
                        [left, right] => MerkleTree::pair_and_hash(left.clone(), right.clone()), // Even num nodes
                        [left] => MerkleTree::pair_and_hash(left.clone(), left.clone()),         // odd num nodes
                        _ => unreachable!(),
                    }
                })
                .collect();
        }
        
        // Set the root node to the final node
        self.root = nodes.into_iter().next();
    }

    // Helper function for initially hashing leaf node contents, used in generate_merkle_root()
    fn hash_account(account: &Account) -> Vec<u8> {

        // new SHA256 hasher
        let mut hasher = Sha256::new();

        // combine bytes and hash
        hasher.update(&account.public_key);
        hasher.update(&account.balance.to_be_bytes());
        hasher.update(&account.nonce.to_be_bytes());    

        // return the hash
        hasher.finalize().to_vec()
    }
    
    // Helper function to hash two nodes together and return a new branch node
    fn pair_and_hash(left: MerkleNode, right: MerkleNode) -> MerkleNode {
        let mut hasher = Sha256::new(); // new SHA256 hasher
    
        // Simplify by directly hashing the contents regardless of leaf or branch
        hasher.update(MerkleTree::extract_hash(&left));
        hasher.update(MerkleTree::extract_hash(&right));
        
        // Hash combined contents of the left and right nodes and return within a new branch node
        let hash = hasher.finalize().to_vec();
        MerkleNode::Branch { hash, left: Box::new(left), right: Box::new(right) }
    }
    
    // Helper function to extract hash from either type of MerkleNode
    fn extract_hash(node: &MerkleNode) -> &[u8] {
        match node { MerkleNode::Leaf { hash } | MerkleNode::Branch { hash, .. } => hash, }
    }

    // Method to save the Merkle tree to a JSON file
    pub fn save_to_json(&self) -> std::io::Result<()> {

        // get merkle root hash 
        let final_hash = match &self.root {
            Some(MerkleNode::Branch { hash, .. }) => hash.clone(),
            Some(MerkleNode::Leaf { hash }) => hash.clone(),
            None => vec![0; 32],
        };

        // Convert the accountsVec to a JSON array
        let accounts_json: Vec<serde_json::Value> = self.accountsVec.iter()
            .map(|account| {
                json!({
                    "public_key": hex::encode(&account.public_key),
                    "balance": account.balance,
                    "nonce": account.nonce,
                })
            })
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

    // Method to load the Merkle tree from a JSON file
    pub fn load_from_json(&mut self) -> std::io::Result<()> {

        // Read the contents of the JSON file
        let mut file = File::open("merkleTree.json")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // Parse the JSON string
        let parsed: Value = serde_json::from_str(&contents)?;

        // Clear existing data
        self.clear();

        // Extract the final hash and accounts from the JSON object, populating the Merkle tree
        let final_hash = hex::decode(parsed["final_hash"].as_str().unwrap()).unwrap();
        let accounts_json = parsed["accounts"].as_array().unwrap();

        // Set the root node to the final hash
        for account in accounts_json {

            // Extract account details from the JSON object
            let public_key = hex::decode(account["public_key"].as_str().unwrap()).unwrap();
            let balance = account["balance"].as_u64().unwrap();
            let nonce = account["nonce"].as_u64().unwrap();

            // Add the account to the Merkle tree
            self.add_account(Account { public_key, balance, nonce });
        }

        Ok(())
    }

    // Helper to clear the Merkle tree 
    fn clear(&mut self) {
        self.root = None;
        self.accountsVec.clear();
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    /**
     * @test test_add_account() is a test function that checks the add_account() method of the MerkleTree struct.
     */
    #[test]
    fn test_add_account() {
        let mut tree = MerkleTree::new(); // create a new MerkleTree instance
        let account = Account { // create a new Account instance w/ mock data
            public_key: vec![1, 2, 3, 4],
            balance: 100,
            nonce: 1,
        };
        tree.add_account(account.clone());
        
        // ensure the account was added to the tree correctly
        assert_eq!(tree.accountsVec.len(), 1);
        assert!(tree.accountsMap.contains_key(&account.public_key));
        assert_eq!(tree.accountsMap[&account.public_key], account.balance);
    }

    /**
     * @test test_retrieve_account_balance() is a test function that checks the retrieve_account_balance() method of the MerkleTree struct.
     */
    #[test]
    fn test_retrieve_account_balance() {
        let mut tree = MerkleTree::new(); // create a new MerkleTree instance
        let account = Account { // create a new Account instance w/ mock data
            public_key: vec![1, 2, 3, 4],
            balance: 100,
            nonce: 1,
        };
        tree.add_account(account.clone());  // add the account to the tree

        // ensure the account balance can be retrieved correctly
        let balance = tree.retrieve_account_balance(account.public_key.clone()).unwrap();
        assert_eq!(balance, account.balance);
    }

    /**
     * @test test_generate_merkle_root_single_account() is a test function that checks the generate_merkle_root() method of the MerkleTree struct.
     */
    #[test]
    fn test_generate_merkle_root_single_account() {
        let mut tree = MerkleTree::new();
        let account = Account {
            public_key: vec![1, 2, 3, 4],
            balance: 100,
            nonce: 1,
        };  

        // add the account to the tree and generate the merkle root
        tree.add_account(account);
        tree.generate_merkle_root();

        // ensure root node hash exists and has the correct length
        assert!(tree.root.is_some());
        if let Some(MerkleNode::Leaf { hash, .. }) = &tree.root {
            assert_eq!(hash.len(), 32);
        } else {
            panic!("Root should be a leaf node");
        }
    }

    #[test]
    fn test_generate_merkle_root_multiple_accounts() {
        let mut tree = MerkleTree::new(); // create a new MerkleTree instance
        let account1 = Account { // create a new Account instance w/ mock data
            public_key: vec![1, 2, 3, 4], 
            balance: 100,
            nonce: 1,
        };
        
        // add 25 account clones to the tree
        for i in 0..25 {
            let mut account = account1.clone();
            account.balance += i;
            tree.add_account(account);
        }

        // generate the merkle root
        tree.generate_merkle_root();

        // ensure root node hash exists and has the correct length
        assert!(tree.root.is_some());
        if let Some(MerkleNode::Branch { hash, .. }) = &tree.root {
            assert_eq!(hash.len(), 32);
        } else {
            panic!("Root should be a branch node");
        }

    }
}
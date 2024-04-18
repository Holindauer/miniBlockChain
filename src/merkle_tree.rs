use sha2::{Sha256, Digest};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

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
 * @param obfuscated_private_key - the obfuscated private key is a hash of the private key of the account when represented
 * as an elliptic curve point. The curve25519_dalek library is used to perform scalar multiplication with the generator point 
 * and the private key. Knowledge of the private key by a user during transaction request is verified in a simple zk proof
 * by provding two numbers that sum to the private key private key, that when added together as elliptic curve points add 
 * to the obfuscated public key.
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub public_key: Vec<u8>,
    pub obfuscated_private_key_hash: Vec<u8>,
    pub balance: u64,
    pub nonce: u64,
}

/**
 * @notice the MerkleNode enum represents a single node leaf/branch in the merkle tree. 
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MerkleNode {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    pub root: Option<MerkleNode>,
    pub accounts_vec: Vec<Account>,
    pub accounts_map: HashMap<Vec<u8>, u64>,
}

impl MerkleTree {

    // constructor 
    pub fn new() -> Self {
        MerkleTree {
            root: None,
            accounts_vec: Vec::new(),
            accounts_map: HashMap::new(),
        }
    }

    // Hashes an Account struct and inserts into accountsMap HashMap
    pub fn insert_account(&mut self, account: Account) {
        self.accounts_vec.push(account.clone());    // accou
        self.accounts_map.insert(account.public_key.clone(), account.balance); // Use public key as key and balance as value
    }

    // Retrieves account balance from accountsMap public key
    pub fn get_account_balance(&self, public_key: Vec<u8>) -> Option<u64> {
        self.accounts_map.get(&public_key).cloned()
    }

    // Returns an accounts private key hash
    pub fn get_private_key_hash(&self, public_key: Vec<u8>) -> Option<Vec<u8>> {
        self.accounts_vec.iter().find(|account| account.public_key == public_key).map(|account| account.obfuscated_private_key_hash.clone())
    }

    // Returns the nonce of a specific public key
    pub fn get_nonce(&self, public_key: Vec<u8>) -> Option<u64> {
        self.accounts_vec.iter().find(|account| account.public_key == public_key).map(|account| account.nonce)
    }
    
    // Checks if an account exists
    pub fn account_exists(&self, public_key: Vec<u8>) -> bool {
        self.accounts_map.contains_key(&public_key)
    }

    // Changes the balance of an account
    pub fn change_balance(&mut self, public_key: Vec<u8>, new_balance: u64) {
        if let Some(balance) = self.accounts_map.get_mut(&public_key) {
            *balance = new_balance;
        }
    }

    // Increments the nonce of an account
    pub fn increment_nonce(&mut self, public_key: Vec<u8>) {
        if let Some(account) = self.accounts_vec.iter_mut().find(|account| account.public_key == public_key) {
            account.nonce += 1;
        }
    }

    /**
     * @notice generate_merkle_root() is a method that generates the root hash of the merkle tree. It is called during the consensus 
     * process to validate new blocks being written to the blockchain. The method transforms accounts into leaf nodes and hashes them 
     * together in pairs until only one node remains. The final node is set as the root node of the merkle tree.
     */
    fn generate_merkle_root(&mut self) {

        // Transform accounts into leaf nodes w/ .map()
        let mut nodes: Vec<MerkleNode> = self.accounts_vec.iter()
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
        hasher.update(&account.obfuscated_private_key_hash);

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
}


#[cfg(test)]
mod tests {
    use super::*;
    
    /**
     * @test test_insert_account() is a test function that checks the insert_account() method of the MerkleTree struct.
     */
    #[test]
    fn test_insert_account() {
        let mut tree = MerkleTree::new(); // create a new MerkleTree instance
        let account = Account { // create a new Account instance w/ mock data
            public_key: vec![1, 2, 3, 4],
            obfuscated_private_key_hash: vec![1, 2, 3, 4],
            balance: 100,
            nonce: 1,
        };
        tree.insert_account(account.clone());
        
        // ensure the account was added to the tree correctly
        assert_eq!(tree.accounts_vec.len(), 1);
        assert!(tree.accounts_map.contains_key(&account.public_key));
        assert_eq!(tree.accounts_map[&account.public_key], account.balance);
    }

    /**
     * @test test_retrieve_account_balance() is a test function that checks the retrieve_account_balance() method of the MerkleTree struct.
     */
    #[test]
    fn test_retrieve_account_balance() {
        let mut tree = MerkleTree::new(); // create a new MerkleTree instance
        let account = Account { // create a new Account instance w/ mock data
            public_key: vec![1, 2, 3, 4],
            obfuscated_private_key_hash : vec![1, 2, 3, 4],
            balance: 100,
            nonce: 1,
        };
        tree.insert_account(account.clone());  // add the account to the tree

        // ensure the account balance can be retrieved correctly
        let balance = tree.get_account_balance(account.public_key.clone()).unwrap();
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
            obfuscated_private_key_hash: vec![1, 2, 3, 4],   
            balance: 100,
            nonce: 1,
        };  

        // add the account to the tree and generate the merkle root
        tree.insert_account(account);
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
    fn test_account_exists(){

        let mut tree = MerkleTree::new();
        let account = Account {
            public_key: vec![1, 2, 3, 4],
            obfuscated_private_key_hash: vec![1, 2, 3, 4],
            balance: 100,
            nonce: 1,
        };

        tree.insert_account(account.clone());
        assert!(tree.account_exists(account.public_key.clone()));
    }

    #[test]
    fn test_generate_merkle_root_multiple_accounts() {
        let mut tree = MerkleTree::new(); // create a new MerkleTree instance
        let account1 = Account { // create a new Account instance w/ mock data
            public_key: vec![1, 2, 3, 4], 
            obfuscated_private_key_hash: vec![1, 2, 3, 4],
            balance: 100,
            nonce: 1,
        };
        
        // add 25 account clones to the tree
        for i in 0..25 {
            let mut account = account1.clone();
            account.balance += i;
            tree.insert_account(account);
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
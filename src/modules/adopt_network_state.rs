use tokio::sync::{Mutex, Notify};
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use sha2::{Digest, Sha256};

use serde::{Serializer, Deserializer, Deserialize, Serialize};
use serde::de::{self, Visitor, MapAccess};
use serde::ser::SerializeMap;
use serde_json::Value;

use base64::{encode, decode};

use std::{fmt, collections::HashMap};
use std::sync::Arc;

use crate::modules::validation::ValidatorNode;
use crate::modules::blockchain::{BlockChain, Block};
use crate::modules::merkle_tree::{MerkleTree, Account};
use crate::modules::requests;


/**
 * @notice adopt_network_state.rs contains the logic for validator nodes connecting to the network to adopt the majority
 * ledger state of the network. This file contains the logic for sending a request to all other nodes for a copy of their
 * blockchain, merkle tree, and all previously used zk-proofs. The validator node will wait for all peer responses (as per
 * the time of the call to adopt_network_state()) and then determine the majority state of the network. The validator node
 * will then update its local blockchain and merkle tree to reflect the majority state of the network.
*/

/**
 * @notice The PeerLedgerResponse struct is a serializable struct that is used to package the blockchain and merkle tree
 * data of a validator node. This struct is used to send the blockchain and merkle tree data to other validators when
 * they request it. The struct is also used to store the blockchain and merkle tree data of other validators when they
 * send it to this validator node.
 * @dev the custom serialization functions are used because of the complexity of the HashMap fields in the struct. This
 * allowes for calls to serde_json::to_string() and serde_json::from_str() to work as expected.
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerLedgerResponse {
    action: String,
    blockchain: Vec<Block>,
    accounts_vec: Vec<Account>,
    #[serde(serialize_with = "serialize_account_balances_map", deserialize_with = "deserialize_account_balances_map")] 
    accounts_map: HashMap<Vec<u8>, u64>,
    #[serde(serialize_with = "serialize_used_zk_proofs_map", deserialize_with = "deserialize_used_zk_proofs_map")]
    used_zk_proofs: HashMap<Vec<u8>, Vec<String>>,
}


/**
 * @notice adopt_network_state() is an asynchronous function that fascililtates the process of updating 
 * the local blockchain and merkle tree to the majority state of the network. This function is called by
 * validation::run_validation() when booting up a new validator node.
*/
pub async fn adopt_network_state(validator_node: ValidatorNode) {
    println!("\nSending request to peers for majority networtk state...");   

    // Send request to all peers for their blockchain and merkle tree data
    requests::send_peer_ledger_request(validator_node.clone()).await;

    // Wait for all active peers to respond with their ledgers
    validator_node.await_all_peer_ledger_states_received().await;

    // Determine the majority state of the network and update the local blockchain and merkle tree
    adopt_majority(validator_node.clone()).await;
 }


/**
 * @notice handle_peer_ledger_request() is passed a serde_json::Value struct containing a request from the master 
 * event handler in network.rs for a copy of this node's ledger state (blockchain, merkle tree, used zk proofs). 
 * This function will package the blockchain and merkle tree data into a PeerLedgerResponse struct and send it back 
 * to the requesting peer node.
 */
 pub async fn handle_peer_ledger_request(request: Value, validator_node: ValidatorNode)-> Result<(), Box<dyn std::error::Error>> {

    // lock blockchain
    let blockchain: Arc<Mutex<BlockChain>> = validator_node.blockchain.clone();
    let blockchain_guard = blockchain.lock().await;
    
    // lock merkle tree
    let merkle_tree : Arc<Mutex<MerkleTree>> = validator_node.merkle_tree.clone();
    let merkle_tree_guard = merkle_tree.lock().await;

    // lock the used zk proofs map
    let used_zk_proofs: Arc<Mutex<HashMap<Vec<u8>, Vec<String>>>> = validator_node.used_zk_proofs.clone();
    let used_zk_proofs: HashMap<Vec<u8>, Vec<String>> = used_zk_proofs.lock().await.clone();

    // retrieve blockchain and merkle tree data
    let blockchain: Vec<Block> = blockchain_guard.chain.clone();
    let accounts_vec: Vec<Account> = merkle_tree_guard.accounts_vec.clone();
    let accounts_map: HashMap<Vec<u8>, u64> = merkle_tree_guard.accounts_map.clone();

    // Package data into a PeerLedgerResponse struct
    let response: PeerLedgerResponse = PeerLedgerResponse {
        action: "PeerLedgerResponse".to_string(),
        blockchain,
        accounts_vec,
        accounts_map,
        used_zk_proofs
    };

    // serialize the PeerLedgerResponse struct into a JSON string 
    let ledger_json: String = serde_json::to_string(&response).unwrap();

    // Retrieve response port from request
    let response_port: String = request["response_port"].as_str().unwrap().to_string();

    // Connect to port and send message  
    match TcpStream::connect(response_port.clone()).await {

        // Send message to port if connection is successful
        Ok(mut stream) => {

            // Write to the Stream
            if let Err(e) = stream.write_all(ledger_json.as_bytes()).await { 
                eprintln!("Failed to send message to {}: {}", response_port, e); 
                return Ok(());
            }
            println!("Sent repsonse to conensus request to: {}", response_port); 
        },
        Err(_) => { println!("Failed to connect to {}, There may not be a listener...", response_port); }
    }

    Ok(())
 }

 /**
  * @handle_peer_ledger_response() is passed the serde::json repsonse sent back by peers using handle_peer_ledger_request() 
  * from all  that takes a serde_json::Value struct containing the blockchain, merkle tree, and used zk-proofs of the peer node. 
  * This function will stores the data in the peer_ledger_state field of the client's ValidatorNode struct for majority consensus 
  * determination once the listening period has ended.
  * @dev As responses come in, the notify_all_ledgers_received field of the ValidatorNode struct is notified to check if all
  * peers have responded. If all peers have responded, the adopt_majority() function is called to determine the majority state
  */
 pub async fn handle_peer_ledger_response(response: Value, validator_node: ValidatorNode)-> Result<(), Box<dyn std::error::Error>> {

    // Deserialize the JSON string into a PeerLedgerResponse struct
    let peer_ledger_response: PeerLedgerResponse = serde_json::from_str(&response.to_string()).unwrap();

    // Lock the peer_ledger_state mutex
    let peer_ledger_state: Arc<Mutex<Vec<PeerLedgerResponse>>> = validator_node.peer_ledger_states.clone();
    let mut peer_ledger_state_guard = peer_ledger_state.lock().await;

    // Store the PeerLedgerResponse struct in the peer_ledger_state vector
    peer_ledger_state_guard.push(peer_ledger_response);

    // Notify the main thread that a new response is in. This will trigger an updated check of 
    // whether all peers have responded. (See the validator_node impl in validation.rs)
    let notify_all_ledgers_received: Arc<Notify> = validator_node.notify_all_ledgers_received.clone();
    notify_all_ledgers_received.notify_one();
    
    Ok(())
 }

 /**
  * @notice adopt_majority() determines the majority state of the collected peer_ledger_states and update the local blockchain, 
  * merkle tree, and used zk proofs of the validator node to reflect the majority state of the network. 
  * @dev The majority is determine by hashing the blockchain and merkle tree data of each peer_ledger_state and counting the 
  * number of occurences of each hash using a hash map. The hash with the most occurences is considered the majority state.
  */
  async fn adopt_majority(validator_node: ValidatorNode){
    println!("\nDetermining majority network state...");

    // Lock the peer_ledger_states mutex
    let peer_ledger_states: Arc<Mutex<Vec<PeerLedgerResponse>>> = validator_node.peer_ledger_states.clone();
    let peer_ledger_states_guard = peer_ledger_states.lock().await;

    // If there are no peer_ledger_states to adopt, return (maintaining the empty local state)
    if peer_ledger_states_guard.is_empty() { 
        println!("No peer ledger states to adopt..."); 
        return; 
    }

    // HashMap to store the hashes of each ledger in each peer_ledger_state
    let mut ledger_hash_map: HashMap<Vec<u8>, u32> = HashMap::new();

    // Iterate through each peer_ledger_state and hash the blockchain and merkle tree data
    for peer_ledger_state in peer_ledger_states_guard.iter() {

        // hash the entire PeerLedgerResponse struct with a new SHA256 hasher
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_string(&peer_ledger_state).unwrap());
        let hash: Vec<u8> = hasher.finalize().to_vec();

        // increment the hash's count in the ledger_hash_map
        let count: &mut u32 = ledger_hash_map.entry(hash).or_insert(0);
        *count += 1;
    }

    // Find the hash with the most occurences in the ledger_hash_map
    let (_, _) = ledger_hash_map.iter()
        .max_by_key(|entry| entry.1)
        .map(|(hash, count)| (hash.clone(), *count))
        .unwrap();  // Assuming there will be at least one entry

    // Find the peer_ledger_state with the majority hash
    let majority_peer_ledger_state = ledger_hash_map.iter()
        .max_by_key(|entry| entry.1)
        .and_then(|(hash, _)| {
            peer_ledger_states_guard.iter()
                .find(|state| {

                    // hash the entire state
                    let mut hasher = Sha256::new();
                    hasher.update(serde_json::to_string(&state).unwrap());
                    hasher.finalize().to_vec() == *hash
                })
        })
        .unwrap_or_else(|| {
            // Default to the first state if no majority is found (ie no consensus)
            peer_ledger_states_guard.first().expect("There must be at least one state")
        });

    // lock the blockchain, merkle tree, and used zk proofs map
    let mut blockchain_guard = validator_node.blockchain.lock().await;
    let mut merkle_tree_guard = validator_node.merkle_tree.lock().await;
    let mut used_zk_proofs_guard = validator_node.used_zk_proofs.lock().await;

    // Update the local blockchain state to reflect the majority state
    blockchain_guard.chain = majority_peer_ledger_state.blockchain.clone();

    // update the local merkle tree state to reflect the network majority
    merkle_tree_guard.accounts_vec = majority_peer_ledger_state.accounts_vec.clone();
    merkle_tree_guard.accounts_map = majority_peer_ledger_state.accounts_map.clone();

    // update the local used zk proofs map to reflect the network majority
    *used_zk_proofs_guard = majority_peer_ledger_state.used_zk_proofs.clone();
        
    println!("\n\n--- Adopted majority network state ---\n\n");   
}


/**
 * @notice The PeerLedgerResponse struct is a serializable struct that is used to package the blockchain and merkle tree
 * data of a validator node. This struct is used to send the blockchain and merkle tree data to other validators when
 * they request it. The struct is also used to store the blockchain and merkle tree data of other validators when they
 * send it to this validator node.
 */
fn serialize_account_balances_map<S>(map: &HashMap<Vec<u8>, u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{   
    // Encode the keys of the map to base64 before serialization
    let map: HashMap<String, u64> = map
        .iter()
        .map(|(k, v)| (encode(k), *v))
        .collect();
    map.serialize(serializer)
}

/**
 * @notice The serialize_map() function is a custom serialization function that serializes the accounts_map field of the
 * PeerLedgerResponse struct. The accounts_map field is a HashMap with keys of type Vec<u8> and values of type u64. The
 * keys are base64 encoded before serialization to JSON to ensure that the keys are valid JSON strings.
 */
fn deserialize_account_balances_map<'de, D>(deserializer: D) -> Result<HashMap<Vec<u8>, u64>, D::Error>
where
    D: Deserializer<'de>,
{       
    // Custom visitor for deserializing the accounts map
    struct BytesMapVisitor;

    // Implement the visitor trait for the custom visitor
    impl<'de> Visitor<'de> for BytesMapVisitor {
        type Value = HashMap<Vec<u8>, u64>;
        
        // Define the error type for the visitor
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a map of base64 strings to u64 integers")
        }
        
        // Deserialize the map
        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {   
            // Create a new HashMap to store the deserialized map
            let mut map: HashMap<Vec<u8>, u64> = HashMap::new();
            while let Some((key, value)) = access.next_entry::<String, u64>()? {
                map.insert(decode(&key).map_err(de::Error::custom)?, value);
            }
            Ok(map)
        }
    }

    // call the visitor to deserialize the map
    deserializer.deserialize_map(BytesMapVisitor)
}


/**
 * @notice The serialize_used_zk_proofs_map() function is a custom serialization function that serializes the used_zk_proofs
 * field of the PeerLedgerResponse struct. The used_zk_proofs field is a HashMap with keys of type Vec<u8> and values of type
 * Vec<String>. The keys are base64 encoded before serialization to JSON to ensure that the keys are valid JSON strings.
 */
fn serialize_used_zk_proofs_map<S>(map: &HashMap<Vec<u8>, Vec<String>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map_ser = serializer.serialize_map(Some(map.len()))?;
    for (key, value) in map {
        map_ser.serialize_entry(&encode(key), value)?;
    }
    map_ser.end()
}

/**
 * @notice The deserialize_used_zk_proofs_map() function is a custom deserialization function that deserializes the used_zk_proofs
 * field of the PeerLedgerResponse struct. The used_zk_proofs field is a HashMap with keys of type Vec<u8> and values of type
 * Vec<String>. The keys are base64 encoded and must be decoded before being inserted into the HashMap.
 */
fn deserialize_used_zk_proofs_map<'de, D>(deserializer: D) -> Result<HashMap<Vec<u8>, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{

    // Custom visitor for deserializing the used zk proofs map
    struct UsedZkProofsVisitor;

    // Implement the visitor trait for the custom visitor
    impl<'de> Visitor<'de> for UsedZkProofsVisitor {
        type Value = HashMap<Vec<u8>, Vec<String>>;

        // error type for the visitor
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a map of base64 strings to list of strings")
        }

        // Deserialize the map
        fn visit_map<M>(self, mut access: M) -> Result<HashMap<Vec<u8>, Vec<String>>, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut map = HashMap::new();
            while let Some((key, value)) = access.next_entry::<String, Vec<String>>()? {
                map.insert(decode(&key).map_err(de::Error::custom)?, value);
            }
            return Ok(map);
        }
    }

    // call the visitor to deserialize the map
    deserializer.deserialize_map(UsedZkProofsVisitor)
}


#[cfg(test)]
mod tests {
    use super::*;  // Import the necessary structs and functions from the parent module.

    #[test]
    fn test_serialize_deserialize_cycle() {
        // Setup a sample PeerLedgerResponse instance
        let peer_ledger_response = PeerLedgerResponse {
            action: "PeerLedgerResponse".to_string(),
            blockchain: vec![
                Block::Genesis { time: 1633046400 },
                Block::Transaction {
                    sender: vec![1, 2, 3],
                    sender_balance: 500,
                    recipient: vec![4, 5, 6],
                    recipient_balance: 450,
                    amount: 50,
                    time: 1633046450,
                    sender_nonce: 1,
                    hash: vec![7, 8, 9],
                },
            ],
            accounts_vec: vec![
                Account {
                    public_key: vec![1, 2, 3],
                    public_key_hash: vec![4, 5, 6],
                    balance: 1000,
                    nonce: 0,
                },
            ],
            accounts_map: {
                let mut map = HashMap::new();
                map.insert(vec![1, 2, 3], 1000u64);
                map
            },
            used_zk_proofs: {
                let mut map = HashMap::new();
                map.insert(vec![1, 2, 3], vec!["proof1".to_string(), "proof2".to_string()]);
                map
            },
        };

        // Serialize the response
        let serialized = serde_json::to_string(&peer_ledger_response)
            .expect("Serialization should succeed");

        // Deserialize it back to an object
        let deserialized = serde_json::from_str(&serialized)
            .expect("Deserialization should succeed");


        // Assert that the deserialized object matches the original
        assert_eq!(peer_ledger_response, deserialized, "Deserialized object should be equal to the original");
    }

    #[test]
    fn test_peer_ledger_response_serialization() {
        let mut used_zk_proofs = HashMap::new();
        used_zk_proofs.insert(vec![1, 2, 3], vec![String::from("example_proof")]);

        let response = PeerLedgerResponse {
            action: String::from("PeerLedgerResponse"),
            blockchain: vec![],
            accounts_vec: vec![],
            accounts_map: HashMap::new(),
            used_zk_proofs,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        println!("Serialized: {}", serialized);

        let deserialized: PeerLedgerResponse = serde_json::from_str(&serialized).unwrap();
        println!("Deserialized: {:?}", deserialized);
    }
}

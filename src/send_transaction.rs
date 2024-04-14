extern crate secp256k1;
extern crate hex;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use serde::{Serialize, Deserialize};
use serde_json;

use base64;

use std::{io, fs};

use crate::zk_proof;
use crate::constants::VERBOSE_STACK;
use crate::network::NetworkConfig;

/**
 * @notice send_tranasaction.rs contains the logic for sending a network to request a transaction of value between two
 *         accounts in the network. 
 * 
 * Transaction requests on the client side follow a simple protocol: 
 * 
 *    A sender must provide the following arguments in order to make a transction request:
 *    
 *       - The sender's private key
 *       - The recipient's public key
 *       - The amount to be sent
 * 
 *    As well, derived from these arguments are the following two additionall pieces of information:
 * 
 *       - the sender's public key derrived from the private
 *       - an elliptic curve representation of the sender's private key for use in a simple zk proof scheme. 
 * 
 *    The elliptic curve representation takes the provided private key, randomly split it into two scalars, multiplies the 
 *    scalars by the generator point of the curve25519 elliplic curve over a finite field (using the curve25519_dalek library).
 *    These points are base64 encoded and sent to validator nodes in the network. 
 *    
 *    Due to the properties of elliptic curve cryptography, the sum of these points will be the same as the original private
 *    key multiplied by the generator point. A hash of each account's private key in this elliptic curve representation is 
 *    stored in the merkle tree. Validators will ensure that the provided eliptic curve points sent with a transaction request
 *    are the same when added and hashed together as the hash in the tree. This verifies the sender has knowledge of the private 
 *    key without revealing it. It is also computationally intractable to derive the private key from the two points due to the 
 *    group homomorphism present.
 */

 /**
  * @notice KeyPair encapsulate a new private and public key generated for a new 
  * blockchain account for the purpose of sending to other nodes in the network.
  */
 #[derive(Serialize, Deserialize)]
 pub struct TransactionRequest {
     pub action: String,
     pub sender_public_key: String,     
     pub sender_obfuscated_private_key_part1: String,
     pub sender_obfuscated_private_key_part2: String,
     pub recipient_public_key: String,
     pub amount: String,
}
 


 /**
 * @notice send_transcation_request() asynchonously packages a transaction request and sends it to the network.
 * @dev The sender's private key is split into two parts, each multiplied by the generator point of the curve25519
 *      elliptic curve. The points are base64 encoded and sent to the network along w/ other transaction details.
 */
pub async fn send_transaction_request(sender_private_key: String, recipient_public_key: String, amount: String ) {
    if VERBOSE_STACK { println!("send_transaction::send_transaction_request() : Sending transaction request...") };

    // derive the public key from the private key
    let sender_public_key: String = zk_proof::derive_public_key_from_private_key(&sender_private_key);

    // Convert the private key to two RistrettoPoints (elliptic curve points)
    let (point1, point2) = zk_proof::private_key_to_curve_points(&sender_private_key);

    // Base64 encode the points to send over the network
    let encoded_key_point_1: String = base64::encode(point1.compress().to_bytes());
    let encoded_key_point_2: String = base64::encode(point2.compress().to_bytes());

    // Package the message
    let request: TransactionRequest = TransactionRequest {
        action: "transaction".to_string(),
        sender_public_key,
        sender_obfuscated_private_key_part1: encoded_key_point_1,
        sender_obfuscated_private_key_part2: encoded_key_point_2,
        recipient_public_key,
        amount,
    };
    let request_json: String = serde_json::to_string(&request).unwrap();    

    // Load accepted ports configuration
    let config_data: String = fs::read_to_string("accepted_ports.json").map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
    let config: NetworkConfig = serde_json::from_str(&config_data).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
  
    // Send account creation request to all accepted po
    for node in config.nodes.iter() {
        let addr = format!("{}:{}", node.address, node.port);
        if let Ok(mut stream) = TcpStream::connect(&addr).await {
            let _ = stream.write_all(request_json.as_bytes()).await;
        }
    }
}



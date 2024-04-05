extern crate secp256k1;
extern crate hex;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use serde::{Serialize, Deserialize};
use serde_json;

use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT; // generator point
use curve25519_dalek::scalar::Scalar;

use base64;
use std::io;

use crate::zk_proof;
use crate::constants::{PORT_NUMBER, VERBOSE};


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
 * @notice send_transaction() is a wrapper called within main.rs that instigates the process of accessing
 * the network from the client side for sending a transaction request. 
 * @dev the provided keys are hexadecimal strings and the amount is string integer
 */
pub fn send_transaction(
    sender_public_key: &String, 
    sender_private_key: &String, 
    recipient_public_key: 
    &String, amount: &String) { // TODO derive pub key from private key
        if VERBOSE { println!("send_transaction::send_transaction() : Sending transaction request...") };

        // Create a new Tokio runtime 
        let rt = tokio::runtime::Runtime::new().unwrap();

        // block_on the async account creation process, display the results   
        match rt.block_on(send_transaction_request(
            sender_public_key.to_string(), 
            sender_private_key.to_string(), 
            recipient_public_key.to_string(), 
            amount.to_string())
        ) 
        {Ok(_) => { 
            if VERBOSE { println!("send_transaction::send_transaction() : Transaction request sent successfully") }; 
        },
        Err(e) => { eprintln!("Account creation failed: {}", e); return; }, };       
}   

/**
 * @notice send_transcation_request() asynchonously packages a transaction request and sends it to the network.
 * @dev The sender's private key is split into two parts, each multiplied by the generator point of the curve25519
 *      elliptic curve. The points are base64 encoded and sent to the network along w/ other transaction details.
 */
async fn send_transaction_request(
    sender_public_key: String, 
    sender_private_key: String, 
    recipient_public_key: String, 
    amount: String
) -> Result<(), io::Error> {
    if VERBOSE { println!("send_transaction::send_transaction_request() : Sending transaction request...") };

    // Convert the private key to two RistrettoPoints (elliptic curve points)
    let (point1, point2) = zk_proof::private_key_to_curve_points(&sender_private_key);

    // Base64 encode the points to send over the network
    let encoded_point1: String = base64::encode(point1.compress().to_bytes());
    let encoded_point2: String = base64::encode(point2.compress().to_bytes());

    // Package the message
    let message: TransactionRequest = TransactionRequest {
        action: "transaction".to_string(),
        sender_public_key,
        sender_obfuscated_private_key_part1: encoded_point1,
        sender_obfuscated_private_key_part2: encoded_point2,
        recipient_public_key,
        amount,
    };

    // Connect and send the message
    let mut stream: TcpStream = TcpStream::connect(PORT_NUMBER).await?; 
    let message_json: String = serde_json::to_string(&message)?;
    stream.write_all(message_json.as_bytes()).await?;

    Ok(())
}



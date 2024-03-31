use curve25519_dalek::ristretto::RistrettoPoint;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;
use std::io;

extern crate secp256k1;
extern crate hex;



use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT; // generator point
use curve25519_dalek::scalar::Scalar;
use base64::encode;

use rand::rngs::OsRng; // A cryptographically secure RNG
use rand::RngCore;

use std::convert::TryInto;


/**
 * @notice send_tranasaction.rs contains the logic for sending a network
 * request to transact value between two accounts in the network.
 */

 const PORT_NUMBER: &str = "127.0.0.1:8080"; // TODO figure out how to link thi between src files

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
 
pub fn send_transaction(sender_public_key: &String, sender_private_key: &String, recipient_public_key: &String, amount: &String) {

        // Create a new Tokio runtime 
        let rt = tokio::runtime::Runtime::new().unwrap();

        // block_on the account creation process, display the results   
        match rt.block_on(send_transaction_request(
            sender_public_key.to_string(), sender_private_key.to_string(), recipient_public_key.to_string(), amount.to_string())
        ) { 
            Ok(result) => { println!("Transaction request sent successfully"); },
            Err(e) => { eprintln!("Account creation failed: {}", e); return; },
        };       
}   

/**
 * @notice send_account_creation_msg() asynchonously creates and packages a new keypair. Then sends
 * uses tohe send_network_msg() func to distribute it to other nodes in the network.
 * @return a tuple of the secret and public key generated for the new account.
 */
async fn send_transaction_request(sender_public_key: String, sender_private_key: String, recipient_public_key: String, amount: String) -> Result<(), io::Error> {
    println!("\nSending transaction request to network...");

    // Convert the private key to a scalar
    let private_key_bytes: Vec<u8> = hex::decode(sender_private_key).expect("Decoding failed");
    let private_key_scalar: Scalar = Scalar::from_bits(private_key_bytes.try_into().expect("Invalid length"));

    // Generate a random scalar to split the private key into two parts with
    let mut rng = OsRng;
    let mut random_bytes: [u8; 32] = [0u8; 32]; // Array to hold 32 bytes of random data (same length as max scalar value)
    rng.fill_bytes(&mut random_bytes);
    let random_scalar: Scalar = Scalar::from_bits(random_bytes);

    // 'Split' the scalar into two parts
    let scalar_part1: Scalar = private_key_scalar - random_scalar;
    let scalar_part2: Scalar = random_scalar;

    // Convert to Ristretto points (elliptic curve points)
    let point1: RistrettoPoint = RISTRETTO_BASEPOINT_POINT * scalar_part1;
    let point2: RistrettoPoint = RISTRETTO_BASEPOINT_POINT * scalar_part2;

    // Base64 encode (compress) the points to send over the network (as strings
    let encoded_point1: String = encode(point1.compress().to_bytes());
    let encoded_point2: String = encode(point2.compress().to_bytes());

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




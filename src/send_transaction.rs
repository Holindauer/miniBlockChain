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

use rand::rngs::OsRng; // cryptographically secure RNG
use rand::RngCore;

use std::io;
use std::convert::TryInto;


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
 
/**
 * @notice send_transaction() is a wrapper called within main.rs that instigates the process of accessing
 * the network from the client side for sending a transaction request. 
 * @dev the provided keys are hexadecimal strings and the amount is string integer
 */
pub fn send_transaction(
    sender_public_key: &String, sender_private_key: &String, recipient_public_key: &String, amount: &String) { // TODO derive pub key from private key

        // Create a new Tokio runtime 
        let rt = tokio::runtime::Runtime::new().unwrap();

        // block_on the async account creation process, display the results   
        match rt.block_on(send_transaction_request(
            sender_public_key.to_string(), 
            sender_private_key.to_string(), 
            recipient_public_key.to_string(), 
            amount.to_string())
        ) 
        {Ok(_) => { println!("Transaction request sent successfully"); },
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

    println!("\nSending transaction request to network...");

    // Convert the private key to two RistrettoPoints (elliptic curve points)
    let (point1, point2) = private_key_to_curve_points(&sender_private_key);

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


/**
 * @notice private_key_to_curve_points() accepts a private key hexadecimal string. The key is converted to a curve25519_dalek::scalar::Scalar 
 * structure for use with curve25519_dalek eliptic curve operations. A random number generator is used to split the private key into two parts. 
 * Each part is then multiplied by the generator point of the curve25519 elliptic curve over a finite field and returned as a tuple of two 
 * RistrettoPoint structures.
*/
pub fn private_key_to_curve_points(private_key: &String) -> (RistrettoPoint, RistrettoPoint) {

    // Convert the private key to a scalar
    let private_key_bytes: Vec<u8> = hex::decode(private_key).expect("Decoding failed");
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

    (point1, point2)
}



// Funcs needed for testing
use crate::account_creation::generate_keypair;

// Tests
#[cfg(test)]
mod tests {
    use super::*;


    /**
     * @test this this verifies that calling the private_key_to_curve_points() function with a private key string generated 
     * in the same way that the client would generate a private key for a new account, returns two elliptic curve points that 
     * sum to the original private key when represented as an elliptic curve point (via scalar multiplication).
     */
    #[test]
    fn test_private_key_to_curve_points() {
        
        // Generate Keypair and convert to string
        let (secret_key, _) = generate_keypair().unwrap(); //  (secp256k1::SecretKey type)
        let secret_key_hex: String = secret_key.to_string();

        // Convert the secret key to two elliptic curve points using the function ebing tested
        let (point1, point2) = private_key_to_curve_points(&secret_key_hex);

        // Add the two points together
        let curve_point_sum: RistrettoPoint = point1 + point2;

        // Applly the generator point to the secret original key
        let original_key_scalar: Scalar = Scalar::from_bits(hex::decode(secret_key_hex).unwrap().try_into().unwrap());
        let original_key_curve_point = RISTRETTO_BASEPOINT_POINT * original_key_scalar;

        // Check that the sum of the two points is the same as the original key
        assert_eq!(curve_point_sum.compress(), original_key_curve_point.compress());


    }
}
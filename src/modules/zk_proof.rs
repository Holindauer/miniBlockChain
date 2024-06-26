
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT; // generator point
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use secp256k1::{SecretKey, PublicKey, Secp256k1};
use sha2::{Digest, Sha256};
use base64::decode;
use std::convert::TryInto;
use std::collections::HashMap;
use std::io;
use rand::rngs::OsRng; // cryptographically secure RNG
use rand::{thread_rng, RngCore};
use tokio::sync::Mutex;
use std::sync::Arc;

use crate::modules::constants::INTEGRATION_TEST;
use crate::modules::validation::ValidatorNode; // Ensure thread_rng is imported here

/**
 * @notice zk_proof.rs contains the logic for generating a simple zero-knowledge proof for verification of knowledge of 
 * the private key of an account for transaction requests. 
 * 
 * Protocol: 
 *    When an account is created, a unique identifier for the account is created by multiplying the private key of the account
 *    with the generator point of an elliptic curve (curve25519) over a finite field. This curve point is then hashed with sha256
 *    and stored in the account information within the merkle tree. 
 * 
 *    When a transaction is requested, the sender client will accept the private key of the account. This private key is then 
*     split into a sum of two scalars at a random split point. These scalars are each multiplied with the generator. They are 
 *    sent to the network, to which validators will verify that the hash of the two points added together is the same as the 
 *    one in the merkle tree for the account in question.
 * 
 *    All previous two point pairs will also be tracked in a hash map to ensure that the same proof is not used twice. This will 
 *    prevent a malicious actor from gaining knowledge of a proof that has already been used and taking advantage of the fact that
 *    it will add/hash to the value stored in the merkle tree. 
 */

/**
 * @notice obscure_private_key() accepts a secret key and uses the curve25519_dalek library to perform scalar multiplication with the 
 * generator point of the curve25519 elliptic curve. The result is returned as a curve25519_dalek::ristretto::RistrettoPoint. 
 * @dev this function is called within account_creation.rs to generate the obfuscated private key that will be stored in the merkle tree.
*/
pub fn obfuscate_private_key(secret_key: SecretKey) -> RistrettoPoint {
    println!("zk_proof::obfuscate_private_key() : Converting private key to single elliptic curve point..."); 

    // Convert the secret key to a hex encoded string 
    let secret_key_hex_str: String = secret_key.to_string();

    // Convert the secret key to a Scalar struct (curve25519_dalek scalar type)
    let original_key_scalar: Scalar = Scalar::from_bits(
        hex::decode(secret_key_hex_str).unwrap().try_into().unwrap()
    );

    // Scalar multiply the secret key by the generator point
    let original_key_curve_point: RistrettoPoint = RISTRETTO_BASEPOINT_POINT * original_key_scalar;

    original_key_curve_point
}


/**
 * @notice private_key_to_curve_points() accepts a private key hexadecimal string. The key is converted to a curve25519_dalek::scalar::Scalar 
 * structure for use with curve25519_dalek eliptic curve operations. A random number generator is used to split the private key into two parts. 
 * Each part is then multiplied by the generator point of the curve25519 elliptic curve over a finite field and returned as a tuple of two 
 * RistrettoPoint structures.
 * @dev this function is used within send_transaction.rs to generate the two points that will be sent to the network for verification of
 * knowledge of the private key.
*/
pub fn private_key_to_curve_points(private_key: &String) -> (RistrettoPoint, RistrettoPoint) {
    println!("zk_proof::private_key_to_curve_points() : Converting private key to the sum of two curve points..."); 

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


/**
 * @notice hash_obscured_private_key() accepts a RistrattoPoint and returns the hash of the point using sha256
*/
pub fn hash_obfuscated_private_key(obscured_private_key: RistrettoPoint) -> Vec<u8> {
    println!("zk_proof::hash_obfuscated_private_key() : Hashing curve point representation of private key..."); 

    let obscured_private_key_bytes: [u8; 32] = obscured_private_key.compress().to_bytes();
    Sha256::digest(&obscured_private_key_bytes).to_vec()
}


/**
 * @notice decompress_curve_points() accepts two Base64 encoded points and returns the decompressed Ristretto points
 */
fn decompress_curve_points( encoded_key_curve_point_1: &str, encoded_key_curve_point_2: &str,) -> Result<(RistrettoPoint, RistrettoPoint), &'static str> {
    println!("zk_proof::decompress_curve_points() : Decompressing Base64 encoded curve points to Ristretto..."); 

    // convert encoded points (str) to bytes
    let point1_bytes: Vec<u8> = decode(encoded_key_curve_point_1).map_err(|_| "Failed to decode point 1 from Base64")?;
    let point2_bytes: Vec<u8> = decode(encoded_key_curve_point_2).map_err(|_| "Failed to decode point 2 from Base64")?;

    // Decompress the points from bytes
    let point1: RistrettoPoint = CompressedRistretto(point1_bytes.try_into().expect("Invalid length for point 1"))
        .decompress()
        .ok_or("Failed to decompress point 1")?;
    let point2: RistrettoPoint = CompressedRistretto(point2_bytes.try_into().expect("Invalid length for point 2"))
        .decompress()
        .ok_or("Failed to decompress point 2")?;

    Ok((point1, point2))
}


/** 
 * @notice verify_points_sum_hash() accepts two encoded points and a hash. It decompresses the points, adds them together,
 * compresses the sum, hashes it, and compares the hash to the expected hash. If they match, the function returns true.
 * If they do not match, the function returns false.
 */
pub async fn verify_points_sum_hash(
    encoded_key_curve_point_1: &str, 
    encoded_key_curve_point_2: &str, 
    expected_hash: Vec<u8>, 
    sender_address: Vec<u8>,
    validator_node: ValidatorNode
) -> bool {
    println!("zk_proof::verify_points_sum_hash() : Verifying hash of curve point sum adds to private key obfuscated curve point hash..."); 

    // Retrieve the zk_proofs that have already been used
    let used_zk_proofs: Arc<Mutex<HashMap<Vec<u8>, Vec<String>>>> = validator_node.used_zk_proofs.clone();   
    let mut used_zk_proofs_guard = used_zk_proofs.lock().await;

    // hash the proof as it was recieved
    let hash_of_proof: String = hash_zk_proof(encoded_key_curve_point_1, encoded_key_curve_point_2);

    // Reject the proof if it has already been used
    if let Some(used_proofs) = used_zk_proofs_guard.get(&sender_address) {
        if used_proofs.contains(&hash_of_proof) {
            
            println!("Proof has already been used, rejecting...");

            // save a json file called "proof_rejected.json" with a single 1 inside for integration testing
            if INTEGRATION_TEST { 
                let proof_rejected = serde_json::to_string(&1).unwrap();
                std::fs::write("proof_rejected.json", proof_rejected).expect("Unable to write file");
            }

            return false;
        }
    }

    // convert encoded points (str) to Ristretto points
    let (point1, point2) = decompress_curve_points(
        encoded_key_curve_point_1, 
        encoded_key_curve_point_2
    ).unwrap();

    // Add the two Ristretto points together
    let sum_point: RistrettoPoint = point1 + point2;
    
    // Compress the sum to bytes and hash it using SHA-256
    let sum_point_bytes: [u8; 32] = sum_point.compress().to_bytes();
    let hash_of_sum = Sha256::digest(&sum_point_bytes);
    
    // The generated hash with the expected hash
    if hash_of_sum.as_slice() == expected_hash.as_slice() {

        // add the hash of the proof to the used zk_proofs hash map
        if let Some(proofs) = used_zk_proofs_guard.get_mut(&sender_address) {
            proofs.push(hash_of_proof);
        } else {
            used_zk_proofs_guard.insert(sender_address, vec![hash_of_proof]);
        }
        
        // indicate valid knowledge of private key
        return true;
    }   

    // indicate invalid knowledge of private key
    false
}

/**
 * @notice this function hashes the encoded string representation of the two curve points that make 
 * up the zk proof provided by the requester. The points are hashed using sha256 and returned.
 */
fn hash_zk_proof(encoded_key_curve_point_1: &str, encoded_key_curve_point_2: &str) -> String {

    // Decode the Base64 encoded points
    let point1_bytes: Vec<u8> = decode(encoded_key_curve_point_1).unwrap();
    let point2_bytes: Vec<u8> = decode(encoded_key_curve_point_2).unwrap();

    // Hash the two points
    let mut hasher = Sha256::new();
    hasher.update(point1_bytes);
    hasher.update(point2_bytes);
    let hash = hasher.finalize();

    // Return the hash as a hex string
    hex::encode(hash)
}


/**
 * @notice generate_keypair() uses the sepc256k1 eliptic curve to randomly generate a new private/public keypair.
 * @return a tuple of the secret and public key generated for the new account.
 */
pub fn generate_keypair() -> Result<(SecretKey, PublicKey), io::Error> {

    // Create a new secp256k1 context
    let secp = Secp256k1::new();

    // Generate a new cryptographically random number generator  
    let mut rng = thread_rng();

    // Generate a new secret key
    let mut secret_key_bytes = [0u8; 32]; // arr of 32 bytes
    rng.fill_bytes(&mut secret_key_bytes);    // fill w/ random bytes
    
    // encapsulate the secret key bytes into a SecretKey type for safer handling
    let secret_key: SecretKey = SecretKey::from_slice(&secret_key_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string())).unwrap(); // map error to io::Error from secp256k1::Error

    // Derive the public key from the secret key
    let public_key: PublicKey = PublicKey::from_secret_key(&secp, &secret_key);

    Ok((secret_key, public_key))
}


/**
 * @notice derive_public_key_from_private_key() accepts a private key as a hex encoded string and returns the public key
 * derived from the private key as a hex encoded string.
 */
pub fn derive_public_key_from_private_key( private_key: &String ) -> String {

    // Create a new secp256k1 context
    let secp = Secp256k1::new();
 
    // Convert the private key to a SecretKey struct and derive the public key from it
    let secret_key = SecretKey::from_slice(&hex::decode(private_key).unwrap()).unwrap();
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Serialize the public key and return it as a hex string
    hex::encode(public_key.serialize())
}



#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek::scalar::Scalar;

    /**
     * @test test_obfuscate_private_key() verifies that the obfuscate_private_key() function returns a RistrettoPoint that is the
     * result of scalar multiplication of the private key with the generator point of the curve25519 elliptic curve.
     */
    #[test]
    fn test_obfuscate_private_key() {
        
        // Generate Keypair and convert to string
        let (secret_key, _) = generate_keypair().unwrap(); //  (secp256k1::SecretKey type)
        let secret_key_hex_str: String = secret_key.to_string();

        // Convert the secret key to a scalar
        let original_key_scalar: Scalar = Scalar::from_bits(
            hex::decode(secret_key_hex_str).unwrap().try_into().unwrap()
        );

        // Apply the generator point to the secret key
        let original_key_curve_point: RistrettoPoint = RISTRETTO_BASEPOINT_POINT * original_key_scalar;

        // Call the function being tested
        let obfuscated_key_curve_point: RistrettoPoint = obfuscate_private_key(secret_key);

        // Check that the obfuscated key is the same as the original key
        assert_eq!(obfuscated_key_curve_point.compress(), original_key_curve_point.compress());
    }


    /**
     * @test test_private_key_to_curve_points() verifies that calling the private_key_to_curve_points() function with a private key 
     * string generated in the same way that the client would generate a private key for a new account, returns two elliptic curve 
     * points that sum to the original private key when represented as an elliptic curve point (via scalar multiplication).
     */
    #[test]
    fn test_private_key_to_curve_points() {
        
        // Generate Keypair and convert to string
        let (secret_key, _) = generate_keypair().unwrap(); //  (secp256k1::SecretKey type)
        let secret_key_hex_str: String = secret_key.to_string();

        // Convert the secret key to two elliptic curve points using the function ebing tested
        let (point1, point2) = private_key_to_curve_points(&secret_key_hex_str);

        // Add the two points together
        let curve_point_sum: RistrettoPoint = point1 + point2;

        // Applly the generator point to the secret original key
        let original_key_scalar: Scalar = Scalar::from_bits(hex::decode(secret_key_hex_str).unwrap().try_into().unwrap());
        let original_key_curve_point = RISTRETTO_BASEPOINT_POINT * original_key_scalar;

        // Check that the sum of the two points is the same as the original key
        assert_eq!(curve_point_sum.compress(), original_key_curve_point.compress());
    }

}


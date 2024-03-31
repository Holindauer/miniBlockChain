
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT; // generator point
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use secp256k1::{Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use base64::decode;
use std::convert::TryInto;

/**
 * @notice zk_proof.rs contains the logic for generating a simple zero-knowledge proof for verification of knowledge of 
 * the private key of an account for transaction requests. 
 * 
 * Protocol: 
 *  When an account is created, a unique identifier for the account is created by multiplying the private key of the account
 *  with the generator point of an elliptic curve (curve25519) over a finite field. This curve point is then hashed with sha256
 *  and stored in the account information within the merkle tree. 
 * 
 *  When a transaction is requested, the sender client will accept the private key of the account. This private key is then 
 *  split into a sum of two scalars at a random split point. These scalars are each multiplied with the generator. They are 
 *  sent to the network, to which validators will verify that the hash of the two points added together is the same as the 
 *  one in the merkle tree for the account in question.
 * 
 *  All previous two point pairs will also be tracked in a hash map to ensure that the same proof is not used twice. This will 
 *  prevent a malicious actor from gaining knowledge of a proof that has already been used and taking advantage of the fact that
 *  it will add/hash to the value stored in the merkle tree. 
 */

/**
 * @notice obscure_private_key() accepts a private key and uses the curve25519_dalek library is used to perform scalar 
 * multiplication with the generator point of an elliptic curve (curve25519) over a finite field, returning the result
*/
pub fn obfuscate_private_key(private_key: SecretKey) -> RistrettoPoint {
    RISTRETTO_BASEPOINT_POINT * Scalar::from_bytes_mod_order(*private_key.as_ref())
}

/**
 * @notice hash_obscured_private_key() accepts a RistrattoPoint and returns the hash of the point using sha256
*/
pub fn hash_obfuscated_private_key(obscured_private_key: RistrettoPoint) -> Vec<u8> {
    let obscured_private_key_bytes = obscured_private_key.compress().to_bytes();
    Sha256::digest(&obscured_private_key_bytes).to_vec()
}


/**
 * @notice decompress_curve_points() accepts two Base64 encoded points and returns the decompressed Ristretto points
 */
fn decompress_curve_points( encoded_point1: &str, encoded_point2: &str,) -> Result<(RistrettoPoint, RistrettoPoint), &'static str> {

    // convert encoded points (str) to bytes
    let point1_bytes: Vec<u8> = decode(encoded_point1).map_err(|_| "Failed to decode point 1 from Base64")?;
    let point2_bytes: Vec<u8> = decode(encoded_point2).map_err(|_| "Failed to decode point 2 from Base64")?;

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
pub fn verify_points_sum_hash(encoded_point1: &str, encoded_point2: &str, expected_hash: Vec<u8>) -> bool {
    println!("Validating Knowledge of Private Key...");

    // convert encoded points (str) to Ristretto points
    let (point1, point2) = decompress_curve_points(encoded_point1, encoded_point2).unwrap();

    // Add the two Ristretto points together
    let sum_point: RistrettoPoint = point1 + point2;
    
    // Compress the sum to bytes and hash it using SHA-256
    let sum_point_bytes: [u8; 32] = sum_point.compress().to_bytes();
    let hash_of_sum = Sha256::digest(&sum_point_bytes);
    
    // Compare the generated hash with the expected hash
    hash_of_sum.as_slice() == expected_hash.as_slice()
}


// test of obscure_private_key
#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek::scalar::Scalar;
    use secp256k1::Secp256k1;

    #[test]
    fn test_verify_points_sum_hash() {

         // Initialize the RNG and generate two scalars
         let scalar1: Scalar = Scalar::from(1234567890u64);
         let scalar2: Scalar = Scalar::from(987654321u64);
 
         // Generate two points by multiplying the base point by the scalars
         let point1: RistrettoPoint = RISTRETTO_BASEPOINT_POINT * scalar1;
         let point2: RistrettoPoint = RISTRETTO_BASEPOINT_POINT * scalar2;
 
         // Encode points to Base64
         let encoded_point1: String = base64::encode(point1.compress().to_bytes());
         let encoded_point2: String = base64::encode(point2.compress().to_bytes());
 
         // Sum the points and compute SHA-256 hash of the result
         let sum_point: RistrettoPoint = point1 + point2;
         let sum_point_bytes: [u8; 32] = sum_point.compress().to_bytes();
         let expected_hash = Sha256::digest(&sum_point_bytes);
 
         // Convert the SHA-256 hash to a Vec<u8> to match the function signature
         let expected_hash_vec: Vec<u8> = expected_hash.to_vec();
 
         // Run the verification function with the encoded points and the expected hash
         assert!(verify_points_sum_hash(&encoded_point1, &encoded_point2, expected_hash_vec),
                 "The verification of the points sum hash should succeed.");
     }
}


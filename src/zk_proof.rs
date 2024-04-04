
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT; // generator point
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use secp256k1::{Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use base64::decode;
use std::convert::TryInto;
use rand::rngs::OsRng; // cryptographically secure RNG
use rand::RngCore;

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
    use crate::account_creation::generate_keypair;

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


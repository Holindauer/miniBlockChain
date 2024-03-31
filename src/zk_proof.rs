
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT; // generator point
use curve25519_dalek::ristretto::RistrettoPoint;
use secp256k1::{Secp256k1, SecretKey};
use sha2::{Digest, Sha256};


pub use crate::account_creation::generate_keypair;


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





// test of obscure_private_key
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obscure_private_key() {
        // let private_key_hex

    }

}
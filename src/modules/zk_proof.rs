use secp256k1::{SecretKey, PublicKey, Secp256k1, Message, Signature};
use sha2::{Digest, Sha256};
use std::io;
use rand::thread_rng;
use rand::RngCore;
use tokio::sync::Mutex;
use std::sync::Arc;

use crate::modules::constants::INTEGRATION_TEST;
use crate::modules::validation::ValidatorNode;

/**
 * @notice zk_proof.rs contains the logic for generating and verifying digital signatures
 * for transaction authentication. Instead of zero-knowledge proofs, we use standard
 * ECDSA signatures with the secp256k1 curve.
 * 
 * Protocol: 
 *    When an account is created, the public key is derived from the private key and stored.
 *    When a transaction is requested, the sender signs the transaction details with their
 *    private key. Validators verify the signature using the sender's public key.
 *    
 *    All previous signatures are tracked to prevent replay attacks.
 */

/**
 * @notice create_transaction_message() creates a message hash from transaction details
 * that will be signed by the sender's private key.
 */
fn create_transaction_message(
    sender_public_key: &str,
    recipient_public_key: &str, 
    amount: &str,
    nonce: u64
) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(sender_public_key.as_bytes());
    hasher.update(recipient_public_key.as_bytes());
    hasher.update(amount.as_bytes());
    hasher.update(nonce.to_le_bytes());
    hasher.finalize().to_vec()
}

/**
 * @notice sign_transaction() accepts transaction details and a private key, 
 * creates a signature for the transaction.
 */
pub fn sign_transaction(
    private_key: &String,
    sender_public_key: &String,
    recipient_public_key: &String,
    amount: &String,
    nonce: u64
) -> Result<String, String> {
    println!("zk_proof::sign_transaction() : Signing transaction with private key..."); 

    // Create secp256k1 context
    let secp = Secp256k1::new();
    
    // Convert private key from hex string to SecretKey
    let secret_key = SecretKey::from_slice(&hex::decode(private_key)
        .map_err(|e| format!("Failed to decode private key: {}", e))?)
        .map_err(|e| format!("Invalid private key: {}", e))?;
    
    // Create message hash from transaction details
    let message_bytes = create_transaction_message(sender_public_key, recipient_public_key, amount, nonce);
    let message = Message::from_slice(&message_bytes)
        .map_err(|e| format!("Failed to create message: {}", e))?;
    
    // Sign the message
    let signature = secp.sign(&message, &secret_key);
    
    // Return signature as hex string
    Ok(hex::encode(signature.serialize_compact()))
}

/**
 * @notice verify_transaction_signature() verifies that a signature is valid for the given
 * transaction details and sender's public key.
 */
pub async fn verify_transaction_signature(
    signature_hex: &str,
    sender_public_key_hex: &str,
    recipient_public_key: &str,
    amount: &str,
    nonce: u64,
    validator_node: ValidatorNode
) -> bool {
    println!("zk_proof::verify_transaction_signature() : Verifying transaction signature..."); 

    // Create secp256k1 context
    let secp = Secp256k1::new();
    
    // Parse signature from hex
    let signature_bytes = match hex::decode(signature_hex) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    let signature = match Signature::from_compact(&signature_bytes) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    
    // Parse public key from hex
    let public_key_bytes = match hex::decode(sender_public_key_hex) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    
    let public_key = match PublicKey::from_slice(&public_key_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    
    // Create message hash from transaction details
    let message_bytes = create_transaction_message(sender_public_key_hex, recipient_public_key, amount, nonce);
    let message = match Message::from_slice(&message_bytes) {
        Ok(msg) => msg,
        Err(_) => return false,
    };
    
    // Verify signature
    if secp.verify(&message, &signature, &public_key).is_err() {
        return false;
    }
    
    // Check if signature has been used before (replay attack prevention)
    let used_signatures = validator_node.used_zk_proofs.clone();
    let mut used_signatures_guard = used_signatures.lock().await;
    
    let sender_address = sender_public_key_hex.as_bytes().to_vec();
    
    // Check if this signature has been used before
    if let Some(used_sigs) = used_signatures_guard.get(&sender_address) {
        if used_sigs.contains(&signature_hex.to_string()) {
            println!("Signature has already been used, rejecting...");
            
            // Save rejection indicator for integration testing
            if INTEGRATION_TEST { 
                let proof_rejected = serde_json::to_string(&1).unwrap();
                std::fs::write("proof_rejected.json", proof_rejected).expect("Unable to write file");
            }
            
            return false;
        }
    }
    
    // Add signature to used signatures
    if let Some(sigs) = used_signatures_guard.get_mut(&sender_address) {
        sigs.push(signature_hex.to_string());
    } else {
        used_signatures_guard.insert(sender_address, vec![signature_hex.to_string()]);
    }
    
    true
}

/**
 * @notice generate_keypair() uses the secp256k1 elliptic curve to randomly generate a new private/public keypair.
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
pub fn derive_public_key_from_private_key(private_key: &String) -> String {
    // Create a new secp256k1 context
    let secp = Secp256k1::new();
 
    // Convert the private key to a SecretKey struct and derive the public key from it
    let secret_key = SecretKey::from_slice(&hex::decode(private_key).unwrap()).unwrap();
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Serialize the public key and return it as a hex string
    hex::encode(public_key.serialize())
}

/**
 * @notice get_public_key_hash() returns a hash of the public key for storage in the merkle tree.
 * This replaces the obfuscated private key hash.
 */
pub fn get_public_key_hash(public_key: &PublicKey) -> Vec<u8> {
    println!("zk_proof::get_public_key_hash() : Hashing public key...");
    let public_key_bytes = public_key.serialize();
    Sha256::digest(&public_key_bytes).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    /**
     * @test test_sign_and_verify_transaction() verifies that signing and verifying transactions works correctly
     */
    #[tokio::test]
    async fn test_sign_and_verify_transaction() {
        // Generate keypair
        let (secret_key, public_key) = generate_keypair().unwrap();
        let private_key_hex = secret_key.to_string();
        let public_key_hex = hex::encode(public_key.serialize());
        
        // Transaction details
        let recipient_public_key = "04abcd1234...";
        let amount = "100";
        let nonce = 1u64;
        
        // Sign transaction
        let signature = sign_transaction(
            &private_key_hex,
            &public_key_hex,
            &recipient_public_key.to_string(),
            &amount.to_string(),
            nonce
        ).unwrap();
        
        // Create a mock validator node
        let validator_node = ValidatorNode::new();
        
        // Verify signature
        let is_valid = verify_transaction_signature(
            &signature,
            &public_key_hex,
            recipient_public_key,
            amount,
            nonce,
            validator_node
        ).await;
        
        assert!(is_valid);
    }
    
    /**
     * @test test_invalid_signature_rejection() verifies that invalid signatures are rejected
     */
    #[tokio::test]
    async fn test_invalid_signature_rejection() {
        // Generate two different keypairs
        let (_, public_key1) = generate_keypair().unwrap();
        let (secret_key2, _) = generate_keypair().unwrap();
        
        let public_key_hex = hex::encode(public_key1.serialize());
        let private_key_hex = secret_key2.to_string(); // Wrong private key!
        
        // Transaction details
        let recipient_public_key = "04abcd1234...";
        let amount = "100";
        let nonce = 1u64;
        
        // Sign with wrong private key
        let signature = sign_transaction(
            &private_key_hex,
            &public_key_hex,
            &recipient_public_key.to_string(),
            &amount.to_string(),
            nonce
        ).unwrap();
        
        // Create a mock validator node
        let validator_node = ValidatorNode::new();
        
        // Verify signature (should fail)
        let is_valid = verify_transaction_signature(
            &signature,
            &public_key_hex,
            recipient_public_key,
            amount,
            nonce,
            validator_node
        ).await;
        
        assert!(!is_valid);
    }
}
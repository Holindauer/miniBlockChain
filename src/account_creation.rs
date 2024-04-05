use curve25519_dalek::ristretto::RistrettoPoint;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;
use std::io;

extern crate secp256k1;
extern crate rand;

use secp256k1::{Secp256k1, SecretKey, PublicKey};
use rand::{thread_rng, RngCore}; // Ensure thread_rng is imported here

use crate::zk_proof::{obfuscate_private_key, hash_obfuscated_private_key};
use crate::constants::{VERBOSE, PORT_NUMBER};

/**
 * @notice account_creation.rs contains the logic for sending a request to the network to create a new account.
 * 
 * The following protocol is followed when sending an account creation request:
 * 
 *    First, a private/public key-pair is generated using the secp256k1 elliptic curve over a finite field. The public key
 *    acts as the address of the account, the private key acts as the password. As well, the private key is multiplied by
 *    the generator point of the curve25519 elliptic curve over a finite field and hashed using sha256. The public key and 
 *    this hash are packaged and sent to validators in the network. The user is responsible for storing the private key, 
 *    which will be displayed upon an account creation request and at no other time.
 * 
 *    Validators recieving the request will check that the account does not already exist in the merkle tree. If it does not,
 *    the account will be added to the tree and a new block will be created in the blockchain. The hash of the elliptic curve
 *    representation of the private key will be stored in the block for use in a zk proof of knowledge of the private key when 
 *    sending transaction requests. 
*/

/**
 * @notice AccountCreationRequest encapsulate the details of a request to create a new account on the blockchain
 *        network. This includes the public key of the account, the obfuscated elliptic curve private key hash.
 */
#[derive(Serialize, Deserialize)]
pub struct AccountCreationRequest {
    pub action: String,
    pub public_key: String,
    pub obfuscated_private_key_hash: String,
}

/**
 * @notice account_creation() is a wrapper called within main.rs that instigates the process of accessing 
 * the network from the client side for account creation within the network.
 */
pub fn account_creation() {
    if VERBOSE { println!("account_creation::account_creation() : Sending account creation request...") };

    // Create a new Tokio runtime 
    let rt = tokio::runtime::Runtime::new().unwrap();

    // block_on the account creation process, display the results   
    match rt.block_on(send_account_creation_request()) { 
        Ok(keys) => { // (SecretKey, PublicKey)  // TODO THIS will potentiall need to be another json output for testing, but for now its fine
            if VERBOSE { 
                println!("Account details sucessfully created: ");
                println!("Secret Key: {:?}", keys.0.to_string());
                println!("Public Key: {:?}", keys.1.to_string());
            };
        },
        Err(e) => { eprintln!("Account creation failed: {}", e); return; },
    };
}

/**
 * @notice send_account_creation_msg() asynchonously creates a new private/public keypair, creates the 
 * obfuscated private key hash, and sends the account creation request to the network as a json object.
 */
async fn send_account_creation_request() -> Result<(SecretKey, PublicKey), io::Error> {
    if VERBOSE { println!("account_creation::send_account_creation_request() : Sending account creation request...") };

    // Generate a new keypair
    let (secret_key, public_key) = generate_keypair()?;

    // Obfuscate the private key for zk-proof
    let obscured_private_key: RistrettoPoint = obfuscate_private_key(secret_key);
    let obfuscated_private_key_hash: Vec<u8> = hash_obfuscated_private_key(obscured_private_key);

    // Package account creation request
    let request: AccountCreationRequest = AccountCreationRequest {
        action: "make".to_string(),
        public_key: public_key.to_string(),
        obfuscated_private_key_hash: hex::encode(obfuscated_private_key_hash),
    };

    // Connect to the server at the specified port number
    let mut stream: TcpStream = TcpStream::connect(PORT_NUMBER).await?;

    // Serialize request to JSON, write to stream
    let request_json: String = serde_json::to_string(&request)?;
    stream.write_all(request_json.as_bytes()).await?;

    // Return the generated secret key and public key
    Ok((secret_key, public_key))
}


/**
 * @notice generate_keypair() uses the sepc256k1 eliptic curve to randomly generate a new private/public keypair.
 * @return a tuple of the secret and public key generated for the new account.
 */
pub fn generate_keypair() -> Result<(SecretKey, PublicKey), io::Error> {
    if VERBOSE { println!("account_creation::generate_keypair() : Generating new keypair...") };

    // Create a new secp256k1 context
    let secp = Secp256k1::new();

    // Generate a new cryptographically random number generator  
    let mut rng = thread_rng();

    // Generate a new secret key
    let mut secret_key_bytes = [0u8; 32]; // arr of 32 bytes
    rng.fill_bytes(&mut secret_key_bytes);    // fill w/ random bytes
    
    // encapsulate the secret key bytes into a SecretKey type for safer handling
    let secret_key: SecretKey = SecretKey::from_slice(&secret_key_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?; // map error to io::Error from secp256k1::Error

    // Derive the public key from the secret key
    let public_key: PublicKey = PublicKey::from_secret_key(&secp, &secret_key);

    Ok((secret_key, public_key))
}


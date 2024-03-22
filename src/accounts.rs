use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;

extern crate secp256k1;
extern crate rand;

use secp256k1::{Secp256k1, SecretKey, PublicKey};
use rand::{thread_rng, RngCore}; // Ensure thread_rng is imported here
use secp256k1::Error;

#[derive(Serialize, Deserialize)]
struct NetworkMessage {
    command: String,
    public_key: String,
    private_key: String,
}

async fn send_network_message(addr: &str, message: NetworkMessage) -> tokio::io::Result<()> {

    // Connect to the server
    let mut stream: TcpStream = TcpStream::connect(addr).await?;

    // Serialize the message to JSON
    let message_json: String = serde_json::to_string(&message)?;

    // Write the message to the server
    stream.write_all(message_json.as_bytes()).await?;

    Ok(())
}

async fn send_account_creation_msg() -> tokio::io::Result<()> {

    // Generate a new keypair
    let (secret_key, public_key) = generate_keypair()?;

    // example msg of account creation
    let message: NetworkMessage = NetworkMessage {
        command: "create_account".to_string(),
        public_key: public_key.to_string(),
        private_key: secret_key.to_string(),
    };

    // Sending the message to a specific address or broadcast it
    send_network_message("127.0.0.1:8080", message).await
}

fn generate_keypair() -> Result<(SecretKey, PublicKey), secp256k1::Error> {

    // create instance of secp256k1 
    let secp = Secp256k1::new();

    // Create a new thread_rng (cryptographically secure random number generator)
    let mut rng = thread_rng();

    // Generate a random 256-bit number for the private key
    let mut secret_key_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_key_bytes);

    // Create a SecretKey from the random bytes 
    let secret_key = SecretKey::from_slice(&secret_key_bytes)?;

    // Derive the public key from the secret key
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    Ok((secret_key, public_key))
}

pub fn account_creation() {

    // Create a new Tokio runtime 
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Use the runtime to block on (run an async process in a synchronous context) the 
    // account creation process. Output from send_account creation_msg() is a future
    // that will resolve to a Result type.   
    match rt.block_on(send_account_creation_msg()) { 

        Ok(_) => println!("Account creation process initiated successfully."),
        Err(e) => eprintln!("Account creation failed: {}", e),
    }
}

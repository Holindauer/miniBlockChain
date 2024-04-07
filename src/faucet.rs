use serde::{Serialize, Deserialize};
use serde_json;
use std::io;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt; 

use crate::constants::{PORT_NUMBER, VERBOSE_STACK};

/**
 * @notice faucet.rs contains the logic for sending a network request to validator nodes to 
 *         provide a given public key with a small amount of tokens for transactions.
 */

 /**
  * @notice FaucetRequest encapsulate the details of a request to use the faucet
*/
 #[derive(Serialize, Deserialize)]
 pub struct FaucetRequest {
     pub action: String,
     pub public_key: String,
}
 
/**
 * @notice use_faucet() is a wrapper called within main.rs that instigates the process of accessing 
 * the network from the client side, creating a new account, and saving the account details within ledger
 */
pub fn use_faucet(public_key: &String) {
    if VERBOSE_STACK { println!("faucet::use_faucet() : Establishing network connection..."); }

    // Create a new Tokio runtime 
    let rt = tokio::runtime::Runtime::new().unwrap();

    // block_on the account creation process, display the results   
    match rt.block_on(send_faucet_request(public_key.to_string())) { 
        Ok(_) => { println!("Faucet request sent successfully"); },
        Err(e) => { eprintln!("Faucet request failed: {}", e); return; },
    };       
}   

/**
 * @notice send_faucet_request() sends a request to the network to provide a given public key with a small amount of tokens
 */
async fn send_faucet_request(public_key: String) -> Result<(), io::Error> {
    if VERBOSE_STACK { println!("faucet::send_faucet_request() : Sending faucet request..."); }

    // Package the message for network transmission
    let message: FaucetRequest = FaucetRequest {
        action: "faucet".to_string(),
        public_key: public_key.to_string(),
    };

    // Connect and send the message
    let mut stream: TcpStream = TcpStream::connect(PORT_NUMBER).await?; 
    let message_json: String = serde_json::to_string(&message)?;
    stream.write_all(message_json.as_bytes()).await?;

    Ok(()) 
}




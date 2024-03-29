use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;
use std::io;

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
     pub sender: String,
     pub recipient: String,
     pub amount: String,
 }
 

pub fn send_transaction(sender: &String, recipient: &String, amount: &String) {

        // Create a new Tokio runtime 
        let rt = tokio::runtime::Runtime::new().unwrap();

        // block_on the account creation process, display the results   
        match rt.block_on(send_transaction_request(sender.to_string(), recipient.to_string(), amount.to_string())) { 
            Ok(result) => { println!("Transaction request sent successfully"); },
            Err(e) => { eprintln!("Account creation failed: {}", e); return; },
        };       
}   



/**
 * @notice send_account_creation_msg() asynchonously creates and packages a new keypair. Then sends
 * uses tohe send_network_msg() func to distribute it to other nodes in the network.
 * @return a tuple of the secret and public key generated for the new account.
 */
async fn send_transaction_request(sender: String, recipient: String, amount: String) -> Result<(), io::Error> {
    println!("\nSending transaction request to network...");

    // example msg of account creation
    let message: TransactionRequest = TransactionRequest {
        action: "transaction".to_string(),
        sender: sender.to_string(),
        recipient: recipient.to_string(),
        amount: amount.to_string(),

    };

    // Connect to the server at the specified port number
    let mut stream: TcpStream = TcpStream::connect(PORT_NUMBER).await?;

    // Serialize message to JSON, write to stream
    let message_json: String = serde_json::to_string(&message)?;
    stream.write_all(message_json.as_bytes()).await?;

    Ok(())
}




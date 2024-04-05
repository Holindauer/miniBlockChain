use serde::{Serialize, Deserialize};
use serde_json;
use std::io;

use crate::constants::{PORT_NUMBER, VERBOSE_STACK};

/**
 * @notice fountain.rs contains the logic for sending a network request to validator nodes to 
 *         provide a given public key with a small amount of tokens for transactions.
 */



 /**
  * @notice FountainRequest encapsulate the details of a request to use the fountain
  */
 #[derive(Serialize, Deserialize)]
 pub struct FountainRequest {
     pub action: String,
     pub public_key: String,
}
 
// /**
//  * @notice use_fountain() is a wrapper called within main.rs that instigates the process of accessing 
//  * the network from the client side, creating a new account, and saving the account details within ledger
//  */
// pub fn use_fountain(public_key: &String) {

//         // Create a new Tokio runtime 
//         let rt = tokio::runtime::Runtime::new().unwrap();

//         // block_on the account creation process, display the results   
//         match rt.block_on(send_fountain_request(public_key.to_string())) { 
//             Ok(result) => { println!("Transaction request sent successfully"); },
//             Err(e) => { eprintln!("Account creation failed: {}", e); return; },
//         };       
// }   

// /**
//  * @notice send_account_creation_msg() asynchonously creates and packages a new keypair. Then sends
//  * uses tohe send_network_msg() func to distribute it to other nodes in the network.
//  * @return a tuple of the secret and public key generated for the new account.
//  */
// async fn send_fountain_request(public_key: String) -> Result<(), io::Error> {
//     if VERBOSE { println!("\nSending transaction request to network..."); }

//     // // Package the message
//     // let message: FountainRequest = FountainRequest {
//     //     action: "fountain".to_string(),
//     //     public_key: public_key.to_string(),
//     // };

//     // // Connect and send the message
//     // let mut stream: TcpStream = TcpStream::connect(PORT_NUMBER).await?; 
//     // let message_json: String = serde_json::to_string(&message)?;
//     stream.write_all(message_json.as_bytes()).await?;

//     // Ok(())
// }




use serde::{Serialize, Deserialize};
use serde_json;
use std::{io, fs};
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt; 

use crate::constants::VERBOSE_STACK;
use crate::network::NetworkConfig;

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
 * @notice send_faucet_request() sends a request to the network to provide a given public key with a small amount of tokens
 */
pub async fn send_faucet_request(public_key: String)  {
    if VERBOSE_STACK { println!("faucet::send_faucet_request() : Sending faucet request..."); }

    // Package the message for network transmission
    let request: FaucetRequest = FaucetRequest {
        action: "faucet".to_string(),
        public_key: public_key.to_string(),
    };
    let request_json: String = serde_json::to_string(&request).unwrap();


    // Load accepted ports configuration
    let config_data: String = fs::read_to_string("accepted_ports.json").map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
    let config: NetworkConfig = serde_json::from_str(&config_data).map_err(|e| io::Error::new(io::ErrorKind::Other, e)).unwrap();
      
    // Send account creation request to all accepted po
    for node in config.nodes.iter() {
        let addr = format!("{}:{}", node.address, node.port);
        if let Ok(mut stream) = TcpStream::connect(&addr).await {
            let _ = stream.write_all(request_json.as_bytes()).await;
        }
    }
}




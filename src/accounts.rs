use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::net::TcpStream;
use serde::{Serialize, Deserialize};
use serde_json;


#[derive(Serialize, Deserialize)]
struct NetworkMessage {
    command: String,
    data: String, // Consider more specific types or structures as needed
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

    // example msg of account creation
    let message: NetworkMessage = NetworkMessage {
        command: "create_account".to_string(),
        data: "Sample data for account creation".to_string(), // Sample data
    };

    // Sending the message to a specific address or broadcast it
    send_network_message("127.0.0.1:8080", message).await
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

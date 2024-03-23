use tokio::net::TcpListener;
use tokio::sync::Mutex;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};


// validation.rs

const PORT_NUMBER: &str = "127.0.0.1:8080";

/**
 * 
 * 
 */
pub async fn listen_for_connections(address: &str, connection_counter: Arc<Mutex<u32>>) -> tokio::io::Result<()> {

    // Create a new TCP listener bound to the specified address
    let listener = TcpListener::bind(address).await?;
    println!("Validation server listening on {}", address);

    // Loop to accept incoming connections
    loop {
        // create a new socket (stream) for each incoming connection
        let (mut socket, _) = listener.accept().await?;

        // Clone the connection counter for use in the spawned task
        let counter_clone = connection_counter.clone();
        
        // Spawn a new asynchronous task to handle the incoming connection
        tokio::spawn(async move {
            
            // create byte buffer for incoming data
            let mut buf: [u8; 1024] = [0; 1024];

            // Lock mutex the connection counter when a new connection is established
            let mut connection_counter = counter_clone.lock().await;
            *connection_counter += 1;

            // Handle Incoming Msg
            match socket.read(&mut buf).await {
                Ok(_) => {
                    // Handle the message
                
                },
                Err(e) => eprintln!("Failed to read from socket: {}", e),
            }

            // Decrement the counter when a connection is closed
            *connection_counter -= 1;
        });
    }
}

/**
 * @notice run_validation() is a wrapper called within main.rs that instigates the process of accessing
 * the network from the client side for running the validation process. 
 * 
 */
pub fn run_validation(private_key: &String) {
    // TODO - integrate private_key into validation process for reward distribution/slashing


    // Create a new Tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();

    // connection_counter is used inside listen_for_connections to track the number of active connections
    // for the purpose of determining majority consensus across nodes. It is an Arc(atomic reference counter) 
    // wrapped in a Mutex to ensure thread safety, and maintained on the client side.
    let connection_counter: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));


    // Use block_on to start a new asynchronous task that listens for incoming connections
    rt.block_on(async {

        // port 8080 is used for the server
        let addr: &str = PORT_NUMBER; 

        // Start the server and listen for incoming connections
        match listen_for_connections(addr, connection_counter.clone()).await {
            Ok(_) => println!("Validation listener terminated."),
            Err(e) => eprintln!("Validation listener encountered an error: {}", e),
        }
    });
}

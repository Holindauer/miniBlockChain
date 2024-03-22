// server.rs
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// this function will start the server and listen for incoming connections
// it will read the incoming data and print it to the console
// this is temporary for dev and responsibility for running the server wil be moved to validation.rs
pub async fn start_server() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server is listening on port 8080");

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            match socket.read(&mut buf).await {
                Ok(_) => {
                    println!("Received: {}", String::from_utf8_lossy(&buf));
                    if let Err(e) = socket.write_all(&buf).await {
                        eprintln!("Failed to write to socket: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read from socket: {}", e);
                }
            }
        });
    }
}

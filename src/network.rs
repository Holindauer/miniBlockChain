use std::env;
use std::net::{TcpListener, TcpStream};
use std::io::Read;
use std::thread;

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    match stream.read(&mut buffer) {
        Ok(_) => {
            println!("Received: {}", String::from_utf8_lossy(&buffer));
        },
        Err(e) => println!("Failed to receive data: {}", e),
    }
}


fn main() -> std::io::Result<()> {

    // read CLI args into vector
    let args: Vec<String> = env::args().collect();


    // create account specified  
    if args.len() > 1 && args[1] == "make" {

        // create new account and write into the blockchain
        create_account();

        println!("New address created: {}", new_address);

        return Ok(());

    } // Transaction specified
    else if args.len() == 4 {

        let private_key = &args[1];
        let transaction_amount = &args[2];
        let recipient = &args[3];

        println!("Private Key: {}, Recipient: {}, Transaction Amount: {}", private_key, recipient, transaction_amount);
    }
    else {// improper usage
        println!("ERROR!\nUsage: program [make] or [private_key] [recipient] [transaction_amount]");
        return Ok(());
    }
    



    // At some point here: Implement either the account creation or transaction protocol
    // into the blockchain network, assuming it is running.

    // Create a new TcpListener and start listening for incoming connections
    let listener = TcpListener::bind("0.0.0.0:7878")?;
    println!("Server listening on port 7878");

    // For each incoming connection, spawn a new thread  
    for stream in listener.incoming() {

        match stream {
            Ok(stream) => {

                println!("New connection: {}", stream.peer_addr().unwrap());
                thread::spawn(move || handle_client(stream));
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
    Ok(())
}

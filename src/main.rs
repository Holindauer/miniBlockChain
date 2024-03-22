use std::env;
use std::net::{TcpListener, TcpStream};
use std::io::Read;
use std::thread;

mod accounts;
mod block;
mod transactions;
mod validation;

/**
 * @notice main.rs runs a blockchain node. There are three options when connecting a node from the CLI: 
 *
 * Acount Creation:
 *  
 * 1.) The first is to make an account by providing the [make] argument. This will trigger a validation 
 *     event across all users running validation nodes. A new public and private key will be generated
 *     using elliptic curve cryptography (secp256k1). Validation by the network minimal in this case. 
 *     Just checking that a prexisting account is not being overwritten (extremely unlikely since using 
 *     secp256k1). The new accoount will aslo be integrated into the merkel tree that contains all 
 *     (private-key, account-balance) pairs of all accounts created up to this point. Accounts not creates 
 *     yet are implicitly at a balance of 0 and "become accounts" that can transactions once created using 
 *     [make] from CLI or because ownership has been transfered to an account that has not been created 
 *     yet, which will trigger its creation (not possible to create private key given discrete log, but 
 *     account ablance could potentially be stored...)
 * 
 *     This is done in the accounts.rs modulue. 
 * 
 *     As well, a block will be added to the chain, indicating that a new account was created with current
 *     balance zero. This is done in block.rs.
 * 
 * Transaction: 
 * 
 * 2.) The second option is to send a transaction by providing the following arguments to the CLI:
 * 
 *     [private key] [recipiant public key] [transaction amount]
 * 
 *     This will trigger a validation event to all users currently running nodes that a new transaction 
 *     is waiting to be validated. Validation will involves searching the merkel tree to find the private 
 *     key's account balance, and checking if there is sufficient balance for sending the amount specified. 
 *     
 *     Assuming validation is successful, the balance of the sender will be decreased by the transaction
 *     amount, vice versa for the recipiant. A new block will be added to the chain reflecting this change.
 *  
 * Validation:
 * 
 * 3.) Validation will be a proof of stake system. Meaning that in order to participate, you must provide 
 *     an amount of tokens to participate as a validator. Validators will listen to all incoming events 
 *     entering the network and apply vallidation for either account creation or transactions. This will 
 *     be minimal until the above two protocols are in place. 
 */



// main.rs

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

    // Account Creation Specified  
    if args.len() == 2 && args[1] == "make" { 

        // validate account creation
        accounts::account_creation();
        return Ok(());

    } // Transaction Specified
    else if args.len() == 4 { 

        // extract provided arguments:
        let private_key = &args[1];
        let recipient = &args[2];
        let transaction_amount = &args[3];

        println!("Client Side Arguments");
        println!("Private Key: {}", private_key);
        println!("Recipient: {}", recipient);
        println!("Transaction Amount: {}", transaction_amount);
        
        // validate transaction 
        transactions::send_transaction(private_key, recipient, transaction_amount);

    }// Validation Specified 
    else if args.len() == 3 && args[1] == "validate" { 

        let private_key = &args[2];

        validation::run_validation(private_key);
    } 
    else { // Improper Command
        println!("ERROR! Unrecognized Command");
        return Ok(());
    }
    

    // // ! At some point here: Split this logic up into seperate functions 
    // // ! for either making transactions, account creation, or validation

    // // Create a new TcpListener and start listening for incoming connections
    // let listener = TcpListener::bind("0.0.0.0:7878")?;
    // println!("Server listening on port 7878");

    // // For each incoming connection, spawn a new thread  
    // for stream in listener.incoming() {

    //     match stream {
    //         Ok(stream) => {

    //             println!("New connection: {}", stream.peer_addr().unwrap());
    //             thread::spawn(move || handle_client(stream));
    //         }
    //         Err(e) => {
    //             println!("Error: {}", e);
    //         }
    //     }
    // }


    Ok(())
}

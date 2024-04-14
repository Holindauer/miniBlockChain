mod account_creation;
mod blockchain;
mod send_transaction;
mod validation;
mod merkle_tree;
mod zk_proof;
mod constants;
mod faucet;
mod chain_consensus;
mod block_consensus;
mod network;

use std::env;

use constants::VERBOSE_STACK;
/**
 * @notice main.rs runs a blockchain node which connects to a TCP server in order to write to the blockchain. 
 *         There are three options when connecting a node from the CLI: Account Creaction, Trancation, Validation
 *
 * 1.) Acount Creation:
 *  
 *     An account can be made by running the following command: 
 *  
 *         cargo run make
 *  
 *     On the client side, a new private/public key pair will be generated using the secp256k1 elliptic curve
 *     over a finite field. The client will send a network request to all validators running nodes that a new 
 *     account creation is being requested. Validators will check that the account does not already exist in the
 *     merkel tree, and if not, it will be added and a new block will be created in the blockchain. Otherwise, 
 *     the account creation will be rejected. Additionally, in each block will be stored the hash of an elliptic 
 *     curve representation of the new user's private key will be stored for a zk proofs of knowledge verification 
 *     of the user's account balance when sending transactions.
 * 
 * 2.) Transaction: 
 * 
 *     To send a transaction provide the following arguments to the CLI:
 * 
 *     cargo run transaction [sender private key] [recipiant public key] [transaction amount]
 * 
 *     On the client side, the private key will be converted into an obfuscated representation as multiple ellitpic 
 *     curve points that sum to the elliptic curve representaiton of the original privte key (scalar multiplication). 
 *     This and the other provided details of the transaction (including the derived public key) will be packaged and 
 *     sent as a transaction request to all validators running nodes.
 * 
 *     Validators will sum the elliptic curve points and hash the result. If this hash matches the hash recorded in 
 *     the merkle tree for the provded account, the user who sent the request will be assumed to have knowledge of the 
 *     private key. The account will be checked to have sufficient balance to send the transaciton.
 *     
 *     Assuming validation is successful, the balance of the sender will be decreased by the transaction amount, and 
 *     vice versa for the recipiant. A new block will be added to the chain reflecting this change.
 *  
 * 3.) Validation:
 * 
 *     A validator node can be run by providing the following arguments to the CLI:
 * 
 *     cargo run validate [private key]
 * 
 *     This will trigger the node software to send a network request to all other validator nodes that a new node is 
 *     requesting the current state of the blockchain and merkel tree. Each node will send their current state to the 
 *     connecting node. Each of these states will be hashed and counted to determine the majority consensis of the 
 *     network. This state will then be adopted by the connecting node and stored/maintained locally. If this is the 
 *     first node of the network, the connecting node will create the genesis block and establish an empty merkel tree.
 * 
 *     Once a node has connected, it will begin to listen for incoming transactions and account creations. The logic 
 *     of the above described processes will be fasciliated by the node software.  
 * 
 * 4.) Faucet: 
 *     
 *     Using the faucet command will send a network request to validator nodes to provide a given public key with a 
 *     small amount of tokens that can be used to send transactions with. This is for testing purposes.
 */



#[tokio::main]
async fn main() -> std::io::Result<()> {
    if VERBOSE_STACK  { println!("main.rs: main() called"); }

    // read CLI args into vector
    let args: Vec<String> = env::args().collect();

    // Send Account Creation Request Specified  
    if args[1] == "make" && args.len() == 2{ 
        account_creation::send_account_creation_request().await;

    } // Transaction Specified
    else if args[1] == "transaction" && args.len() == 5{  

        // extract provided arguments:
        let sender_private_key: String = args[2].to_string();
        let recipient_public_key: String = args[3].to_string();
        let transaction_amount: String = args[4].to_string();

        // send transaction request to validator nodes
        send_transaction::send_transaction_request(
            sender_private_key, recipient_public_key, transaction_amount
        ).await;
 
    }// Validation Specified 
    else if args[1] == "validate" && args.len() == 3{ 

        // extract provided arguments:
        let private_key: &String = &args[2];
            
        // Run node as a validator
        validation::run_validation(private_key).await;

    } // Faucet Specified
    else if args[1] == "faucet" && args.len() == 3 {
    
        let public_key: String = args[2].to_string(); 
        faucet::send_faucet_request(public_key).await;
    }
    else { // Improper Command
        println!("ERROR! Unrecognized Command");
        return Ok(());
    }
    
    Ok(())
}

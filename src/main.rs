use std::env;
use validation::run_validation;

mod account_creation;
mod blockchain;
mod send_transaction;
mod validation;
mod merkle_tree;

/**
 * @notice main.rs runs a blockchain node which connects to a TCP server in order to interact with the blockchain. 
 *         There are three options when connecting a node from the CLI: Account Creaction, Trancation, Validation
 *
 * Acount Creation:
 *  
 * 1.) The first is to make an account by providing the [make] argument when running the node software. 
 *  
 *      cargo run make
 *  
 *     This will alert validators that a new public and private key has been generated and needs to be updated 
 *     across the distributed network. The key is generated on the client side using elliptic curve cryptography 
 *     (secp256k1)and distributed to the network. Validators are responsible for fascilitaing these 
 *     transcations within the network, which will include minimal checks at this stage in the development.
 *     Just checking that a prexisting account is not being overwritten (extremely unlikely since using 
 *     secp256k1). The new account will aslo be integrated into the merkel tree that contains all 
 *     (private-key, account-balance) pairs of all accounts created up to this point. Accounts not creates 
 *     yet are implicitly at a balance of 0 and "become accounts" that can transactions once created using 
 *     [make] from CLI.
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
 *     cargo run [private key] [recipiant public key] [transaction amount]
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
 * 3.) Whereas Transactions and Account Creations are shorted lived processes for the user. Running the [validate]
 *     argument will contunually run the validation process until the program is exited. Validators keep the TCP
 *     server online, fascilitating the computation and documentation of incoming transactions and account creations 
 *     within the shared ledger (blockchain). 
 *     
 *     There are two datastructures that are maintained by validators: 
 * 
 *          - The blockchain is a linked list of Block structs storing events that have occured.
 *          - A merkel tree that stores (public-key, account-balance) pairs. 
 * 
 *     The blockchain data and merkel tree data are both stored in seperate json files. Each file contains the data
 *     structure itself, as well as its accumulated hash. These files are maintained on the client side by users
 *     running the validation process of the node software. Validation ensures the integrity of the blockchain
 *     and merkel tree data in the following two ways:
 *     
 *          - Transactions are checked for sufficient balance. Accounts are checked for duplication.
 *          - The more import validation check is to make sure that the hash of the blockchain and merkel tree
 *            before and after the transaction are the same across all nodes currently running the validation
 *            process. A majority consensis across nodes is required to validate a transaction.
 * 
 */

fn main() -> std::io::Result<()> {

    // read CLI args into vector
    let args: Vec<String> = env::args().collect();

    // Account Creation Specified  
    if args[1] == "make" && args.len() == 2  { 

        // validate account creation
        account_creation::account_creation();
        return Ok(());

    } // Transaction Specified
    else if args[1] == "transaction" && args.len() == 5 {  

        // extract provided arguments:
        let private_key = &args[2];
        let recipient = &args[3];
        let transaction_amount = &args[4];

        // validate transaction 
        send_transaction::send_transaction(private_key, recipient, transaction_amount);
 
    }// Validation Specified 
    else if args.len() == 3 && args[1] == "validate" { 

        // extract provided arguments:
        let private_key: &String = &args[2];
        
        // Run node as a validator
        run_validation(private_key);
    } 
    else { // Improper Command
        println!("ERROR! Unrecognized Command");
        return Ok(());
    }
    
    Ok(())
}

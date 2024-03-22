

// account.rs is a file that contains structs and functions for fascilitating the creation 
// and use of accouunts that perform transactions withi the blockchain 

// ! implement a fountain address

/**
 * accounts.rs contains structs and functions for fascilitating the creation and storage/retrieval
 * of account information. The following is a high level overview of the protocol this file implements:
 * 
 * 1.) At the start of the blockchain there are not accounts:
 * 
 * 2.) When accessing the network, a user can specify the "make" command to create a new account:
 *     This will generate a new private key and public key pair using elliptic curve cryptography
 *     (secp256k1). The private key acts as the user's "password" and the public key acts as the 
 *     user's "username". The discrete logarithm problem is used to ensure that the private key 
 *     cannot be derived from the public key.
 * 
 *     Accounts made by the the "make" protocol will be stored in a merkle tree with an initial 
 *     balance of 0. The merkel tree will be used for lookup of account balance and verification 
 *     of transactions. 
 * 
 * 3.) When a user wants to make a transaction, they will need to specify the private key of their
 *     account, the recipient's public key, and the amount of the transaction. The user will access
 *     the LAN network providing this information. This will trigger the validation process
 * 
 *  ! Figure out how to decentralize the validation process
 * 
 * 4.) The validation process begins with the the lookup of the private key in the merkle tree. If 
 *     the private key is found, their account balance will be checked against the transaction amount. 
 *     If the balance is sufficient, the transaction is valid.
 * 
 * 5.) After a transaction has been validated, it will be signed with the private key and balances will 
 *     be updated in the merkle tree. A new block will be created and added to the blockchain./*  */
 */




fn generate_keys() -> (String, String) {
    // Stub function - replace with actual implementation
    ("private_key".to_string(), "public_key".to_string())   
}
//
pub fn account_creation() {
    // Stub function - replace with actual implementation

    // generate a new key pair
    let (private_key, public_key) = generate_keys();

    println!("New Account Created!");
    println!("Private Key: {}", private_key);
    println!("Public Key: {}", public_key);
}   
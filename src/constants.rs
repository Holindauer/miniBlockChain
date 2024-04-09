/**
 * @notice constants.rs contains the global constants used throughout the blockchain node software.
 */


use std::time::Duration;

// port number for coordinating the blockchain network
pub const PORT_NUMBER: &str = "127.0.0.1:8080"; 

// durtion to listen for blockchain records from peers when booting up
pub const DURATION_GET_PEER_CHAINS: Duration = Duration::from_secs(1);  

// Verbosity parameters
pub const VERBOSE_STACK: bool = true; // stack processes

// TEST controls whether to save json block updates during integration testing
pub const INTEGRATION_TEST: bool = true;

// amount of tokens to send to accounts when a faucet request is made
pub const FAUCET_AMOUNT: u64 = 100; 

// duration to listen for block consensus requests
pub const BLOCK_CONSENSUS_LISTENING: Duration = Duration::from_secs(2); 
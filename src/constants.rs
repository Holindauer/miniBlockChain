/**
 * @notice constants.rs contains the global constants used throughout the blockchain node software.
 */


use std::time::Duration;

// port number for coordinating the blockchain network
pub const PORT_NUMBER: &str = "127.0.0.1:8080"; 

// durtion to listen for blockchain records from peers when booting up
pub const DURATION_GET_PEER_CHAINS: Duration = Duration::from_secs(1);  

// Verbosity parameters // ! These cannot both be set to true during testing
pub const VERBOSE_STACK: bool = false; // stack processes
pub const VERBOSE_TEST: bool = true; // printing terminal outputs to be read by shell scripts

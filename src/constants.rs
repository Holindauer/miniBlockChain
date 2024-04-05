/**
 * @notice constants.rs contains the global constants used throughout the blockchain node software.
 */


use std::time::Duration;


pub const PORT_NUMBER: &str = "127.0.0.1:8080"; 
pub const VERBOSE: bool = true;

// durtion to listen for blockchain records from peers when booting up
pub const DURATION_GET_PEER_CHAINS: Duration = Duration::from_secs(1);  
/**
 * @notice constants.rs contains the global constants used throughout the blockchain node software.
 */


use std::time::Duration;

// durations to wait before sending for consensus
pub const PEER_STATE_RECEPTION_DURATION: Duration = Duration::from_secs(2);  

// Heartbeat durations
pub const HEARTBEAT_PERIOD: Duration = Duration::from_secs(5);
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);

// TEST controls whether to save json block updates during integration testing
pub const INTEGRATION_TEST: bool = true;

// amount of tokens to send to accounts when a faucet request is made
pub const FAUCET_AMOUNT: u64 = 100; 



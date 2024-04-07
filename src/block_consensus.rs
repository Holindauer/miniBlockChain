

/**
 * @notice block_consensus.rs contains the logic for peer validator nodes to reach a consensus over whether or not to accept a new block
 * into the blockchain.
 * 
 * Each time a request for a transaction/account creation is made, the validator nodes will independently check the validity of the request.
 * Then, upon their independent decision, they will send a request to all other validator nodes for their decision. The majority decision will
 * be accepted by the network regardless of the individual validator node's decision.
 */
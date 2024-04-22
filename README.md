# Block Chain Implementation in Rust

# Concept Overview:

This repository implements a peer-to-peer network that fascilitates the execution of a minimal blockchain protocol for tracking ownership of tokens between privately owned accounts. 

A blockchain, in this context is a decentralized append-only database that can be written to by anyone and tampered with by no one. Each peer node in the peer-to-peer network is responsible for maintaining a local copy of this database. When a transaction is requested, it is sent to each peer for an independent validation. Upon the validation of a request on the client side, the node sends out to all other peers for their own independent decisions. The majority decision among the responses is adopted by the peer node, and a new block is either written to the chain or the transaction is rejected. 

This database is thus a permanent ledger of all transactions between privately owned accounts that have been validated by the majority of peers. 

While this implementation is focused more so on the implementation of a peer-to-peer consensus protocol, there are also some game-theoretical safeguards deincentivising the manipulation of the ledger for individual gain. At this stage, this includes providing validator nodes a token reward for reaching a consensus with peers.

# Protocol Overview:

## Two Types of Interation

There are two basic ways to interact with the peer-to-peer network:

- Sending requests to the network
- Running the node as a validator

I will talk first about the fundamental mechanics of peer consensus, then build off that when explaining each of the three types of request that can be sent:

- Account Creation 
- Transaction 
- Faucet

## Connecting a Validator Node

Peer nodes maintain a shared ledger state by individualy maintaining local copies of two datastructures: the BlockChain and MerkelTree. 

- The BlockChain is a linked list of Blocks, whereby each Block contains the information of accepted transactions in the order they were accepted by the network.
- The MerkelTree is a form of binary tree that is used to store account balances. It uses a hashing functionality where each leaf node is a hash of its data, and each non-leaf node is a hash of its respective child nodes. Thus creating a single hash at the root representing the status of the entire tree.

To connect a node as a validator, run the following commmand:

    cargo run validate 

This will caused the following chain of events:

1. An empty BlockChain and MerkelTree will be initialized.

2. An attempt to connect a TCP listener to one of the 4 port addresses within the accepted_ports.json file. 

3. Upon connecting to a port, the node will spawn an asynchronous task for sending periodic heartbeat signals to all other nodes, indicating to other nodes that there is a peer on the port specified in the signal. Peers, upon receiving a heartbeat signal, will maintain a list of the currently active nodes. If a peer stops sending their heartbeart, the node will take notice of this and drop them from the list of active peers. The list of active peers is used to know who to send consensus requests to (more on this later...). This heartbeat protocol runs in the background as long as the node is active. 

4. Briefly after listening has begun, and once a list of active peers has been established, a new node will send a request to all currently active peers for their locally maintained ledger states. Active peers will respond back with their copy of the BlockChain and MerkelTree. The merkel root hash and and blockchain hash will be used to determine the majority state of the network. The majority state will then be adopted by the connecting peer. If there is no majority state, which could happen when the first peer node connects or a tie is made, then the node will use the empty datastructures or split the tie based on who was first to respond, respectively. 

After the node has connected a TCP listener and has adopted the majority state of the network, it is considered to be on and will handle requests as they come in. 

NOTE: *It is not neccessary to provide a private key when booting up a validator node. However, no validator reward will be alloted if not. More on this later...*

## Consensus Protocol for Transaction Approval

The logic for validating the 3 different types of requests is unique, however they all institute the same consensus protocol when handling requests:

1. After a request is recieved it is filtered into the event handler that matches the request type. 

2. An independent validation on the client side is performed for that request.

3. Upon the an independent client decision being made, the client node will send a request to all currently active peers for their own independent decisions. The node will wait for all currently active peers to respond. 

4. Upon recieving all responses, the majority decision will be adopted by the network.


## Account Creation Request Protocol

Assuming there is at least one active node, an account can be created by running:

    cargo run make

This will result in an output that looks like this:

    Account details sucessfully created: 
    Secret Key: "041f2be2bdac054d7e1539b3e70b9eac31dac91350a340bad9abadb747de4a24"
    Public Key: "03b0866fbd47f93763c195d86f0e38a30fee7fffda718da0dc4b0866f49ee08ae4"

Under the hood, the following processes are happening:

1. A private key is created by generating a random 256 bit integer that was generated on the client side by the requester. 

2. The public key is derived from the private key by applying scalar multiplication of the private key to the generator point of the sepc256k1 elliptic curve over a finite field. Due to the discrete logarithm problem, it is cryptogrpahically infeasible to determine the private key from just this public key.

3. To establish a means of validating ownership of an account without revealing the private key, a simple zero knowledge proof scheme is set up. The private key is scalar multiplied by the generator term provided in curve25519_dalek eliptic curve library. This seperate curve is used in order to perform explicit elliptic curve point addition and scalar multiplication of the generator. The scalar multiplication of the generator with the private key weill be stored, along with other account information, in the merkel tree. Knowledge of the private key is be validated by the transaction requester client randomly splitting their private key into a sum of two integers. When added, this sum is the original private key. The two integers of the sum are scalar multiplied by the generator term of the curve25519 generator. Due to the the group homomorphism of *ellipic curve point addition and scalar multiplication of the generator over a finite field* that is shared between *integer addition and multiplication*, the two curve point transformation of each term of the sum should add to the curve point representaiton of the original private key curve point representation stored in the merkel tree. Validating that this is true is how validity of knowledge of the private key is established.

4. A request for account creation is sent to the network with the public key and the curve point representation of the private key.

Upon recieving the request, the validator client will check if there is already an account that exists with the given public key. If no account exists, the client decsion will be to accept the account creation request. A consensus request will be sent to peers. If a majority decision to accept is approved, the account will be added to the merkel tree and a block will be written to the chain indicating this:

    Current State of Blockchain as Maintained on Client Side:

    Block 0: 
            Genesis Block
            Time: 1713547478

    Block 1: 
            New Account: 03b0866fbd47f93763c195d86f0e38a30fee7fffda718da0dc4b0866f49ee08ae4
            Account Balance: 0
            Time: 1713547482
            Hash: ad439784b0cedbb7f1d15d04cb512958b3cb1e16b77ab753150898237b99064d

Once the account is created, the public key serves as the username and the private key is the password. 

## Faucet Request Protocol

There is a faucet implemented for issuing 100 tokens to accounts per request. Currently, there is no limit to how much an account can request. 

    cargo run [public key]

This will send a network request asking for tokens to be sent to the public key specified. Node clients will verify the account exists, send for consensus, and issue the funds.

Current State of Blockchain as Maintained on Client Side:

    Block 0: 
            Genesis Block
            Time: 1713547478

    Block 1: 
            New Account: 03b0866fbd47f93763c195d86f0e38a30fee7fffda718da0dc4b0866f49ee08ae4
            Account Balance: 0
            Time: 1713547482
            Hash: ad439784b0cedbb7f1d15d04cb512958b3cb1e16b77ab753150898237b99064d

    Block 2: 
            Faucet Used By: 03b0866fbd47f93763c195d86f0e38a30fee7fffda718da0dc4b0866f49ee08ae4
            Account Balance: 100
            Time: 1713547558
            Hash: a3e9978ece568d7fc9c84fbf19b36b8dae3c7a4186dbf55f623193cf5c672ed9

## Transaction Request Protocol

A trasaction can be request by running the following command:

    cargo run transaction [sender private key] [recipient public key] [amount]

This will result in the following processes:

1. The public key of the sender will be derived from the provided private key.

2. The private key will be split into two integers and converted into elliptic curve points. 

3. The public key of the sender, public key of the recipient, elliptic curve point representation of the sender private key, and the transaction amount will be sent to the network for validatin.

Upon recieving this request, validator nodes will check the following to vlaidate the transaction: 

1. Both the sender and the recipient exist in the merkel tree.

2. The curve points add to the curve point representaiton of the private key within the merkel tree for the sender public key provided.

3. If the zk-proof is valid, the proof will be stored. Subsequest proofs for the same sender will not allow this proof to be used again. This is to not allow for replay attacks by using the same proof again. 

4. The sender has sufficient funds for sending the specified transactiona amount.

If these are all true, the client decision is to validate and a request for consensus is sent to the network. If the majority comes back as yes, the account balances are updated in the merkel tree and a new block is written to the chain.

    Current State of Blockchain as Maintained on Client Side:

    Block 0: 
            Genesis Block
            Time: 1713547478

    Block 1: 
            New Account: 03b0866fbd47f93763c195d86f0e38a30fee7fffda718da0dc4b0866f49ee08ae4
            Account Balance: 0
            Time: 1713547482
            Hash: ad439784b0cedbb7f1d15d04cb512958b3cb1e16b77ab753150898237b99064d

    Block 2: 
            Faucet Used By: 03b0866fbd47f93763c195d86f0e38a30fee7fffda718da0dc4b0866f49ee08ae4
            Account Balance: 100
            Time: 1713547558
            Hash: a3e9978ece568d7fc9c84fbf19b36b8dae3c7a4186dbf55f623193cf5c672ed9

    Block 3: 
            New Account: 027a8038a0a2f89096dc6021282009c24ef5a5544286fa88189c27d208d79551de
            Account Balance: 0
            Time: 1713547617
            Hash: 3c93d62ed0bac36c0d2836ee2f06a34dd3f5bb45956baaf8ffb088cc94d930b8

    Block 4: 
            Sender: 03b0866fbd47f93763c195d86f0e38a30fee7fffda718da0dc4b0866f49ee08ae4
            Sender Balance: 50
            Sender Nonce: 1
            Recipient: 027a8038a0a2f89096dc6021282009c24ef5a5544286fa88189c27d208d79551de
            Recipient Balance: 50
            Amount: 50
            Time: 1713547728
            Hash: 55442950180c4e60b9583a8cbe154ad2bb03337f430d9d8b97aeee4f5c6d4646
    

## Integration Testing Dependencies 

    xterm
    jq
 


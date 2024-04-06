# Block Chain Implementation in Rust


This repository contains the implementation of a simple blockchain protocol fascilitated by a TCP-based network. Put simply, a blockchain is a decentralized database that can be written to by anyone and can be tampered with by no one. 

This repo, implements a blockchain for tracking exchange of ownership between privately owned accounts that are maintained by a network of validator nodes. A validator node is a client run server that accepts incoming transaction requests, processes them, and coordinates their documentation across all currently running validator node TCP servers in the network. 

The src director contains the implementation of the validator node with three basic functionalities:
- Fascilidate validation 
- Create an account
- Send a transaction



Integration Testing Dependencies 

    xterm
    jq
 
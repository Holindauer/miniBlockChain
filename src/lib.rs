// src/lib.rs

// Make each submodule public to be accessible from main.rs or other parts
pub mod modules {
    pub mod adopt_network_state;
    pub mod blockchain;
    pub mod consensus;
    pub mod constants;
    pub mod merkle_tree;
    pub mod network;
    pub mod requests;
    pub mod validation;
    pub mod zk_proof;
}

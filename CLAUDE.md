# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based blockchain implementation that creates a peer-to-peer network for tracking token ownership. It demonstrates key concepts including distributed consensus, cryptographic validation, and zero-knowledge proofs.

## Essential Commands

### Building
```bash
cargo build                  # Debug build
cargo build --release        # Optimized build
```

### Testing
```bash
# Unit tests
cargo test                   # Run all unit tests
cargo test -- --nocapture    # Show test output
cargo test test_name         # Run specific test

# Integration tests (requires xterm and jq)
./run_integration_tests.sh   # Run all integration tests
```

### Linting and Formatting
```bash
cargo fmt                    # Format code
cargo fmt -- --check         # Check formatting
cargo clippy                 # Run linter
cargo clippy --all-targets   # Lint including tests
```

### Running the Application
```bash
cargo run make                                                           # Create new account
cargo run validate [private_key]                                         # Run validator node
cargo run faucet [public_key]                                           # Request 100 tokens
cargo run transaction [sender_private_key] [recipient_public_key] [amount]  # Send transaction
```

## Architecture Overview

### Core Components

1. **Blockchain (blockchain.rs)**: Implements a linked-list blockchain with multiple block types (Genesis, Transaction, AccountCreation, Faucet). Uses pending transaction queue for ordering.

2. **Validation (validation.rs)**: The heart of the system. ValidatorNode manages local ledger state, consensus decisions, and peer coordination using Arc<Mutex<>> for thread safety.

3. **Network (network.rs)**: Event-driven TCP networking with master event handler. Manages peer discovery via heartbeat protocol and routes messages to appropriate handlers.

4. **Consensus (consensus.rs)**: Implements peer-to-peer voting where nodes independently validate then reach majority consensus. Uses async notify for coordination.

5. **Merkle Tree (merkle_tree.rs)**: Hybrid HashMap + Tree structure for O(1) balance lookups with cryptographic verification. Tracks account nonces for replay protection.

6. **Zero-Knowledge Proofs (zk_proof.rs)**: Uses Curve25519 for proving private key ownership without revealing the key. Prevents proof reuse attacks.

### Data Flow
```
User Command → main.rs → requests.rs → TCP Network → 
network.rs (event handler) → validation.rs → consensus.rs → 
blockchain.rs (state update) → merkle_tree.rs (balances) → JSON persistence
```

### Key Design Patterns
- **Async/await with Tokio** for all I/O operations
- **Message-passing** via JSON over TCP
- **Shared state** using Arc<Mutex<>>
- **Event-driven** architecture with typed handlers
- **Modular separation** between network, consensus, and data layers

### Testing Approach
Integration tests spawn multiple validator nodes in xterm windows, perform operations, and verify blockchain state consistency across nodes. Tests clean up by killing xterm processes.

### Network Configuration
- Runs on localhost (127.0.0.1)
- Uses ports 8080-8083 (defined in accepted_ports.json)
- Nodes discover peers via heartbeat protocol

### Important Implementation Details
- Validator nodes persist state to `Node_[address]:[port]/` directories
- Consensus requires majority agreement from active peers
- Zero-knowledge proofs are tracked to prevent reuse
- Faucet distributes 100 tokens per request
- Validators receive 1 token reward per validated transaction
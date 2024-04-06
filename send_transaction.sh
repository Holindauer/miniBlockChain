#!/bin/bash

# Check if the correct number of arguments are passed
if [ "$#" -ne 4 ]; then
    echo "Usage: $0 sender_public_key sender_private_key recipient_public_key amount"
    exit 1
fi

# Assign arguments to variables for clarity
blockchain_command="transaction"
sender_public_key="$1"
sender_private_key="$2"
recipient_public_key="$3"
amount="$4"

# Run the cargo command with the provided arguments
cargo run $blockchain_command "$sender_public_key" "$sender_private_key" "$recipient_public_key" "$amount"

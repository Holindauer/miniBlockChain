#!/bin/bash

# This script contains an integration tests that when the incorrect private key is used, the transaction will 
# fail. It will create two accounts, save their public and private key information, and send a transaction between 
# the two accounts using the private key of the recipient instead of the sender. This should cause a failure
# in the transaction and the block should not be added to the blockchain.
#
# Ensure that the INTEGRATION_TEST flag is set to true in constants.rs before running this script.

# Ensure jq and xterm are installed
if ! [ -x "$(command -v jq)" ]; then
    echo 'Error: jq is not installed.' >&2
    echo
    echo "Install? (y/n)"
    read install_jq
    if [ "$install_jq" == "y" ]; then
        sudo apt-get install jq
    else
        exit 1
    fi
fi

if ! [ -x "$(command -v xterm)" ]; then
    echo 'Error: xterm is not installed.' >&2
    echo 
    echo "Install? (y/n)"
    read install_xterm
    if [ "$install_xterm" == "y" ]; then
        sudo apt-get install xterm
    else
        exit 1
    fi
fi

# stop any xterm processes that may be running
killall xterm

# Saved ledger directories
saved_ledger_directories=(
    "Node_127.0.0.1:8080" 
    "Node_127.0.0.1:8081" 
    "Node_127.0.0.1:8082" 
    "Node_127.0.0.1:8083"
)

# Remove the saved ledger directories before running the validator node
for dir in "${saved_ledger_directories[@]}"; do
    rm -rf "./$dir"
done

# Open a new terminal to run a validator node
xterm -hold -e "bash -c 'cargo run validate'" &

# Wait for the validator node to initialize and start
sleep 3

# Open 3 more terminals to run additional validator nodes
xterm -hold -e "bash -c 'cargo run validate'" &
xterm -hold -e "bash -c 'cargo run validate'" &
xterm -hold -e "bash -c 'cargo run validate'" &

# Wait 7 seconds for the new nodes to adopt the network state
sleep 7

# Create and extract the info from two accounts
for i in 1 2; do
    echo "Creating account $i..."

    # Make a new account (this will save the account details to new_account_details.json)
    cargo run make 

    # Check if the account details file was created successfully
    if [ -f "./new_account_details.json" ]; then
        echo "File for account $i found."

        # Read the output from the file
        account_json=$(< "./new_account_details.json")
        echo "Raw JSON output for account $i: $account_json"  # Debug output

        # Use jq to parse the JSON and extract keys
        secret_key=$(echo "$account_json" | jq -r '.secret_key')
        public_key=$(echo "$account_json" | jq -r '.public_key')

        # Check for empty results which indicate jq did not find the data
        if [[ -z "$secret_key" || -z "$public_key" ]]; then
            echo "Error parsing keys for account $i. Check JSON format."
            continue  # Skip this iteration
        fi

        # Save keys to variables dynamically named
        declare "secret_key_$i=$secret_key"
        declare "public_key_$i=$public_key"

        echo "Account $i: Public Key: $public_key, Secret Key: $secret_key"

        # Remove the file after extracting the necessary information
        rm -f "./new_account_details_$i.json"
    else
        echo "Failed to find account creation output for account $i."
        killall xterm
        exit 1
    fi
done

# Use the faucet to add 100 tokens to the first account
cargo run faucet "$public_key_1"

# send transaction to account 2 using a random private key not stored on the network
echo
echo "Sending transaction from account 1 to account 2 with incorrect private key..."
cargo run -- transaction cca90f3b21bb49a7a27aab8bebd4df68307f5cbd9ec989f663348d64ad432516 "$public_key_2" 50
clear

# Check for most_recent_block.json file
if [ -f "./failed_transaction.json" ]; then
    # Read the output from the file
    transaction_status=$(< "./failed_transaction.json")
else
    echo "Failed to find failed transaction indicator."
fi

# extract the information from the most_recent_block.json file
failed_transaction=$(jq '.' failed_transaction.json)

# Remove the file after reading
rm -f ./failed_transaction.json

killall xterm

# Check the value
if [ "$failed_transaction" -eq 1 ]; then
    echo "A failed transaction was detected."
    exit 0 # success
else
    echo "No failed transaction detected."
    exit 1 # failure
fi

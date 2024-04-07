#!/bin/bash

# This script contains an integration test for using the faucet to recieve funds from the faucet

# Open a new terminal and run validator node
xterm -hold -e "bash -c 'cargo run validate private_key'" &

# Wait for the validator node to initialize
sleep 3

# Execute the script to create a new account and redirect output to a file
cargo run make

# Wait for the account creation json file to be created by the make_new_account.sh script
if [ -f "./new_account_details.json" ]; then

    # Read the output from the file
    account_1_json=$(< "./new_account_details.json")
else
    echo "Failed to find account creation output."
fi

# Remove the file
rm -f ./new_account_details.json

# Create another account
cargo run make 

sleep 2

# Repeat for the second account as needed, potentially using a different output file
# and located in the current directory
if [ -f "./new_account_details.json" ]; then
    # Read the output from the file
    account_2_json=$(< "./new_account_details.json")
else
    echo "Failed to find account creation output."
fi

# Remove the file
rm -f ./new_account_details.json

# create a faucet request for account 1
echo
echo "Creating faucet request for account 1..."

# extract the secret_key and public_key from each json
secret_key_1=$(echo $account_1_json | jq -r '.secret_key')
public_key_1=$(echo $account_1_json | jq -r '.public_key')

# send faucet request from account 1
cargo run -- faucet "$public_key_1"

# retrieve most_recent_block.json file
if [ -f "./most_recent_block.json" ]; then

    # Read the output from the file
    most_recent_block_json=$(< "./most_recent_block.json")
else
    echo "Failed to find most recent block."
fi

# Close the xterm window
killall xterm

# extract the block_hash from the json
address=$(echo $most_recent_block_json | jq -r '.address')
rm -f ./most_recent_block.json

echo "Address: $address"
echo "Public Key: $public_key_1"
echo "Secret Key: $secret_key_1"

# ensure address matches the public key of account 1
if [ "$address" != "$public_key_1" ]; then
    echo "Failed to match address to public key."
    exit 1 # fail
else  
    echo "Address matches public key."
    exit 0 # pass
fi

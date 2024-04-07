#!/bin/bash

#!/bin/bash

# This script contains an integration tests that when the incorrect private key is used, the transaction will 
# fail. It will create two accounts, save their public and private key information, and send a transaction between 
# the two accounts using the private key of the recipient instead of the sender. This should cause a failure
# in the transaction and the block should not be added to the blockchain.
#
# Ensure that the INTEGRATION_TEST flag is set to true in constants.rs before running this script.

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


# extract the secret_key and public_key from each json
secret_key_1=$(echo $account_1_json | jq -r '.secret_key')
public_key_1=$(echo $account_1_json | jq -r '.public_key')

secret_key_2=$(echo $account_2_json | jq -r '.secret_key')
public_key_2=$(echo $account_2_json | jq -r '.public_key')

# send transaction from account 1 to account 2
echo
echo "Sending transaction from account 1 to account 2 with incorrect private key..."
cargo run -- transaction "$public_key_1" "$secret_key_2" "$public_key_2" 0
clear

# stop the validator node
killall xterm

# Check for most_recent_block.json file
if [ -f "./failed_transaction.json" ]; then
    # Read the output from the file
    transaction_status=$(< "./failed_transaction.json")
else
    echo "Failed to find most recent block."
fi

# extract the information from the most_recent_block.json file
failed_transaction=$(jq '.' failed_transaction.json)


# Check the value
if [ "$failed_transaction" -eq 1 ]; then
    echo "A failed transaction was detected."
    exit 0 # success
else
    echo "No failed transaction detected."
    exit 1 # failure
fi
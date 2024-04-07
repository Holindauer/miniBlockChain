#!/bin/bash

# This script contains an integration test for sending a transaction between two newly created
# accounts. It will create two new accounts, save their public and private key information, and 
# send an intitial transaction of zero tokens between each account. 
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
echo "Sending transaction from account 1 to account 2..."
cargo run -- transaction "$public_key_1" "$secret_key_1" "$public_key_2" 0

# Check for most_recent_block.json file
if [ -f "./most_recent_block.json" ]; then
    # Read the output from the file
    most_recent_block_json=$(< "./most_recent_block.json")
else
    echo "Failed to find most recent block."
fi

# use jq to extract the block 
timestamp=$(echo $most_recent_block_json | jq -r '.time')
amount=$(echo $most_recent_block_json | jq -r '.amount')
sender_nonce=$(echo $most_recent_block_json | jq -r '.sender_nonce')
sender=$(echo $most_recent_block_json | jq -r '.sender')
recipient=$(echo $most_recent_block_json | jq -r '.recipient')


# remove the most_recent_block.json file now that we have the information
rm -f ./most_recent_block.json

clear

# stop the validator node
killall xterm

# print the json of the most recent block
echo 
echo "Most Recent Block JSON: $most_recent_block_json"

# print both account jsons
echo
echo "Account 1 JSON: $account_1_json"
echo "Account 2 JSON: $account_2_json"

# echo the block information
echo
echo "Block Timestamp: $timestamp"
echo "Block Transaction Amount: $amount"
echo "Block Sender Nonce: $sender_nonce"
echo "Block Sender: $sender"
echo "Block Recipient: $recipient"

# check if the transaction was successful
if [ "$amount" -eq 0 ]; then
    echo "Amount test passed."
else
    echo "Amount test failed."
    exit 1 # return failure
fi

if [ "$sender_nonce" -eq 1 ]; then
    echo "Sender Nonce test passed."
else
    echo "Sender Nonce test failed."
    exit 1 # return failure
fi

if [ "$sender" == "$public_key_1" ]; then
    echo "Sender test passed."
else
    echo "Sender test failed."
    exit 1 # return failure
fi

# Indicate success
echo "Transaction test passed."
exit 0 # return success
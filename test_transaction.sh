#!/bin/bash

# This script contains an integration test for sending a transaction between two newly created
# accounts. It will create two new accounts, save their public and private key information, and 
# send an intitial transaction of zero tokens between each account. 

# Ensure that the INTEGRATION_TEST flag is set to true in constants.rs before running this script.



# Open a new terminal and run validator node
xterm -hold -e "bash -c './run_validation.sh'" &

# Wait for the validator node to initialize
sleep 5

# Execute the script to create a new account and redirect output to a file
./make_new_account.sh

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
./make_new_account.sh

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
./send_transaction.sh $public_key_1 $secret_key_1 $public_key_2 0

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
# rm -f ./most_recent_block.json


clear

# print both account jsons
echo "Account 1 JSON: $account_1_json"
echo "Account 2 JSON: $account_2_json"

# echo the block information
echo "Block Timestamp: $timestamp"
echo "Block Transaction Amount: $amount"
echo "Block Sender Nonce: $sender_nonce"
echo "Block Sender: $sender"
echo "Block Recipient: $recipient"

# Check that the information in the block matches the transaction
if [ "$amount" -eq 0 ] && [ "$sender_nonce" -eq 1 ] && [ "$sender" == "$public_key_1" ] && [ "$recipient" == "$public_key_2" ]; then
    echo "Transaction test passed."
else
    echo "Transaction test failed."
fi


# release the validator node
killall xterm

echo "Transaction test passed."

exit 0
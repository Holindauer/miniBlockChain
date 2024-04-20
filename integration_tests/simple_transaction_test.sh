#!/bin/bash
#!/bin/bash

# This script contains an integration test for sending a transaction between two newly created
# accounts. It will create two new accounts, save their public and private key information, and 
# send an initial transaction of zero tokens between each account.

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
xterm -hold -e "bash -c 'cargo run validate private_key'" &

# Wait for the validator node to initialize and start
sleep 3

# Open 3 more terminals to run additional validator nodes
xterm -hold -e "bash -c 'cargo run validate private_key'" &
xterm -hold -e "bash -c 'cargo run validate private_key'" &
xterm -hold -e "bash -c 'cargo run validate private_key'" &

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

# Send a transaction of 50 tokens from account 1 to account 2
cargo run transaction "$secret_key_1" "$public_key_2" 50

# Define the array to hold JSON data
blockchain_data=()

# From the saved ledger directories, extract the blockchain.json data using jq
for dir in "${saved_ledger_directories[@]}"; do

    # Extract JSON data from each file and append it to the array
    blockchain_json=$(jq -r 'tojson' "./$dir/blockchain.json")
    blockchain_data+=("$blockchain_json")
done

# Iterate over the blockchain_data array to display or process each JSON entry
for json_entry in "${blockchain_data[@]}"; do
    echo "$json_entry"
done

# collected data from all nodes
sender_account_balance=0

# Assuming blockchain_data is populated correctly as described earlier
for json_entry in "${blockchain_data[@]}"; do

    # Extract the list of keys for the first level objects in each JSON entry
    keys=$(echo "$json_entry" | jq -r '.[] | keys[]')

    # Check if 'Genesis' is the first key
    if [[ "$(echo "$json_entry" | jq -r '.[0] | keys[]')" != "Genesis" ]]; then
        echo "Error: The first block is not 'Genesis'."
        exit 1
    else
        echo "Genesis block verified."
    fi

    # Check if 'NewAccount' is the second key
    if [[ "$(echo "$json_entry" | jq -r '.[1] | keys[]')" != "NewAccount" ]]; then
        echo "Error: The second block is not 'NewAccount'. It is '$(echo "$json_entry" | jq -r '.[1] | keys[]')'."
        exit 1
    else
        echo "NewAccount block verified."
    fi

    # Extract NewAccount address and check against expected public key
    new_account_address=$(echo "$json_entry" | jq -r '.[1].NewAccount.address')
    echo "Extracted NewAccount address: $new_account_address"

    if [[ "$new_account_address" != "$public_key_1" ]]; then
        echo "Error: The address in the NewAccount block does not match the expected public key. Found: $new_account_address"
        exit 1
    else
        echo "Address matches the public key."
    fi

    # Check if 'NewAccount' is the third key
    if [[ "$(echo "$json_entry" | jq -r '.[2] | keys[]')" != "NewAccount" ]]; then
        echo "Error: The third block is not 'NewAccount'. It is '$(echo "$json_entry" | jq -r '.[2] | keys[]')'."
        exit 1
    else
        echo "NewAccount block verified."
    fi

    # check if the address in the NewAccount block matches the expected public key
    if [[ "$(echo "$json_entry" | jq -r '.[2].NewAccount.address')" != "$public_key_2" ]]; then
        echo "Error: The address in the NewAccount block does not match the expected public key."
        exit 1
    else
        echo "Address matches the public key."
    fi

    # Check that 'Faucet' is the fourth key
    if [[ "$(echo "$json_entry" | jq -r '.[3] | keys[]')" != "Faucet" ]]; then
        echo "Error: The fourth block is not 'Faucet'. It is '$(echo "$json_entry" | jq -r '.[3] | keys[]')'."
        exit 1
    else
        echo "Faucet block verified."
    fi

    # Checl that the address in the Faucet block matches the expected public key
    if [[ "$(echo "$json_entry" | jq -r '.[3].Faucet.address')" != "$public_key_1" ]]; then
        echo "Error: The address in the Faucet block does not match the expected public key."
        exit 1
    else
        echo "Address matches the public key."
    fi

    # Check that the account_balance balance in the Faucet block is 100
    if [[ "$(echo "$json_entry" | jq -r '.[3].Faucet.account_balance')" != 100 ]]; then
        echo "Error: The balance in the Faucet block is not 100."
        exit 1
    else
        echo "Balance is 100."
    fi

    # Check that 'Transaction' is the fifth key
    if [[ "$(echo "$json_entry" | jq -r '.[4] | keys[]')" != "Transaction" ]]; then
        echo "Error: The fifth block is not 'Transaction'. It is '$(echo "$json_entry" | jq -r '.[4] | keys[]')'."
        exit 1
    else
        echo "Transaction block verified."
    fi

    # Extract the sender address and check against the expected public key
    sender_address=$(echo "$json_entry" | jq -r '.[4].Transaction.sender')
    echo "Extracted sender address: $sender_address"

    # check that the sender address matches the expected public key
    if [[ "$sender_address" != "$public_key_1" ]]; then
        echo "Error: The sender address in the Transaction block does not match the expected public key."
        exit 1
    else
        echo "Sender address matches the public key."
    fi

    # check that the sender_balance in the Transaction block is 50
    if [[ "$(echo "$json_entry" | jq -r '.[4].Transaction.sender_balance')" != 50 ]]; then
        echo "Error: The sender balance in the Transaction block is not 50."
        exit 1
    else
        echo "Sender balance is 50."
    fi

    # Extract the recipient address and check against the expected public key
    recipient_address=$(echo "$json_entry" | jq -r '.[4].Transaction.recipient')
    echo "Extracted recipient address: $recipient_address"

    if [[ "$recipient_address" != "$public_key_2" ]]; then
        echo "Error: The recipient address in the Transaction block does not match the expected public key."
        exit 1
    else
        echo "Recipient address matches the public key."
    fi

    # Check that the recipient_balance in the Transaction block is 50
    if [[ "$(echo "$json_entry" | jq -r '.[4].Transaction.recipient_balance')" != 50 ]]; then
        echo "Error: The recipient balance in the Transaction block is not 50."
        exit 1
    else
        echo "Recipient balance is 50."
    fi 
done


killall xterm

# Indicate success and return exit status 0 to test driver
echo
echo "Integration test completed successfully."
exit 0

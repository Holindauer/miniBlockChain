#!/bin/bash

# This script contains an integration test for using the faucet to recieve funds from the faucet
# A new account is made and the faucet is used for it. The test ensures that 100 tokens were 
# recieved by the reqeust

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
    fi

    echo "Public Key: $public_key, Secret Key: $secret_key"

    # Remove the file after extracting the necessary information
    rm -f "./new_account_details_$i.json"
else
    echo "Failed to find account creation output for account $i."
    killall xterm
    exit 1
fi

# Use the faucet to add 100 tokens to the first account
cargo run faucet "$public_key"



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


# Assuming blockchain_data is populated correctly as described earlier
for json_entry in "${blockchain_data[@]}"; do

    # Extract the list of keys for the first level objects in each JSON entry
    keys=$(echo "$json_entry" | jq -r '.[] | keys[]')

    # Check if 'Genesis' is the first key
    if [[ "$(echo "$json_entry" | jq -r '.[0] | keys[]')" != "Genesis" ]]; then
        echo "Error: The first block is not 'Genesis'."
        killall xterm
        exit 1
    else
        echo "Genesis block verified."
    fi

    # Check if 'NewAccount' is the second key
    if [[ "$(echo "$json_entry" | jq -r '.[1] | keys[]')" != "NewAccount" ]]; then
        echo "Error: The second block is not 'NewAccount'. It is '$(echo "$json_entry" | jq -r '.[1] | keys[]')'."
        killall xterm
        exit 1
    else
        echo "NewAccount block verified."
    fi

    # Extract NewAccount address and check against expected public key
    new_account_address=$(echo "$json_entry" | jq -r '.[1].NewAccount.address')
    echo "Extracted NewAccount address: $new_account_address"

    if [[ "$new_account_address" != "$public_key" ]]; then
        echo "Error: The address in the NewAccount block does not match the expected public key. Found: $new_account_address"
        killall xterm
        exit 1
    else
        echo "Address matches the public key."
    fi

    # Check if 'Faucet' is the third key
    if [[ "$(echo "$json_entry" | jq -r '.[2] | keys[]')" != "Faucet" ]]; then
        echo "Error: The third block is not 'Faucet'. It is '$(echo "$json_entry" | jq -r '.[2] | keys[]')'."
        killall xterm
        exit 1
    else
        echo "Faucet block verified."
    fi

    # Check that the balance of the account is 100
    account_balance=$(echo "$json_entry" | jq -r '.[2].Faucet.account_balance')
    if [ "$account_balance" -eq 100 ]; then
        echo "Account balance is 100."
    else
        echo "Error: Account balance is not 100. Found: $account_balance"
        killall xterm
        exit 1
    fi

done

# Indicate success to the test driver
echo "All tests passed."
killall xterm
exit 0
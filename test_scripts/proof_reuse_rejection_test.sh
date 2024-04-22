#!/bin/bash

## This test uses the reuse_same_zk_proofs.rs script within the tests directory to create
## Two new accounts, and send a transaction from the first account that has been modified 
## To use the same zk proof as the original transaction. The test then checks that the
## Transaction is rejected by the network.

## The test is also run with 4 validator nodes to ensure that network consensus is reached
## on this matter as well


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

# Run test scrip that sends the same zk proof twice after creating two new accounts
cargo test --test reuse_same_zk_proof


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


sleep 5


# Assuming blockchain_data is populated correctly as described earlier
for json_entry in "${blockchain_data[@]}"; do


    # Ensure that there are only 3 blocks in the chain
    if [[ "$(echo "$json_entry" | jq -r 'length')" != 4 ]]; then
        echo "Error: The blockchain does not contain 4 blocks."
        killall xterm
        exit 1
    else
        echo "Blockchain contains 4 blocks."
    fi


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

    # Check if 'NewAccount' is the third key
    if [[ "$(echo "$json_entry" | jq -r '.[2] | keys[]')" != "NewAccount" ]]; then
        echo "Error: The third block is not 'NewAccount'. It is '$(echo "$json_entry" | jq -r '.[2] | keys[]')'."
        killall xterm
        exit 1
    else
        echo "NewAccount block verified."
    fi

    # Check if 'Transaction' is the fourth key
    if [[ "$(echo "$json_entry" | jq -r '.[3] | keys[]')" != "Transaction" ]]; then
        echo "Error: The fourth block is not 'Transaction'. It is '$(echo "$json_entry" | jq -r '.[3] | keys[]')'."
        killall xterm
        exit 1
    else
        echo "Transaction block verified."
    fi
done


killall xterm

# Indicate success and return exit status 0 to test driver
echo
echo "Integration test completed successfully."
exit 0

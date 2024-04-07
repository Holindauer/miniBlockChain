#!/bin/bash

echo
echo "Running all integration tests..."
echo



# run simple transaction integration test between two newly made accounts accounts
./simple_transaction_test.sh
test_1_result=$?

# run simple transaction w/ incorrect private key integration test between two newly made accounts accounts
./incorrect_private_key_test.sh
test_2_result=$?


# check result of simple transaction test
if [ "$test_1_result" -eq 0 ]; then
    echo "Simple Transaction Test... pass"
else
    echo "Simple Transaction Test... FAIL!"
fi

# check result of simple transaction w/ incorrect private key test
if [ "$test_2_result" -eq 0 ]; then
    echo "Incorrect Private Key Test... pass"
else
    echo
    echo "Incorrect Private Key Test... FAIL!"
    echo
fi




#!/bin/bash

echo
echo "Running all integration tests..."
echo



# run simple transaction integration test between two newly made accounts accounts
./test_scripts/simple_transaction_test.sh
test_1_result=$?

# run simple transaction w/ incorrect private key integration test between two newly made accounts accounts
./test_scripts/incorrect_private_key_test.sh
test_2_result=$?

# run faucet request integration test
./test_scripts/faucet_test.sh
test_3_result=$?

# run faucet request integration test
./test_scripts/peer_state_adoption_test.sh
test_4_result=$?

# run faucet request integration test
./test_scripts/proof_reuse_rejection_test.sh
test_5_result=$?


clear

# check result of simple transaction test
if [ "$test_1_result" -eq 0 ]; then
    echo "Simple Transaction Test... pass"
else
    echo " - "
    echo " - Simple Transaction Test... FAIL!"
    echo " - "
fi

# check result of simple transaction w/ incorrect private key test
if [ "$test_2_result" -eq 0 ]; then
    echo "Incorrect Private Key Test... pass"
else
    echo " - "
    echo " - Incorrect Private Key Test... FAIL!"
    echo " - "
fi
# check result of faucet request test
if [ "$test_3_result" -eq 0 ]; then
    echo "Faucet Request Test... pass"
else
    echo " - "
    echo " - Faucet Request Test... FAIL!"
    echo " - "
fi

# check result of faucet request test
if [ "$test_4_result" -eq 0 ]; then
    echo "Peer Ledger State Adoption Test... pass"
else
    echo " - "
    echo " - Peer Ledger State Adoption Test... FAIL!"
    echo " - "
fi


# check result of faucet request test
if [ "$test_5_result" -eq 0 ]; then
    echo "ZK Proof Reuse Rehjection Test... pass"
else
    echo " - "
    echo " - ZK Proof Reuse Rehjection Test... FAIL!"
    echo " - "
fi







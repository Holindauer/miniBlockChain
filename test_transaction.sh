#!/bin/bash

# test_transaction.sh

# Open Terminal and run validator node
xterm -hold -e "bash -c './run_validation.sh'" &

# wait 5 seconds
sleep 5

# Make two new accounts
xterm -hold -e "bash -c './make_new_account.sh'" &
xterm -hold -e "bash -c './make_new_account.sh'" &

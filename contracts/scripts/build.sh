#!/usr/bin/env bash
set -exu

forge build
# CONTRACTS_DIR="contracts/src"

# for contract in "$CONTRACTS_DIR"/*
# do
#   echo "$contract"
#     if [ -d "$contract" ]; then
#         continue
#     fi
#     forge flatten --output "contracts/src/flattened/$(basename "$contract")" "$contract"
# done

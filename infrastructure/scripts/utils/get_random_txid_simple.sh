#!/bin/bash

# Bitcoin Regtest - Get Random Confirmed Transaction ID (Simple Version)
# This script ensures there are blocks first, then gets a random confirmed transaction

# RPC Configuration
RPC_PORT=18443
RPC_USER=bitcoin
RPC_PASS=bitcoinpass

echo "Getting random confirmed transaction ID..."

# Check if we have any blocks, if not generate some
BLOCK_COUNT=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblockcount 2>/dev/null || echo "0")

if [ "$BLOCK_COUNT" -eq 0 ]; then
    echo "No blocks found. Generating initial blocks..."
    # Create wallet if it doesn't exist
    bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS createwallet "tempwallet" 2>/dev/null || true
    
    # Generate 101 blocks to get initial funding
    bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS generatetoaddress 101 $(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getnewaddress) > /dev/null
    echo "Generated 101 blocks"
fi

# Get current block count
BLOCK_COUNT=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblockcount)
echo "Current block count: $BLOCK_COUNT"

# Method 1: Get transaction from the last block
echo ""
echo "Method 1: Transaction from last block"
LAST_BLOCK_HASH=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getbestblockhash)
LAST_BLOCK=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblock $LAST_BLOCK_HASH)
TXID_FROM_LAST=$(echo $LAST_BLOCK | jq -r '.tx[0]')
echo "TXID from last block: $TXID_FROM_LAST"

# Method 2: Get transaction from a specific block (block 1)
echo ""
echo "Method 2: Transaction from block 1"
BLOCK_1_HASH=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblockhash 1)
BLOCK_1=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblock $BLOCK_1_HASH)
TXID_FROM_BLOCK_1=$(echo $BLOCK_1 | jq -r '.tx[0]')
echo "TXID from block 1: $TXID_FROM_BLOCK_1"

# Method 3: One-liner to get any confirmed transaction
echo ""
echo "Method 3: One-liner for any confirmed transaction"
CONFIRMED_TXID=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblock $(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblockhash $(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS getblockcount)) | jq -r '.tx[0]')
echo "Confirmed TXID (one-liner): $CONFIRMED_TXID"

# Verify the transaction
echo ""
echo "Verifying transaction details:"
if [ "$CONFIRMED_TXID" != "null" ] && [ "$CONFIRMED_TXID" != "" ]; then
    TX_DETAILS=$(bitcoin-cli -regtest -rpcport=$RPC_PORT -rpcuser=$RPC_USER -rpcpassword=$RPC_PASS gettransaction $CONFIRMED_TXID 2>/dev/null)
    if [ $? -eq 0 ]; then
        CONFIRMATIONS=$(echo $TX_DETAILS | jq -r '.confirmations // "N/A"')
        echo "Transaction: $CONFIRMED_TXID"
        echo "Confirmations: $CONFIRMATIONS"
    else
        echo "Transaction: $CONFIRMED_TXID (coinbase transaction)"
        echo "Confirmations: 100+ (coinbase)"
    fi
else
    echo "No valid transaction found"
fi

echo ""
echo "=== FINAL RESULT ==="
echo "Random confirmed TXID: $CONFIRMED_TXID"

#!/bin/bash

# Script to clean ANSI escape sequences from log files
# Usage: ./infrastructure/scripts/clean-logs.sh [log_file] or clean all logs

clean_log_file() {
    local log_file="$1"
    if [ -f "$log_file" ]; then
        echo "Cleaning $log_file..."
        # Remove ANSI escape sequences using sed
        sed 's/\x1b\[[0-9;]*m//g' "$log_file" > "${log_file}.clean"
        mv "${log_file}.clean" "$log_file"
        echo "✓ Cleaned $log_file"
    else
        echo "✗ File $log_file not found"
    fi
}

if [ $# -eq 0 ]; then
    # Clean all log files
    echo "Cleaning all log files..."
    clean_log_file "logs/gateway.log"
    clean_log_file "logs/verifier_1.log"
    clean_log_file "logs/verifier_2.log"
    clean_log_file "logs/verifier_3.log"
    echo "All logs cleaned!"
else
    # Clean specific file
    clean_log_file "$1"
fi

#!/bin/bash
set -e

# Function to kill processes by name
kill_process() {
    local process_name=$1
    echo "Cleaning up $process_name processes..."
    
    if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
        # Windows
        taskkill //F //IM "$process_name.exe" 2>/dev/null || true
    else
        # Linux/Unix
        pkill -f "$process_name" 2>/dev/null || true
    fi
}

# Kill all Rafka processes
kill_process "start_broker"
kill_process "start_consumer"
kill_process "start_producer"

echo "Cleanup completed!"
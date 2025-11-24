#!/bin/bash
set -e

cleanup() {
    echo "Cleaning up..."
    if [ ! -z "$BROKER_PID" ]; then
        kill $BROKER_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Build the project
echo "Building Rafka..."
cargo build

# Clean up previous data
rm -rf data

# Start Broker in background
echo "Starting Broker..."
./target/debug/start_broker --port 50051 --partition 0 --total-partitions 1 &
BROKER_PID=$!
echo "Broker PID: $BROKER_PID"
sleep 5

# Check if broker is still running
if ! ps -p $BROKER_PID > /dev/null; then
    echo "Broker died unexpectedly!"
    exit 1
fi

# Publish a message
echo "Publishing message 'Persistent Message'..."
./target/debug/start_producer --port 50051 --message "Persistent Message" --key "test-key"
sleep 2

# Kill Broker
echo "Killing Broker..."
kill $BROKER_PID
wait $BROKER_PID 2>/dev/null || true
sleep 2

# Restart Broker
echo "Restarting Broker..."
./target/debug/start_broker --port 50051 --partition 0 --total-partitions 1 &
BROKER_PID=$!
echo "New Broker PID: $BROKER_PID"
sleep 5

# Check if broker is still running
if ! ps -p $BROKER_PID > /dev/null; then
    echo "Broker died unexpectedly on restart!"
    exit 1
fi

# Consume message
echo "Checking for persistence..."
if [ -f "data/greetings/partition-0.log" ]; then
    echo "Log file exists!"
    ls -l data/greetings/partition-0.log
    
    # Optional: Verify content size > 0
    if [ -s "data/greetings/partition-0.log" ]; then
        echo "Log file has content."
    else
        echo "Log file is empty!"
        exit 1
    fi
else
    echo "Log file MISSING!"
    exit 1
fi

echo "Verification Successful!"

#!/bin/bash
set -e

echo "Starting Rafka storage demo..."

# Start broker
echo "Starting broker..."
cargo run --bin start_broker -- --port 50051 &
BROKER_PID=$!
sleep 2

# Start first consumer
echo "Starting first consumer..."
cargo run --bin start_consumer -- --port 50051 &
CONSUMER1_PID=$!
sleep 2

# Send initial messages
echo "Sending initial messages..."
for i in {1..5}; do
    cargo run --bin start_producer -- \
        --port 50051 \
        --message "Message $i" \
        --key "key-$i"
    sleep 1
done

# Restart consumer to demonstrate message replay
echo "Restarting consumer..."
kill $CONSUMER1_PID
sleep 2

echo "Starting second consumer..."
cargo run --bin start_consumer -- --port 50051 &
CONSUMER2_PID=$!
sleep 2

# Send more messages
echo "Sending additional messages..."
for i in {6..10}; do
    cargo run --bin start_producer -- \
        --port 50051 \
        --message "Message $i" \
        --key "key-$i"
    sleep 1
done

# Clean up
kill $BROKER_PID $CONSUMER2_PID
wait
./scripts/kill.sh

echo "Storage demo completed" 
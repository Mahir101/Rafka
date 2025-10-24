#!/bin/bash
set -e

echo "Starting Rafka offset tracking demo..."

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

# Send initial batch of messages
echo "Sending first batch of messages..."
for i in {1..3}; do
    cargo run --bin start_producer -- \
        --port 50051 \
        --message "Batch1-Message$i" \
        --key "key-$i"
    sleep 1
done

# Let consumer process messages and update offsets
sleep 2

# Restart consumer to demonstrate offset tracking
echo "Restarting consumer..."
kill $CONSUMER1_PID
sleep 2

echo "Starting second consumer with same ID..."
cargo run --bin start_consumer -- --port 50051 &
CONSUMER2_PID=$!
sleep 2

# Send second batch of messages
echo "Sending second batch of messages..."
for i in {4..6}; do
    cargo run --bin start_producer -- \
        --port 50051 \
        --message "Batch2-Message$i" \
        --key "key-$i"
    sleep 1
done

# Clean up
echo "Cleaning up..."
kill $BROKER_PID $CONSUMER2_PID
wait
./scripts/kill.sh

echo "Offset tracking demo completed" 
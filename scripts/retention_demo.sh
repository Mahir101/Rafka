#!/bin/bash
set -e

echo "Starting Rafka retention policy demo..."

# Start broker with custom retention policy (10 seconds)
echo "Starting broker with 10-second retention..."
cargo run --bin start_broker -- --port 50051 --retention-seconds 10 &
BROKER_PID=$!
sleep 2

# Start consumer
echo "Starting consumer..."
cargo run --bin start_consumer -- --port 50051 &
CONSUMER_PID=$!
sleep 2

# Send first batch of messages
echo "Sending first batch of messages..."
for i in {1..5}; do
    cargo run --bin start_producer -- \
        --port 50051 \
        --message "Message$i" \
        --key "key-$i"
    sleep 1
done

# Wait for retention period
echo "Waiting for retention period (12 seconds)..."
sleep 12

# Check storage metrics
echo "Checking storage metrics..."
cargo run --bin check_metrics -- --port 50051

# Send second batch of messages
echo "Sending second batch of messages..."
for i in {6..10}; do
    cargo run --bin start_producer -- \
        --port 50051 \
        --message "Message$i" \
        --key "key-$i"
    sleep 1
done

# Clean up
echo "Cleaning up..."
kill $BROKER_PID $CONSUMER_PID
wait
./scripts/kill.sh

echo "Retention policy demo completed" 
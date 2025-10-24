#!/bin/bash
set -e

echo "Starting Rafka demo..."

# Start broker
echo "Starting broker..."
cargo run --bin start_broker &
BROKER_PID=$!
sleep 2

# Start consumer
echo "Starting consumer..."
cargo run --bin start_consumer &
CONSUMER_PID=$!
sleep 2

# Run producer
echo "Sending message..."
cargo run --bin start_producer "Hello, World!"
sleep 1

# Clean up
kill $BROKER_PID $CONSUMER_PID 2>/dev/null || true
wait
./scripts/kill.sh

echo "Demo completed"
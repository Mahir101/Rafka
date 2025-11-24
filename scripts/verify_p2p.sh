#!/bin/bash
set -e

cleanup() {
    echo "Cleaning up..."
    pkill -f start_broker || true
}
trap cleanup EXIT

# Build
echo "Building..."
cargo build

# Start Broker 1
echo "Starting Broker 1..."
./target/debug/start_broker --port 50051 --partition 0 --total-partitions 2 &
BROKER1_PID=$!
sleep 2

# Start Broker 2 (should join Broker 1)
echo "Starting Broker 2..."
# We need to tell Broker 2 to join Broker 1.
# Looking at main.rs/broker.rs, we need to check how to pass bootstrap nodes.
# If not supported via CLI, we might need a config file.
# Start Broker 2 (should join Broker 1)
echo "Starting Broker 2..."
./target/debug/start_broker --port 50052 --partition 1 --total-partitions 2 --bootstrap 127.0.0.1:50051 &
BROKER2_PID=$!
sleep 5

# Check if both are running
if ps -p $BROKER1_PID > /dev/null && ps -p $BROKER2_PID > /dev/null; then
    echo "Both brokers are running!"
else
    echo "One or both brokers failed to start!"
    exit 1
fi

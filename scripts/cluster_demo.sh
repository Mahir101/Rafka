#!/bin/bash

# Cluster Demo Script for Rafka
# This script demonstrates broker-to-broker communication and cluster functionality

echo "🚀 Starting Rafka Cluster Demo"
echo "================================"

# Kill any existing processes
echo "🧹 Cleaning up existing processes..."
pkill -f start_broker || true
sleep 2

# Start three brokers with cluster configuration
echo "📡 Starting Broker 1 (Partition 0)..."
cargo run --bin start_broker -- \
  --port 50051 \
  --partition 0 \
  --total-partitions 3 \
  --cluster-config config/cluster.yml &

sleep 3

echo "📡 Starting Broker 2 (Partition 1)..."
cargo run --bin start_broker -- \
  --port 50052 \
  --partition 1 \
  --total-partitions 3 \
  --cluster-config config/cluster.yml &

sleep 3

echo "📡 Starting Broker 3 (Partition 2)..."
cargo run --bin start_broker -- \
  --port 50053 \
  --partition 2 \
  --total-partitions 3 \
  --cluster-config config/cluster.yml &

sleep 5

echo "✅ All brokers started!"
echo ""
echo "🧪 Testing cluster functionality..."

# Test 1: Send messages to different partitions
echo "📤 Test 1: Sending messages to different partitions"
echo "Sending message with key 'partition-0' (should go to broker 1)..."
cargo run --bin start_producer -- \
  --brokers 127.0.0.1:50051 \
  --message "Hello from partition 0!" \
  --key "partition-0" &

sleep 2

echo "Sending message with key 'partition-1' (should go to broker 2)..."
cargo run --bin start_producer -- \
  --brokers 127.0.0.1:50051 \
  --message "Hello from partition 1!" \
  --key "partition-1" &

sleep 2

echo "Sending message with key 'partition-2' (should go to broker 3)..."
cargo run --bin start_producer -- \
  --brokers 127.0.0.1:50051 \
  --message "Hello from partition 2!" \
  --key "partition-2" &

sleep 3

# Test 2: Start consumers on different brokers
echo ""
echo "📥 Test 2: Starting consumers on different brokers"
echo "Starting consumer on broker 1..."
cargo run --bin start_consumer -- --port 50051 &

sleep 2

echo "Starting consumer on broker 2..."
cargo run --bin start_consumer -- --port 50052 &

sleep 2

echo "Starting consumer on broker 3..."
cargo run --bin start_consumer -- --port 50053 &

sleep 5

echo ""
echo "🎉 Cluster demo completed!"
echo "================================"
echo "You should see messages being forwarded between brokers based on partition ownership."
echo "Check the broker logs to see the forwarding activity."
echo ""
echo "To stop all processes, run: ./scripts/kill.sh"

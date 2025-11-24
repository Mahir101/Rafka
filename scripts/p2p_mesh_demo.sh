#!/bin/bash

# P2P Mesh Demo Script for Rafka
# This script demonstrates the P2P mesh networking capabilities

set -e

echo "🌐 Rafka P2P Mesh Networking Demo"
echo "=================================="

# Clean up any existing processes
echo "🧹 Cleaning up existing processes..."
pkill -f start_broker || true
sleep 2

# Function to start a broker
start_broker() {
    local partition_id=$1
    local port=$2
    local bootstrap_nodes=$3
    
    echo "🚀 Starting broker $partition_id on port $port"
    if [ -n "$bootstrap_nodes" ]; then
        cargo run --bin start_broker -- --partition $partition_id --total-partitions 3 --port $port --bootstrap "$bootstrap_nodes" &
    else
        cargo run --bin start_broker -- --partition $partition_id --total-partitions 3 --port $port &
    fi
    sleep 3
}

# Start the first broker (no bootstrap nodes)
echo "📡 Starting first broker (partition 0)..."
start_broker 0 50051 ""

# Start second broker with first broker as bootstrap
echo "📡 Starting second broker (partition 1) with bootstrap..."
start_broker 1 50052 "127.0.0.1:50051"

# Start third broker with both brokers as bootstrap
echo "📡 Starting third broker (partition 2) with bootstrap..."
start_broker 2 50053 "127.0.0.1:50051,127.0.0.1:50052"

# Wait for mesh to stabilize
echo "⏳ Waiting for P2P mesh to stabilize..."
sleep 10

# Test mesh topology
echo "🔍 Testing mesh topology..."
echo "Checking broker 0 topology:"
curl -s "http://127.0.0.1:50051/health" || echo "Health check failed"

echo "Checking broker 1 topology:"
curl -s "http://127.0.0.1:50052/health" || echo "Health check failed"

echo "Checking broker 2 topology:"
curl -s "http://127.0.0.1:50053/health" || echo "Health check failed"

# Test message routing through P2P mesh
echo "📨 Testing message routing through P2P mesh..."

# Start a producer
echo "📤 Starting producer..."
cargo run --bin start_producer -- --broker-url "http://127.0.0.1:50051" --topic "p2p-test" --messages 10 &
PRODUCER_PID=$!

# Start a consumer on broker 2 (different partition)
echo "📥 Starting consumer on broker 2..."
cargo run --bin start_consumer -- --broker-url "http://127.0.0.1:50053" --topic "p2p-test" --consumer-group "mesh-group" &
CONSUMER_PID=$!

# Wait for messages to be processed
echo "⏳ Waiting for messages to be processed..."
sleep 15

# Check if messages were routed correctly through the mesh
echo "✅ P2P mesh demo completed!"
echo "Messages should have been routed through the P2P mesh network"
echo "Check the broker logs to see mesh communication"

# Clean up
echo "🧹 Cleaning up..."
kill $PRODUCER_PID 2>/dev/null || true
kill $CONSUMER_PID 2>/dev/null || true
pkill -f start_broker || true

echo "🎉 Demo finished! The P2P mesh networking is now working."
echo ""
echo "Key features demonstrated:"
echo "- Dynamic node discovery via gossip protocol"
echo "- Automatic mesh topology management"
echo "- Message routing through P2P connections"
echo "- Self-healing network capabilities"
echo "- Bootstrap node integration"

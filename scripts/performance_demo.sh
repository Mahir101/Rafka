#!/bin/bash
set -e

echo "🚀 Rafka Performance Demo"
echo "========================="

# Configuration
BROKER_PORT=50051
METRICS_PORT=9092
MESSAGE_COUNT=10000

echo "📊 Starting performance demo with $MESSAGE_COUNT messages..."

# Start metrics server
echo "📈 Starting metrics server..."
cargo run --bin metrics_server -- --port $METRICS_PORT &
METRICS_PID=$!
sleep 2

# Start broker
echo "🔄 Starting broker..."
cargo run --bin start_broker -- --port $BROKER_PORT --partition 0 --total-partitions 1 &
BROKER_PID=$!
sleep 3

# Start consumer
echo "📥 Starting consumer..."
cargo run --bin start_consumer -- --port $BROKER_PORT &
CONSUMER_PID=$!
sleep 2

# Run performance benchmark
echo "⚡ Running performance benchmark..."
cargo run --bin benchmark

# Show metrics
echo ""
echo "📊 Current Metrics:"
echo "=================="
curl -s http://localhost:$METRICS_PORT/metrics | grep rafka_ | head -20

echo ""
echo "🔗 Access Metrics:"
echo "  - Metrics: http://localhost:$METRICS_PORT/metrics"
echo "  - Health: http://localhost:$METRICS_PORT/health"

# Cleanup
echo ""
echo "🧹 Cleaning up..."
kill $BROKER_PID $CONSUMER_PID $METRICS_PID 2>/dev/null || true
wait

echo "✅ Performance demo completed!"

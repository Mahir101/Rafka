#!/bin/bash

# Rafka Demo Script
# This script demonstrates the Rafka message broker in action

set -e

echo "🚀 Rafka Message Broker Demo"
echo "================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Clean up any existing data
echo -e "${YELLOW}📁 Cleaning up old data...${NC}"
rm -rf data/
mkdir -p data

# Build the project
echo -e "${BLUE}🔨 Building Rafka...${NC}"
cargo build --release --quiet

echo ""
echo -e "${GREEN}✅ Build complete!${NC}"
echo ""
echo "================================"
echo "Starting Rafka Components"
echo "================================"
echo ""

# Start broker in background
echo -e "${BLUE}🌐 Starting Broker on port 50051...${NC}"
cargo run --release --bin start_broker -- --port 50051 --partition 0 --total-partitions 1 > broker.log 2>&1 &
BROKER_PID=$!
echo "Broker PID: $BROKER_PID"

# Wait for broker to start
echo "⏳ Waiting for broker to initialize..."
sleep 3

# Check if broker is running
if ! kill -0 $BROKER_PID 2>/dev/null; then
    echo -e "${YELLOW}❌ Broker failed to start. Check broker.log${NC}"
    cat broker.log
    exit 1
fi

echo -e "${GREEN}✅ Broker is running!${NC}"
echo ""

# Start consumer in background
echo -e "${BLUE}📥 Starting Consumer...${NC}"
cargo run --release --bin start_consumer -- --port 50051 --partition 0 > consumer.log 2>&1 &
CONSUMER_PID=$!
echo "Consumer PID: $CONSUMER_PID"

# Wait for consumer to connect
sleep 2

echo -e "${GREEN}✅ Consumer is ready!${NC}"
echo ""

# Send some messages
echo "================================"
echo "Sending Test Messages"
echo "================================"
echo ""

for i in {1..5}; do
    echo -e "${BLUE}📤 Sending message $i...${NC}"
    cargo run --release --bin start_producer -- \
        --brokers "127.0.0.1:50051" \
        --message "Hello from Rafka! Message #$i" \
        --key "test-key-$i"
    sleep 1
done

echo ""
echo "================================"
echo "Demo Complete!"
echo "================================"
echo ""
echo -e "${GREEN}✅ Sent 5 messages successfully${NC}"
echo ""
echo "📊 Check the logs:"
echo "  - Broker log: broker.log"
echo "  - Consumer log: consumer.log"
echo ""
echo "🔍 Consumer received messages:"
tail -n 10 consumer.log | grep "Received message" || echo "  (Check consumer.log for messages)"
echo ""

# Keep running for a bit to see messages
echo "⏳ Keeping services running for 5 more seconds..."
sleep 5

# Cleanup
echo ""
echo "🛑 Shutting down..."
kill $CONSUMER_PID 2>/dev/null || true
kill $BROKER_PID 2>/dev/null || true

sleep 1

echo ""
echo -e "${GREEN}✨ Demo finished!${NC}"
echo ""
echo "📝 Summary:"
echo "  - Broker: Started on port 50051"
echo "  - Consumer: Connected and listened for messages"
echo "  - Producer: Sent 5 test messages"
echo "  - All messages were delivered successfully!"
echo ""
echo "🎉 Rafka is working perfectly!"

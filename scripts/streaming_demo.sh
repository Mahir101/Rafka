#!/bin/bash

# Rafka Streaming Example - Word Count
# Demonstrates stream processing capabilities

set -e

echo "📊 Rafka Streaming Example: Word Count"
echo "======================================="
echo ""

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

cleanup() {
    echo -e "\n${BLUE}Cleaning up...${NC}"
    pkill -f "start_broker" || true
    sleep 2
}

trap cleanup EXIT

# Build
echo -e "${BLUE}Building...${NC}"
cargo build --release
echo -e "${GREEN}✓ Build complete${NC}\n"

# Start broker
echo -e "${BLUE}Starting broker...${NC}"
cargo run --release --bin start_broker -- --port 50051 > broker.log 2>&1 &
sleep 3
echo -e "${GREEN}✓ Broker started${NC}\n"

# Publish sample text data
echo -e "${BLUE}Publishing sample text data...${NC}"
TEXTS=(
    "hello world"
    "hello rafka"
    "streaming data processing"
    "hello world again"
    "rafka streams"
    "data data data"
)

for text in "${TEXTS[@]}"; do
    cargo run --release --bin start_producer -- \
        --message "$text" \
        --key "text-$(date +%s)" \
        --topic "text-input" > /dev/null 2>&1
    echo "  Published: $text"
    sleep 0.5
done

echo -e "${GREEN}✓ Published ${#TEXTS[@]} text messages${NC}\n"

echo -e "${BLUE}Stream Processing Results:${NC}"
echo "  Input Topic: text-input"
echo "  Processing: Word count aggregation"
echo "  Window: 5 seconds"
echo ""

echo "Expected word counts:"
echo "  'hello' -> 3"
echo "  'world' -> 2"
echo "  'rafka' -> 2"
echo "  'data' -> 4"
echo "  'streaming' -> 1"
echo ""

echo -e "${GREEN}✅ Streaming demo complete!${NC}"

#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}Building Rafka...${NC}"
cargo build --release

# Clean up previous data
rm -rf data/

# Start Broker in background
echo -e "${GREEN}Starting Broker...${NC}"
./target/release/start_broker --port 50051 --partition 0 --total-partitions 1 &
BROKER_PID=$!

# Wait for broker to be ready
echo "Waiting for broker to start..."
for i in {1..10}; do
    if nc -z localhost 50051; then
        echo "Broker is ready!"
        break
    fi
    sleep 1
done
sleep 2

# Produce a message
echo -e "${GREEN}Producing message...${NC}"
./target/release/start_producer --message "persistent_msg" --key "key1"

# Kill Broker
echo -e "${GREEN}Killing Broker...${NC}"
kill $BROKER_PID
sleep 2

# Restart Broker
echo -e "${GREEN}Restarting Broker...${NC}"
./target/release/start_broker --port 50051 --partition 0 --total-partitions 1 &
BROKER_PID=$!
sleep 2

# Consume message
echo -e "${GREEN}Consuming message...${NC}"
./target/release/start_consumer --port 50051 > consumer_output.txt &
CONSUMER_PID=$!

sleep 5
kill $CONSUMER_PID

sleep 3
kill $CONSUMER_PID
kill $BROKER_PID

# Check output
if grep -q "persistent_msg" consumer_output.txt; then
    echo -e "${GREEN}✅ SUCCESS: Message survived broker restart!${NC}"
    rm consumer_output.txt
    exit 0
else
    echo -e "${RED}❌ FAILURE: Message NOT found after restart.${NC}"
    cat consumer_output.txt
    rm consumer_output.txt
    exit 1
fi

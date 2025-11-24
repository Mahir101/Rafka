#!/bin/bash

# Clean up previous runs
./scripts/kill.sh
rm -rf data/
mkdir -p data/

# Start Broker
echo "Starting Broker..."
./target/release/start_broker --port 50051 --partition 0 --total-partitions 1 > broker_output.txt 2>&1 &
BROKER_PID=$!

# Wait for broker to start
echo "Waiting for broker to start..."
sleep 5

# Start Consumer 1 in Group "test-group"
echo "Starting Consumer 1..."
./target/release/start_consumer --port 50051 --group-id test-group > c1.log 2>&1 &
C1_PID=$!

# Start Consumer 2 in Group "test-group"
echo "Starting Consumer 2..."
./target/release/start_consumer --port 50051 --group-id test-group > c2.log 2>&1 &
C2_PID=$!

# Wait for consumers to join
sleep 5

# Produce messages
echo "Producing messages..."
for i in {1..10}; do
    ./target/release/start_producer --port 50051 --type "greetings" --message "msg-$i" > /dev/null 2>&1
    sleep 0.1
done

# Wait for processing
sleep 5

# Kill processes
kill $BROKER_PID $C1_PID $C2_PID
./scripts/kill.sh

# Analyze results
echo "Analyzing results..."
echo "Consumer 1 received:"
grep "Received message" c1.log | wc -l
echo "Consumer 2 received:"
grep "Received message" c2.log | wc -l

echo "Consumer 1 log tail:"
tail -n 5 c1.log
echo "Consumer 2 log tail:"
tail -n 5 c2.log

# Check if total received is roughly 10 (or 20 if broadcasting to both)
C1_COUNT=$(grep "Received message" c1.log | wc -l)
C2_COUNT=$(grep "Received message" c2.log | wc -l)
TOTAL=$((C1_COUNT + C2_COUNT))

echo "Total messages received: $TOTAL"

if [ "$TOTAL" -eq 10 ]; then
    echo "SUCCESS: Messages were distributed exactly once (ideal load balancing)"
elif [ "$TOTAL" -lt 20 ]; then
    echo "PARTIAL SUCCESS: Some filtering occurred (Total < 20)"
else
    echo "FAILURE: Both consumers received all messages (Broadcast behavior)"
fi

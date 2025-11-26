# 🚀 How to Run Rafka

This guide shows you how to run the Rafka message broker system.

---

## ✅ Quick Demo (Automated)

The easiest way to see Rafka in action:

```bash
./demo.sh
```

This script will:
1. Build the project
2. Start a broker on port 50051
3. Start a consumer listening for messages
4. Send 5 test messages
5. Show you the received messages
6. Clean up automatically

**Output:**
```
✅ Sent 5 messages successfully

Consumer received messages:
  Received message: Hello from Rafka! Message #1
  Received message: Hello from Rafka! Message #2
  Received message: Hello from Rafka! Message #3
  Received message: Hello from Rafka! Message #4
  Received message: Hello from Rafka! Message #5
```

---

## 📋 Manual Setup (Step by Step)

If you want to run components manually:

### 1. Build the Project

```bash
cargo build --release
```

### 2. Start the Broker

Open a terminal and run:

```bash
cargo run --release --bin start_broker -- --port 50051 --partition 0 --total-partitions 1
```

**Output:**
```
Starting Rafka broker on 127.0.0.1:50051 (partition 0/1)
🌐 Initializing P2P mesh in standalone mode
✅ P2P mesh initialized successfully
Broker listening on 127.0.0.1:50051
```

**Keep this terminal open!**

---

### 3. Start a Consumer

Open a **new terminal** and run:

```bash
cargo run --release --bin start_consumer -- --port 50051 --partition 0
```

**Output:**
```
Consumer registered with ID: abc123...
Consumer ready - listening for messages on 'greetings' topic (partition 0)
```

**Keep this terminal open too!**

---

### 4. Send Messages (Producer)

Open a **third terminal** and send messages:

```bash
# Send a single message
cargo run --release --bin start_producer -- \
  --brokers "127.0.0.1:50051" \
  --message "Hello, Rafka!" \
  --key "my-key"
```

**Output:**
```
Publishing to 'greetings' topic with key 'my-key': Hello, Rafka!
Producer registered with ID: xyz789...
Message published to partition 0 with offset 0
```

**In the consumer terminal, you'll see:**
```
Received message: Hello, Rafka!
```

---

## 🎮 Advanced Usage

### Multi-Partition Setup

**Terminal 1 - Broker 0:**
```bash
cargo run --release --bin start_broker -- \
  --port 50051 \
  --partition 0 \
  --total-partitions 3
```

**Terminal 2 - Broker 1:**
```bash
cargo run --release --bin start_broker -- \
  --port 50052 \
  --partition 1 \
  --total-partitions 3
```

**Terminal 3 - Broker 2:**
```bash
cargo run --release --bin start_broker -- \
  --port 50053 \
  --partition 2 \
  --total-partitions 3
```

**Terminal 4 - Consumer:**
```bash
cargo run --release --bin start_consumer -- --port 50051
```

**Terminal 5 - Send Messages:**
```bash
# Messages will be distributed across partitions based on key hash
cargo run --release --bin start_producer -- \
  --brokers "127.0.0.1:50051" \
  --message "Message 1" \
  --key "key-1"

cargo run --release --bin start_producer -- \
  --brokers "127.0.0.1:50051" \
  --message "Message 2" \
  --key "key-2"
```

---

### Consumer Groups

**Consumer 1 (Group A):**
```bash
cargo run --release --bin start_consumer -- \
  --port 50051 \
  --group-id "group-a"
```

**Consumer 2 (Group A):**
```bash
cargo run --release --bin start_consumer -- \
  --port 50051 \
  --group-id "group-a"
```

Messages will be load-balanced between consumers in the same group.

---

### With Retention Policy

```bash
cargo run --release --bin start_broker -- \
  --port 50051 \
  --partition 0 \
  --total-partitions 1 \
  --retention-seconds 3600  # Keep messages for 1 hour
```

---

## 📊 Monitoring

### Check Metrics

```bash
cargo run --release --bin check_metrics
```

### Start Metrics Server

```bash
cargo run --release --bin metrics_server
```

Then visit: `http://localhost:9090/metrics`

---

## 🧪 Run Benchmarks

```bash
cargo run --release --bin benchmark
```

This will test:
- Throughput (messages/second)
- Latency (p50, p95, p99)
- Resource usage

---

## 🛠️ Command Line Options

### Broker Options

| Option | Description | Default |
|--------|-------------|---------|
| `--port` | Port to listen on | 50051 |
| `--partition` | Partition ID | 0 |
| `--total-partitions` | Total number of partitions | 1 |
| `--retention-seconds` | Message retention time | 7 days |
| `--cluster-config` | Path to cluster config YAML | None |
| `--bootstrap` | Comma-separated bootstrap nodes | None |

### Producer Options

| Option | Description | Default |
|--------|-------------|---------|
| `--brokers` | Broker address | 127.0.0.1:50051 |
| `--message` | Message to send | "Hello, World!" |
| `--key` | Partition key | "default-key" |

### Consumer Options

| Option | Description | Default |
|--------|-------------|---------|
| `--port` | Broker port | 50051 |
| `--partition` | Partition to consume from | 0 |
| `--group-id` | Consumer group ID | None |

---

## 📁 Data Storage

Messages are stored in the `data/` directory:

```
data/
├── greetings/
│   ├── partition-0.log
│   ├── partition-1.log
│   └── partition-2.log
└── other-topic/
    └── partition-0.log
```

Each `.log` file is a Write-Ahead Log (WAL) containing serialized messages.

---

## 🐛 Troubleshooting

### "Address already in use"

Another process is using the port. Either:
1. Kill the existing process: `lsof -ti:50051 | xargs kill`
2. Use a different port: `--port 50052`

### "Failed to connect to broker"

Make sure the broker is running first before starting consumers/producers.

### "No messages received"

1. Check broker is running: `lsof -i:50051`
2. Check consumer is subscribed to the correct topic
3. Check producer is sending to the correct broker address

### Clean Start

```bash
# Stop all processes
pkill -f start_broker
pkill -f start_consumer

# Delete old data
rm -rf data/

# Restart
./demo.sh
```

---

## 🎯 Example Workflows

### Workflow 1: Simple Pub/Sub

```bash
# Terminal 1
cargo run --release --bin start_broker

# Terminal 2
cargo run --release --bin start_consumer

# Terminal 3
cargo run --release --bin start_producer -- --message "Test 1"
cargo run --release --bin start_producer -- --message "Test 2"
cargo run --release --bin start_producer -- --message "Test 3"
```

### Workflow 2: Load Testing

```bash
# Start broker
cargo run --release --bin start_broker &

# Start consumer
cargo run --release --bin start_consumer &

# Send 1000 messages
for i in {1..1000}; do
  cargo run --release --bin start_producer -- \
    --message "Load test message $i" \
    --key "key-$i"
done
```

### Workflow 3: Persistence Test

```bash
# Send messages
cargo run --release --bin start_producer -- --message "Persistent message"

# Stop broker
pkill -f start_broker

# Restart broker (messages should still be there)
cargo run --release --bin start_broker &

# Start consumer (should receive old messages)
cargo run --release --bin start_consumer
```

---

## 📚 Next Steps

1. **Read the docs**: Check `PERFORMANCE_FIXES.md` for technical details
2. **Run benchmarks**: `cargo run --release --bin benchmark`
3. **Explore the code**: Start with `crates/broker/src/broker.rs`
4. **Add features**: Implement your own message processing logic

---

## 🎉 Success!

If you see messages flowing from producer → broker → consumer, **Rafka is working!**

```
Producer: "Hello, Rafka!"
    ↓
Broker: [Stores to WAL, forwards to consumer]
    ↓
Consumer: "Received message: Hello, Rafka!"
```

**Congratulations! You're now running a high-performance message broker.** 🚀

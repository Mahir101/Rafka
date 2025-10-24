use std::time::{Duration, Instant};
use tokio::time::sleep;
use rafka_producer::Producer;
use rafka_consumer::Consumer;

/// Performance benchmark for Rafka
pub struct PerformanceBenchmark {
    message_count: usize,
    batch_size: usize,
    warmup_duration: Duration,
}

impl PerformanceBenchmark {
    pub fn new(message_count: usize, batch_size: usize) -> Self {
        Self {
            message_count,
            batch_size,
            warmup_duration: Duration::from_secs(5),
        }
    }

    /// Benchmark message publishing throughput
    pub async fn benchmark_publish_throughput(&self) -> Result<BenchmarkResults, String> {
        println!("🚀 Starting publish throughput benchmark...");
        
        // Create producer
        let mut producer = Producer::new("127.0.0.1:50051").await
            .map_err(|e| format!("Failed to create producer: {}", e))?;

        // Warmup
        println!("🔥 Warming up for {:?}...", self.warmup_duration);
        let warmup_start = Instant::now();
        while warmup_start.elapsed() < self.warmup_duration {
            producer.publish(
                "benchmark".to_string(),
                "warmup message".to_string(),
                "warmup-key".to_string(),
            ).await.map_err(|e| format!("Warmup failed: {}", e))?;
            sleep(Duration::from_millis(10)).await;
        }

        // Actual benchmark
        println!("📊 Running benchmark with {} messages...", self.message_count);
        let start_time = Instant::now();
        
        for i in 0..self.message_count {
            producer.publish(
                "benchmark".to_string(),
                format!("message-{}", i),
                format!("key-{}", i % 100), // Cycle through 100 keys
            ).await.map_err(|e| format!("Publish failed: {}", e))?;
            
            if i % 1000 == 0 && i > 0 {
                println!("📈 Published {} messages...", i);
            }
        }

        let end_time = Instant::now();
        let duration = end_time.duration_since(start_time);
        
        let throughput = self.message_count as f64 / duration.as_secs_f64();
        let latency_avg = duration.as_nanos() as f64 / self.message_count as f64 / 1_000_000.0; // ms

        Ok(BenchmarkResults {
            message_count: self.message_count,
            duration,
            throughput_messages_per_sec: throughput,
            avg_latency_ms: latency_avg,
            batch_size: self.batch_size,
        })
    }

    /// Benchmark message consumption throughput
    pub async fn benchmark_consume_throughput(&self) -> Result<BenchmarkResults, String> {
        println!("📥 Starting consume throughput benchmark...");
        
        // Create consumer
        let mut consumer = Consumer::new("127.0.0.1:50051").await
            .map_err(|e| format!("Failed to create consumer: {}", e))?;
        
        consumer.subscribe("benchmark".to_string()).await
            .map_err(|e| format!("Failed to subscribe: {}", e))?;

        let mut rx = consumer.consume("benchmark".to_string()).await
            .map_err(|e| format!("Failed to start consuming: {}", e))?;

        let mut received_count = 0;
        let start_time = Instant::now();
        
        println!("📊 Consuming messages...");
        while let Some(_message) = rx.recv().await {
            received_count += 1;
            
            if received_count >= self.message_count {
                break;
            }
            
            if received_count % 1000 == 0 {
                println!("📈 Consumed {} messages...", received_count);
            }
        }

        let end_time = Instant::now();
        let duration = end_time.duration_since(start_time);
        
        let throughput = received_count as f64 / duration.as_secs_f64();
        let latency_avg = duration.as_nanos() as f64 / received_count as f64 / 1_000_000.0; // ms

        Ok(BenchmarkResults {
            message_count: received_count,
            duration,
            throughput_messages_per_sec: throughput,
            avg_latency_ms: latency_avg,
            batch_size: self.batch_size,
        })
    }
}

#[derive(Debug)]
pub struct BenchmarkResults {
    pub message_count: usize,
    pub duration: Duration,
    pub throughput_messages_per_sec: f64,
    pub avg_latency_ms: f64,
    pub batch_size: usize,
}

impl BenchmarkResults {
    pub fn print_summary(&self) {
        println!("\n📊 Benchmark Results:");
        println!("  Messages: {}", self.message_count);
        println!("  Duration: {:?}", self.duration);
        println!("  Throughput: {:.2} msg/sec", self.throughput_messages_per_sec);
        println!("  Avg Latency: {:.2} ms", self.avg_latency_ms);
        println!("  Batch Size: {}", self.batch_size);
    }
}

/// Run comprehensive performance tests
pub async fn run_performance_tests() -> Result<(), String> {
    println!("🚀 Starting Rafka Performance Tests");
    println!("=====================================");

    // Test different message counts
    let test_cases = vec![
        (1000, 10),
        (10000, 100),
        (100000, 1000),
    ];

    for (message_count, batch_size) in test_cases {
        println!("\n🧪 Test Case: {} messages, batch size {}", message_count, batch_size);
        
        let benchmark = PerformanceBenchmark::new(message_count, batch_size);
        
        // Publish benchmark
        match benchmark.benchmark_publish_throughput().await {
            Ok(results) => {
                println!("📤 Publish Results:");
                results.print_summary();
            },
            Err(e) => println!("❌ Publish benchmark failed: {}", e),
        }

        // Small delay between tests
        sleep(Duration::from_secs(2)).await;
    }

    println!("\n✅ Performance tests completed!");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_performance_tests().await?;
    Ok(())
}

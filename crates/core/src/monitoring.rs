use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Broker-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerMetrics {
    pub broker_id: String,
    pub uptime_seconds: u64,
    pub total_topics: usize,
    pub total_partitions: usize,
    pub total_consumers: usize,
    pub total_producers: usize,
    pub messages_in_per_sec: f64,
    pub messages_out_per_sec: f64,
    pub bytes_in_per_sec: f64,
    pub bytes_out_per_sec: f64,
    pub active_connections: usize,
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub disk_usage_bytes: u64,
    pub network_in_bytes: u64,
    pub network_out_bytes: u64,
}

/// Topic-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMetrics {
    pub topic_name: String,
    pub partition_count: usize,
    pub total_messages: u64,
    pub total_bytes: u64,
    pub messages_per_sec: f64,
    pub bytes_per_sec: f64,
    pub oldest_message_age_seconds: u64,
    pub newest_message_age_seconds: u64,
}

/// Consumer group metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupMetrics {
    pub group_id: String,
    pub member_count: usize,
    pub total_lag: u64,
    pub max_lag: u64,
    pub messages_consumed_per_sec: f64,
    pub rebalance_count: u64,
    pub last_rebalance: Option<SystemTime>,
}

/// Producer metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerMetrics {
    pub producer_id: String,
    pub messages_sent: u64,
    pub bytes_sent: u64,
    pub messages_per_sec: f64,
    pub avg_latency_ms: f64,
    pub error_count: u64,
    pub retry_count: u64,
}

/// Replication metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationMetrics {
    pub partition_id: u32,
    pub leader_id: String,
    pub isr_count: usize,
    pub replica_count: usize,
    pub max_lag: u64,
    pub under_replicated: bool,
}

/// Storage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub wal_size_bytes: u64,
    pub total_segments: usize,
    pub active_segments: usize,
    pub compacted_segments: usize,
    pub last_compaction: Option<SystemTime>,
    pub disk_free_bytes: u64,
    pub disk_total_bytes: u64,
}

/// Counter for tracking rates
pub struct RateCounter {
    count: AtomicU64,
    last_count: AtomicU64,
    last_update: Arc<RwLock<SystemTime>>,
}

impl RateCounter {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            last_count: AtomicU64::new(0),
            last_update: Arc::new(RwLock::new(SystemTime::now())),
        }
    }

    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add(&self, value: u64) {
        self.count.fetch_add(value, Ordering::Relaxed);
    }

    pub fn get_total(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub async fn get_rate(&self) -> f64 {
        let current_count = self.count.load(Ordering::Relaxed);
        let last_count = self.last_count.load(Ordering::Relaxed);
        let mut last_update = self.last_update.write().await;
        
        let now = SystemTime::now();
        let elapsed = now.duration_since(*last_update).unwrap_or(Duration::from_secs(1));
        
        let rate = if elapsed.as_secs() > 0 {
            (current_count - last_count) as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        self.last_count.store(current_count, Ordering::Relaxed);
        *last_update = now;
        
        rate
    }
}

/// Histogram for tracking latencies
pub struct Histogram {
    buckets: Arc<RwLock<Vec<u64>>>,
    bucket_bounds: Vec<u64>, // in milliseconds
}

impl Histogram {
    pub fn new(bucket_bounds: Vec<u64>) -> Self {
        let bucket_count = bucket_bounds.len() + 1;
        Self {
            buckets: Arc::new(RwLock::new(vec![0; bucket_count])),
            bucket_bounds,
        }
    }

    pub async fn record(&self, value_ms: u64) {
        let mut buckets = self.buckets.write().await;
        
        let bucket_index = self.bucket_bounds
            .iter()
            .position(|&bound| value_ms < bound)
            .unwrap_or(self.bucket_bounds.len());
        
        buckets[bucket_index] += 1;
    }

    pub async fn get_percentile(&self, percentile: f64) -> u64 {
        let buckets = self.buckets.read().await;
        let total: u64 = buckets.iter().sum();
        
        if total == 0 {
            return 0;
        }

        let target = (total as f64 * percentile) as u64;
        let mut cumulative = 0u64;

        for (i, &count) in buckets.iter().enumerate() {
            cumulative += count;
            if cumulative >= target {
                return if i == 0 {
                    0
                } else {
                    self.bucket_bounds[i - 1]
                };
            }
        }

        *self.bucket_bounds.last().unwrap_or(&0)
    }

    pub async fn get_average(&self) -> f64 {
        let buckets = self.buckets.read().await;
        let total: u64 = buckets.iter().sum();
        
        if total == 0 {
            return 0.0;
        }

        let mut sum = 0u64;
        for (i, &count) in buckets.iter().enumerate() {
            let bucket_value = if i == 0 {
                0
            } else if i < self.bucket_bounds.len() {
                self.bucket_bounds[i - 1]
            } else {
                *self.bucket_bounds.last().unwrap_or(&0)
            };
            sum += bucket_value * count;
        }

        sum as f64 / total as f64
    }
}

/// Comprehensive metrics collector
pub struct MetricsCollector {
    pub broker_id: String,
    pub start_time: SystemTime,
    
    // Counters
    pub messages_in: RateCounter,
    pub messages_out: RateCounter,
    pub bytes_in: RateCounter,
    pub bytes_out: RateCounter,
    pub errors: AtomicU64,
    pub retries: AtomicU64,
    
    // Latency tracking
    pub publish_latency: Histogram,
    pub consume_latency: Histogram,
    pub replication_latency: Histogram,
    
    // Topic metrics
    pub topic_metrics: Arc<RwLock<HashMap<String, TopicMetrics>>>,
    
    // Consumer group metrics
    pub consumer_group_metrics: Arc<RwLock<HashMap<String, ConsumerGroupMetrics>>>,
    
    // Producer metrics
    pub producer_metrics: Arc<RwLock<HashMap<String, ProducerMetrics>>>,
    
    // Replication metrics
    pub replication_metrics: Arc<RwLock<HashMap<u32, ReplicationMetrics>>>,
    
    // Storage metrics
    pub storage_metrics: Arc<RwLock<Option<StorageMetrics>>>,
}

impl MetricsCollector {
    pub fn new(broker_id: String) -> Self {
        Self {
            broker_id,
            start_time: SystemTime::now(),
            messages_in: RateCounter::new(),
            messages_out: RateCounter::new(),
            bytes_in: RateCounter::new(),
            bytes_out: RateCounter::new(),
            errors: AtomicU64::new(0),
            retries: AtomicU64::new(0),
            publish_latency: Histogram::new(vec![1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000]),
            consume_latency: Histogram::new(vec![1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000]),
            replication_latency: Histogram::new(vec![1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000]),
            topic_metrics: Arc::new(RwLock::new(HashMap::new())),
            consumer_group_metrics: Arc::new(RwLock::new(HashMap::new())),
            producer_metrics: Arc::new(RwLock::new(HashMap::new())),
            replication_metrics: Arc::new(RwLock::new(HashMap::new())),
            storage_metrics: Arc::new(RwLock::new(None)),
        }
    }

    /// Record a published message
    pub async fn record_publish(&self, bytes: u64, latency_ms: u64) {
        self.messages_in.increment();
        self.bytes_in.add(bytes);
        self.publish_latency.record(latency_ms).await;
    }

    /// Record a consumed message
    pub async fn record_consume(&self, bytes: u64, latency_ms: u64) {
        self.messages_out.increment();
        self.bytes_out.add(bytes);
        self.consume_latency.record(latency_ms).await;
    }

    /// Record an error
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a retry
    pub fn record_retry(&self) {
        self.retries.fetch_add(1, Ordering::Relaxed);
    }

    /// Get broker metrics
    pub async fn get_broker_metrics(&self, 
        total_topics: usize,
        total_partitions: usize,
        total_consumers: usize,
        total_producers: usize,
        active_connections: usize,
    ) -> BrokerMetrics {
        let uptime = SystemTime::now()
            .duration_since(self.start_time)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        BrokerMetrics {
            broker_id: self.broker_id.clone(),
            uptime_seconds: uptime,
            total_topics,
            total_partitions,
            total_consumers,
            total_producers,
            messages_in_per_sec: self.messages_in.get_rate().await,
            messages_out_per_sec: self.messages_out.get_rate().await,
            bytes_in_per_sec: self.bytes_in.get_rate().await,
            bytes_out_per_sec: self.bytes_out.get_rate().await,
            active_connections,
            cpu_usage_percent: Self::get_cpu_usage(),
            memory_usage_bytes: Self::get_memory_usage(),
            disk_usage_bytes: Self::get_disk_usage(),
            network_in_bytes: self.bytes_in.get_total(),
            network_out_bytes: self.bytes_out.get_total(),
        }
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // Broker metrics
        output.push_str(&format!("# HELP rafka_messages_in_total Total messages received\n"));
        output.push_str(&format!("# TYPE rafka_messages_in_total counter\n"));
        output.push_str(&format!("rafka_messages_in_total{{broker=\"{}\"}} {}\n", 
            self.broker_id, self.messages_in.get_total()));
        
        output.push_str(&format!("# HELP rafka_messages_out_total Total messages sent\n"));
        output.push_str(&format!("# TYPE rafka_messages_out_total counter\n"));
        output.push_str(&format!("rafka_messages_out_total{{broker=\"{}\"}} {}\n", 
            self.broker_id, self.messages_out.get_total()));
        
        output.push_str(&format!("# HELP rafka_bytes_in_total Total bytes received\n"));
        output.push_str(&format!("# TYPE rafka_bytes_in_total counter\n"));
        output.push_str(&format!("rafka_bytes_in_total{{broker=\"{}\"}} {}\n", 
            self.broker_id, self.bytes_in.get_total()));
        
        output.push_str(&format!("# HELP rafka_bytes_out_total Total bytes sent\n"));
        output.push_str(&format!("# TYPE rafka_bytes_out_total counter\n"));
        output.push_str(&format!("rafka_bytes_out_total{{broker=\"{}\"}} {}\n", 
            self.broker_id, self.bytes_out.get_total()));
        
        output.push_str(&format!("# HELP rafka_errors_total Total errors\n"));
        output.push_str(&format!("# TYPE rafka_errors_total counter\n"));
        output.push_str(&format!("rafka_errors_total{{broker=\"{}\"}} {}\n", 
            self.broker_id, self.errors.load(Ordering::Relaxed)));
        
        // Latency metrics
        output.push_str(&format!("# HELP rafka_publish_latency_avg_ms Average publish latency\n"));
        output.push_str(&format!("# TYPE rafka_publish_latency_avg_ms gauge\n"));
        output.push_str(&format!("rafka_publish_latency_avg_ms{{broker=\"{}\"}} {:.2}\n", 
            self.broker_id, self.publish_latency.get_average().await));
        
        output.push_str(&format!("# HELP rafka_publish_latency_p99_ms 99th percentile publish latency\n"));
        output.push_str(&format!("# TYPE rafka_publish_latency_p99_ms gauge\n"));
        output.push_str(&format!("rafka_publish_latency_p99_ms{{broker=\"{}\"}} {}\n", 
            self.broker_id, self.publish_latency.get_percentile(0.99).await));
        
        output
    }

    // System metrics helpers
    fn get_cpu_usage() -> f64 {
        // Placeholder - would use sysinfo crate in production
        0.0
    }

    fn get_memory_usage() -> u64 {
        // Placeholder - would use sysinfo crate in production
        0
    }

    fn get_disk_usage() -> u64 {
        // Placeholder - would use sysinfo crate in production
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_counter() {
        let counter = RateCounter::new();
        
        for _ in 0..100 {
            counter.increment();
        }
        
        assert_eq!(counter.get_total(), 100);
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        let rate = counter.get_rate().await;
        assert!(rate > 0.0);
    }

    #[tokio::test]
    async fn test_histogram() {
        let histogram = Histogram::new(vec![10, 50, 100, 500, 1000]);
        
        histogram.record(5).await;
        histogram.record(25).await;
        histogram.record(75).await;
        histogram.record(150).await;
        histogram.record(750).await;
        
        let avg = histogram.get_average().await;
        assert!(avg > 0.0);
        
        let p50 = histogram.get_percentile(0.5).await;
        assert!(p50 > 0);
    }
}

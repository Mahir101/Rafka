use std::sync::Arc;
use std::time::Duration;
use prometheus::{
    Counter, Histogram, Gauge, Registry, TextEncoder,
    HistogramOpts, Opts,
};

/// Comprehensive metrics collector for Rafka
pub struct RafkaMetrics {
    // Message metrics
    pub messages_published_total: Counter,
    pub messages_consumed_total: Counter,
    pub messages_failed_total: Counter,
    
    // Latency metrics
    pub publish_latency: Histogram,
    pub consume_latency: Histogram,
    pub broker_response_latency: Histogram,
    
    // Throughput metrics
    pub publish_throughput: Gauge,
    pub consume_throughput: Gauge,
    
    // System metrics
    pub active_connections: Gauge,
    pub active_topics: Gauge,
    pub active_consumers: Gauge,
    pub memory_usage_bytes: Gauge,
    
    // Storage metrics
    pub storage_messages_total: Gauge,
    pub storage_bytes_total: Gauge,
    pub storage_oldest_message_age: Gauge,
    
    // Error metrics
    pub connection_errors_total: Counter,
    pub partition_errors_total: Counter,
    pub storage_errors_total: Counter,
    
    registry: Registry,
}

impl RafkaMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        
        // Message metrics
        let messages_published_total = Counter::with_opts(Opts::new(
            "rafka_messages_published_total",
            "Total number of messages published"
        ))?;
        
        let messages_consumed_total = Counter::with_opts(Opts::new(
            "rafka_messages_consumed_total", 
            "Total number of messages consumed"
        ))?;
        
        let messages_failed_total = Counter::with_opts(Opts::new(
            "rafka_messages_failed_total",
            "Total number of failed messages"
        ))?;
        
        // Latency metrics
        let publish_latency = Histogram::with_opts(HistogramOpts::new(
            "rafka_publish_latency_seconds",
            "Publish operation latency in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]))?;
        
        let consume_latency = Histogram::with_opts(HistogramOpts::new(
            "rafka_consume_latency_seconds",
            "Consume operation latency in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]))?;
        
        let broker_response_latency = Histogram::with_opts(HistogramOpts::new(
            "rafka_broker_response_latency_seconds",
            "Broker response latency in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]))?;
        
        // Throughput metrics
        let publish_throughput = Gauge::with_opts(Opts::new(
            "rafka_publish_throughput_messages_per_second",
            "Current publish throughput in messages per second"
        ))?;
        
        let consume_throughput = Gauge::with_opts(Opts::new(
            "rafka_consume_throughput_messages_per_second", 
            "Current consume throughput in messages per second"
        ))?;
        
        // System metrics
        let active_connections = Gauge::with_opts(Opts::new(
            "rafka_active_connections",
            "Number of active connections"
        ))?;
        
        let active_topics = Gauge::with_opts(Opts::new(
            "rafka_active_topics",
            "Number of active topics"
        ))?;
        
        let active_consumers = Gauge::with_opts(Opts::new(
            "rafka_active_consumers",
            "Number of active consumers"
        ))?;
        
        let memory_usage_bytes = Gauge::with_opts(Opts::new(
            "rafka_memory_usage_bytes",
            "Memory usage in bytes"
        ))?;
        
        // Storage metrics
        let storage_messages_total = Gauge::with_opts(Opts::new(
            "rafka_storage_messages_total",
            "Total number of messages in storage"
        ))?;
        
        let storage_bytes_total = Gauge::with_opts(Opts::new(
            "rafka_storage_bytes_total",
            "Total bytes stored"
        ))?;
        
        let storage_oldest_message_age = Gauge::with_opts(Opts::new(
            "rafka_storage_oldest_message_age_seconds",
            "Age of oldest message in storage"
        ))?;
        
        // Error metrics
        let connection_errors_total = Counter::with_opts(Opts::new(
            "rafka_connection_errors_total",
            "Total number of connection errors"
        ))?;
        
        let partition_errors_total = Counter::with_opts(Opts::new(
            "rafka_partition_errors_total",
            "Total number of partition errors"
        ))?;
        
        let storage_errors_total = Counter::with_opts(Opts::new(
            "rafka_storage_errors_total",
            "Total number of storage errors"
        ))?;
        
        // Register all metrics
        registry.register(Box::new(messages_published_total.clone()))?;
        registry.register(Box::new(messages_consumed_total.clone()))?;
        registry.register(Box::new(messages_failed_total.clone()))?;
        registry.register(Box::new(publish_latency.clone()))?;
        registry.register(Box::new(consume_latency.clone()))?;
        registry.register(Box::new(broker_response_latency.clone()))?;
        registry.register(Box::new(publish_throughput.clone()))?;
        registry.register(Box::new(consume_throughput.clone()))?;
        registry.register(Box::new(active_connections.clone()))?;
        registry.register(Box::new(active_topics.clone()))?;
        registry.register(Box::new(active_consumers.clone()))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;
        registry.register(Box::new(storage_messages_total.clone()))?;
        registry.register(Box::new(storage_bytes_total.clone()))?;
        registry.register(Box::new(storage_oldest_message_age.clone()))?;
        registry.register(Box::new(connection_errors_total.clone()))?;
        registry.register(Box::new(partition_errors_total.clone()))?;
        registry.register(Box::new(storage_errors_total.clone()))?;
        
        Ok(Self {
            messages_published_total,
            messages_consumed_total,
            messages_failed_total,
            publish_latency,
            consume_latency,
            broker_response_latency,
            publish_throughput,
            consume_throughput,
            active_connections,
            active_topics,
            active_consumers,
            memory_usage_bytes,
            storage_messages_total,
            storage_bytes_total,
            storage_oldest_message_age,
            connection_errors_total,
            partition_errors_total,
            storage_errors_total,
            registry,
        })
    }
    
    /// Get metrics in Prometheus format
    pub fn get_metrics(&self) -> Result<String, prometheus::Error> {
        let metric_families = self.registry.gather();
        let encoder = TextEncoder::new();
        encoder.encode_to_string(&metric_families)
    }
    
    /// Record publish operation
    pub fn record_publish(&self, latency: Duration) {
        self.messages_published_total.inc();
        self.publish_latency.observe(latency.as_secs_f64());
    }
    
    /// Record consume operation
    pub fn record_consume(&self, latency: Duration) {
        self.messages_consumed_total.inc();
        self.consume_latency.observe(latency.as_secs_f64());
    }
    
    /// Record failed message
    pub fn record_failed_message(&self) {
        self.messages_failed_total.inc();
    }
    
    /// Update throughput metrics
    pub fn update_throughput(&self, publish_rate: f64, consume_rate: f64) {
        self.publish_throughput.set(publish_rate);
        self.consume_throughput.set(consume_rate);
    }
    
    /// Update system metrics
    pub fn update_system_metrics(&self, connections: i64, topics: i64, consumers: i64, memory_bytes: i64) {
        self.active_connections.set(connections as f64);
        self.active_topics.set(topics as f64);
        self.active_consumers.set(consumers as f64);
        self.memory_usage_bytes.set(memory_bytes as f64);
    }
    
    /// Update storage metrics
    pub fn update_storage_metrics(&self, messages: i64, bytes: i64, oldest_age_secs: f64) {
        self.storage_messages_total.set(messages as f64);
        self.storage_bytes_total.set(bytes as f64);
        self.storage_oldest_message_age.set(oldest_age_secs);
    }
    
    /// Record error
    pub fn record_error(&self, error_type: ErrorType) {
        match error_type {
            ErrorType::Connection => self.connection_errors_total.inc(),
            ErrorType::Partition => self.partition_errors_total.inc(),
            ErrorType::Storage => self.storage_errors_total.inc(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    Connection,
    Partition,
    Storage,
}

/// Metrics server for exposing Prometheus metrics
pub struct MetricsServer {
    metrics: Arc<RafkaMetrics>,
    port: u16,
}

impl MetricsServer {
    pub fn new(metrics: Arc<RafkaMetrics>, port: u16) -> Self {
        Self { metrics, port }
    }
    
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        use hyper::service::{make_service_fn, service_fn};
        use hyper::{Body, Request, Response, Server};
        
        let metrics = self.metrics.clone();
        
        let make_svc = make_service_fn(move |_conn| {
            let metrics = metrics.clone();
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                    let metrics = metrics.clone();
                    async move {
                        if req.uri().path() == "/metrics" {
                            match metrics.get_metrics() {
                                Ok(metrics_text) => {
                                    Ok::<_, hyper::Error>(Response::new(Body::from(metrics_text)))
                                },
                                Err(_) => {
                                    Ok::<_, hyper::Error>(Response::builder()
                                        .status(500)
                                        .body(Body::from("Failed to get metrics"))
                                        .unwrap())
                                }
                            }
                        } else if req.uri().path() == "/health" {
                            Ok::<_, hyper::Error>(Response::new(Body::from("OK")))
                        } else {
                            Ok::<_, hyper::Error>(Response::builder()
                                .status(404)
                                .body(Body::from("Not Found"))
                                .unwrap())
                        }
                    }
                }))
            }
        });
        
        let addr = ([0, 0, 0, 0], self.port).into();
        let server = Server::bind(&addr).serve(make_svc);
        
        println!("📊 Metrics server running on http://0.0.0.0:{}/metrics", self.port);
        
        if let Err(e) = server.await {
            eprintln!("❌ Metrics server error: {}", e);
        }
        
        Ok(())
    }
}

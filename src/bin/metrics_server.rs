use rafka_core::metrics::{RafkaMetrics, MetricsServer};
use std::sync::Arc;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let port = args.iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(9092);

    println!("🚀 Starting Rafka Metrics Server on port {}", port);

    // Create metrics collector
    let metrics = Arc::new(RafkaMetrics::new()?);
    
    // Start metrics server
    let metrics_server = MetricsServer::new(metrics, port);
    metrics_server.start().await?;

    Ok(())
}

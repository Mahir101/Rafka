use rafka_core::proto::rafka::broker_service_client::BrokerServiceClient;
use std::env;
use rafka_core::proto::rafka::GetMetricsRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let port = args.iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(50051);

    let mut client = BrokerServiceClient::connect(format!("http://127.0.0.1:{}", port)).await?;
    
    // Get metrics
    let response = client
        .get_metrics(GetMetricsRequest {})
        .await?;

    let metrics = response.into_inner();
    println!("Storage Metrics:");
    println!("Total Messages: {}", metrics.total_messages);
    println!("Total Size: {} bytes", metrics.total_bytes);
    println!("Oldest Message Age: {} seconds", metrics.oldest_message_age_secs);

    Ok(())
} 
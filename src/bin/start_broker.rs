use rafka_broker::Broker;
use rafka_storage::db::RetentionPolicy;
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let port = args.iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(50051);

    let partition_id = args.iter()
        .position(|arg| arg == "--partition")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);

    let total_partitions = args.iter()
        .position(|arg| arg == "--total-partitions")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(1);

    let retention_secs = args.iter()
        .position(|arg| arg == "--retention-seconds")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u64>().ok())
        .map(Duration::from_secs);

    let retention_policy = retention_secs.map(|secs| RetentionPolicy {
        max_age: secs,
        max_bytes: 1024 * 1024 * 1024, // 1GB default
    });

    println!("Starting Rafka broker on 127.0.0.1:{} (partition {}/{})", 
             port, partition_id, total_partitions);

    let broker = Broker::new(partition_id, total_partitions, retention_policy);
    broker.serve(&format!("127.0.0.1:{}", port)).await?;
    Ok(())
} 
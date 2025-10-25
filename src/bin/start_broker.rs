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

    let cluster_config_path = args.iter()
        .position(|arg| arg == "--cluster-config")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string());

    let bootstrap_nodes: Vec<String> = args.iter()
        .position(|arg| arg == "--bootstrap")
        .and_then(|i| args.get(i + 1))
        .map(|nodes| nodes.split(',').map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let retention_policy = retention_secs.map(|secs| RetentionPolicy {
        max_age: secs,
        max_bytes: 1024 * 1024 * 1024, // 1GB default
    });

    println!("Starting Rafka broker on 127.0.0.1:{} (partition {}/{})", 
             port, partition_id, total_partitions);

    let broker = Broker::new_with_cluster(partition_id, total_partitions, retention_policy, "127.0.0.1", port);
    
    // Load cluster configuration if provided
    if let Some(config_path) = cluster_config_path {
        println!("Loading cluster configuration from: {}", config_path);
        if let Err(e) = broker.load_cluster_config(&config_path).await {
            println!("Warning: Failed to load cluster config: {}", e);
        }
    } else {
        println!("No cluster configuration provided. Running in standalone mode.");
    }
    
    // Initialize P2P mesh networking
    if !bootstrap_nodes.is_empty() {
        println!("🌐 Initializing P2P mesh with bootstrap nodes: {:?}", bootstrap_nodes);
        if let Err(e) = broker.initialize_p2p_mesh(bootstrap_nodes).await {
            println!("Warning: Failed to initialize P2P mesh: {}", e);
        }
    } else {
        println!("🌐 Initializing P2P mesh in standalone mode");
        if let Err(e) = broker.initialize_p2p_mesh(vec![]).await {
            println!("Warning: Failed to initialize P2P mesh: {}", e);
        }
    }
    
    broker.serve(&format!("127.0.0.1:{}", port)).await?;
    Ok(())
} 
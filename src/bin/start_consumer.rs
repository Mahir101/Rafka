use rafka_consumer::Consumer;
use std::env;

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

    let group_id = args.iter()
        .position(|arg| arg == "--group-id")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let mut consumer = Consumer::new(&format!("127.0.0.1:{}", port), group_id.clone()).await?;
    consumer.subscribe("greetings".to_string()).await?;
    
    let group_msg = if let Some(gid) = group_id {
        format!(" (group: {})", gid)
    } else {
        "".to_string()
    };
    println!("Consumer ready - listening for messages on 'greetings' topic (partition {}){}", partition_id, group_msg);
    
    let mut rx = consumer.consume("greetings".to_string()).await?;
    while let Some(message) = rx.recv().await {
        println!("Received message: {}", message);
    }
    
    Ok(())
} 
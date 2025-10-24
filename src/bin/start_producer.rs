use rafka_producer::Producer;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    // Get broker addresses from command line or use default
    let broker_addrs: Vec<String> = if let Some(pos) = args.iter().position(|arg| arg == "--brokers") {
        args.get(pos + 1)
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_else(|| vec!["127.0.0.1:50051".to_string()])
    } else {
        vec!["127.0.0.1:50051".to_string()]
    };

    let message = args.iter()
        .position(|arg| arg == "--message")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Hello, World!".to_string());

    let key = args.iter()
        .position(|arg| arg == "--key")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default-key".to_string());

    println!("Publishing to 'greetings' topic with key '{}': {}", key, message);
    let mut producer = Producer::new(&broker_addrs[0]).await?;
    producer.publish("greetings".to_string(), message, key).await?;
    Ok(())
} 
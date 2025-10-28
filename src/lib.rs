//! Rafka - A high-performance distributed message broker written in Rust
//! 
//! This crate provides the main workspace for the Rafka message broker system.
//! It includes all the core components needed to build distributed messaging applications.
//!
//! ## Components
//!
//! - **rafka-core**: Core library with message structures and protocols
//! - **rafka-broker**: Broker implementation for message routing
//! - **rafka-producer**: Producer client for sending messages
//! - **rafka-consumer**: Consumer client for receiving messages
//! - **rafka-storage**: Storage engine for message persistence
//!
//! ## Quick Start
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! rafka-core = "0.1.0"
//! rafka-broker = "0.1.0"
//! rafka-producer = "0.1.0"
//! rafka-consumer = "0.1.0"
//! rafka-storage = "0.1.0"
//! ```
//!
//! ## Features
//!
//! - High-performance message processing
//! - P2P mesh networking
//! - Dynamic node discovery
//! - Partition-based message routing
//! - Zero-copy message handling
//! - Memory pooling for efficiency

// Re-export the published crates for convenience
pub extern crate rafka_core;
pub extern crate rafka_broker;
pub extern crate rafka_producer;
pub extern crate rafka_consumer;
pub extern crate rafka_storage;

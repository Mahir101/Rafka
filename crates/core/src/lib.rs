pub mod message;
pub mod zero_copy;
pub mod memory_pool;
pub mod metrics;
pub mod proto {
    pub mod rafka {
        tonic::include_proto!("rafka");
    }
}

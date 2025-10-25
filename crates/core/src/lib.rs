pub mod message;
pub mod zero_copy;
pub mod memory_pool;
pub mod metrics;
pub mod cluster;
pub mod p2p_mesh;
pub mod proto {
    pub mod rafka {
        tonic::include_proto!("rafka");
    }
}

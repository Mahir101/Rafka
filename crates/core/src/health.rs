use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Health status of a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub component: String,
    pub status: HealthStatus,
    pub message: String,
    pub timestamp: SystemTime,
    pub details: HashMap<String, String>,
}

impl HealthCheck {
    pub fn healthy(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Healthy,
            message: "OK".to_string(),
            timestamp: SystemTime::now(),
            details: HashMap::new(),
        }
    }

    pub fn degraded(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Degraded,
            message: message.into(),
            timestamp: SystemTime::now(),
            details: HashMap::new(),
        }
    }

    pub fn unhealthy(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Unhealthy,
            message: message.into(),
            timestamp: SystemTime::now(),
            details: HashMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

/// Health monitor for tracking system health
pub struct HealthMonitor {
    checks: Arc<RwLock<HashMap<String, HealthCheck>>>,
    check_interval: Duration,
    failure_threshold: u32,
    failure_counts: Arc<RwLock<HashMap<String, u32>>>,
}

impl HealthMonitor {
    pub fn new(check_interval: Duration, failure_threshold: u32) -> Self {
        Self {
            checks: Arc::new(RwLock::new(HashMap::new())),
            check_interval,
            failure_threshold,
            failure_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a health check
    pub async fn register_check(&self, check: HealthCheck) {
        let mut checks = self.checks.write().await;
        checks.insert(check.component.clone(), check);
    }

    /// Update health check status
    pub async fn update_check(&self, component: &str, status: HealthStatus, message: String) {
        let mut checks = self.checks.write().await;
        if let Some(check) = checks.get_mut(component) {
            check.status = status;
            check.message = message;
            check.timestamp = SystemTime::now();
        }

        // Update failure count
        let mut failures = self.failure_counts.write().await;
        match status {
            HealthStatus::Unhealthy => {
                let count = failures.entry(component.to_string()).or_insert(0);
                *count += 1;
            }
            HealthStatus::Healthy => {
                failures.remove(component);
            }
            _ => {}
        }
    }

    /// Get all health checks
    pub async fn get_all_checks(&self) -> Vec<HealthCheck> {
        let checks = self.checks.read().await;
        checks.values().cloned().collect()
    }

    /// Get overall health status
    pub async fn get_overall_status(&self) -> HealthStatus {
        let checks = self.checks.read().await;
        
        if checks.is_empty() {
            return HealthStatus::Unknown;
        }

        let mut has_unhealthy = false;
        let mut has_degraded = false;

        for check in checks.values() {
            match check.status {
                HealthStatus::Unhealthy => has_unhealthy = true,
                HealthStatus::Degraded => has_degraded = true,
                _ => {}
            }
        }

        if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Check if a component has exceeded failure threshold
    pub async fn is_circuit_broken(&self, component: &str) -> bool {
        let failures = self.failure_counts.read().await;
        failures.get(component).map(|&count| count >= self.failure_threshold).unwrap_or(false)
    }

    /// Reset failure count for a component
    pub async fn reset_failures(&self, component: &str) {
        let mut failures = self.failure_counts.write().await;
        failures.remove(component);
    }

    /// Start background health check scheduler
    pub async fn start_scheduler<F>(&self, check_fn: F)
    where
        F: Fn() -> Vec<HealthCheck> + Send + Sync + 'static,
    {
        let checks = self.checks.clone();
        let interval = self.check_interval;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;
                
                let new_checks = check_fn();
                let mut checks_map = checks.write().await;
                
                for check in new_checks {
                    checks_map.insert(check.component.clone(), check);
                }
            }
        });
    }
}

/// Heartbeat manager for tracking broker liveness
pub struct HeartbeatManager {
    last_heartbeats: Arc<RwLock<HashMap<String, SystemTime>>>,
    timeout: Duration,
}

impl HeartbeatManager {
    pub fn new(timeout: Duration) -> Self {
        Self {
            last_heartbeats: Arc::new(RwLock::new(HashMap::new())),
            timeout,
        }
    }

    /// Record a heartbeat from a broker
    pub async fn record_heartbeat(&self, broker_id: &str) {
        let mut heartbeats = self.last_heartbeats.write().await;
        heartbeats.insert(broker_id.to_string(), SystemTime::now());
    }

    /// Check if a broker is alive
    pub async fn is_alive(&self, broker_id: &str) -> bool {
        let heartbeats = self.last_heartbeats.read().await;
        
        if let Some(last_heartbeat) = heartbeats.get(broker_id) {
            if let Ok(elapsed) = SystemTime::now().duration_since(*last_heartbeat) {
                return elapsed < self.timeout;
            }
        }
        
        false
    }

    /// Get all dead brokers
    pub async fn get_dead_brokers(&self) -> Vec<String> {
        let heartbeats = self.last_heartbeats.read().await;
        let now = SystemTime::now();
        
        heartbeats
            .iter()
            .filter_map(|(broker_id, last_heartbeat)| {
                if let Ok(elapsed) = now.duration_since(*last_heartbeat) {
                    if elapsed >= self.timeout {
                        return Some(broker_id.clone());
                    }
                }
                None
            })
            .collect()
    }

    /// Remove a broker from tracking
    pub async fn remove_broker(&self, broker_id: &str) {
        let mut heartbeats = self.last_heartbeats.write().await;
        heartbeats.remove(broker_id);
    }

    /// Start background heartbeat checker
    pub async fn start_checker<F>(&self, on_dead: F)
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        let heartbeats = self.last_heartbeats.clone();
        let timeout = self.timeout;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(timeout / 2);
            loop {
                interval.tick().await;
                
                let dead_brokers = {
                    let hb = heartbeats.read().await;
                    let now = SystemTime::now();
                    
                    hb.iter()
                        .filter_map(|(broker_id, last_heartbeat)| {
                            if let Ok(elapsed) = now.duration_since(*last_heartbeat) {
                                if elapsed >= timeout {
                                    return Some(broker_id.clone());
                                }
                            }
                            None
                        })
                        .collect::<Vec<_>>()
                };

                for broker_id in dead_brokers {
                    println!("⚠️  Broker {} is dead (no heartbeat for {:?})", broker_id, timeout);
                    on_dead(broker_id);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_monitor() {
        let monitor = HealthMonitor::new(Duration::from_secs(5), 3);
        
        // Register checks
        monitor.register_check(HealthCheck::healthy("broker")).await;
        monitor.register_check(HealthCheck::healthy("storage")).await;
        
        // Overall should be healthy
        assert_eq!(monitor.get_overall_status().await, HealthStatus::Healthy);
        
        // Update one to unhealthy
        monitor.update_check("broker", HealthStatus::Unhealthy, "Connection lost".to_string()).await;
        assert_eq!(monitor.get_overall_status().await, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_heartbeat_manager() {
        let manager = HeartbeatManager::new(Duration::from_millis(100));
        
        // Record heartbeat
        manager.record_heartbeat("broker-1").await;
        assert!(manager.is_alive("broker-1").await);
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!manager.is_alive("broker-1").await);
    }
}

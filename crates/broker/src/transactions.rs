use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{SystemTime, Duration};
use uuid::Uuid;

/// Transaction state
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionState {
    /// Transaction is being prepared
    Preparing,
    /// Transaction is ready to commit
    Prepared,
    /// Transaction is being committed
    Committing,
    /// Transaction has been committed
    Committed,
    /// Transaction is being aborted
    Aborting,
    /// Transaction has been aborted
    Aborted,
    /// Transaction has timed out
    TimedOut,
}

/// A write operation in a transaction
#[derive(Debug, Clone)]
pub struct TransactionalWrite {
    pub topic: String,
    pub partition: i32,
    pub key: String,
    pub value: Vec<u8>,
    pub offset: Option<i64>, // Set when committed
}

/// Transaction metadata
#[derive(Debug)]
pub struct Transaction {
    pub transaction_id: String,
    pub producer_id: String,
    pub state: TransactionState,
    pub writes: Vec<TransactionalWrite>,
    pub started_at: SystemTime,
    pub timeout: Duration,
}

impl Transaction {
    pub fn new(producer_id: String, timeout: Duration) -> Self {
        Self {
            transaction_id: Uuid::new_v4().to_string(),
            producer_id,
            state: TransactionState::Preparing,
            writes: Vec::new(),
            started_at: SystemTime::now(),
            timeout,
        }
    }

    pub fn add_write(&mut self, write: TransactionalWrite) -> Result<(), String> {
        if self.state != TransactionState::Preparing {
            return Err(format!("Cannot add writes in state {:?}", self.state));
        }
        self.writes.push(write);
        Ok(())
    }

    pub fn is_timed_out(&self) -> bool {
        SystemTime::now()
            .duration_since(self.started_at)
            .unwrap_or(Duration::from_secs(0))
            > self.timeout
    }
}

/// Idempotent producer tracking
#[derive(Debug)]
pub struct ProducerState {
    pub producer_id: String,
    pub producer_epoch: i64,
    pub sequence_number: i64,
    pub last_update: SystemTime,
}

impl ProducerState {
    pub fn new(producer_id: String) -> Self {
        Self {
            producer_id,
            producer_epoch: 0,
            sequence_number: 0,
            last_update: SystemTime::now(),
        }
    }

    pub fn next_sequence(&mut self) -> i64 {
        self.sequence_number += 1;
        self.last_update = SystemTime::now();
        self.sequence_number
    }
}

/// Transaction Coordinator manages distributed transactions
pub struct TransactionCoordinator {
    /// Active transactions
    transactions: Arc<RwLock<HashMap<String, Transaction>>>,
    /// Producer states for idempotency
    producers: Arc<RwLock<HashMap<String, ProducerState>>>,
    /// Default transaction timeout
    default_timeout: Duration,
}

impl TransactionCoordinator {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            producers: Arc::new(RwLock::new(HashMap::new())),
            default_timeout: Duration::from_secs(60), // 60 seconds default
        }
    }

    /// Begin a new transaction
    pub async fn begin_transaction(&self, producer_id: String) -> Result<String, String> {
        let mut transactions = self.transactions.write().await;
        
        // Check if producer already has an active transaction
        for (_, txn) in transactions.iter() {
            if txn.producer_id == producer_id 
                && (txn.state == TransactionState::Preparing 
                    || txn.state == TransactionState::Prepared) {
                return Err("Producer already has an active transaction".to_string());
            }
        }

        let transaction = Transaction::new(producer_id.clone(), self.default_timeout);
        let txn_id = transaction.transaction_id.clone();
        
        transactions.insert(txn_id.clone(), transaction);
        
        // Ensure producer state exists
        let mut producers = self.producers.write().await;
        producers.entry(producer_id.clone())
            .or_insert_with(|| ProducerState::new(producer_id));

        Ok(txn_id)
    }

    /// Add a write to a transaction
    pub async fn add_write(
        &self,
        transaction_id: &str,
        write: TransactionalWrite,
    ) -> Result<(), String> {
        let mut transactions = self.transactions.write().await;
        
        if let Some(txn) = transactions.get_mut(transaction_id) {
            if txn.is_timed_out() {
                txn.state = TransactionState::TimedOut;
                return Err("Transaction timed out".to_string());
            }
            txn.add_write(write)
        } else {
            Err("Transaction not found".to_string())
        }
    }

    /// Prepare transaction for commit (2PC Phase 1)
    pub async fn prepare_transaction(&self, transaction_id: &str) -> Result<(), String> {
        let mut transactions = self.transactions.write().await;
        
        if let Some(txn) = transactions.get_mut(transaction_id) {
            if txn.is_timed_out() {
                txn.state = TransactionState::TimedOut;
                return Err("Transaction timed out".to_string());
            }

            if txn.state != TransactionState::Preparing {
                return Err(format!("Cannot prepare transaction in state {:?}", txn.state));
            }

            // Validate all writes (check partitions exist, etc.)
            // For now, just mark as prepared
            txn.state = TransactionState::Prepared;
            Ok(())
        } else {
            Err("Transaction not found".to_string())
        }
    }

    /// Commit transaction (2PC Phase 2)
    pub async fn commit_transaction(&self, transaction_id: &str) -> Result<Vec<TransactionalWrite>, String> {
        let mut transactions = self.transactions.write().await;
        
        if let Some(txn) = transactions.get_mut(transaction_id) {
            if txn.state != TransactionState::Prepared {
                return Err(format!("Cannot commit transaction in state {:?}", txn.state));
            }

            txn.state = TransactionState::Committing;
            
            // Return writes to be applied atomically
            let writes = txn.writes.clone();
            
            txn.state = TransactionState::Committed;
            
            Ok(writes)
        } else {
            Err("Transaction not found".to_string())
        }
    }

    /// Abort transaction
    pub async fn abort_transaction(&self, transaction_id: &str) -> Result<(), String> {
        let mut transactions = self.transactions.write().await;
        
        if let Some(txn) = transactions.get_mut(transaction_id) {
            if txn.state == TransactionState::Committed {
                return Err("Cannot abort committed transaction".to_string());
            }

            txn.state = TransactionState::Aborting;
            txn.writes.clear();
            txn.state = TransactionState::Aborted;
            
            Ok(())
        } else {
            Err("Transaction not found".to_string())
        }
    }

    /// Get next sequence number for idempotent producer
    pub async fn next_sequence(&self, producer_id: &str) -> Result<i64, String> {
        let mut producers = self.producers.write().await;
        
        if let Some(producer) = producers.get_mut(producer_id) {
            Ok(producer.next_sequence())
        } else {
            Err("Producer not found".to_string())
        }
    }

    /// Validate sequence number for exactly-once semantics
    pub async fn validate_sequence(
        &self,
        producer_id: &str,
        sequence: i64,
    ) -> Result<bool, String> {
        let producers = self.producers.read().await;
        
        if let Some(producer) = producers.get(producer_id) {
            // Sequence should be exactly next expected
            Ok(sequence == producer.sequence_number + 1)
        } else {
            Err("Producer not found".to_string())
        }
    }

    /// Clean up old transactions
    pub async fn cleanup_old_transactions(&self) {
        let mut transactions = self.transactions.write().await;
        
        transactions.retain(|_, txn| {
            // Keep only active transactions
            txn.state == TransactionState::Preparing 
                || txn.state == TransactionState::Prepared
                || !txn.is_timed_out()
        });
    }

    /// Get transaction state
    pub async fn get_transaction_state(&self, transaction_id: &str) -> Option<TransactionState> {
        let transactions = self.transactions.read().await;
        transactions.get(transaction_id).map(|txn| txn.state.clone())
    }
}

//! Transaction management for MSSQL.
//!
//! This module provides transaction support including savepoints,
//! isolation levels, and nested transactions.

use async_trait::async_trait;
use tracing::{debug, error, info};

use crate::connection::{MssqlConnection, MssqlRow};
use crate::error::{MssqlError, MssqlResult, TransactionError};
use crate::types::SqlValue;

/// Transaction isolation levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    /// Read uncommitted data (dirty reads possible).
    ReadUncommitted,
    /// Read only committed data (default for SQL Server).
    #[default]
    ReadCommitted,
    /// Repeatable reads within transaction.
    RepeatableRead,
    /// Serializable transactions.
    Serializable,
    /// Snapshot isolation (optimistic concurrency).
    Snapshot,
}

impl IsolationLevel {
    /// Convert to SQL Server SET TRANSACTION ISOLATION LEVEL statement.
    pub fn to_sql(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "SET TRANSACTION ISOLATION LEVEL READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
            IsolationLevel::RepeatableRead => "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ",
            IsolationLevel::Serializable => "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
            IsolationLevel::Snapshot => "SET TRANSACTION ISOLATION LEVEL SNAPSHOT",
        }
    }
}

impl std::fmt::Display for IsolationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IsolationLevel::ReadUncommitted => write!(f, "READ UNCOMMITTED"),
            IsolationLevel::ReadCommitted => write!(f, "READ COMMITTED"),
            IsolationLevel::RepeatableRead => write!(f, "REPEATABLE READ"),
            IsolationLevel::Serializable => write!(f, "SERIALIZABLE"),
            IsolationLevel::Snapshot => write!(f, "SNAPSHOT"),
        }
    }
}

/// Transaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction not started.
    NotStarted,
    /// Transaction is active.
    Active,
    /// Transaction was committed.
    Committed,
    /// Transaction was rolled back.
    RolledBack,
}

/// A database transaction.
pub struct Transaction<'a> {
    connection: &'a MssqlConnection,
    state: TransactionState,
    isolation_level: IsolationLevel,
    savepoints: Vec<String>,
    name: Option<String>,
}

impl<'a> Transaction<'a> {
    /// Create a new transaction (internal use).
    pub(crate) fn new(connection: &'a MssqlConnection) -> Self {
        Self {
            connection,
            state: TransactionState::NotStarted,
            isolation_level: IsolationLevel::default(),
            savepoints: Vec::new(),
            name: None,
        }
    }

    /// Set the isolation level (must be called before begin).
    pub fn isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Set a name for the transaction.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Begin the transaction.
    pub async fn begin(mut self) -> MssqlResult<Self> {
        if self.state != TransactionState::NotStarted {
            return Err(MssqlError::Transaction(TransactionError::AlreadyStarted));
        }

        debug!("Beginning transaction with isolation level: {}", self.isolation_level);

        // Set isolation level
        self.connection
            .execute_batch(self.isolation_level.to_sql())
            .await?;

        // Begin transaction
        let begin_sql = match &self.name {
            Some(name) => format!("BEGIN TRANSACTION [{}]", name),
            None => "BEGIN TRANSACTION".to_string(),
        };
        self.connection.execute_batch(&begin_sql).await?;

        self.state = TransactionState::Active;
        info!("Transaction started");

        Ok(self)
    }

    /// Commit the transaction.
    pub async fn commit(mut self) -> MssqlResult<()> {
        if self.state != TransactionState::Active {
            return Err(MssqlError::Transaction(TransactionError::NotStarted));
        }

        debug!("Committing transaction");

        let commit_sql = match &self.name {
            Some(name) => format!("COMMIT TRANSACTION [{}]", name),
            None => "COMMIT TRANSACTION".to_string(),
        };

        match self.connection.execute_batch(&commit_sql).await {
            Ok(_) => {
                self.state = TransactionState::Committed;
                info!("Transaction committed");
                Ok(())
            }
            Err(e) => {
                error!("Transaction commit failed: {}", e);
                self.state = TransactionState::RolledBack;
                Err(MssqlError::Transaction(TransactionError::CommitFailed(
                    e.to_string(),
                )))
            }
        }
    }

    /// Rollback the transaction.
    pub async fn rollback(mut self) -> MssqlResult<()> {
        if self.state != TransactionState::Active {
            return Err(MssqlError::Transaction(TransactionError::NotStarted));
        }

        debug!("Rolling back transaction");

        let rollback_sql = match &self.name {
            Some(name) => format!("ROLLBACK TRANSACTION [{}]", name),
            None => "ROLLBACK TRANSACTION".to_string(),
        };

        self.connection.execute_batch(&rollback_sql).await?;
        self.state = TransactionState::RolledBack;
        info!("Transaction rolled back");

        Ok(())
    }

    /// Create a savepoint.
    pub async fn savepoint(&mut self, name: impl Into<String>) -> MssqlResult<()> {
        if self.state != TransactionState::Active {
            return Err(MssqlError::Transaction(TransactionError::NotStarted));
        }

        let name = name.into();
        debug!("Creating savepoint: {}", name);

        self.connection
            .execute_batch(&format!("SAVE TRANSACTION [{}]", name))
            .await?;

        self.savepoints.push(name.clone());
        info!("Savepoint created: {}", name);

        Ok(())
    }

    /// Rollback to a savepoint.
    pub async fn rollback_to(&mut self, name: &str) -> MssqlResult<()> {
        if self.state != TransactionState::Active {
            return Err(MssqlError::Transaction(TransactionError::NotStarted));
        }

        if !self.savepoints.contains(&name.to_string()) {
            return Err(MssqlError::Transaction(TransactionError::SavepointNotFound {
                name: name.to_string(),
            }));
        }

        debug!("Rolling back to savepoint: {}", name);

        self.connection
            .execute_batch(&format!("ROLLBACK TRANSACTION [{}]", name))
            .await?;

        // Remove all savepoints after this one
        if let Some(pos) = self.savepoints.iter().position(|s| s == name) {
            self.savepoints.truncate(pos + 1);
        }

        info!("Rolled back to savepoint: {}", name);

        Ok(())
    }

    /// Release a savepoint (SQL Server doesn't support RELEASE SAVEPOINT,
    /// but we track it for consistency).
    pub fn release_savepoint(&mut self, name: &str) -> MssqlResult<()> {
        if let Some(pos) = self.savepoints.iter().position(|s| s == name) {
            self.savepoints.remove(pos);
            Ok(())
        } else {
            Err(MssqlError::Transaction(TransactionError::SavepointNotFound {
                name: name.to_string(),
            }))
        }
    }

    /// Execute a query within the transaction.
    pub async fn query(&self, sql: &str, params: &[SqlValue]) -> MssqlResult<Vec<MssqlRow>> {
        if self.state != TransactionState::Active {
            return Err(MssqlError::Transaction(TransactionError::NotStarted));
        }
        self.connection.query(sql, params).await
    }

    /// Execute a non-query command within the transaction.
    pub async fn execute(&self, sql: &str, params: &[SqlValue]) -> MssqlResult<u64> {
        if self.state != TransactionState::Active {
            return Err(MssqlError::Transaction(TransactionError::NotStarted));
        }
        self.connection.execute(sql, params).await
    }

    /// Execute a batch of SQL within the transaction.
    pub async fn execute_batch(&self, sql: &str) -> MssqlResult<()> {
        if self.state != TransactionState::Active {
            return Err(MssqlError::Transaction(TransactionError::NotStarted));
        }
        self.connection.execute_batch(sql).await
    }

    /// Get the current transaction state.
    pub fn state(&self) -> TransactionState {
        self.state
    }

    /// Check if the transaction is active.
    pub fn is_active(&self) -> bool {
        self.state == TransactionState::Active
    }

    /// Get the list of active savepoints.
    pub fn savepoints(&self) -> &[String] {
        &self.savepoints
    }
}

impl<'a> Drop for Transaction<'a> {
    fn drop(&mut self) {
        if self.state == TransactionState::Active {
            // Transaction was not properly committed or rolled back
            // In a real implementation, we'd want to rollback here,
            // but we can't do async operations in drop
            error!(
                "Transaction dropped while still active. \
                 You should explicitly commit or rollback transactions."
            );
        }
    }
}

/// Extension trait to add transaction support to connections.
#[async_trait]
pub trait TransactionExt {
    /// Begin a new transaction.
    async fn begin_transaction(&self) -> MssqlResult<Transaction<'_>>;

    /// Begin a transaction with a specific isolation level.
    async fn begin_transaction_with_level(
        &self,
        level: IsolationLevel,
    ) -> MssqlResult<Transaction<'_>>;

    /// Execute a closure within a transaction.
    async fn transaction<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'t> FnOnce(&'t Transaction<'_>) -> futures::future::BoxFuture<'t, Result<T, E>> + Send,
        E: From<MssqlError> + Send,
        T: Send;
}

#[async_trait]
impl TransactionExt for MssqlConnection {
    async fn begin_transaction(&self) -> MssqlResult<Transaction<'_>> {
        Transaction::new(self).begin().await
    }

    async fn begin_transaction_with_level(
        &self,
        level: IsolationLevel,
    ) -> MssqlResult<Transaction<'_>> {
        Transaction::new(self).isolation_level(level).begin().await
    }

    async fn transaction<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'t> FnOnce(&'t Transaction<'_>) -> futures::future::BoxFuture<'t, Result<T, E>> + Send,
        E: From<MssqlError> + Send,
        T: Send,
    {
        let tx = self.begin_transaction().await.map_err(E::from)?;

        match f(&tx).await {
            Ok(result) => {
                tx.commit().await.map_err(E::from)?;
                Ok(result)
            }
            Err(e) => {
                // Try to rollback, but don't mask the original error
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

/// Transaction builder for more complex transaction configurations.
#[derive(Debug)]
pub struct TransactionBuilder<'a> {
    connection: &'a MssqlConnection,
    isolation_level: IsolationLevel,
    name: Option<String>,
    retry_count: u32,
    retry_on_deadlock: bool,
}

impl<'a> TransactionBuilder<'a> {
    /// Create a new transaction builder.
    pub fn new(connection: &'a MssqlConnection) -> Self {
        Self {
            connection,
            isolation_level: IsolationLevel::default(),
            name: None,
            retry_count: 0,
            retry_on_deadlock: false,
        }
    }

    /// Set the isolation level.
    pub fn isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Set a transaction name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Enable automatic retry on deadlock.
    pub fn retry_on_deadlock(mut self, count: u32) -> Self {
        self.retry_count = count;
        self.retry_on_deadlock = true;
        self
    }

    /// Begin the transaction.
    pub async fn begin(self) -> MssqlResult<Transaction<'a>> {
        let mut tx = Transaction::new(self.connection).isolation_level(self.isolation_level);

        if let Some(name) = self.name {
            tx = tx.name(name);
        }

        tx.begin().await
    }

    /// Execute a closure with automatic retry on deadlock.
    pub async fn run<F, T>(self, mut f: F) -> MssqlResult<T>
    where
        F: for<'t> FnMut(&'t Transaction<'_>) -> futures::future::BoxFuture<'t, MssqlResult<T>>,
        T: Send,
    {
        let mut attempts = 0;
        let max_attempts = if self.retry_on_deadlock {
            self.retry_count + 1
        } else {
            1
        };

        loop {
            attempts += 1;

            let tx = {
                let mut tx = Transaction::new(self.connection).isolation_level(self.isolation_level);
                if let Some(ref name) = self.name {
                    tx = tx.name(name.clone());
                }
                tx.begin().await?
            };

            match f(&tx).await {
                Ok(result) => {
                    tx.commit().await?;
                    return Ok(result);
                }
                Err(e) => {
                    let _ = tx.rollback().await;

                    // Check if it's a deadlock and we should retry
                    if self.retry_on_deadlock && attempts < max_attempts {
                        if let MssqlError::Query(crate::error::QueryError::Deadlock) = e {
                            debug!("Deadlock detected, retrying (attempt {}/{})", attempts, max_attempts);
                            // Add a small delay before retry
                            tokio::time::sleep(tokio::time::Duration::from_millis(100 * attempts as u64)).await;
                            continue;
                        }
                    }

                    return Err(e);
                }
            }
        }
    }
}

/// Create a transaction builder.
pub fn transaction(connection: &MssqlConnection) -> TransactionBuilder<'_> {
    TransactionBuilder::new(connection)
}

/// Distributed transaction support (basic implementation).
#[derive(Debug)]
pub struct DistributedTransaction {
    transaction_id: String,
    connections: Vec<String>, // Connection identifiers
}

impl DistributedTransaction {
    /// Create a new distributed transaction.
    pub fn new() -> Self {
        Self {
            transaction_id: uuid::Uuid::new_v4().to_string(),
            connections: Vec::new(),
        }
    }

    /// Get the transaction ID.
    pub fn id(&self) -> &str {
        &self.transaction_id
    }

    /// Get the SQL to begin this distributed transaction.
    pub fn begin_sql(&self) -> String {
        format!("BEGIN DISTRIBUTED TRANSACTION [{}]", self.transaction_id)
    }

    /// Get the SQL to commit this distributed transaction.
    pub fn commit_sql(&self) -> String {
        "COMMIT TRANSACTION".to_string()
    }

    /// Get the SQL to rollback this distributed transaction.
    pub fn rollback_sql(&self) -> String {
        "ROLLBACK TRANSACTION".to_string()
    }
}

impl Default for DistributedTransaction {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_level_to_sql() {
        assert_eq!(
            IsolationLevel::ReadUncommitted.to_sql(),
            "SET TRANSACTION ISOLATION LEVEL READ UNCOMMITTED"
        );
        assert_eq!(
            IsolationLevel::Serializable.to_sql(),
            "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"
        );
    }

    #[test]
    fn test_distributed_transaction() {
        let dtx = DistributedTransaction::new();
        assert!(!dtx.id().is_empty());
        assert!(dtx.begin_sql().contains("BEGIN DISTRIBUTED TRANSACTION"));
    }
}

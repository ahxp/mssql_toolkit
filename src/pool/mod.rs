//! Connection pooling for MSSQL.
//!
//! This module provides connection pool implementations for managing
//! database connections efficiently.

use std::time::Duration;

use async_trait::async_trait;
use bb8::{ManageConnection, Pool, PooledConnection};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use tracing::{debug, error, info};

use crate::connection::{ConnectionConfig, MssqlConnection, MssqlRow};
use crate::error::{ConnectionError, MssqlError, MssqlResult, PoolError};
use crate::types::SqlValue;

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool.
    pub max_size: u32,
    /// Minimum number of idle connections to maintain.
    pub min_idle: Option<u32>,
    /// Maximum time to wait for a connection.
    pub connection_timeout: Duration,
    /// Maximum lifetime of a connection.
    pub max_lifetime: Option<Duration>,
    /// Idle timeout for connections.
    pub idle_timeout: Option<Duration>,
    /// Whether to test connections on checkout.
    pub test_on_checkout: bool,
    /// Number of retries for connection creation.
    pub connection_retries: u32,
    /// Delay between connection retries.
    pub retry_delay: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            min_idle: Some(1),
            connection_timeout: Duration::from_secs(30),
            max_lifetime: Some(Duration::from_secs(30 * 60)), // 30 minutes
            idle_timeout: Some(Duration::from_secs(10 * 60)), // 10 minutes
            test_on_checkout: true,
            connection_retries: 3,
            retry_delay: Duration::from_millis(500),
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum pool size.
    pub fn max_size(mut self, size: u32) -> Self {
        self.max_size = size;
        self
    }

    /// Set the minimum idle connections.
    pub fn min_idle(mut self, min: u32) -> Self {
        self.min_idle = Some(min);
        self
    }

    /// Set the connection timeout.
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set the maximum connection lifetime.
    pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_lifetime = Some(lifetime);
        self
    }

    /// Set the idle timeout.
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = Some(timeout);
        self
    }

    /// Enable or disable test on checkout.
    pub fn test_on_checkout(mut self, test: bool) -> Self {
        self.test_on_checkout = test;
        self
    }

    /// Set the number of connection retries.
    pub fn connection_retries(mut self, retries: u32) -> Self {
        self.connection_retries = retries;
        self
    }

    /// Set the retry delay.
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }
}

/// Connection manager for bb8 pool.
#[derive(Debug)]
pub struct MssqlConnectionManager {
    config: ConnectionConfig,
    test_query: String,
}

impl MssqlConnectionManager {
    /// Create a new connection manager.
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            test_query: "SELECT 1".to_string(),
        }
    }

    /// Set the health check query.
    pub fn with_test_query(mut self, query: impl Into<String>) -> Self {
        self.test_query = query.into();
        self
    }
}

/// Wrapper type for pooled connection client.
pub struct PooledClient {
    client: tiberius::Client<Compat<TcpStream>>,
    config: ConnectionConfig,
}

impl std::fmt::Debug for PooledClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledClient")
            .field("host", &self.config.host)
            .field("database", &self.config.database)
            .finish()
    }
}

#[async_trait]
impl ManageConnection for MssqlConnectionManager {
    type Connection = PooledClient;
    type Error = MssqlError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        debug!(
            "Creating new pooled connection to {}:{}",
            self.config.host, self.config.port
        );

        let tiberius_config = self.config.to_tiberius_config()?;

        let tcp = TcpStream::connect(format!("{}:{}", self.config.host, self.config.port))
            .await
            .map_err(|e| {
                MssqlError::Connection(ConnectionError::ConnectionFailed {
                    host: self.config.host.clone(),
                    port: self.config.port,
                    reason: e.to_string(),
                })
            })?;

        tcp.set_nodelay(true).ok();

        let client = tiberius::Client::connect(tiberius_config, tcp.compat_write())
            .await
            .map_err(MssqlError::Driver)?;

        info!(
            "Pooled connection created to {}:{}",
            self.config.host, self.config.port
        );

        Ok(PooledClient {
            client,
            config: self.config.clone(),
        })
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        debug!("Checking connection validity");

        conn.client
            .simple_query(&self.test_query)
            .await
            .map_err(MssqlError::Driver)?;

        Ok(())
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        // We can't easily check if the connection is broken without async
        // The pool will handle this through is_valid
        false
    }
}

/// A connection pool for MSSQL databases.
pub struct MssqlPool {
    pool: Pool<MssqlConnectionManager>,
    config: ConnectionConfig,
    pool_config: PoolConfig,
}

impl MssqlPool {
    /// Create a new connection pool.
    pub async fn new(
        connection_config: ConnectionConfig,
        pool_config: PoolConfig,
    ) -> MssqlResult<Self> {
        let manager = MssqlConnectionManager::new(connection_config.clone());

        let pool = Pool::builder()
            .max_size(pool_config.max_size)
            .min_idle(pool_config.min_idle)
            .connection_timeout(pool_config.connection_timeout)
            .max_lifetime(pool_config.max_lifetime)
            .idle_timeout(pool_config.idle_timeout)
            .test_on_check_out(pool_config.test_on_checkout)
            .build(manager)
            .await
            .map_err(|e| {
                MssqlError::Pool(PoolError::InitializationFailed(e.to_string()))
            })?;

        info!(
            "Connection pool created with max_size={}, min_idle={:?}",
            pool_config.max_size, pool_config.min_idle
        );

        Ok(Self {
            pool,
            config: connection_config,
            pool_config,
        })
    }

    /// Create a pool from a connection string.
    pub async fn from_connection_string(
        conn_str: &str,
        pool_config: PoolConfig,
    ) -> MssqlResult<Self> {
        let config = ConnectionConfig::from_connection_string(conn_str)?;
        Self::new(config, pool_config).await
    }

    /// Get a connection from the pool.
    pub async fn get(&self) -> MssqlResult<PooledMssqlConnection<'_>> {
        let conn = self.pool.get().await.map_err(|e| {
            error!("Failed to get connection from pool: {}", e);
            MssqlError::Pool(PoolError::GetConnectionFailed(e.to_string()))
        })?;

        Ok(PooledMssqlConnection { conn })
    }

    /// Get a dedicated (non-pooled) connection.
    pub async fn get_dedicated(&self) -> MssqlResult<MssqlConnection> {
        MssqlConnection::connect(self.config.clone()).await
    }

    /// Get pool statistics.
    pub fn stats(&self) -> PoolStats {
        let state = self.pool.state();
        PoolStats {
            connections: state.connections,
            idle_connections: state.idle_connections,
            max_size: self.pool_config.max_size,
        }
    }

    /// Get the connection configuration.
    pub fn config(&self) -> &ConnectionConfig {
        &self.config
    }

    /// Get the pool configuration.
    pub fn pool_config(&self) -> &PoolConfig {
        &self.pool_config
    }
}

/// Pool statistics.
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of connections.
    pub connections: u32,
    /// Number of idle connections.
    pub idle_connections: u32,
    /// Maximum pool size.
    pub max_size: u32,
}

impl PoolStats {
    /// Get the number of active (in-use) connections.
    pub fn active_connections(&self) -> u32 {
        self.connections.saturating_sub(self.idle_connections)
    }

    /// Get the pool utilization percentage.
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.active_connections() as f64 / self.max_size as f64) * 100.0
        }
    }
}

/// A pooled connection that returns to the pool on drop.
pub struct PooledMssqlConnection<'a> {
    conn: PooledConnection<'a, MssqlConnectionManager>,
}

impl<'a> PooledMssqlConnection<'a> {
    /// Execute a query that returns rows.
    pub async fn query(&mut self, sql: &str, _params: &[SqlValue]) -> MssqlResult<Vec<MssqlRow>> {
        debug!("Executing pooled query: {}", sql);

        let stream = self.conn.client.query(sql, &[]).await.map_err(MssqlError::Driver)?;

        let rows = stream_to_rows(stream).await?;
        Ok(rows)
    }

    /// Execute a query that returns a single row.
    pub async fn query_one(&mut self, sql: &str, params: &[SqlValue]) -> MssqlResult<MssqlRow> {
        let rows = self.query(sql, params).await?;

        match rows.len() {
            0 => Err(MssqlError::Query(crate::error::QueryError::NoRowsAffected)),
            1 => Ok(rows.into_iter().next().unwrap()),
            n => Err(MssqlError::Query(crate::error::QueryError::UnexpectedRowCount { count: n })),
        }
    }

    /// Execute a non-query command.
    pub async fn execute(&mut self, sql: &str, _params: &[SqlValue]) -> MssqlResult<u64> {
        debug!("Executing pooled command: {}", sql);

        let result = self.conn.client.execute(sql, &[]).await.map_err(MssqlError::Driver)?;
        Ok(result.total())
    }

    /// Execute a batch of SQL statements.
    pub async fn execute_batch(&mut self, sql: &str) -> MssqlResult<()> {
        debug!("Executing pooled batch");

        self.conn.client.simple_query(sql).await.map_err(MssqlError::Driver)?;
        Ok(())
    }

    /// Get the current database name.
    pub async fn current_database(&mut self) -> MssqlResult<String> {
        let row = self.query_one("SELECT DB_NAME()", &[]).await?;
        row.get::<String>(0)
    }
}

// Helper function to convert query stream to rows
async fn stream_to_rows(mut stream: tiberius::QueryStream<'_>) -> MssqlResult<Vec<MssqlRow>> {
    use futures::TryStreamExt;

    let mut rows = Vec::new();
    let mut columns: Option<Vec<String>> = None;

    while let Some(item) = stream.try_next().await.map_err(MssqlError::Driver)? {
        match item {
            tiberius::QueryItem::Metadata(meta) => {
                columns = Some(
                    meta.columns()
                        .iter()
                        .map(|c| c.name().to_string())
                        .collect(),
                );
            }
            tiberius::QueryItem::Row(row) => {
                let cols = columns.clone().unwrap_or_default();
                let values = tiberius_row_to_values(&row, cols.len())?;
                rows.push(MssqlRow::new(cols, values));
            }
        }
    }

    Ok(rows)
}

fn tiberius_row_to_values(row: &tiberius::Row, column_count: usize) -> MssqlResult<Vec<SqlValue>> {
    let mut values = Vec::with_capacity(column_count);

    for i in 0..column_count {
        let value = if let Some(v) = row.try_get::<i32, _>(i).ok().flatten() {
            SqlValue::Int(v)
        } else if let Some(v) = row.try_get::<i64, _>(i).ok().flatten() {
            SqlValue::BigInt(v)
        } else if let Some(v) = row.try_get::<bool, _>(i).ok().flatten() {
            SqlValue::Bool(v)
        } else if let Some(v) = row.try_get::<f32, _>(i).ok().flatten() {
            SqlValue::Float(v)
        } else if let Some(v) = row.try_get::<f64, _>(i).ok().flatten() {
            SqlValue::Double(v)
        } else if let Some(v) = row.try_get::<&str, _>(i).ok().flatten() {
            SqlValue::String(v.to_string())
        } else if let Some(v) = row.try_get::<uuid::Uuid, _>(i).ok().flatten() {
            SqlValue::Uuid(v)
        } else {
            SqlValue::Null
        };

        values.push(value);
    }

    Ok(values)
}

/// Builder for creating connection pools.
#[derive(Debug)]
pub struct PoolBuilder {
    connection_config: Option<ConnectionConfig>,
    pool_config: PoolConfig,
}

impl Default for PoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolBuilder {
    /// Create a new pool builder.
    pub fn new() -> Self {
        Self {
            connection_config: None,
            pool_config: PoolConfig::default(),
        }
    }

    /// Set the connection configuration.
    pub fn connection_config(mut self, config: ConnectionConfig) -> Self {
        self.connection_config = Some(config);
        self
    }

    /// Set from a connection string.
    pub fn connection_string(mut self, conn_str: &str) -> MssqlResult<Self> {
        self.connection_config = Some(ConnectionConfig::from_connection_string(conn_str)?);
        Ok(self)
    }

    /// Set the maximum pool size.
    pub fn max_size(mut self, size: u32) -> Self {
        self.pool_config.max_size = size;
        self
    }

    /// Set the minimum idle connections.
    pub fn min_idle(mut self, min: u32) -> Self {
        self.pool_config.min_idle = Some(min);
        self
    }

    /// Set the connection timeout.
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.pool_config.connection_timeout = timeout;
        self
    }

    /// Set the maximum connection lifetime.
    pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
        self.pool_config.max_lifetime = Some(lifetime);
        self
    }

    /// Set the idle timeout.
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_config.idle_timeout = Some(timeout);
        self
    }

    /// Enable or disable test on checkout.
    pub fn test_on_checkout(mut self, test: bool) -> Self {
        self.pool_config.test_on_checkout = test;
        self
    }

    /// Build the connection pool.
    pub async fn build(self) -> MssqlResult<MssqlPool> {
        let config = self.connection_config.ok_or_else(|| {
            MssqlError::Configuration("Connection configuration is required".to_string())
        })?;

        MssqlPool::new(config, self.pool_config).await
    }
}

/// Create a pool builder.
pub fn pool() -> PoolBuilder {
    PoolBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_builder() {
        let config = PoolConfig::new()
            .max_size(20)
            .min_idle(5)
            .connection_timeout(Duration::from_secs(60));

        assert_eq!(config.max_size, 20);
        assert_eq!(config.min_idle, Some(5));
        assert_eq!(config.connection_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_pool_stats() {
        let stats = PoolStats {
            connections: 10,
            idle_connections: 3,
            max_size: 20,
        };

        assert_eq!(stats.active_connections(), 7);
        assert_eq!(stats.utilization(), 35.0);
    }
}

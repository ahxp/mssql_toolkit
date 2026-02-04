//! Connection management for MSSQL databases.
//!
//! This module provides connection handling, configuration, and lifecycle management
//! for SQL Server database connections.

pub mod config;

use std::sync::Arc;

use tiberius::{Client, QueryStream, Row};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use tracing::{debug, error, info, instrument};

pub use config::{AuthMode, ConnectionConfig, ConnectionConfigBuilder, EncryptionMode};

use crate::error::{ConnectionError, MssqlError, MssqlResult, QueryError};
use crate::types::SqlValue;

/// Represents a connection to an MSSQL database.
pub struct MssqlConnection {
    client: Arc<Mutex<Client<Compat<TcpStream>>>>,
    config: ConnectionConfig,
    is_closed: bool,
}

impl std::fmt::Debug for MssqlConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MssqlConnection")
            .field("host", &self.config.host)
            .field("database", &self.config.database)
            .field("is_closed", &self.is_closed)
            .finish()
    }
}

impl MssqlConnection {
    /// Connect to the database using the provided configuration.
    #[instrument(skip(config), fields(host = %config.host, database = %config.database))]
    pub async fn connect(config: ConnectionConfig) -> MssqlResult<Self> {
        config.validate()?;

        let tiberius_config = config.to_tiberius_config()?;

        debug!("Connecting to SQL Server at {}:{}", config.host, config.port);

        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))
            .await
            .map_err(|e| {
                error!("Failed to establish TCP connection: {}", e);
                MssqlError::Connection(ConnectionError::ConnectionFailed {
                    host: config.host.clone(),
                    port: config.port,
                    reason: e.to_string(),
                })
            })?;

        tcp.set_nodelay(true).ok();

        let client = Client::connect(tiberius_config, tcp.compat_write())
            .await
            .map_err(|e| {
                error!("Failed to connect to SQL Server: {}", e);
                match e {
                    tiberius::error::Error::Server(ref server_err) => {
                        let msg = server_err.message();
                        if msg.contains("Login failed") {
                            MssqlError::Connection(ConnectionError::AuthenticationFailed {
                                user: config.username.clone(),
                                reason: msg.to_string(),
                            })
                        } else if msg.contains("Cannot open database") {
                            MssqlError::Connection(ConnectionError::DatabaseNotFound {
                                database: config.database.clone(),
                            })
                        } else {
                            MssqlError::Driver(e)
                        }
                    }
                    _ => MssqlError::Driver(e),
                }
            })?;

        info!(
            "Successfully connected to {}:{}",
            config.host, config.port
        );

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            config,
            is_closed: false,
        })
    }

    /// Connect using a connection string.
    pub async fn connect_with_string(conn_str: &str) -> MssqlResult<Self> {
        let config = ConnectionConfig::from_connection_string(conn_str)?;
        Self::connect(config).await
    }

    /// Execute a query that returns rows.
    #[instrument(skip(self, params), fields(sql = %sql))]
    pub async fn query<'a>(
        &'a self,
        sql: &str,
        params: &[SqlValue],
    ) -> MssqlResult<Vec<MssqlRow>> {
        let mut client = self.client.lock().await;

        debug!("Executing query with {} parameters", params.len());

        // For now, we only support parameterless queries
        // Parameterized queries require more complex handling
        let stream = client.query(sql, &[]).await;

        let stream = stream.map_err(|e| {
            error!("Query execution failed: {}", e);
            query_error_from_tiberius(e)
        })?;

        // Collect all rows
        let rows = stream_to_rows(stream).await?;

        debug!("Query returned {} rows", rows.len());
        Ok(rows)
    }

    /// Execute a query that returns a single row.
    pub async fn query_one(&self, sql: &str, params: &[SqlValue]) -> MssqlResult<MssqlRow> {
        let rows = self.query(sql, params).await?;

        match rows.len() {
            0 => Err(MssqlError::Query(QueryError::NoRowsAffected)),
            1 => Ok(rows.into_iter().next().unwrap()),
            n => Err(MssqlError::Query(QueryError::UnexpectedRowCount { count: n })),
        }
    }

    /// Execute a query that returns an optional single row.
    pub async fn query_optional(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> MssqlResult<Option<MssqlRow>> {
        let rows = self.query(sql, params).await?;

        match rows.len() {
            0 => Ok(None),
            1 => Ok(rows.into_iter().next()),
            n => Err(MssqlError::Query(QueryError::UnexpectedRowCount { count: n })),
        }
    }

    /// Execute a non-query command (INSERT, UPDATE, DELETE, etc.).
    #[instrument(skip(self, params), fields(sql = %sql))]
    pub async fn execute(&self, sql: &str, params: &[SqlValue]) -> MssqlResult<u64> {
        let mut client = self.client.lock().await;

        debug!("Executing command with {} parameters", params.len());

        // For now, we only support parameterless execution
        let result = client.execute(sql, &[]).await;

        let result = result.map_err(|e| {
            error!("Command execution failed: {}", e);
            query_error_from_tiberius(e)
        })?;

        let rows_affected = result.total();
        debug!("Command affected {} rows", rows_affected);

        Ok(rows_affected)
    }

    /// Execute a batch of SQL statements.
    #[instrument(skip(self))]
    pub async fn execute_batch(&self, sql: &str) -> MssqlResult<()> {
        let mut client = self.client.lock().await;

        debug!("Executing batch query");

        client.simple_query(sql).await.map_err(|e| {
            error!("Batch execution failed: {}", e);
            query_error_from_tiberius(e)
        })?;

        Ok(())
    }

    /// Execute a query and return a scalar value.
    pub async fn query_scalar<T: FromSqlRow>(&self, sql: &str, params: &[SqlValue]) -> MssqlResult<T> {
        let row = self.query_one(sql, params).await?;
        T::from_row(&row)
    }

    /// Execute a query and return an optional scalar value.
    pub async fn query_scalar_optional<T: FromSqlRow>(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> MssqlResult<Option<T>> {
        match self.query_optional(sql, params).await? {
            Some(row) => Ok(Some(T::from_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Check if the connection is alive.
    pub async fn is_alive(&self) -> bool {
        if self.is_closed {
            return false;
        }

        match self.query("SELECT 1", &[]).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Get the current database name.
    pub async fn current_database(&self) -> MssqlResult<String> {
        let row = self.query_one("SELECT DB_NAME()", &[]).await?;
        row.get::<String>(0)
    }

    /// Get the server version.
    pub async fn server_version(&self) -> MssqlResult<String> {
        let row = self.query_one("SELECT @@VERSION", &[]).await?;
        row.get::<String>(0)
    }

    /// Get the connection configuration.
    pub fn config(&self) -> &ConnectionConfig {
        &self.config
    }

    /// Close the connection.
    pub async fn close(mut self) -> MssqlResult<()> {
        self.is_closed = true;
        // Drop the client
        drop(self.client);
        info!("Connection closed");
        Ok(())
    }
}

/// A row returned from a query.
#[derive(Debug)]
pub struct MssqlRow {
    columns: Vec<String>,
    values: Vec<SqlValue>,
}

impl MssqlRow {
    /// Create a new row from columns and values.
    pub fn new(columns: Vec<String>, values: Vec<SqlValue>) -> Self {
        Self { columns, values }
    }

    /// Get a value by column index.
    pub fn get<T: FromSqlValue>(&self, index: usize) -> MssqlResult<T> {
        let value = self.values.get(index).ok_or_else(|| {
            MssqlError::Query(QueryError::InvalidColumn {
                column: index.to_string(),
                table: "result".to_string(),
            })
        })?;
        T::from_sql_value(value.clone())
    }

    /// Get a value by column name.
    pub fn get_by_name<T: FromSqlValue>(&self, name: &str) -> MssqlResult<T> {
        let index = self
            .columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                MssqlError::Query(QueryError::InvalidColumn {
                    column: name.to_string(),
                    table: "result".to_string(),
                })
            })?;
        self.get(index)
    }

    /// Try to get a value by column index, returning None if NULL or missing.
    pub fn try_get<T: FromSqlValue>(&self, index: usize) -> MssqlResult<Option<T>> {
        match self.values.get(index) {
            Some(SqlValue::Null) | None => Ok(None),
            Some(value) => T::from_sql_value(value.clone()).map(Some),
        }
    }

    /// Try to get a value by column name, returning None if NULL or missing.
    pub fn try_get_by_name<T: FromSqlValue>(&self, name: &str) -> MssqlResult<Option<T>> {
        let index = self
            .columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name));

        match index {
            Some(i) => self.try_get(i),
            None => Ok(None),
        }
    }

    /// Get the raw SQL value by index.
    pub fn get_raw(&self, index: usize) -> Option<&SqlValue> {
        self.values.get(index)
    }

    /// Get the raw SQL value by name.
    pub fn get_raw_by_name(&self, name: &str) -> Option<&SqlValue> {
        let index = self
            .columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))?;
        self.values.get(index)
    }

    /// Get all column names.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Get the number of columns.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the row is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Convert the row to a map of column name to value.
    pub fn to_map(&self) -> std::collections::HashMap<String, SqlValue> {
        self.columns
            .iter()
            .cloned()
            .zip(self.values.iter().cloned())
            .collect()
    }
}

/// Trait for converting SQL values to Rust types.
pub trait FromSqlValue: Sized {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self>;
}

impl FromSqlValue for SqlValue {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        Ok(value)
    }
}

impl FromSqlValue for i32 {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_i32()
    }
}

impl FromSqlValue for i64 {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_i64()
    }
}

impl FromSqlValue for String {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_string()
    }
}

impl FromSqlValue for bool {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_bool()
    }
}

impl FromSqlValue for rust_decimal::Decimal {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_decimal()
    }
}

impl<T: FromSqlValue> FromSqlValue for Option<T> {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        if value.is_null() {
            Ok(None)
        } else {
            T::from_sql_value(value).map(Some)
        }
    }
}

/// Trait for converting rows to Rust types.
pub trait FromSqlRow: Sized {
    fn from_row(row: &MssqlRow) -> MssqlResult<Self>;
}

impl FromSqlRow for MssqlRow {
    fn from_row(row: &MssqlRow) -> MssqlResult<Self> {
        Ok(MssqlRow {
            columns: row.columns.clone(),
            values: row.values.clone(),
        })
    }
}

impl FromSqlRow for i32 {
    fn from_row(row: &MssqlRow) -> MssqlResult<Self> {
        row.get(0)
    }
}

impl FromSqlRow for i64 {
    fn from_row(row: &MssqlRow) -> MssqlResult<Self> {
        row.get(0)
    }
}

impl FromSqlRow for String {
    fn from_row(row: &MssqlRow) -> MssqlResult<Self> {
        row.get(0)
    }
}

impl FromSqlRow for bool {
    fn from_row(row: &MssqlRow) -> MssqlResult<Self> {
        row.get(0)
    }
}

// Helper functions

// Parameter conversion is handled inline for now
// Future: implement proper parameterized query support

fn query_error_from_tiberius(e: tiberius::error::Error) -> MssqlError {
    match e {
        tiberius::error::Error::Server(ref server_err) => {
            let msg = server_err.message();
            let code = server_err.code();

            // Parse common SQL Server error codes
            match code {
                207 => MssqlError::Query(QueryError::InvalidColumn {
                    column: extract_identifier(msg),
                    table: "unknown".to_string(),
                }),
                208 => MssqlError::Query(QueryError::TableNotFound {
                    table: extract_identifier(msg),
                }),
                547 => MssqlError::Query(QueryError::ForeignKeyViolation(msg.to_string())),
                2601 | 2627 => MssqlError::Query(QueryError::DuplicateKey {
                    key: extract_identifier(msg),
                }),
                1205 => MssqlError::Query(QueryError::Deadlock),
                _ => MssqlError::Driver(e),
            }
        }
        _ => MssqlError::Driver(e),
    }
}

fn extract_identifier(msg: &str) -> String {
    // Simple extraction of quoted identifier from error message
    if let Some(start) = msg.find('\'') {
        if let Some(end) = msg[start + 1..].find('\'') {
            return msg[start + 1..start + 1 + end].to_string();
        }
    }
    "unknown".to_string()
}

async fn stream_to_rows(mut stream: QueryStream<'_>) -> MssqlResult<Vec<MssqlRow>> {
    use futures::TryStreamExt;

    let mut rows = Vec::new();
    let mut columns: Option<Vec<String>> = None;

    while let Some(item) = stream.try_next().await.map_err(|e| MssqlError::Driver(e))? {
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

fn tiberius_row_to_values(row: &Row, column_count: usize) -> MssqlResult<Vec<SqlValue>> {
    let mut values = Vec::with_capacity(column_count);

    for i in 0..column_count {
        // Try different types
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mssql_row_get() {
        let row = MssqlRow::new(
            vec!["id".to_string(), "name".to_string()],
            vec![SqlValue::Int(1), SqlValue::String("test".to_string())],
        );

        assert_eq!(row.get::<i32>(0).unwrap(), 1);
        assert_eq!(row.get::<String>(1).unwrap(), "test");
        assert_eq!(row.get_by_name::<i32>("id").unwrap(), 1);
        assert_eq!(row.get_by_name::<String>("name").unwrap(), "test");
    }
}

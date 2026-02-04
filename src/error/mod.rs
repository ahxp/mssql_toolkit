//! Error types for the MSSQL Toolkit library.
//!
//! This module provides comprehensive error handling with detailed error types
//! for different failure scenarios in database operations.

use std::fmt;
use thiserror::Error;

/// The main error type for MSSQL operations.
#[derive(Error, Debug)]
pub enum MssqlError {
    /// Connection-related errors
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),

    /// Query execution errors
    #[error("Query error: {0}")]
    Query(#[from] QueryError),

    /// Transaction errors
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),

    /// Schema manipulation errors
    #[error("Schema error: {0}")]
    Schema(#[from] SchemaError),

    /// Migration errors
    #[error("Migration error: {0}")]
    Migration(#[from] MigrationError),

    /// Pool errors
    #[error("Pool error: {0}")]
    Pool(#[from] PoolError),

    /// Type conversion errors
    #[error("Type conversion error: {0}")]
    TypeConversion(#[from] TypeConversionError),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Underlying Tiberius driver error
    #[error("Driver error: {0}")]
    Driver(#[from] tiberius::error::Error),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error with message
    #[error("{0}")]
    Other(String),
}

/// Connection-specific errors.
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Failed to connect to server: {host}:{port} - {reason}")]
    ConnectionFailed {
        host: String,
        port: u16,
        reason: String,
    },

    #[error("Authentication failed for user '{user}': {reason}")]
    AuthenticationFailed { user: String, reason: String },

    #[error("Database '{database}' not found or inaccessible")]
    DatabaseNotFound { database: String },

    #[error("Connection timeout after {timeout_secs} seconds")]
    Timeout { timeout_secs: u64 },

    #[error("Connection string parse error: {0}")]
    InvalidConnectionString(String),

    #[error("SSL/TLS error: {0}")]
    TlsError(String),

    #[error("Connection pool exhausted")]
    PoolExhausted,

    #[error("Connection was closed unexpectedly")]
    ConnectionClosed,

    #[error("Maximum retry attempts ({attempts}) exceeded")]
    MaxRetriesExceeded { attempts: u32 },
}

/// Query execution errors.
#[derive(Error, Debug)]
pub enum QueryError {
    #[error("SQL syntax error near '{near}': {message}")]
    SyntaxError { near: String, message: String },

    #[error("Invalid column '{column}' in table '{table}'")]
    InvalidColumn { column: String, table: String },

    #[error("Table '{table}' does not exist")]
    TableNotFound { table: String },

    #[error("Constraint violation: {constraint} - {message}")]
    ConstraintViolation { constraint: String, message: String },

    #[error("Duplicate key violation on '{key}'")]
    DuplicateKey { key: String },

    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("NULL value not allowed for column '{column}'")]
    NullConstraint { column: String },

    #[error("Query timeout after {timeout_secs} seconds")]
    Timeout { timeout_secs: u64 },

    #[error("Deadlock detected, transaction was rolled back")]
    Deadlock,

    #[error("No rows affected by the operation")]
    NoRowsAffected,

    #[error("Expected single row, got {count} rows")]
    UnexpectedRowCount { count: usize },

    #[error("Parameter '{name}' is missing or invalid")]
    InvalidParameter { name: String },

    #[error("Query builder error: {0}")]
    BuilderError(String),

    #[error("Result mapping error: {0}")]
    MappingError(String),
}

/// Transaction-specific errors.
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Transaction already started")]
    AlreadyStarted,

    #[error("No active transaction")]
    NotStarted,

    #[error("Transaction was rolled back: {reason}")]
    RolledBack { reason: String },

    #[error("Savepoint '{name}' not found")]
    SavepointNotFound { name: String },

    #[error("Nested transaction error: {0}")]
    NestedError(String),

    #[error("Transaction isolation level '{level}' not supported")]
    UnsupportedIsolationLevel { level: String },

    #[error("Transaction commit failed: {0}")]
    CommitFailed(String),
}

/// Schema manipulation errors.
#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Table '{table}' already exists")]
    TableAlreadyExists { table: String },

    #[error("Table '{table}' does not exist")]
    TableNotFound { table: String },

    #[error("Column '{column}' already exists in table '{table}'")]
    ColumnAlreadyExists { table: String, column: String },

    #[error("Column '{column}' does not exist in table '{table}'")]
    ColumnNotFound { table: String, column: String },

    #[error("Index '{index}' already exists")]
    IndexAlreadyExists { index: String },

    #[error("Index '{index}' does not exist")]
    IndexNotFound { index: String },

    #[error("Cannot drop primary key from table '{table}'")]
    CannotDropPrimaryKey { table: String },

    #[error("Invalid data type: {0}")]
    InvalidDataType(String),

    #[error("Schema '{schema}' does not exist")]
    SchemaNotFound { schema: String },

    #[error("Invalid constraint definition: {0}")]
    InvalidConstraint(String),
}

/// Migration-specific errors.
#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("Migration '{version}' not found")]
    NotFound { version: String },

    #[error("Migration '{version}' already applied")]
    AlreadyApplied { version: String },

    #[error("Migration '{version}' failed: {reason}")]
    Failed { version: String, reason: String },

    #[error("Migration checksum mismatch for '{version}'")]
    ChecksumMismatch { version: String },

    #[error("Cannot rollback migration '{version}': no down migration defined")]
    NoDownMigration { version: String },

    #[error("Migration lock acquisition failed")]
    LockFailed,

    #[error("Migration directory not found: {path}")]
    DirectoryNotFound { path: String },

    #[error("Invalid migration file format: {0}")]
    InvalidFormat(String),
}

/// Connection pool errors.
#[derive(Error, Debug)]
pub enum PoolError {
    #[error("Pool initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Failed to get connection from pool: {0}")]
    GetConnectionFailed(String),

    #[error("Pool is closed")]
    PoolClosed,

    #[error("Connection health check failed")]
    HealthCheckFailed,

    #[error("Pool configuration error: {0}")]
    ConfigurationError(String),
}

/// Type conversion errors.
#[derive(Error, Debug)]
pub enum TypeConversionError {
    #[error("Cannot convert '{from}' to '{to}'")]
    ConversionFailed { from: String, to: String },

    #[error("Value overflow when converting to '{target_type}'")]
    Overflow { target_type: String },

    #[error("Invalid format for type '{target_type}': {value}")]
    InvalidFormat { target_type: String, value: String },

    #[error("Unexpected NULL value for non-nullable type")]
    UnexpectedNull,

    #[error("Invalid date/time format: {0}")]
    InvalidDateTime(String),

    #[error("Invalid UUID format: {0}")]
    InvalidUuid(String),

    #[error("Invalid decimal format: {0}")]
    InvalidDecimal(String),
}

/// Result type alias for MSSQL operations.
pub type MssqlResult<T> = Result<T, MssqlError>;

/// Extension trait for adding context to errors.
pub trait ErrorContext<T> {
    /// Add context to an error.
    fn with_context<F, S>(self, f: F) -> MssqlResult<T>
    where
        F: FnOnce() -> S,
        S: Into<String>;

    /// Add static context to an error.
    fn context<S: Into<String>>(self, msg: S) -> MssqlResult<T>;
}

impl<T, E: Into<MssqlError>> ErrorContext<T> for Result<T, E> {
    fn with_context<F, S>(self, f: F) -> MssqlResult<T>
    where
        F: FnOnce() -> S,
        S: Into<String>,
    {
        self.map_err(|e| {
            let original = e.into();
            MssqlError::Other(format!("{}: {}", f().into(), original))
        })
    }

    fn context<S: Into<String>>(self, msg: S) -> MssqlResult<T> {
        self.map_err(|e| {
            let original = e.into();
            MssqlError::Other(format!("{}: {}", msg.into(), original))
        })
    }
}

/// SQL Server error code mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlServerErrorCode {
    /// Deadlock victim (1205)
    Deadlock = 1205,
    /// Lock request timeout (1222)
    LockTimeout = 1222,
    /// Duplicate key (2601, 2627)
    DuplicateKey = 2627,
    /// Foreign key violation (547)
    ForeignKeyViolation = 547,
    /// Invalid object name (208)
    InvalidObjectName = 208,
    /// Invalid column name (207)
    InvalidColumnName = 207,
    /// Syntax error (102)
    SyntaxError = 102,
    /// Permission denied (229)
    PermissionDenied = 229,
    /// Database does not exist (4060)
    DatabaseNotExist = 4060,
    /// Login failed (18456)
    LoginFailed = 18456,
    /// Connection timeout (258)
    ConnectionTimeout = 258,
}

impl SqlServerErrorCode {
    /// Create from a SQL Server error number.
    pub fn from_number(number: u32) -> Option<Self> {
        match number {
            1205 => Some(Self::Deadlock),
            1222 => Some(Self::LockTimeout),
            2601 | 2627 => Some(Self::DuplicateKey),
            547 => Some(Self::ForeignKeyViolation),
            208 => Some(Self::InvalidObjectName),
            207 => Some(Self::InvalidColumnName),
            102 => Some(Self::SyntaxError),
            229 => Some(Self::PermissionDenied),
            4060 => Some(Self::DatabaseNotExist),
            18456 => Some(Self::LoginFailed),
            258 => Some(Self::ConnectionTimeout),
            _ => None,
        }
    }
}

impl fmt::Display for SqlServerErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deadlock => write!(f, "Deadlock"),
            Self::LockTimeout => write!(f, "Lock timeout"),
            Self::DuplicateKey => write!(f, "Duplicate key"),
            Self::ForeignKeyViolation => write!(f, "Foreign key violation"),
            Self::InvalidObjectName => write!(f, "Invalid object name"),
            Self::InvalidColumnName => write!(f, "Invalid column name"),
            Self::SyntaxError => write!(f, "Syntax error"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::DatabaseNotExist => write!(f, "Database does not exist"),
            Self::LoginFailed => write!(f, "Login failed"),
            Self::ConnectionTimeout => write!(f, "Connection timeout"),
        }
    }
}

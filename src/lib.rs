//! # MSSQL Toolkit
//!
//! A comprehensive Rust library for managing Microsoft SQL Server databases.
//!
//! ## Features
//!
//! - **Connection Management**: Easy-to-use connection handling with support for
//!   connection strings, named instances, and various authentication modes.
//! - **Query Builder**: Type-safe, fluent API for building SELECT, INSERT, UPDATE,
//!   DELETE queries with support for JOINs, CTEs, and more.
//! - **Schema Management**: Create and modify tables, indexes, constraints, and
//!   other database objects.
//! - **Transaction Support**: Full transaction support with savepoints, isolation
//!   levels, and automatic retry on deadlock.
//! - **Connection Pooling**: Built-in connection pooling with bb8 for efficient
//!   connection management.
//! - **Migration System**: Database migrations with version tracking, checksums,
//!   and up/down migrations.
//! - **Stored Procedures**: Create, execute, and manage stored procedures and
//!   user-defined functions.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use mssql_toolkit::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), MssqlError> {
//!     // Connect to the database
//!     let conn = MssqlConnection::connect(
//!         ConnectionConfig::new("localhost", "my_database")
//!             .username("sa")
//!             .password("my_password")
//!             .trust_server_certificate(true)
//!     ).await?;
//!
//!     // Execute a query
//!     let rows = conn.query("SELECT * FROM users WHERE active = @p0", &[SqlValue::Bool(true)]).await?;
//!
//!     for row in rows {
//!         let name: String = row.get(1)?;
//!         println!("User: {}", name);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Using the Query Builder
//!
//! ```rust,no_run
//! use mssql_toolkit::query::*;
//!
//! // Build a SELECT query
//! let query = select()
//!     .select(["id", "name", "email"])
//!     .from("users")
//!     .where_clause(col("status").eq("active"))
//!     .order_by_asc("name")
//!     .limit(10)
//!     .build()
//!     .unwrap();
//!
//! // Build an INSERT query
//! let insert = insert_into("users")
//!     .columns(["name", "email"])
//!     .values_literal(["John Doe", "john@example.com"])
//!     .output_inserted(["id"])
//!     .build()
//!     .unwrap();
//! ```
//!
//! ## Schema Management
//!
//! ```rust,no_run
//! use mssql_toolkit::schema::*;
//!
//! // Create a table
//! let create_sql = create_table("users")
//!     .column(column("id").int().identity().not_null())
//!     .column(column("name").nvarchar(100).not_null())
//!     .column(column("email").nvarchar(255).not_null())
//!     .column(column("created_at").datetime2(7).not_null().default_now())
//!     .primary_key(primary_key(["id"]).name("PK_users"))
//!     .unique(unique(["email"]).name("UQ_users_email"))
//!     .to_create_sql()
//!     .unwrap();
//! ```
//!
//! ## Connection Pooling
//!
//! ```rust,no_run
//! use mssql_toolkit::pool::*;
//! use mssql_toolkit::connection::ConnectionConfig;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), mssql_toolkit::error::MssqlError> {
//!     let pool = MssqlPool::new(
//!         ConnectionConfig::new("localhost", "my_database")
//!             .username("sa")
//!             .password("password"),
//!         PoolConfig::new()
//!             .max_size(10)
//!             .min_idle(2)
//!     ).await?;
//!
//!     let mut conn = pool.get().await?;
//!     let rows = conn.query("SELECT 1", &[]).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Transactions
//!
//! ```rust,no_run
//! use mssql_toolkit::prelude::*;
//! use mssql_toolkit::transaction::*;
//!
//! async fn transfer_funds(conn: &MssqlConnection) -> MssqlResult<()> {
//!     let tx = conn.begin_transaction_with_level(IsolationLevel::Serializable).await?;
//!
//!     tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = 1", &[]).await?;
//!     tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = 2", &[]).await?;
//!
//!     tx.commit().await?;
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod connection;
pub mod error;
pub mod migration;
pub mod pool;
pub mod query;
pub mod schema;
pub mod stored_procedure;
pub mod transaction;
pub mod types;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::connection::{
        ConnectionConfig, ConnectionConfigBuilder, MssqlConnection, MssqlRow,
        FromSqlRow, FromSqlValue,
    };
    pub use crate::error::{MssqlError, MssqlResult};
    pub use crate::query::{
        col, lit, raw, select, select_all, insert_into, update_table, delete_from,
        QueryBuilder, Expr,
    };
    pub use crate::schema::{
        create_table, column, primary_key, unique, foreign_key, check,
        create_index,
    };
    pub use crate::transaction::{IsolationLevel, TransactionExt};
    pub use crate::types::{SqlType, SqlValue};
}

// Re-exports for convenience
pub use connection::MssqlConnection;
pub use error::{MssqlError, MssqlResult};
pub use pool::MssqlPool;
pub use types::SqlValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelude_imports() {
        use prelude::*;

        // Test that common types are accessible
        let _config = ConnectionConfig::new("localhost", "testdb")
            .username("sa")
            .password("password");

        let _value = SqlValue::Int(42);
    }

    #[test]
    fn test_query_builder() {
        use query::*;

        let sql = select()
            .select(["id", "name"])
            .from("users")
            .where_clause(col("active").eq(true))
            .build()
            .unwrap();

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM [users]"));
    }

    #[test]
    fn test_schema_builder() {
        use schema::*;

        let sql = create_table("test")
            .column(column("id").int().not_null())
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("CREATE TABLE"));
    }
}

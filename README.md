# mssql_toolkit

A comprehensive Rust library for Microsoft SQL Server database management.

## Features

- **Connection Management** - Easy connection handling with support for various authentication modes
- **Query Builder** - Type-safe, fluent API for SELECT, INSERT, UPDATE, DELETE queries
- **Schema Management** - Create and modify tables, indexes, and constraints
- **Connection Pooling** - Built-in pooling with bb8
- **Transaction Support** - Full transactions with savepoints and isolation levels
- **Migration System** - Version-tracked database migrations
- **Stored Procedures** - Create and execute stored procedures

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mssql_toolkit = "0.1.0"
```

## Quick Start

```rust
use mssql_toolkit::prelude::*;

#[tokio::main]
async fn main() -> Result<(), MssqlError> {
    // Connect to the database
    let conn = MssqlConnection::connect(
        ConnectionConfig::new("localhost", "my_database")
            .username("sa")
            .password("my_password")
            .trust_server_certificate(true)
    ).await?;

    // Execute a query
    let rows = conn.query("SELECT * FROM users WHERE active = @p0", &[SqlValue::Bool(true)]).await?;

    for row in rows {
        let name: String = row.get(1)?;
        println!("User: {}", name);
    }

    Ok(())
}
```

## Usage Examples

### Query Builder

```rust
use mssql_toolkit::query::*;

// SELECT query
let query = select()
    .select(["id", "name", "email"])
    .from("users")
    .where_clause(col("status").eq("active"))
    .order_by_asc("name")
    .limit(10)
    .build()
    .unwrap();

// INSERT query
let insert = insert_into("users")
    .columns(["name", "email"])
    .values_literal(["John Doe", "john@example.com"])
    .output_inserted(["id"])
    .build()
    .unwrap();
```

### Schema Management

```rust
use mssql_toolkit::schema::*;

let create_sql = create_table("users")
    .column(column("id").int().identity().not_null())
    .column(column("name").nvarchar(100).not_null())
    .column(column("email").nvarchar(255).not_null())
    .column(column("created_at").datetime2(7).not_null().default_now())
    .primary_key(primary_key(["id"]).name("PK_users"))
    .unique(unique(["email"]).name("UQ_users_email"))
    .to_create_sql()
    .unwrap();
```

### Connection Pooling

```rust
use mssql_toolkit::pool::*;
use mssql_toolkit::connection::ConnectionConfig;

#[tokio::main]
async fn main() -> Result<(), mssql_toolkit::error::MssqlError> {
    let pool = MssqlPool::new(
        ConnectionConfig::new("localhost", "my_database")
            .username("sa")
            .password("password"),
        PoolConfig::new()
            .max_size(10)
            .min_idle(2)
    ).await?;

    let mut conn = pool.get().await?;
    let rows = conn.query("SELECT 1", &[]).await?;

    Ok(())
}
```

### Transactions

```rust
use mssql_toolkit::prelude::*;
use mssql_toolkit::transaction::*;

async fn transfer_funds(conn: &MssqlConnection) -> MssqlResult<()> {
    let tx = conn.begin_transaction_with_level(IsolationLevel::Serializable).await?;

    tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = 1", &[]).await?;
    tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = 2", &[]).await?;

    tx.commit().await?;
    Ok(())
}
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

Ahmed Alshehri
https://github.com/ahxp

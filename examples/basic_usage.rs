//! Basic usage example for MSSQL Toolkit.
//!
//! This example demonstrates:
//! - Connecting to a database
//! - Executing queries
//! - Using the query builder
//! - Basic CRUD operations

use mssql_toolkit::prelude::*;
use mssql_toolkit::query::*;

#[tokio::main]
async fn main() -> MssqlResult<()> {
    // Configure the connection
    let config = ConnectionConfig::new("localhost", "my_database")
        .username("sa")
        .password("YourPassword123!")
        .trust_server_certificate(true);

    // Alternative: Use a connection string
    // let conn = MssqlConnection::connect_with_string(
    //     "Server=localhost,1433;Database=my_database;User Id=sa;Password=YourPassword123!;TrustServerCertificate=true"
    // ).await?;

    // Connect to the database
    println!("Connecting to database...");
    let conn = MssqlConnection::connect(config).await?;

    // Get server information
    let version = conn.server_version().await?;
    println!("Connected to: {}", version);

    let current_db = conn.current_database().await?;
    println!("Current database: {}", current_db);

    // Execute a simple query
    println!("\n--- Simple Query ---");
    let rows = conn.query("SELECT @@VERSION AS Version", &[]).await?;
    for row in &rows {
        let version: String = row.get(0)?;
        println!("SQL Server Version: {}", version);
    }

    // Using parameters
    println!("\n--- Parameterized Query ---");
    let rows = conn
        .query(
            "SELECT TOP 5 name, create_date FROM sys.databases WHERE name LIKE @p0",
            &[SqlValue::String("%master%".to_string())],
        )
        .await?;

    for row in &rows {
        let name: String = row.get(0)?;
        println!("Database: {}", name);
    }

    // Using the query builder
    println!("\n--- Query Builder ---");

    // SELECT query
    let select_sql = select()
        .select(["name", "database_id", "create_date"])
        .from("sys.databases")
        .where_clause(col("database_id").le(4))
        .order_by_asc("name")
        .build()?;

    println!("Generated SQL: {}", select_sql);

    let rows = conn.query(&select_sql, &[]).await?;
    println!("Found {} databases", rows.len());

    // INSERT query (example - doesn't actually execute)
    let insert_sql = insert_into("users")
        .columns(["name", "email", "age"])
        .values_literal(["John Doe", "john@example.com", "30"])
        .output_inserted(["id"])
        .build()?;

    println!("\nGenerated INSERT: {}", insert_sql);

    // UPDATE query (example)
    let update_sql = update_table("users")
        .set_literal("status", "active")
        .set("updated_at", Expr::raw("GETDATE()"))
        .where_clause(col("id").eq(1))
        .build()?;

    println!("Generated UPDATE: {}", update_sql);

    // DELETE query (example)
    let delete_sql = delete_from("users")
        .where_clause(col("status").eq("deleted").and(col("deleted_at").lt("2024-01-01")))
        .build()?;

    println!("Generated DELETE: {}", delete_sql);

    // Complex query with joins
    println!("\n--- Complex Query ---");
    let complex_sql = select()
        .distinct()
        .select(["u.name", "u.email"])
        .select_as(col("o.total").sum(), "total_orders")
        .from("users")
        .inner_join("orders", col("u.id").eq(Expr::col("o.user_id")))
        .where_clause(col("u.status").eq("active"))
        .group_by_col("u.name")
        .group_by_col("u.email")
        .having(col("o.total").sum().gt(100))
        .order_by_desc("total_orders")
        .limit(10)
        .build()?;

    println!("Complex query:\n{}", complex_sql);

    // Execute batch (DDL statements)
    println!("\n--- Batch Execution ---");
    conn.execute_batch(
        r#"
        -- This is a batch of SQL statements
        PRINT 'Hello from MSSQL Toolkit!';
        SELECT GETDATE() AS CurrentTime;
        "#,
    )
    .await?;
    println!("Batch executed successfully");

    // Close the connection
    conn.close().await?;
    println!("\nConnection closed. Goodbye!");

    Ok(())
}

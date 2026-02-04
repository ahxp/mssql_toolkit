//! Connection pooling example for MSSQL Toolkit.
//!
//! This example demonstrates:
//! - Creating a connection pool
//! - Acquiring and using pooled connections
//! - Pool configuration options
//! - Pool statistics

use std::time::Duration;

use mssql_toolkit::connection::ConnectionConfig;
use mssql_toolkit::pool::*;
use mssql_toolkit::error::MssqlResult;

#[tokio::main]
async fn main() -> MssqlResult<()> {
    // =====================================================
    // Basic Pool Creation
    // =====================================================
    println!("=== Basic Pool Creation ===\n");

    let connection_config = ConnectionConfig::new("localhost", "my_database")
        .username("sa")
        .password("YourPassword123!")
        .trust_server_certificate(true);

    let pool_config = PoolConfig::new()
        .max_size(10)
        .min_idle(2)
        .connection_timeout(Duration::from_secs(30))
        .max_lifetime(Duration::from_secs(30 * 60))
        .idle_timeout(Duration::from_secs(10 * 60))
        .test_on_checkout(true);

    println!("Creating connection pool...");
    let pool = MssqlPool::new(connection_config, pool_config).await?;

    println!("Pool created successfully!");
    print_pool_stats(&pool);

    // =====================================================
    // Using Pooled Connections
    // =====================================================
    println!("\n=== Using Pooled Connections ===\n");

    // Get a connection from the pool
    {
        let mut conn = pool.get().await?;
        println!("Got connection from pool");

        // Execute a query
        let db_name = conn.current_database().await?;
        println!("Current database: {}", db_name);

        let rows = conn.query("SELECT @@VERSION", &[]).await?;
        for row in &rows {
            let version: String = row.get(0)?;
            println!("Server version: {}...", &version[..80.min(version.len())]);
        }

        // Connection automatically returns to pool when dropped
    }
    println!("Connection returned to pool");

    print_pool_stats(&pool);

    // =====================================================
    // Parallel Connection Usage
    // =====================================================
    println!("\n=== Parallel Connection Usage ===\n");

    // Simulate concurrent database access
    let tasks: Vec<_> = (0..5)
        .map(|i| {
            let pool_ref = &pool;
            async move {
                let mut conn = pool_ref.get().await?;
                let rows = conn
                    .query(&format!("SELECT {} AS TaskId, GETDATE() AS Time", i), &[])
                    .await?;

                if let Some(row) = rows.first() {
                    let task_id: i32 = row.get(0)?;
                    println!("Task {} completed", task_id);
                }
                Ok::<_, mssql_toolkit::error::MssqlError>(())
            }
        })
        .collect();

    // Run tasks concurrently
    // Note: In a real application, you'd use tokio::spawn
    for task in tasks {
        task.await?;
    }

    print_pool_stats(&pool);

    // =====================================================
    // Pool Builder Pattern
    // =====================================================
    println!("\n=== Pool Builder Pattern ===\n");

    // Alternative way to create a pool using the builder
    let _pool2 = PoolBuilder::new()
        .connection_string("Server=localhost,1433;Database=my_database;User Id=sa;Password=YourPassword123!;TrustServerCertificate=true")?
        .max_size(20)
        .min_idle(5)
        .connection_timeout(Duration::from_secs(60))
        .test_on_checkout(true)
        .build()
        .await;

    // Note: This will fail if the database isn't available,
    // which is fine for demonstration purposes
    match _pool2 {
        Ok(p) => {
            println!("Pool 2 created successfully");
            print_pool_stats(&p);
        }
        Err(e) => {
            println!("Pool 2 creation failed (expected if DB not available): {}", e);
        }
    }

    // =====================================================
    // Dedicated Connections
    // =====================================================
    println!("\n=== Dedicated Connections ===\n");

    // Get a dedicated (non-pooled) connection for special operations
    println!("Creating dedicated connection...");
    match pool.get_dedicated().await {
        Ok(dedicated_conn) => {
            println!("Dedicated connection created");
            // This connection won't return to the pool
            dedicated_conn.close().await?;
            println!("Dedicated connection closed");
        }
        Err(e) => {
            println!("Dedicated connection failed: {}", e);
        }
    }

    print_pool_stats(&pool);

    println!("\n=== Done ===");

    Ok(())
}

fn print_pool_stats(pool: &MssqlPool) {
    let stats = pool.stats();
    println!("Pool Statistics:");
    println!("  - Total connections: {}", stats.connections);
    println!("  - Idle connections: {}", stats.idle_connections);
    println!("  - Active connections: {}", stats.active_connections());
    println!("  - Max size: {}", stats.max_size);
    println!("  - Utilization: {:.1}%", stats.utilization());
}

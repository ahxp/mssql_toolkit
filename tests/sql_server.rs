//! Live SQL Server checks. Run explicitly with:
//! MSSQL_TEST_CONNECTION_STRING='...' cargo test --test sql_server -- --ignored
//! Use an isolated test database.
use chrono::NaiveDate;
use mssql_toolkit::connection::{ConnectionConfig, MssqlConnection};
use mssql_toolkit::pool::{MssqlPool, PoolConfig};
use mssql_toolkit::types::SqlValue;
use rust_decimal::Decimal;

fn config() -> ConnectionConfig {
    ConnectionConfig::from_connection_string(&std::env::var("MSSQL_TEST_CONNECTION_STRING")
        .expect("set MSSQL_TEST_CONNECTION_STRING for live SQL Server tests"))
        .expect("valid test connection string")
}

#[tokio::test]
#[ignore = "requires an isolated SQL Server and MSSQL_TEST_CONNECTION_STRING"]
async fn parameter_and_value_roundtrips_and_batch_errors() {
    let conn = MssqlConnection::connect(config()).await.unwrap();
    let expected = vec![
        SqlValue::Int(42),
        SqlValue::String("O'Brien".into()),
        SqlValue::Binary(vec![0, 1, 255]),
        SqlValue::Decimal(Decimal::new(12345, 2)),
        SqlValue::Date(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()),
        SqlValue::Null,
    ];
    let sql = "SELECT @P1 AS a, @P2 AS b, @P3 AS c, @P4 AS d, @P5 AS e, @P6 AS f";
    let row = conn.query_one(sql, &expected).await.unwrap();
    for (i, value) in expected.iter().enumerate() {
        assert_eq!(row.get_raw(i), Some(value), "column {i}");
    }
    assert!(conn.execute_batch("SELECT 1; THROW 50000, 'batch failed', 1;").await.is_err());
    let pool = MssqlPool::new(config(), PoolConfig::new().min_idle(0)).await.unwrap();
    let mut pooled = pool.get().await.unwrap();
    let row = pooled.query_one(sql, &expected).await.unwrap();
    for (i, value) in expected.iter().enumerate() {
        assert_eq!(row.get_raw(i), Some(value), "pooled column {i}");
    }
    assert!(pooled.execute_batch("SELECT 1; THROW 50000, 'batch failed', 1;").await.is_err());
}

//! Database migration system.
//!
//! This module provides a migration system for managing database schema changes
//! over time, with support for up/down migrations, versioning, and checksums.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::connection::MssqlConnection;
use crate::error::{MigrationError, MssqlError, MssqlResult};

/// A database migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    /// Migration version (timestamp or sequential number).
    pub version: String,
    /// Migration name/description.
    pub name: String,
    /// SQL to run when migrating up.
    pub up_sql: String,
    /// SQL to run when migrating down (optional).
    pub down_sql: Option<String>,
    /// Checksum of the migration content.
    pub checksum: String,
    /// Whether this migration is transactional.
    pub transactional: bool,
}

impl Migration {
    /// Create a new migration.
    pub fn new(version: impl Into<String>, name: impl Into<String>, up_sql: impl Into<String>) -> Self {
        let up_sql = up_sql.into();
        let checksum = Self::calculate_checksum(&up_sql);

        Self {
            version: version.into(),
            name: name.into(),
            up_sql,
            down_sql: None,
            checksum,
            transactional: true,
        }
    }

    /// Set the down migration SQL.
    pub fn down(mut self, sql: impl Into<String>) -> Self {
        self.down_sql = Some(sql.into());
        self
    }

    /// Set whether the migration is transactional.
    pub fn transactional(mut self, value: bool) -> Self {
        self.transactional = value;
        self
    }

    /// Calculate checksum for migration content.
    fn calculate_checksum(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Verify the checksum matches.
    pub fn verify_checksum(&self) -> bool {
        Self::calculate_checksum(&self.up_sql) == self.checksum
    }
}

/// Record of an applied migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub version: String,
    pub name: String,
    pub checksum: String,
    pub applied_at: DateTime<Utc>,
    pub execution_time_ms: i64,
}

/// Migration runner for executing migrations.
pub struct MigrationRunner<'a> {
    connection: &'a MssqlConnection,
    migrations: Vec<Migration>,
    table_name: String,
    schema: String,
}

impl<'a> MigrationRunner<'a> {
    /// Create a new migration runner.
    pub fn new(connection: &'a MssqlConnection) -> Self {
        Self {
            connection,
            migrations: Vec::new(),
            table_name: "__migrations".to_string(),
            schema: "dbo".to_string(),
        }
    }

    /// Set the migrations table name.
    pub fn table_name(mut self, name: impl Into<String>) -> Self {
        self.table_name = name.into();
        self
    }

    /// Set the schema for the migrations table.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = schema.into();
        self
    }

    /// Add a migration.
    pub fn add_migration(mut self, migration: Migration) -> Self {
        self.migrations.push(migration);
        self
    }

    /// Add multiple migrations.
    pub fn add_migrations<I>(mut self, migrations: I) -> Self
    where
        I: IntoIterator<Item = Migration>,
    {
        self.migrations.extend(migrations);
        self
    }

    /// Get the full table name.
    fn full_table_name(&self) -> String {
        format!("[{}].[{}]", self.schema, self.table_name)
    }

    /// Ensure the migrations table exists.
    pub async fn ensure_table(&self) -> MssqlResult<()> {
        let sql = format!(
            r#"
IF NOT EXISTS (SELECT 1 FROM sys.tables t
               JOIN sys.schemas s ON t.schema_id = s.schema_id
               WHERE t.name = '{}' AND s.name = '{}')
BEGIN
    CREATE TABLE {} (
        version NVARCHAR(255) NOT NULL PRIMARY KEY,
        name NVARCHAR(500) NOT NULL,
        checksum NVARCHAR(64) NOT NULL,
        applied_at DATETIME2 NOT NULL DEFAULT GETUTCDATE(),
        execution_time_ms BIGINT NOT NULL DEFAULT 0
    );
END
"#,
            self.table_name,
            self.schema,
            self.full_table_name()
        );

        self.connection.execute_batch(&sql).await?;
        debug!("Migrations table ensured: {}", self.full_table_name());
        Ok(())
    }

    /// Get list of applied migrations.
    pub async fn get_applied_migrations(&self) -> MssqlResult<Vec<MigrationRecord>> {
        self.ensure_table().await?;

        let sql = format!(
            "SELECT version, name, checksum, applied_at, execution_time_ms FROM {} ORDER BY version",
            self.full_table_name()
        );

        let rows = self.connection.query(&sql, &[]).await?;

        let records = rows
            .into_iter()
            .map(|row| {
                Ok(MigrationRecord {
                    version: row.get::<String>(0)?,
                    name: row.get::<String>(1)?,
                    checksum: row.get::<String>(2)?,
                    applied_at: Utc::now(), // Simplified; would need proper DateTime parsing
                    execution_time_ms: row.get::<i64>(4).unwrap_or(0),
                })
            })
            .collect::<MssqlResult<Vec<_>>>()?;

        Ok(records)
    }

    /// Get pending migrations.
    pub async fn get_pending_migrations(&self) -> MssqlResult<Vec<&Migration>> {
        let applied = self.get_applied_migrations().await?;
        let applied_versions: std::collections::HashSet<_> =
            applied.iter().map(|r| r.version.as_str()).collect();

        let pending: Vec<_> = self
            .migrations
            .iter()
            .filter(|m| !applied_versions.contains(m.version.as_str()))
            .collect();

        Ok(pending)
    }

    /// Run all pending migrations.
    pub async fn migrate(&self) -> MssqlResult<Vec<String>> {
        self.ensure_table().await?;

        let pending = self.get_pending_migrations().await?;

        if pending.is_empty() {
            info!("No pending migrations");
            return Ok(Vec::new());
        }

        let mut applied = Vec::new();

        for migration in pending {
            info!("Running migration: {} - {}", migration.version, migration.name);

            let start = std::time::Instant::now();

            // Execute the migration
            if migration.transactional {
                self.connection
                    .execute_batch(&format!(
                        "BEGIN TRANSACTION; {} COMMIT TRANSACTION;",
                        migration.up_sql
                    ))
                    .await
                    .map_err(|e| {
                        error!("Migration {} failed: {}", migration.version, e);
                        MssqlError::Migration(MigrationError::Failed {
                            version: migration.version.clone(),
                            reason: e.to_string(),
                        })
                    })?;
            } else {
                self.connection.execute_batch(&migration.up_sql).await.map_err(|e| {
                    error!("Migration {} failed: {}", migration.version, e);
                    MssqlError::Migration(MigrationError::Failed {
                        version: migration.version.clone(),
                        reason: e.to_string(),
                    })
                })?;
            }

            let execution_time = start.elapsed().as_millis() as i64;

            // Record the migration
            let record_sql = format!(
                "INSERT INTO {} (version, name, checksum, execution_time_ms) VALUES ('{}', '{}', '{}', {})",
                self.full_table_name(),
                migration.version.replace('\'', "''"),
                migration.name.replace('\'', "''"),
                migration.checksum,
                execution_time
            );

            self.connection.execute_batch(&record_sql).await?;

            info!(
                "Migration {} completed in {}ms",
                migration.version, execution_time
            );
            applied.push(migration.version.clone());
        }

        Ok(applied)
    }

    /// Rollback the last N migrations.
    pub async fn rollback(&self, count: usize) -> MssqlResult<Vec<String>> {
        self.ensure_table().await?;

        let applied = self.get_applied_migrations().await?;

        if applied.is_empty() {
            warn!("No migrations to rollback");
            return Ok(Vec::new());
        }

        // Get the last N applied migrations
        let to_rollback: Vec<_> = applied.iter().rev().take(count).collect();

        let mut rolled_back = Vec::new();

        for record in to_rollback {
            // Find the migration definition
            let migration = self
                .migrations
                .iter()
                .find(|m| m.version == record.version)
                .ok_or_else(|| {
                    MssqlError::Migration(MigrationError::NotFound {
                        version: record.version.clone(),
                    })
                })?;

            let down_sql = migration.down_sql.as_ref().ok_or_else(|| {
                MssqlError::Migration(MigrationError::NoDownMigration {
                    version: record.version.clone(),
                })
            })?;

            info!("Rolling back migration: {} - {}", record.version, record.name);

            // Execute the down migration
            if migration.transactional {
                self.connection
                    .execute_batch(&format!("BEGIN TRANSACTION; {} COMMIT TRANSACTION;", down_sql))
                    .await?;
            } else {
                self.connection.execute_batch(down_sql).await?;
            }

            // Remove the migration record
            let delete_sql = format!(
                "DELETE FROM {} WHERE version = '{}'",
                self.full_table_name(),
                record.version.replace('\'', "''")
            );
            self.connection.execute_batch(&delete_sql).await?;

            info!("Migration {} rolled back", record.version);
            rolled_back.push(record.version.clone());
        }

        Ok(rolled_back)
    }

    /// Rollback to a specific version.
    pub async fn rollback_to(&self, version: &str) -> MssqlResult<Vec<String>> {
        let applied = self.get_applied_migrations().await?;

        // Find how many migrations to rollback
        let target_index = applied
            .iter()
            .position(|r| r.version == version)
            .ok_or_else(|| {
                MssqlError::Migration(MigrationError::NotFound {
                    version: version.to_string(),
                })
            })?;

        let count = applied.len() - target_index - 1;

        if count == 0 {
            info!("Already at version {}", version);
            return Ok(Vec::new());
        }

        self.rollback(count).await
    }

    /// Verify all applied migrations have matching checksums.
    pub async fn verify(&self) -> MssqlResult<Vec<String>> {
        let applied = self.get_applied_migrations().await?;
        let mut mismatches = Vec::new();

        for record in applied {
            if let Some(migration) = self.migrations.iter().find(|m| m.version == record.version) {
                if migration.checksum != record.checksum {
                    warn!(
                        "Checksum mismatch for migration {}: expected {}, got {}",
                        record.version, migration.checksum, record.checksum
                    );
                    mismatches.push(record.version);
                }
            }
        }

        Ok(mismatches)
    }

    /// Get the current version (last applied migration).
    pub async fn current_version(&self) -> MssqlResult<Option<String>> {
        let applied = self.get_applied_migrations().await?;
        Ok(applied.last().map(|r| r.version.clone()))
    }

    /// Get migration status.
    pub async fn status(&self) -> MssqlResult<MigrationStatus> {
        let applied = self.get_applied_migrations().await?;
        let pending = self.get_pending_migrations().await?;

        Ok(MigrationStatus {
            applied_count: applied.len(),
            pending_count: pending.len(),
            current_version: applied.last().map(|r| r.version.clone()),
            applied: applied,
            pending: pending.into_iter().map(|m| m.version.clone()).collect(),
        })
    }
}

/// Migration status information.
#[derive(Debug)]
pub struct MigrationStatus {
    pub applied_count: usize,
    pub pending_count: usize,
    pub current_version: Option<String>,
    pub applied: Vec<MigrationRecord>,
    pub pending: Vec<String>,
}

/// Load migrations from a directory.
pub fn load_migrations_from_dir(dir: &Path) -> MssqlResult<Vec<Migration>> {
    use std::fs;

    if !dir.exists() {
        return Err(MssqlError::Migration(MigrationError::DirectoryNotFound {
            path: dir.display().to_string(),
        }));
    }

    let mut migrations = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| MssqlError::Io(e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().map(|ext| ext == "sql").unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let filename = path.file_stem().unwrap().to_string_lossy();

        // Expected format: VERSION_name.sql or VERSION_name.up.sql / VERSION_name.down.sql
        let parts: Vec<&str> = filename.splitn(2, '_').collect();

        if parts.len() < 2 {
            warn!("Skipping malformed migration file: {:?}", path);
            continue;
        }

        let version = parts[0].to_string();
        let name_part = parts[1];

        // Check if it's an up or down file
        let (name, is_down) = if name_part.ends_with(".down") {
            (name_part.trim_end_matches(".down").to_string(), true)
        } else if name_part.ends_with(".up") {
            (name_part.trim_end_matches(".up").to_string(), false)
        } else {
            (name_part.to_string(), false)
        };

        let content = fs::read_to_string(&path).map_err(|e| MssqlError::Io(e))?;

        if is_down {
            // Find existing migration and add down SQL
            if let Some(m) = migrations.iter_mut().find(|m: &&mut Migration| m.version == version) {
                m.down_sql = Some(content);
            }
        } else {
            // Check if migration already exists (for down file pairing)
            if let Some(_m) = migrations.iter_mut().find(|m: &&mut Migration| m.version == version) {
                // Migration exists, this shouldn't happen for up files
                warn!("Duplicate migration version: {}", version);
            } else {
                migrations.push(Migration::new(version, name, content));
            }
        }
    }

    // Sort by version
    migrations.sort_by(|a, b| a.version.cmp(&b.version));

    Ok(migrations)
}

/// Builder for creating migrations programmatically.
#[derive(Debug, Default)]
pub struct MigrationBuilder {
    version: Option<String>,
    name: Option<String>,
    up_sql: Option<String>,
    down_sql: Option<String>,
    transactional: bool,
}

impl MigrationBuilder {
    /// Create a new migration builder.
    pub fn new() -> Self {
        Self {
            transactional: true,
            ..Default::default()
        }
    }

    /// Set the version.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the up SQL.
    pub fn up(mut self, sql: impl Into<String>) -> Self {
        self.up_sql = Some(sql.into());
        self
    }

    /// Set the down SQL.
    pub fn down(mut self, sql: impl Into<String>) -> Self {
        self.down_sql = Some(sql.into());
        self
    }

    /// Set whether the migration is transactional.
    pub fn transactional(mut self, value: bool) -> Self {
        self.transactional = value;
        self
    }

    /// Build the migration.
    pub fn build(self) -> MssqlResult<Migration> {
        let version = self.version.ok_or_else(|| {
            MssqlError::Migration(MigrationError::InvalidFormat(
                "Version is required".to_string(),
            ))
        })?;

        let name = self.name.ok_or_else(|| {
            MssqlError::Migration(MigrationError::InvalidFormat(
                "Name is required".to_string(),
            ))
        })?;

        let up_sql = self.up_sql.ok_or_else(|| {
            MssqlError::Migration(MigrationError::InvalidFormat(
                "Up SQL is required".to_string(),
            ))
        })?;

        let mut migration = Migration::new(version, name, up_sql).transactional(self.transactional);

        if let Some(down_sql) = self.down_sql {
            migration = migration.down(down_sql);
        }

        Ok(migration)
    }
}

/// Create a migration builder.
pub fn migration() -> MigrationBuilder {
    MigrationBuilder::new()
}

/// Generate a timestamp-based version for a new migration.
pub fn generate_version() -> String {
    Utc::now().format("%Y%m%d%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_checksum() {
        let m1 = Migration::new("001", "test", "CREATE TABLE test (id INT)");
        let m2 = Migration::new("001", "test", "CREATE TABLE test (id INT)");
        let m3 = Migration::new("001", "test", "CREATE TABLE test (id BIGINT)");

        assert_eq!(m1.checksum, m2.checksum);
        assert_ne!(m1.checksum, m3.checksum);
    }

    #[test]
    fn test_migration_builder() {
        let migration = migration()
            .version("20240101120000")
            .name("create_users_table")
            .up("CREATE TABLE users (id INT PRIMARY KEY)")
            .down("DROP TABLE users")
            .build()
            .unwrap();

        assert_eq!(migration.version, "20240101120000");
        assert!(migration.down_sql.is_some());
    }

    #[test]
    fn test_generate_version() {
        let v1 = generate_version();
        let v2 = generate_version();

        assert_eq!(v1.len(), 14);
        assert!(v1 <= v2);
    }
}

//! ALTER TABLE operations.

use crate::error::MssqlResult;
use crate::query::expression::quote_identifier;

use super::column::ColumnDefinition;
use super::constraint::{Constraint, PrimaryKeyConstraint, UniqueConstraint, ForeignKeyConstraint, CheckConstraint};

/// ALTER TABLE builder.
#[derive(Debug, Clone)]
pub struct AlterTableBuilder {
    schema: Option<String>,
    table: String,
    operations: Vec<AlterOperation>,
}

/// ALTER TABLE operation types.
#[derive(Debug, Clone)]
pub enum AlterOperation {
    /// Add a column.
    AddColumn(ColumnDefinition),
    /// Drop a column.
    DropColumn(String),
    /// Alter a column.
    AlterColumn(AlterColumnSpec),
    /// Add a constraint.
    AddConstraint(Constraint),
    /// Drop a constraint.
    DropConstraint(String),
    /// Enable/disable a constraint.
    SetConstraint { name: String, enabled: bool },
    /// Rename a column.
    RenameColumn { old_name: String, new_name: String },
    /// Add a default constraint.
    AddDefault { column: String, expression: String, name: Option<String> },
    /// Drop a default constraint.
    DropDefault(String),
    /// Enable/disable a trigger.
    SetTrigger { name: Option<String>, enabled: bool },
    /// Switch partition.
    SwitchPartition { partition: Option<i32>, target_table: String, target_partition: Option<i32> },
    /// Rebuild table.
    Rebuild { online: Option<bool>, data_compression: Option<String> },
    /// Set filegroup.
    SetFilegroup(String),
}

/// Alter column specification.
#[derive(Debug, Clone)]
pub struct AlterColumnSpec {
    pub name: String,
    pub data_type: Option<String>,
    pub nullable: Option<bool>,
    pub collation: Option<String>,
}

impl AlterColumnSpec {
    /// Create a new alter column spec.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: None,
            nullable: None,
            collation: None,
        }
    }

    /// Set the new data type.
    pub fn data_type(mut self, data_type: impl Into<String>) -> Self {
        self.data_type = Some(data_type.into());
        self
    }

    /// Set as NOT NULL.
    pub fn not_null(mut self) -> Self {
        self.nullable = Some(false);
        self
    }

    /// Set as NULL.
    pub fn null(mut self) -> Self {
        self.nullable = Some(true);
        self
    }

    /// Set the collation.
    pub fn collation(mut self, collation: impl Into<String>) -> Self {
        self.collation = Some(collation.into());
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut parts = Vec::new();
        parts.push(quote_identifier(&self.name));

        if let Some(ref dt) = self.data_type {
            parts.push(dt.clone());
        }

        if let Some(ref coll) = self.collation {
            parts.push(format!("COLLATE {}", coll));
        }

        if let Some(nullable) = self.nullable {
            if nullable {
                parts.push("NULL".to_string());
            } else {
                parts.push("NOT NULL".to_string());
            }
        }

        parts.join(" ")
    }
}

impl AlterTableBuilder {
    /// Create a new ALTER TABLE builder.
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            schema: None,
            table: table.into(),
            operations: Vec::new(),
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Get the full table name.
    fn full_table_name(&self) -> String {
        match &self.schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.table)),
            None => quote_identifier(&self.table),
        }
    }

    /// Add a column.
    pub fn add_column(mut self, column: ColumnDefinition) -> Self {
        self.operations.push(AlterOperation::AddColumn(column));
        self
    }

    /// Drop a column.
    pub fn drop_column(mut self, name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::DropColumn(name.into()));
        self
    }

    /// Alter a column.
    pub fn alter_column(mut self, spec: AlterColumnSpec) -> Self {
        self.operations.push(AlterOperation::AlterColumn(spec));
        self
    }

    /// Add a primary key constraint.
    pub fn add_primary_key(mut self, pk: PrimaryKeyConstraint) -> Self {
        self.operations.push(AlterOperation::AddConstraint(Constraint::PrimaryKey(pk)));
        self
    }

    /// Add a unique constraint.
    pub fn add_unique(mut self, uq: UniqueConstraint) -> Self {
        self.operations.push(AlterOperation::AddConstraint(Constraint::Unique(uq)));
        self
    }

    /// Add a foreign key constraint.
    pub fn add_foreign_key(mut self, fk: ForeignKeyConstraint) -> Self {
        self.operations.push(AlterOperation::AddConstraint(Constraint::ForeignKey(fk)));
        self
    }

    /// Add a check constraint.
    pub fn add_check(mut self, ck: CheckConstraint) -> Self {
        self.operations.push(AlterOperation::AddConstraint(Constraint::Check(ck)));
        self
    }

    /// Add a constraint.
    pub fn add_constraint(mut self, constraint: Constraint) -> Self {
        self.operations.push(AlterOperation::AddConstraint(constraint));
        self
    }

    /// Drop a constraint.
    pub fn drop_constraint(mut self, name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::DropConstraint(name.into()));
        self
    }

    /// Enable a constraint.
    pub fn enable_constraint(mut self, name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::SetConstraint {
            name: name.into(),
            enabled: true,
        });
        self
    }

    /// Disable a constraint (NOCHECK).
    pub fn disable_constraint(mut self, name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::SetConstraint {
            name: name.into(),
            enabled: false,
        });
        self
    }

    /// Rename a column.
    pub fn rename_column(mut self, old_name: impl Into<String>, new_name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::RenameColumn {
            old_name: old_name.into(),
            new_name: new_name.into(),
        });
        self
    }

    /// Add a default constraint.
    pub fn add_default(mut self, column: impl Into<String>, expression: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::AddDefault {
            column: column.into(),
            expression: expression.into(),
            name: None,
        });
        self
    }

    /// Add a named default constraint.
    pub fn add_default_named(
        mut self,
        name: impl Into<String>,
        column: impl Into<String>,
        expression: impl Into<String>,
    ) -> Self {
        self.operations.push(AlterOperation::AddDefault {
            column: column.into(),
            expression: expression.into(),
            name: Some(name.into()),
        });
        self
    }

    /// Drop a default constraint.
    pub fn drop_default(mut self, constraint_name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::DropDefault(constraint_name.into()));
        self
    }

    /// Enable all triggers.
    pub fn enable_all_triggers(mut self) -> Self {
        self.operations.push(AlterOperation::SetTrigger {
            name: None,
            enabled: true,
        });
        self
    }

    /// Disable all triggers.
    pub fn disable_all_triggers(mut self) -> Self {
        self.operations.push(AlterOperation::SetTrigger {
            name: None,
            enabled: false,
        });
        self
    }

    /// Enable a specific trigger.
    pub fn enable_trigger(mut self, name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::SetTrigger {
            name: Some(name.into()),
            enabled: true,
        });
        self
    }

    /// Disable a specific trigger.
    pub fn disable_trigger(mut self, name: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::SetTrigger {
            name: Some(name.into()),
            enabled: false,
        });
        self
    }

    /// Switch partition.
    pub fn switch_partition(mut self, target_table: impl Into<String>) -> Self {
        self.operations.push(AlterOperation::SwitchPartition {
            partition: None,
            target_table: target_table.into(),
            target_partition: None,
        });
        self
    }

    /// Rebuild table.
    pub fn rebuild(mut self) -> Self {
        self.operations.push(AlterOperation::Rebuild {
            online: None,
            data_compression: None,
        });
        self
    }

    /// Rebuild table with options.
    pub fn rebuild_with(mut self, online: bool, compression: Option<String>) -> Self {
        self.operations.push(AlterOperation::Rebuild {
            online: Some(online),
            data_compression: compression,
        });
        self
    }

    /// Build the ALTER TABLE statements.
    ///
    /// Note: Some operations require separate statements, so this returns
    /// a vector of SQL statements.
    pub fn build(&self) -> MssqlResult<Vec<String>> {
        let mut statements = Vec::new();
        let table_name = self.full_table_name();

        for op in &self.operations {
            match op {
                AlterOperation::AddColumn(col) => {
                    statements.push(format!(
                        "ALTER TABLE {} ADD {}",
                        table_name,
                        col.to_sql()
                    ));
                }
                AlterOperation::DropColumn(name) => {
                    statements.push(format!(
                        "ALTER TABLE {} DROP COLUMN {}",
                        table_name,
                        quote_identifier(name)
                    ));
                }
                AlterOperation::AlterColumn(spec) => {
                    statements.push(format!(
                        "ALTER TABLE {} ALTER COLUMN {}",
                        table_name,
                        spec.to_sql()
                    ));
                }
                AlterOperation::AddConstraint(constraint) => {
                    statements.push(format!(
                        "ALTER TABLE {} ADD {}",
                        table_name,
                        constraint.to_sql()
                    ));
                }
                AlterOperation::DropConstraint(name) => {
                    statements.push(format!(
                        "ALTER TABLE {} DROP CONSTRAINT {}",
                        table_name,
                        quote_identifier(name)
                    ));
                }
                AlterOperation::SetConstraint { name, enabled } => {
                    let action = if *enabled { "CHECK" } else { "NOCHECK" };
                    statements.push(format!(
                        "ALTER TABLE {} {} CONSTRAINT {}",
                        table_name,
                        action,
                        quote_identifier(name)
                    ));
                }
                AlterOperation::RenameColumn { old_name, new_name } => {
                    // SQL Server uses sp_rename for column renames
                    statements.push(format!(
                        "EXEC sp_rename '{}.{}', '{}', 'COLUMN'",
                        table_name.replace('[', "").replace(']', ""),
                        old_name,
                        new_name
                    ));
                }
                AlterOperation::AddDefault { column, expression, name } => {
                    let constraint_part = match name {
                        Some(n) => format!("CONSTRAINT {} ", quote_identifier(n)),
                        None => String::new(),
                    };
                    statements.push(format!(
                        "ALTER TABLE {} ADD {}DEFAULT {} FOR {}",
                        table_name,
                        constraint_part,
                        expression,
                        quote_identifier(column)
                    ));
                }
                AlterOperation::DropDefault(name) => {
                    statements.push(format!(
                        "ALTER TABLE {} DROP CONSTRAINT {}",
                        table_name,
                        quote_identifier(name)
                    ));
                }
                AlterOperation::SetTrigger { name, enabled } => {
                    let trigger_ref = match name {
                        Some(n) => quote_identifier(n),
                        None => "ALL".to_string(),
                    };
                    let action = if *enabled { "ENABLE" } else { "DISABLE" };
                    statements.push(format!(
                        "ALTER TABLE {} {} TRIGGER {}",
                        table_name,
                        action,
                        trigger_ref
                    ));
                }
                AlterOperation::SwitchPartition { partition, target_table, target_partition } => {
                    let mut sql = format!("ALTER TABLE {} SWITCH", table_name);
                    if let Some(p) = partition {
                        sql.push_str(&format!(" PARTITION {}", p));
                    }
                    sql.push_str(&format!(" TO {}", quote_identifier(target_table)));
                    if let Some(tp) = target_partition {
                        sql.push_str(&format!(" PARTITION {}", tp));
                    }
                    statements.push(sql);
                }
                AlterOperation::Rebuild { online, data_compression } => {
                    let mut sql = format!("ALTER TABLE {} REBUILD", table_name);
                    let mut options = Vec::new();
                    if let Some(on) = online {
                        options.push(format!("ONLINE = {}", if *on { "ON" } else { "OFF" }));
                    }
                    if let Some(ref dc) = data_compression {
                        options.push(format!("DATA_COMPRESSION = {}", dc));
                    }
                    if !options.is_empty() {
                        sql.push_str(&format!(" WITH ({})", options.join(", ")));
                    }
                    statements.push(sql);
                }
                AlterOperation::SetFilegroup(fg) => {
                    statements.push(format!(
                        "ALTER TABLE {} MOVE TO [{}]",
                        table_name,
                        fg
                    ));
                }
            }
        }

        Ok(statements)
    }

    /// Build a single ALTER TABLE statement (for simple operations).
    pub fn build_single(&self) -> MssqlResult<String> {
        let statements = self.build()?;
        Ok(statements.join(";\n"))
    }
}

/// Create an ALTER TABLE builder.
pub fn alter_table(table: impl Into<String>) -> AlterTableBuilder {
    AlterTableBuilder::new(table)
}

/// Create an alter column specification.
pub fn alter_column(name: impl Into<String>) -> AlterColumnSpec {
    AlterColumnSpec::new(name)
}

/// Rename table builder.
#[derive(Debug, Clone)]
pub struct RenameTableBuilder {
    schema: Option<String>,
    old_name: String,
    new_name: String,
}

impl RenameTableBuilder {
    /// Create a new rename table builder.
    pub fn new(old_name: impl Into<String>, new_name: impl Into<String>) -> Self {
        Self {
            schema: None,
            old_name: old_name.into(),
            new_name: new_name.into(),
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Build the rename statement.
    pub fn build(&self) -> String {
        let full_name = match &self.schema {
            Some(s) => format!("{}.{}", s, self.old_name),
            None => self.old_name.clone(),
        };
        format!("EXEC sp_rename '{}', '{}'", full_name, self.new_name)
    }
}

/// Create a rename table builder.
pub fn rename_table(old_name: impl Into<String>, new_name: impl Into<String>) -> RenameTableBuilder {
    RenameTableBuilder::new(old_name, new_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::column::column;
    use crate::schema::constraint::{foreign_key, check, ReferentialAction};

    #[test]
    fn test_add_column() {
        let stmts = alter_table("users")
            .add_column(column("phone").nvarchar(20).null())
            .build()
            .unwrap();

        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("ADD"));
        assert!(stmts[0].contains("[phone]"));
    }

    #[test]
    fn test_drop_column() {
        let stmts = alter_table("users")
            .drop_column("old_column")
            .build()
            .unwrap();

        assert!(stmts[0].contains("DROP COLUMN [old_column]"));
    }

    #[test]
    fn test_alter_column() {
        let stmts = alter_table("users")
            .alter_column(alter_column("name").data_type("NVARCHAR(200)").not_null())
            .build()
            .unwrap();

        assert!(stmts[0].contains("ALTER COLUMN"));
        assert!(stmts[0].contains("NVARCHAR(200)"));
        assert!(stmts[0].contains("NOT NULL"));
    }

    #[test]
    fn test_add_foreign_key() {
        let stmts = alter_table("orders")
            .add_foreign_key(
                foreign_key(["user_id"], "users", ["id"])
                    .name("FK_orders_users")
                    .on_delete(ReferentialAction::Cascade),
            )
            .build()
            .unwrap();

        assert!(stmts[0].contains("ADD CONSTRAINT"));
        assert!(stmts[0].contains("FOREIGN KEY"));
    }

    #[test]
    fn test_rename_column() {
        let stmts = alter_table("users")
            .rename_column("old_name", "new_name")
            .build()
            .unwrap();

        assert!(stmts[0].contains("sp_rename"));
        assert!(stmts[0].contains("COLUMN"));
    }

    #[test]
    fn test_disable_enable_constraint() {
        let stmts = alter_table("orders")
            .disable_constraint("FK_orders_users")
            .build()
            .unwrap();

        assert!(stmts[0].contains("NOCHECK CONSTRAINT"));
    }

    #[test]
    fn test_multiple_operations() {
        let stmts = alter_table("users")
            .add_column(column("phone").nvarchar(20))
            .drop_column("fax")
            .build()
            .unwrap();

        assert_eq!(stmts.len(), 2);
    }
}

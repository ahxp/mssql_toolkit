//! Table creation and management.

use crate::error::{MssqlResult, SchemaError, MssqlError};
use crate::query::expression::quote_identifier;

use super::column::ColumnDefinition;
use super::constraint::{Constraint, PrimaryKeyConstraint, UniqueConstraint, ForeignKeyConstraint, CheckConstraint};
use super::SchemaObject;

/// CREATE TABLE builder.
#[derive(Debug, Clone)]
pub struct CreateTableBuilder {
    schema: Option<String>,
    name: String,
    columns: Vec<ColumnDefinition>,
    constraints: Vec<Constraint>,
    if_not_exists: bool,
    temporary: bool,
    filegroup: Option<String>,
    text_image_on: Option<String>,
    with_options: Vec<String>,
}

impl CreateTableBuilder {
    /// Create a new table builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
            columns: Vec::new(),
            constraints: Vec::new(),
            if_not_exists: false,
            temporary: false,
            filegroup: None,
            text_image_on: None,
            with_options: Vec::new(),
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Set IF NOT EXISTS behavior.
    pub fn if_not_exists(mut self) -> Self {
        self.if_not_exists = true;
        self
    }

    /// Create as a temporary table.
    pub fn temporary(mut self) -> Self {
        self.temporary = true;
        self
    }

    /// Create as a global temporary table.
    pub fn global_temporary(mut self) -> Self {
        self.temporary = true;
        if !self.name.starts_with("##") {
            self.name = format!("##{}", self.name);
        }
        self
    }

    /// Add a column.
    pub fn column(mut self, column: ColumnDefinition) -> Self {
        self.columns.push(column);
        self
    }

    /// Add multiple columns.
    pub fn columns<I>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = ColumnDefinition>,
    {
        self.columns.extend(columns);
        self
    }

    /// Add a primary key constraint.
    pub fn primary_key(mut self, pk: PrimaryKeyConstraint) -> Self {
        self.constraints.push(Constraint::PrimaryKey(pk));
        self
    }

    /// Add a unique constraint.
    pub fn unique(mut self, uq: UniqueConstraint) -> Self {
        self.constraints.push(Constraint::Unique(uq));
        self
    }

    /// Add a foreign key constraint.
    pub fn foreign_key(mut self, fk: ForeignKeyConstraint) -> Self {
        self.constraints.push(Constraint::ForeignKey(fk));
        self
    }

    /// Add a check constraint.
    pub fn check(mut self, ck: CheckConstraint) -> Self {
        self.constraints.push(Constraint::Check(ck));
        self
    }

    /// Add a generic constraint.
    pub fn constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Set the filegroup.
    pub fn on_filegroup(mut self, filegroup: impl Into<String>) -> Self {
        self.filegroup = Some(filegroup.into());
        self
    }

    /// Set TEXTIMAGE_ON.
    pub fn text_image_on(mut self, filegroup: impl Into<String>) -> Self {
        self.text_image_on = Some(filegroup.into());
        self
    }

    /// Add a WITH option.
    pub fn with_option(mut self, option: impl Into<String>) -> Self {
        self.with_options.push(option.into());
        self
    }

    /// Enable data compression.
    pub fn data_compression(mut self, compression: DataCompression) -> Self {
        self.with_options.push(format!("DATA_COMPRESSION = {}", compression));
        self
    }

    /// Get the full table name.
    pub fn full_name(&self) -> String {
        let table_name = if self.temporary && !self.name.starts_with('#') {
            format!("#{}", self.name)
        } else {
            self.name.clone()
        };

        match &self.schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&table_name)),
            None => quote_identifier(&table_name),
        }
    }

    /// Validate the table definition.
    fn validate(&self) -> MssqlResult<()> {
        if self.columns.is_empty() {
            return Err(MssqlError::Schema(SchemaError::InvalidConstraint(
                "Table must have at least one column".to_string(),
            )));
        }

        Ok(())
    }
}

/// Data compression options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataCompression {
    None,
    Row,
    Page,
    Columnstore,
    ColumnstoreArchive,
}

impl std::fmt::Display for DataCompression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataCompression::None => write!(f, "NONE"),
            DataCompression::Row => write!(f, "ROW"),
            DataCompression::Page => write!(f, "PAGE"),
            DataCompression::Columnstore => write!(f, "COLUMNSTORE"),
            DataCompression::ColumnstoreArchive => write!(f, "COLUMNSTORE_ARCHIVE"),
        }
    }
}

impl SchemaObject for CreateTableBuilder {
    fn to_create_sql(&self) -> MssqlResult<String> {
        self.validate()?;

        let mut sql = String::new();

        if self.if_not_exists {
            sql.push_str(&format!(
                "IF NOT EXISTS (SELECT 1 FROM sys.tables WHERE name = N'{}' AND type = 'U')\nBEGIN\n",
                self.name
            ));
        }

        sql.push_str("CREATE TABLE ");
        sql.push_str(&self.full_name());
        sql.push_str(" (\n");

        // Columns
        let col_stmts: Vec<String> = self.columns.iter().map(|c| format!("    {}", c.to_sql())).collect();

        // Constraints
        let constraint_stmts: Vec<String> = self
            .constraints
            .iter()
            .map(|c| format!("    {}", c.to_sql()))
            .collect();

        let all_parts: Vec<String> = col_stmts
            .into_iter()
            .chain(constraint_stmts.into_iter())
            .collect();

        sql.push_str(&all_parts.join(",\n"));
        sql.push_str("\n)");

        // WITH options
        if !self.with_options.is_empty() {
            sql.push_str(&format!("\nWITH ({})", self.with_options.join(", ")));
        }

        // ON filegroup
        if let Some(ref fg) = self.filegroup {
            sql.push_str(&format!("\nON [{}]", fg));
        }

        // TEXTIMAGE_ON
        if let Some(ref tio) = self.text_image_on {
            sql.push_str(&format!("\nTEXTIMAGE_ON [{}]", tio));
        }

        if self.if_not_exists {
            sql.push_str("\nEND");
        }

        Ok(sql)
    }

    fn to_drop_sql(&self) -> MssqlResult<String> {
        Ok(format!("DROP TABLE IF EXISTS {}", self.full_name()))
    }
}

/// Create a table builder.
pub fn create_table(name: impl Into<String>) -> CreateTableBuilder {
    CreateTableBuilder::new(name)
}

/// Drop table builder.
#[derive(Debug, Clone)]
pub struct DropTableBuilder {
    schema: Option<String>,
    name: String,
    if_exists: bool,
}

impl DropTableBuilder {
    /// Create a new drop table builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
            if_exists: false,
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Set IF EXISTS behavior.
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }

    /// Build the DROP TABLE statement.
    pub fn build(&self) -> String {
        let full_name = match &self.schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.name)),
            None => quote_identifier(&self.name),
        };

        if self.if_exists {
            format!("DROP TABLE IF EXISTS {}", full_name)
        } else {
            format!("DROP TABLE {}", full_name)
        }
    }
}

/// Create a drop table builder.
pub fn drop_table(name: impl Into<String>) -> DropTableBuilder {
    DropTableBuilder::new(name)
}

/// Table information for introspection.
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key: Option<PrimaryKeyInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub indexes: Vec<IndexInfo>,
}

/// Column information.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub max_length: Option<i32>,
    pub precision: Option<u8>,
    pub scale: Option<u8>,
    pub is_nullable: bool,
    pub is_identity: bool,
    pub identity_seed: Option<i64>,
    pub identity_increment: Option<i64>,
    pub default_value: Option<String>,
    pub collation: Option<String>,
    pub ordinal_position: i32,
}

/// Primary key information.
#[derive(Debug, Clone)]
pub struct PrimaryKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub is_clustered: bool,
}

/// Foreign key information.
#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_schema: String,
    pub referenced_columns: Vec<String>,
    pub on_delete: String,
    pub on_update: String,
}

/// Index information.
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_clustered: bool,
    pub is_primary_key: bool,
    pub filter_definition: Option<String>,
}

/// SQL to get table columns.
pub fn get_columns_sql(schema: &str, table: &str) -> String {
    format!(
        r#"
SELECT
    c.name AS column_name,
    t.name AS data_type,
    c.max_length,
    c.precision,
    c.scale,
    c.is_nullable,
    c.is_identity,
    IDENT_SEED('{}.{}') AS identity_seed,
    IDENT_INCR('{}.{}') AS identity_increment,
    dc.definition AS default_value,
    c.collation_name,
    c.column_id AS ordinal_position
FROM sys.columns c
JOIN sys.types t ON c.user_type_id = t.user_type_id
LEFT JOIN sys.default_constraints dc ON c.default_object_id = dc.object_id
WHERE c.object_id = OBJECT_ID(N'{}.{}')
ORDER BY c.column_id
"#,
        schema, table, schema, table, schema, table
    )
}

/// SQL to get table indexes.
pub fn get_indexes_sql(schema: &str, table: &str) -> String {
    format!(
        r#"
SELECT
    i.name AS index_name,
    i.is_unique,
    i.type_desc AS index_type,
    i.is_primary_key,
    i.filter_definition,
    STRING_AGG(c.name, ', ') WITHIN GROUP (ORDER BY ic.key_ordinal) AS columns
FROM sys.indexes i
JOIN sys.index_columns ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id
JOIN sys.columns c ON ic.object_id = c.object_id AND ic.column_id = c.column_id
WHERE i.object_id = OBJECT_ID(N'{}.{}')
    AND i.name IS NOT NULL
GROUP BY i.name, i.is_unique, i.type_desc, i.is_primary_key, i.filter_definition
"#,
        schema, table
    )
}

/// SQL to get foreign keys.
pub fn get_foreign_keys_sql(schema: &str, table: &str) -> String {
    format!(
        r#"
SELECT
    fk.name AS constraint_name,
    STRING_AGG(cp.name, ', ') WITHIN GROUP (ORDER BY fkc.constraint_column_id) AS columns,
    OBJECT_SCHEMA_NAME(fk.referenced_object_id) AS referenced_schema,
    OBJECT_NAME(fk.referenced_object_id) AS referenced_table,
    STRING_AGG(cr.name, ', ') WITHIN GROUP (ORDER BY fkc.constraint_column_id) AS referenced_columns,
    fk.delete_referential_action_desc AS on_delete,
    fk.update_referential_action_desc AS on_update
FROM sys.foreign_keys fk
JOIN sys.foreign_key_columns fkc ON fk.object_id = fkc.constraint_object_id
JOIN sys.columns cp ON fkc.parent_object_id = cp.object_id AND fkc.parent_column_id = cp.column_id
JOIN sys.columns cr ON fkc.referenced_object_id = cr.object_id AND fkc.referenced_column_id = cr.column_id
WHERE fk.parent_object_id = OBJECT_ID(N'{}.{}')
GROUP BY fk.name, fk.referenced_object_id, fk.delete_referential_action_desc, fk.update_referential_action_desc
"#,
        schema, table
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::column::column;
    use crate::schema::constraint::{primary_key, foreign_key, check, ReferentialAction};

    #[test]
    fn test_create_table_basic() {
        let sql = create_table("users")
            .column(column("id").int().identity().not_null())
            .column(column("name").nvarchar(100).not_null())
            .column(column("email").nvarchar(255).not_null())
            .primary_key(primary_key(["id"]).name("PK_users"))
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("CREATE TABLE [users]"));
        assert!(sql.contains("IDENTITY"));
        assert!(sql.contains("PRIMARY KEY"));
    }

    #[test]
    fn test_create_table_with_schema() {
        let sql = create_table("users")
            .schema("dbo")
            .column(column("id").int().not_null())
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("[dbo].[users]"));
    }

    #[test]
    fn test_create_table_with_foreign_key() {
        let sql = create_table("orders")
            .column(column("id").int().identity().not_null())
            .column(column("user_id").int().not_null())
            .column(column("total").decimal(18, 2).not_null())
            .primary_key(primary_key(["id"]).name("PK_orders"))
            .foreign_key(
                foreign_key(["user_id"], "users", ["id"])
                    .name("FK_orders_users")
                    .on_delete(ReferentialAction::Cascade),
            )
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("FOREIGN KEY"));
        assert!(sql.contains("REFERENCES [users]"));
        assert!(sql.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn test_create_table_with_check() {
        let sql = create_table("products")
            .column(column("id").int().not_null())
            .column(column("price").decimal(18, 2).not_null())
            .check(check("price > 0").name("CK_products_price"))
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("CHECK (price > 0)"));
    }

    #[test]
    fn test_create_temp_table() {
        let sql = create_table("temp_data")
            .temporary()
            .column(column("id").int().not_null())
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("[#temp_data]"));
    }

    #[test]
    fn test_drop_table() {
        let sql = drop_table("users").if_exists().build();
        assert_eq!(sql, "DROP TABLE IF EXISTS [users]");
    }
}

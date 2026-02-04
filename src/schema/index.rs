//! Index creation and management.

use crate::error::MssqlResult;
use crate::query::expression::quote_identifier;

use super::constraint::IndexColumn;
use super::SchemaObject;

/// CREATE INDEX builder.
#[derive(Debug, Clone)]
pub struct CreateIndexBuilder {
    name: String,
    table_schema: Option<String>,
    table_name: String,
    columns: Vec<IndexColumn>,
    include_columns: Vec<String>,
    unique: bool,
    clustered: bool,
    where_clause: Option<String>,
    fill_factor: Option<u8>,
    pad_index: Option<bool>,
    sort_in_tempdb: Option<bool>,
    drop_existing: Option<bool>,
    online: Option<bool>,
    allow_row_locks: Option<bool>,
    allow_page_locks: Option<bool>,
    data_compression: Option<String>,
    if_not_exists: bool,
}

impl CreateIndexBuilder {
    /// Create a new index builder.
    pub fn new(name: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            table_schema: None,
            table_name: table.into(),
            columns: Vec::new(),
            include_columns: Vec::new(),
            unique: false,
            clustered: false,
            where_clause: None,
            fill_factor: None,
            pad_index: None,
            sort_in_tempdb: None,
            drop_existing: None,
            online: None,
            allow_row_locks: None,
            allow_page_locks: None,
            data_compression: None,
            if_not_exists: false,
        }
    }

    /// Set the table schema.
    pub fn table_schema(mut self, schema: impl Into<String>) -> Self {
        self.table_schema = Some(schema.into());
        self
    }

    /// Add a column to the index.
    pub fn column(mut self, name: impl Into<String>) -> Self {
        self.columns.push(IndexColumn::new(name));
        self
    }

    /// Add a column with descending order.
    pub fn column_desc(mut self, name: impl Into<String>) -> Self {
        self.columns.push(IndexColumn::new(name).desc());
        self
    }

    /// Add multiple columns.
    pub fn columns<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for col in columns {
            self.columns.push(IndexColumn::new(col));
        }
        self
    }

    /// Add an INCLUDE column.
    pub fn include(mut self, column: impl Into<String>) -> Self {
        self.include_columns.push(column.into());
        self
    }

    /// Add multiple INCLUDE columns.
    pub fn include_columns<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.include_columns.extend(columns.into_iter().map(|c| c.into()));
        self
    }

    /// Make the index unique.
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Make the index clustered.
    pub fn clustered(mut self) -> Self {
        self.clustered = true;
        self
    }

    /// Add a WHERE clause (filtered index).
    pub fn where_clause(mut self, expr: impl Into<String>) -> Self {
        self.where_clause = Some(expr.into());
        self
    }

    /// Set the fill factor.
    pub fn fill_factor(mut self, factor: u8) -> Self {
        self.fill_factor = Some(factor);
        self
    }

    /// Set PAD_INDEX option.
    pub fn pad_index(mut self, value: bool) -> Self {
        self.pad_index = Some(value);
        self
    }

    /// Set SORT_IN_TEMPDB option.
    pub fn sort_in_tempdb(mut self, value: bool) -> Self {
        self.sort_in_tempdb = Some(value);
        self
    }

    /// Set DROP_EXISTING option.
    pub fn drop_existing(mut self) -> Self {
        self.drop_existing = Some(true);
        self
    }

    /// Set ONLINE option.
    pub fn online(mut self, value: bool) -> Self {
        self.online = Some(value);
        self
    }

    /// Set ALLOW_ROW_LOCKS option.
    pub fn allow_row_locks(mut self, value: bool) -> Self {
        self.allow_row_locks = Some(value);
        self
    }

    /// Set ALLOW_PAGE_LOCKS option.
    pub fn allow_page_locks(mut self, value: bool) -> Self {
        self.allow_page_locks = Some(value);
        self
    }

    /// Set DATA_COMPRESSION option.
    pub fn data_compression(mut self, compression: impl Into<String>) -> Self {
        self.data_compression = Some(compression.into());
        self
    }

    /// Set IF NOT EXISTS behavior.
    pub fn if_not_exists(mut self) -> Self {
        self.if_not_exists = true;
        self
    }

    /// Get the full table name.
    fn full_table_name(&self) -> String {
        match &self.table_schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.table_name)),
            None => quote_identifier(&self.table_name),
        }
    }

    /// Build WITH options.
    fn build_with_options(&self) -> Option<String> {
        let mut options = Vec::new();

        if let Some(ff) = self.fill_factor {
            options.push(format!("FILLFACTOR = {}", ff));
        }
        if let Some(pi) = self.pad_index {
            options.push(format!("PAD_INDEX = {}", if pi { "ON" } else { "OFF" }));
        }
        if let Some(st) = self.sort_in_tempdb {
            options.push(format!("SORT_IN_TEMPDB = {}", if st { "ON" } else { "OFF" }));
        }
        if let Some(de) = self.drop_existing {
            options.push(format!("DROP_EXISTING = {}", if de { "ON" } else { "OFF" }));
        }
        if let Some(on) = self.online {
            options.push(format!("ONLINE = {}", if on { "ON" } else { "OFF" }));
        }
        if let Some(arl) = self.allow_row_locks {
            options.push(format!("ALLOW_ROW_LOCKS = {}", if arl { "ON" } else { "OFF" }));
        }
        if let Some(apl) = self.allow_page_locks {
            options.push(format!("ALLOW_PAGE_LOCKS = {}", if apl { "ON" } else { "OFF" }));
        }
        if let Some(ref dc) = self.data_compression {
            options.push(format!("DATA_COMPRESSION = {}", dc));
        }

        if options.is_empty() {
            None
        } else {
            Some(options.join(", "))
        }
    }
}

impl SchemaObject for CreateIndexBuilder {
    fn to_create_sql(&self) -> MssqlResult<String> {
        let mut sql = String::new();

        if self.if_not_exists {
            sql.push_str(&format!(
                "IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE name = N'{}' AND object_id = OBJECT_ID(N'{}'))\nBEGIN\n",
                self.name,
                self.full_table_name()
            ));
        }

        sql.push_str("CREATE ");

        if self.unique {
            sql.push_str("UNIQUE ");
        }

        if self.clustered {
            sql.push_str("CLUSTERED ");
        } else {
            sql.push_str("NONCLUSTERED ");
        }

        sql.push_str(&format!("INDEX {} ON {}", quote_identifier(&self.name), self.full_table_name()));

        // Columns
        let cols: Vec<String> = self.columns.iter().map(|c| c.to_sql()).collect();
        sql.push_str(&format!(" ({})", cols.join(", ")));

        // Include columns
        if !self.include_columns.is_empty() {
            let includes: Vec<String> = self.include_columns.iter().map(|c| quote_identifier(c)).collect();
            sql.push_str(&format!(" INCLUDE ({})", includes.join(", ")));
        }

        // WHERE clause
        if let Some(ref where_clause) = self.where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }

        // WITH options
        if let Some(options) = self.build_with_options() {
            sql.push_str(&format!(" WITH ({})", options));
        }

        if self.if_not_exists {
            sql.push_str("\nEND");
        }

        Ok(sql)
    }

    fn to_drop_sql(&self) -> MssqlResult<String> {
        Ok(format!(
            "DROP INDEX IF EXISTS {} ON {}",
            quote_identifier(&self.name),
            self.full_table_name()
        ))
    }
}

/// Create an index builder.
pub fn create_index(name: impl Into<String>, table: impl Into<String>) -> CreateIndexBuilder {
    CreateIndexBuilder::new(name, table)
}

/// Create a unique index builder.
pub fn create_unique_index(name: impl Into<String>, table: impl Into<String>) -> CreateIndexBuilder {
    CreateIndexBuilder::new(name, table).unique()
}

/// Drop index builder.
#[derive(Debug, Clone)]
pub struct DropIndexBuilder {
    name: String,
    table_schema: Option<String>,
    table_name: String,
    if_exists: bool,
}

impl DropIndexBuilder {
    /// Create a new drop index builder.
    pub fn new(name: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            table_schema: None,
            table_name: table.into(),
            if_exists: false,
        }
    }

    /// Set the table schema.
    pub fn table_schema(mut self, schema: impl Into<String>) -> Self {
        self.table_schema = Some(schema.into());
        self
    }

    /// Set IF EXISTS behavior.
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }

    /// Build the DROP INDEX statement.
    pub fn build(&self) -> String {
        let full_table_name = match &self.table_schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.table_name)),
            None => quote_identifier(&self.table_name),
        };

        if self.if_exists {
            format!(
                "DROP INDEX IF EXISTS {} ON {}",
                quote_identifier(&self.name),
                full_table_name
            )
        } else {
            format!(
                "DROP INDEX {} ON {}",
                quote_identifier(&self.name),
                full_table_name
            )
        }
    }
}

/// Create a drop index builder.
pub fn drop_index(name: impl Into<String>, table: impl Into<String>) -> DropIndexBuilder {
    DropIndexBuilder::new(name, table)
}

/// Rebuild index builder.
#[derive(Debug, Clone)]
pub struct RebuildIndexBuilder {
    index_name: Option<String>,
    table_schema: Option<String>,
    table_name: String,
    online: Option<bool>,
    fill_factor: Option<u8>,
    sort_in_tempdb: Option<bool>,
    max_dop: Option<u8>,
    data_compression: Option<String>,
}

impl RebuildIndexBuilder {
    /// Create a new rebuild index builder for a specific index.
    pub fn index(name: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            index_name: Some(name.into()),
            table_schema: None,
            table_name: table.into(),
            online: None,
            fill_factor: None,
            sort_in_tempdb: None,
            max_dop: None,
            data_compression: None,
        }
    }

    /// Create a new rebuild index builder for all indexes on a table.
    pub fn all(table: impl Into<String>) -> Self {
        Self {
            index_name: None,
            table_schema: None,
            table_name: table.into(),
            online: None,
            fill_factor: None,
            sort_in_tempdb: None,
            max_dop: None,
            data_compression: None,
        }
    }

    /// Set the table schema.
    pub fn table_schema(mut self, schema: impl Into<String>) -> Self {
        self.table_schema = Some(schema.into());
        self
    }

    /// Set ONLINE option.
    pub fn online(mut self, value: bool) -> Self {
        self.online = Some(value);
        self
    }

    /// Set fill factor.
    pub fn fill_factor(mut self, factor: u8) -> Self {
        self.fill_factor = Some(factor);
        self
    }

    /// Set SORT_IN_TEMPDB option.
    pub fn sort_in_tempdb(mut self, value: bool) -> Self {
        self.sort_in_tempdb = Some(value);
        self
    }

    /// Set MAXDOP option.
    pub fn max_dop(mut self, value: u8) -> Self {
        self.max_dop = Some(value);
        self
    }

    /// Set DATA_COMPRESSION option.
    pub fn data_compression(mut self, compression: impl Into<String>) -> Self {
        self.data_compression = Some(compression.into());
        self
    }

    /// Build the ALTER INDEX REBUILD statement.
    pub fn build(&self) -> String {
        let full_table_name = match &self.table_schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.table_name)),
            None => quote_identifier(&self.table_name),
        };

        let index_ref = match &self.index_name {
            Some(name) => quote_identifier(name),
            None => "ALL".to_string(),
        };

        let mut sql = format!("ALTER INDEX {} ON {} REBUILD", index_ref, full_table_name);

        let mut options = Vec::new();

        if let Some(on) = self.online {
            options.push(format!("ONLINE = {}", if on { "ON" } else { "OFF" }));
        }
        if let Some(ff) = self.fill_factor {
            options.push(format!("FILLFACTOR = {}", ff));
        }
        if let Some(st) = self.sort_in_tempdb {
            options.push(format!("SORT_IN_TEMPDB = {}", if st { "ON" } else { "OFF" }));
        }
        if let Some(md) = self.max_dop {
            options.push(format!("MAXDOP = {}", md));
        }
        if let Some(ref dc) = self.data_compression {
            options.push(format!("DATA_COMPRESSION = {}", dc));
        }

        if !options.is_empty() {
            sql.push_str(&format!(" WITH ({})", options.join(", ")));
        }

        sql
    }
}

/// Create a rebuild index builder.
pub fn rebuild_index(name: impl Into<String>, table: impl Into<String>) -> RebuildIndexBuilder {
    RebuildIndexBuilder::index(name, table)
}

/// Create a rebuild all indexes builder.
pub fn rebuild_all_indexes(table: impl Into<String>) -> RebuildIndexBuilder {
    RebuildIndexBuilder::all(table)
}

/// Reorganize index builder.
#[derive(Debug, Clone)]
pub struct ReorganizeIndexBuilder {
    index_name: Option<String>,
    table_schema: Option<String>,
    table_name: String,
    lob_compaction: Option<bool>,
}

impl ReorganizeIndexBuilder {
    /// Create a new reorganize index builder for a specific index.
    pub fn index(name: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            index_name: Some(name.into()),
            table_schema: None,
            table_name: table.into(),
            lob_compaction: None,
        }
    }

    /// Create a new reorganize index builder for all indexes on a table.
    pub fn all(table: impl Into<String>) -> Self {
        Self {
            index_name: None,
            table_schema: None,
            table_name: table.into(),
            lob_compaction: None,
        }
    }

    /// Set the table schema.
    pub fn table_schema(mut self, schema: impl Into<String>) -> Self {
        self.table_schema = Some(schema.into());
        self
    }

    /// Set LOB_COMPACTION option.
    pub fn lob_compaction(mut self, value: bool) -> Self {
        self.lob_compaction = Some(value);
        self
    }

    /// Build the ALTER INDEX REORGANIZE statement.
    pub fn build(&self) -> String {
        let full_table_name = match &self.table_schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.table_name)),
            None => quote_identifier(&self.table_name),
        };

        let index_ref = match &self.index_name {
            Some(name) => quote_identifier(name),
            None => "ALL".to_string(),
        };

        let mut sql = format!("ALTER INDEX {} ON {} REORGANIZE", index_ref, full_table_name);

        if let Some(lc) = self.lob_compaction {
            sql.push_str(&format!(" WITH (LOB_COMPACTION = {})", if lc { "ON" } else { "OFF" }));
        }

        sql
    }
}

/// Create a reorganize index builder.
pub fn reorganize_index(name: impl Into<String>, table: impl Into<String>) -> ReorganizeIndexBuilder {
    ReorganizeIndexBuilder::index(name, table)
}

/// Create a reorganize all indexes builder.
pub fn reorganize_all_indexes(table: impl Into<String>) -> ReorganizeIndexBuilder {
    ReorganizeIndexBuilder::all(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_index() {
        let sql = create_index("IX_users_email", "users")
            .column("email")
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("CREATE NONCLUSTERED INDEX [IX_users_email] ON [users]"));
        assert!(sql.contains("[email]"));
    }

    #[test]
    fn test_create_unique_index() {
        let sql = create_unique_index("UX_users_email", "users")
            .column("email")
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("CREATE UNIQUE NONCLUSTERED INDEX"));
    }

    #[test]
    fn test_create_index_with_include() {
        let sql = create_index("IX_orders_user", "orders")
            .column("user_id")
            .include("created_at")
            .include("total")
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("INCLUDE ([created_at], [total])"));
    }

    #[test]
    fn test_create_filtered_index() {
        let sql = create_index("IX_users_active", "users")
            .column("email")
            .where_clause("is_active = 1")
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("WHERE is_active = 1"));
    }

    #[test]
    fn test_create_index_with_options() {
        let sql = create_index("IX_users_name", "users")
            .column("name")
            .fill_factor(90)
            .online(true)
            .to_create_sql()
            .unwrap();

        assert!(sql.contains("WITH (FILLFACTOR = 90, ONLINE = ON)"));
    }

    #[test]
    fn test_rebuild_index() {
        let sql = rebuild_index("IX_users_email", "users")
            .online(true)
            .fill_factor(80)
            .build();

        assert!(sql.contains("ALTER INDEX [IX_users_email] ON [users] REBUILD"));
        assert!(sql.contains("ONLINE = ON"));
        assert!(sql.contains("FILLFACTOR = 80"));
    }

    #[test]
    fn test_reorganize_index() {
        let sql = reorganize_index("IX_users_email", "users")
            .lob_compaction(true)
            .build();

        assert!(sql.contains("REORGANIZE"));
        assert!(sql.contains("LOB_COMPACTION = ON"));
    }
}

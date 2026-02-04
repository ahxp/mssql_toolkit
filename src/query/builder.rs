//! Core query builder functionality.

use crate::error::MssqlResult;
use crate::types::SqlValue;

use super::expression::{Expr, quote_identifier};

/// Common query trait for all query builders.
pub trait QueryBuilder {
    /// Build the SQL query string.
    fn build(&self) -> MssqlResult<String>;

    /// Build the SQL query with parameters.
    fn build_with_params(&self) -> MssqlResult<(String, Vec<SqlValue>)>;
}

/// Table reference for FROM and JOIN clauses.
#[derive(Debug, Clone)]
pub enum TableRef {
    /// Simple table name.
    Table(String),
    /// Schema-qualified table name.
    SchemaTable { schema: String, table: String },
    /// Table with alias.
    Aliased { table: Box<TableRef>, alias: String },
    /// Subquery as table.
    Subquery { query: String, alias: String },
}

impl TableRef {
    /// Create a simple table reference.
    pub fn table(name: impl Into<String>) -> Self {
        TableRef::Table(name.into())
    }

    /// Create a schema-qualified table reference.
    pub fn schema_table(schema: impl Into<String>, table: impl Into<String>) -> Self {
        TableRef::SchemaTable {
            schema: schema.into(),
            table: table.into(),
        }
    }

    /// Add an alias to this table reference.
    pub fn alias(self, alias: impl Into<String>) -> Self {
        TableRef::Aliased {
            table: Box::new(self),
            alias: alias.into(),
        }
    }

    /// Create a subquery table reference.
    pub fn subquery(query: impl Into<String>, alias: impl Into<String>) -> Self {
        TableRef::Subquery {
            query: query.into(),
            alias: alias.into(),
        }
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        match self {
            TableRef::Table(name) => quote_identifier(name),
            TableRef::SchemaTable { schema, table } => {
                format!("{}.{}", quote_identifier(schema), quote_identifier(table))
            }
            TableRef::Aliased { table, alias } => {
                format!("{} AS {}", table.to_sql(), quote_identifier(alias))
            }
            TableRef::Subquery { query, alias } => {
                format!("({}) AS {}", query, quote_identifier(alias))
            }
        }
    }
}

/// Join type for SQL JOIN clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

impl std::fmt::Display for JoinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoinType::Inner => write!(f, "INNER JOIN"),
            JoinType::Left => write!(f, "LEFT JOIN"),
            JoinType::Right => write!(f, "RIGHT JOIN"),
            JoinType::Full => write!(f, "FULL OUTER JOIN"),
            JoinType::Cross => write!(f, "CROSS JOIN"),
        }
    }
}

/// A JOIN clause.
#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: TableRef,
    pub condition: Option<Expr>,
}

impl JoinClause {
    /// Create a new join clause.
    pub fn new(join_type: JoinType, table: TableRef) -> Self {
        Self {
            join_type,
            table,
            condition: None,
        }
    }

    /// Set the join condition.
    pub fn on(mut self, condition: Expr) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut sql = format!("{} {}", self.join_type, self.table.to_sql());
        if let Some(ref cond) = self.condition {
            sql.push_str(&format!(" ON {}", cond.to_sql()));
        }
        sql
    }
}

/// Common table expression (CTE) for WITH clauses.
#[derive(Debug, Clone)]
pub struct CommonTableExpr {
    pub name: String,
    pub columns: Vec<String>,
    pub query: String,
    pub recursive: bool,
}

impl CommonTableExpr {
    /// Create a new CTE.
    pub fn new(name: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
            query: query.into(),
            recursive: false,
        }
    }

    /// Specify column names.
    pub fn columns(mut self, columns: Vec<String>) -> Self {
        self.columns = columns;
        self
    }

    /// Mark as recursive.
    pub fn recursive(mut self) -> Self {
        self.recursive = true;
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut sql = quote_identifier(&self.name);
        if !self.columns.is_empty() {
            let cols: Vec<String> = self.columns.iter().map(|c| quote_identifier(c)).collect();
            sql.push_str(&format!(" ({})", cols.join(", ")));
        }
        sql.push_str(&format!(" AS ({})", self.query));
        sql
    }
}

/// Pagination helper for OFFSET/FETCH clauses.
#[derive(Debug, Clone)]
pub struct Pagination {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

impl Pagination {
    /// Create a new pagination.
    pub fn new() -> Self {
        Self {
            offset: None,
            limit: None,
        }
    }

    /// Set the offset.
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set the limit.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Render to SQL (SQL Server 2012+ syntax).
    pub fn to_sql(&self) -> String {
        match (self.offset, self.limit) {
            (Some(offset), Some(limit)) => {
                format!("OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, limit)
            }
            (Some(offset), None) => {
                format!("OFFSET {} ROWS", offset)
            }
            (None, Some(limit)) => {
                format!("OFFSET 0 ROWS FETCH NEXT {} ROWS ONLY", limit)
            }
            (None, None) => String::new(),
        }
    }

    /// Check if pagination is set.
    pub fn is_set(&self) -> bool {
        self.offset.is_some() || self.limit.is_some()
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameter collector for parameterized queries.
#[derive(Debug, Default)]
pub struct ParamCollector {
    params: Vec<SqlValue>,
}

impl ParamCollector {
    /// Create a new parameter collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a parameter and return its placeholder.
    pub fn add(&mut self, value: SqlValue) -> String {
        let index = self.params.len();
        self.params.push(value);
        format!("@p{}", index)
    }

    /// Get all collected parameters.
    pub fn params(self) -> Vec<SqlValue> {
        self.params
    }

    /// Get the current number of parameters.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Check if no parameters have been collected.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }
}

/// SQL Server query hints.
#[derive(Debug, Clone)]
pub enum QueryHint {
    /// NOLOCK hint.
    NoLock,
    /// READUNCOMMITTED hint.
    ReadUncommitted,
    /// READCOMMITTED hint.
    ReadCommitted,
    /// REPEATABLEREAD hint.
    RepeatableRead,
    /// SERIALIZABLE hint.
    Serializable,
    /// HOLDLOCK hint.
    HoldLock,
    /// UPDLOCK hint.
    UpdLock,
    /// XLOCK hint.
    XLock,
    /// ROWLOCK hint.
    RowLock,
    /// PAGLOCK hint.
    PageLock,
    /// TABLOCK hint.
    TabLock,
    /// TABLOCKX hint.
    TabLockX,
    /// INDEX hint.
    Index(String),
    /// FORCESEEK hint.
    ForceSeek,
    /// FORCESCAN hint.
    ForceScan,
    /// Custom hint.
    Custom(String),
}

impl QueryHint {
    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        match self {
            QueryHint::NoLock => "NOLOCK".to_string(),
            QueryHint::ReadUncommitted => "READUNCOMMITTED".to_string(),
            QueryHint::ReadCommitted => "READCOMMITTED".to_string(),
            QueryHint::RepeatableRead => "REPEATABLEREAD".to_string(),
            QueryHint::Serializable => "SERIALIZABLE".to_string(),
            QueryHint::HoldLock => "HOLDLOCK".to_string(),
            QueryHint::UpdLock => "UPDLOCK".to_string(),
            QueryHint::XLock => "XLOCK".to_string(),
            QueryHint::RowLock => "ROWLOCK".to_string(),
            QueryHint::PageLock => "PAGLOCK".to_string(),
            QueryHint::TabLock => "TABLOCK".to_string(),
            QueryHint::TabLockX => "TABLOCKX".to_string(),
            QueryHint::Index(name) => format!("INDEX({})", name),
            QueryHint::ForceSeek => "FORCESEEK".to_string(),
            QueryHint::ForceScan => "FORCESCAN".to_string(),
            QueryHint::Custom(hint) => hint.clone(),
        }
    }
}

/// Table with hints.
#[derive(Debug, Clone)]
pub struct TableWithHints {
    pub table: TableRef,
    pub hints: Vec<QueryHint>,
}

impl TableWithHints {
    /// Create a new table with hints.
    pub fn new(table: TableRef) -> Self {
        Self {
            table,
            hints: Vec::new(),
        }
    }

    /// Add a hint.
    pub fn hint(mut self, hint: QueryHint) -> Self {
        self.hints.push(hint);
        self
    }

    /// Add NOLOCK hint.
    pub fn nolock(self) -> Self {
        self.hint(QueryHint::NoLock)
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        if self.hints.is_empty() {
            self.table.to_sql()
        } else {
            let hints: Vec<String> = self.hints.iter().map(|h| h.to_sql()).collect();
            format!("{} WITH ({})", self.table.to_sql(), hints.join(", "))
        }
    }
}

/// Convenience function to create a table reference.
pub fn table(name: impl Into<String>) -> TableRef {
    TableRef::table(name)
}

/// Convenience function to create a schema-qualified table reference.
pub fn schema_table(schema: impl Into<String>, table: impl Into<String>) -> TableRef {
    TableRef::schema_table(schema, table)
}

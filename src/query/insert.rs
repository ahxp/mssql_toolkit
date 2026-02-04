//! INSERT query builder.

use crate::error::{MssqlResult, QueryError, MssqlError};
use crate::types::SqlValue;

use super::builder::*;
use super::expression::*;
use super::select::SelectBuilder;

/// INSERT query builder.
#[derive(Debug, Clone)]
pub struct InsertBuilder {
    /// Target table.
    table: Option<TableRef>,
    /// Column names.
    columns: Vec<String>,
    /// Values to insert.
    values: Vec<Vec<Expr>>,
    /// SELECT for INSERT...SELECT.
    select: Option<Box<SelectBuilder>>,
    /// OUTPUT clause.
    output: Vec<OutputColumn>,
    /// DEFAULT VALUES flag.
    default_values: bool,
    /// Table hints.
    hints: Vec<QueryHint>,
}

/// OUTPUT clause column.
#[derive(Debug, Clone)]
pub enum OutputColumn {
    /// INSERTED.column_name
    Inserted(String),
    /// DELETED.column_name
    Deleted(String),
    /// Expression with alias
    Expression { expr: Expr, alias: Option<String> },
}

impl OutputColumn {
    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        match self {
            OutputColumn::Inserted(col) => format!("INSERTED.{}", quote_identifier(col)),
            OutputColumn::Deleted(col) => format!("DELETED.{}", quote_identifier(col)),
            OutputColumn::Expression { expr, alias } => {
                if let Some(a) = alias {
                    format!("{} AS {}", expr.to_sql(), quote_identifier(a))
                } else {
                    expr.to_sql()
                }
            }
        }
    }
}

impl Default for InsertBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl InsertBuilder {
    /// Create a new INSERT query builder.
    pub fn new() -> Self {
        Self {
            table: None,
            columns: Vec::new(),
            values: Vec::new(),
            select: None,
            output: Vec::new(),
            default_values: false,
            hints: Vec::new(),
        }
    }

    /// Set the target table.
    pub fn into_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::table(table.into()));
        self
    }

    /// Set the target table with schema.
    pub fn into_schema_table(mut self, schema: impl Into<String>, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::schema_table(schema, table));
        self
    }

    /// Set the target table with a TableRef.
    pub fn into_table_ref(mut self, table: TableRef) -> Self {
        self.table = Some(table);
        self
    }

    /// Specify the columns to insert.
    pub fn columns<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.columns = columns.into_iter().map(|c| c.into()).collect();
        self
    }

    /// Add a single row of values.
    pub fn values<I, E>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<Expr>,
    {
        self.values.push(values.into_iter().map(|v| v.into()).collect());
        self
    }

    /// Add a single row of literal values.
    pub fn values_literal<I, V>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<SqlValue>,
    {
        self.values.push(
            values
                .into_iter()
                .map(|v| Expr::Literal(v.into()))
                .collect(),
        );
        self
    }

    /// Add multiple rows of values.
    pub fn values_batch<I, R, E>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoIterator<Item = E>,
        E: Into<Expr>,
    {
        for row in rows {
            self.values.push(row.into_iter().map(|v| v.into()).collect());
        }
        self
    }

    /// Set the SELECT for INSERT...SELECT.
    pub fn select(mut self, query: SelectBuilder) -> Self {
        self.select = Some(Box::new(query));
        self
    }

    /// Use DEFAULT VALUES.
    pub fn default_values(mut self) -> Self {
        self.default_values = true;
        self
    }

    /// Add OUTPUT INSERTED clause.
    pub fn output_inserted<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for col in columns {
            self.output.push(OutputColumn::Inserted(col.into()));
        }
        self
    }

    /// Add OUTPUT INSERTED for a single column.
    pub fn output_inserted_col(mut self, column: impl Into<String>) -> Self {
        self.output.push(OutputColumn::Inserted(column.into()));
        self
    }

    /// Add a table hint.
    pub fn hint(mut self, hint: QueryHint) -> Self {
        self.hints.push(hint);
        self
    }

    /// Validate the INSERT query.
    fn validate(&self) -> MssqlResult<()> {
        if self.table.is_none() {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "INSERT requires a target table".to_string(),
            )));
        }

        // Check for conflicting options
        let has_values = !self.values.is_empty();
        let has_select = self.select.is_some();
        let has_default = self.default_values;

        let option_count = [has_values, has_select, has_default]
            .iter()
            .filter(|&&x| x)
            .count();

        if option_count == 0 {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "INSERT requires VALUES, SELECT, or DEFAULT VALUES".to_string(),
            )));
        }

        if option_count > 1 {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "INSERT cannot have both VALUES and SELECT".to_string(),
            )));
        }

        // Validate value counts match column counts
        if !self.columns.is_empty() && !self.values.is_empty() {
            for (i, row) in self.values.iter().enumerate() {
                if row.len() != self.columns.len() {
                    return Err(MssqlError::Query(QueryError::BuilderError(format!(
                        "Row {} has {} values but {} columns specified",
                        i + 1,
                        row.len(),
                        self.columns.len()
                    ))));
                }
            }
        }

        Ok(())
    }
}

impl QueryBuilder for InsertBuilder {
    fn build(&self) -> MssqlResult<String> {
        self.validate()?;

        let mut sql = String::new();

        // INSERT INTO table
        sql.push_str("INSERT INTO ");
        sql.push_str(&self.table.as_ref().unwrap().to_sql());

        // Table hints
        if !self.hints.is_empty() {
            let hints: Vec<String> = self.hints.iter().map(|h| h.to_sql()).collect();
            sql.push_str(&format!(" WITH ({})", hints.join(", ")));
        }

        // Columns
        if !self.columns.is_empty() {
            let cols: Vec<String> = self.columns.iter().map(|c| quote_identifier(c)).collect();
            sql.push_str(&format!(" ({})", cols.join(", ")));
        }

        // OUTPUT clause
        if !self.output.is_empty() {
            let outputs: Vec<String> = self.output.iter().map(|o| o.to_sql()).collect();
            sql.push_str(&format!(" OUTPUT {}", outputs.join(", ")));
        }

        // VALUES, SELECT, or DEFAULT VALUES
        if self.default_values {
            sql.push_str(" DEFAULT VALUES");
        } else if let Some(ref select) = self.select {
            sql.push_str(&format!(" {}", select.build()?));
        } else {
            sql.push_str(" VALUES ");
            let rows: Vec<String> = self
                .values
                .iter()
                .map(|row| {
                    let vals: Vec<String> = row.iter().map(|v| v.to_sql()).collect();
                    format!("({})", vals.join(", "))
                })
                .collect();
            sql.push_str(&rows.join(", "));
        }

        Ok(sql)
    }

    fn build_with_params(&self) -> MssqlResult<(String, Vec<SqlValue>)> {
        Ok((self.build()?, Vec::new()))
    }
}

/// Create a new INSERT query builder.
pub fn insert() -> InsertBuilder {
    InsertBuilder::new()
}

/// Create an INSERT INTO query for a specific table.
pub fn insert_into(table: impl Into<String>) -> InsertBuilder {
    InsertBuilder::new().into_table(table)
}

/// Builder for bulk insert operations.
#[derive(Debug)]
pub struct BulkInsertBuilder {
    table: Option<TableRef>,
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
    batch_size: usize,
}

impl Default for BulkInsertBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BulkInsertBuilder {
    /// Create a new bulk insert builder.
    pub fn new() -> Self {
        Self {
            table: None,
            columns: Vec::new(),
            rows: Vec::new(),
            batch_size: 1000, // SQL Server limit is ~1000 rows per INSERT
        }
    }

    /// Set the target table.
    pub fn into_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::table(table.into()));
        self
    }

    /// Set the columns.
    pub fn columns<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.columns = columns.into_iter().map(|c| c.into()).collect();
        self
    }

    /// Add a row of values.
    pub fn row<I, V>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<SqlValue>,
    {
        self.rows.push(values.into_iter().map(|v| v.into()).collect());
        self
    }

    /// Add multiple rows.
    pub fn rows<I, R, V>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoIterator<Item = V>,
        V: Into<SqlValue>,
    {
        for row in rows {
            self.rows.push(row.into_iter().map(|v| v.into()).collect());
        }
        self
    }

    /// Set the batch size.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Build batched INSERT statements.
    pub fn build_batches(&self) -> MssqlResult<Vec<String>> {
        if self.table.is_none() {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "Bulk INSERT requires a target table".to_string(),
            )));
        }

        if self.rows.is_empty() {
            return Ok(Vec::new());
        }

        let table_sql = self.table.as_ref().unwrap().to_sql();
        let columns_sql = if self.columns.is_empty() {
            String::new()
        } else {
            let cols: Vec<String> = self.columns.iter().map(|c| quote_identifier(c)).collect();
            format!(" ({})", cols.join(", "))
        };

        let mut statements = Vec::new();

        for chunk in self.rows.chunks(self.batch_size) {
            let rows_sql: Vec<String> = chunk
                .iter()
                .map(|row| {
                    let vals: Vec<String> = row.iter().map(|v| v.to_string()).collect();
                    format!("({})", vals.join(", "))
                })
                .collect();

            let sql = format!(
                "INSERT INTO {}{} VALUES {}",
                table_sql,
                columns_sql,
                rows_sql.join(", ")
            );
            statements.push(sql);
        }

        Ok(statements)
    }

    /// Get the total row count.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get the number of batches.
    pub fn batch_count(&self) -> usize {
        (self.rows.len() + self.batch_size - 1) / self.batch_size
    }
}

/// Create a bulk insert builder.
pub fn bulk_insert() -> BulkInsertBuilder {
    BulkInsertBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_insert() {
        let query = insert_into("users")
            .columns(["name", "email"])
            .values_literal(["John", "john@example.com"])
            .build()
            .unwrap();

        assert!(query.contains("INSERT INTO"));
        assert!(query.contains("VALUES"));
    }

    #[test]
    fn test_insert_multiple_rows() {
        let query = insert_into("users")
            .columns(["name", "email"])
            .values_literal(["John", "john@example.com"])
            .values_literal(["Jane", "jane@example.com"])
            .build()
            .unwrap();

        // Should have two value sets
        assert!(query.matches("),(").count() >= 1 || query.matches("), (").count() >= 1);
    }

    #[test]
    fn test_insert_with_output() {
        let query = insert_into("users")
            .columns(["name", "email"])
            .output_inserted(["id"])
            .values_literal(["John", "john@example.com"])
            .build()
            .unwrap();

        assert!(query.contains("OUTPUT INSERTED.[id]"));
    }

    #[test]
    fn test_insert_default_values() {
        let query = insert_into("audit_log")
            .default_values()
            .build()
            .unwrap();

        assert!(query.contains("DEFAULT VALUES"));
    }

    #[test]
    fn test_bulk_insert_batching() {
        let mut builder = bulk_insert()
            .into_table("users")
            .columns(["name"])
            .batch_size(2);

        for i in 0..5 {
            builder = builder.row([format!("User{}", i)]);
        }

        let batches = builder.build_batches().unwrap();
        assert_eq!(batches.len(), 3); // 5 rows / 2 per batch = 3 batches
    }
}

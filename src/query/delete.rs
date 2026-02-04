//! DELETE query builder.

use crate::error::{MssqlResult, QueryError, MssqlError};
use crate::types::SqlValue;

use super::builder::*;
use super::expression::*;
use super::insert::OutputColumn;

/// DELETE query builder.
#[derive(Debug, Clone)]
pub struct DeleteBuilder {
    /// WITH clause (CTEs).
    with: Vec<CommonTableExpr>,
    /// Target table.
    table: Option<TableRef>,
    /// Table alias.
    alias: Option<String>,
    /// FROM clause for DELETE...FROM.
    from: Option<TableWithHints>,
    /// JOIN clauses.
    joins: Vec<JoinClause>,
    /// WHERE clause.
    where_clause: Option<Expr>,
    /// OUTPUT clause.
    output: Vec<OutputColumn>,
    /// Table hints.
    hints: Vec<QueryHint>,
    /// TOP clause.
    top: Option<u64>,
}

impl Default for DeleteBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DeleteBuilder {
    /// Create a new DELETE query builder.
    pub fn new() -> Self {
        Self {
            with: Vec::new(),
            table: None,
            alias: None,
            from: None,
            joins: Vec::new(),
            where_clause: None,
            output: Vec::new(),
            hints: Vec::new(),
            top: None,
        }
    }

    /// Add a CTE (WITH clause).
    pub fn with_cte(mut self, cte: CommonTableExpr) -> Self {
        self.with.push(cte);
        self
    }

    /// Set the target table.
    pub fn from_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::table(table.into()));
        self
    }

    /// Set the target table with schema.
    pub fn from_schema_table(mut self, schema: impl Into<String>, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::schema_table(schema, table));
        self
    }

    /// Set the target table with a TableRef.
    pub fn from_table_ref(mut self, table: TableRef) -> Self {
        self.table = Some(table);
        self
    }

    /// Set an alias for the table.
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }

    /// Add a TOP clause.
    pub fn top(mut self, count: u64) -> Self {
        self.top = Some(count);
        self
    }

    /// Add additional FROM clause for DELETE...FROM.
    pub fn using(mut self, table: impl Into<String>) -> Self {
        self.from = Some(TableWithHints::new(TableRef::table(table.into())));
        self
    }

    /// Add additional FROM clause with a table reference.
    pub fn using_table(mut self, table: TableRef) -> Self {
        self.from = Some(TableWithHints::new(table));
        self
    }

    /// Add INNER JOIN.
    pub fn inner_join(mut self, table: impl Into<String>, condition: Expr) -> Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Inner,
            table: TableRef::table(table.into()),
            condition: Some(condition),
        });
        self
    }

    /// Add LEFT JOIN.
    pub fn left_join(mut self, table: impl Into<String>, condition: Expr) -> Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Left,
            table: TableRef::table(table.into()),
            condition: Some(condition),
        });
        self
    }

    /// Set the WHERE clause.
    pub fn where_clause(mut self, condition: Expr) -> Self {
        self.where_clause = Some(condition);
        self
    }

    /// Add a condition to the WHERE clause with AND.
    pub fn and_where(mut self, condition: Expr) -> Self {
        self.where_clause = match self.where_clause {
            Some(existing) => Some(existing.and(condition)),
            None => Some(condition),
        };
        self
    }

    /// Add a condition to the WHERE clause with OR.
    pub fn or_where(mut self, condition: Expr) -> Self {
        self.where_clause = match self.where_clause {
            Some(existing) => Some(existing.or(condition)),
            None => Some(condition),
        };
        self
    }

    /// Add OUTPUT DELETED clause.
    pub fn output_deleted<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for col in columns {
            self.output.push(OutputColumn::Deleted(col.into()));
        }
        self
    }

    /// Add OUTPUT DELETED for a single column.
    pub fn output_deleted_col(mut self, column: impl Into<String>) -> Self {
        self.output.push(OutputColumn::Deleted(column.into()));
        self
    }

    /// Add a table hint.
    pub fn hint(mut self, hint: QueryHint) -> Self {
        self.hints.push(hint);
        self
    }

    /// Validate the DELETE query.
    fn validate(&self) -> MssqlResult<()> {
        if self.table.is_none() {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "DELETE requires a target table".to_string(),
            )));
        }

        Ok(())
    }
}

impl QueryBuilder for DeleteBuilder {
    fn build(&self) -> MssqlResult<String> {
        self.validate()?;

        let mut sql = String::new();

        // WITH clause
        if !self.with.is_empty() {
            sql.push_str("WITH ");
            let ctes: Vec<String> = self.with.iter().map(|c| c.to_sql()).collect();
            sql.push_str(&ctes.join(", "));
            sql.push(' ');
        }

        // DELETE
        sql.push_str("DELETE ");

        // TOP
        if let Some(top) = self.top {
            sql.push_str(&format!("TOP ({}) ", top));
        }

        // Table or alias
        if let Some(ref alias) = self.alias {
            sql.push_str(alias);
        } else {
            sql.push_str(&self.table.as_ref().unwrap().to_sql());
        }

        // OUTPUT
        if !self.output.is_empty() {
            let outputs: Vec<String> = self.output.iter().map(|o| o.to_sql()).collect();
            sql.push_str(&format!(" OUTPUT {}", outputs.join(", ")));
        }

        // FROM (required if using alias or for complex deletes)
        if self.alias.is_some() || self.from.is_some() || !self.joins.is_empty() {
            sql.push_str(&format!(" FROM {}", self.table.as_ref().unwrap().to_sql()));

            if let Some(ref alias) = self.alias {
                sql.push_str(&format!(" AS {}", quote_identifier(alias)));
            }

            // Table hints
            if !self.hints.is_empty() {
                let hints: Vec<String> = self.hints.iter().map(|h| h.to_sql()).collect();
                sql.push_str(&format!(" WITH ({})", hints.join(", ")));
            }
        }

        // Additional FROM
        if let Some(ref from) = self.from {
            sql.push_str(&format!(", {}", from.to_sql()));
        }

        // JOINs
        for join in &self.joins {
            sql.push_str(&format!(" {}", join.to_sql()));
        }

        // WHERE
        if let Some(ref where_clause) = self.where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause.to_sql()));
        }

        Ok(sql)
    }

    fn build_with_params(&self) -> MssqlResult<(String, Vec<SqlValue>)> {
        Ok((self.build()?, Vec::new()))
    }
}

/// Create a new DELETE query builder.
pub fn delete() -> DeleteBuilder {
    DeleteBuilder::new()
}

/// Create a DELETE FROM query for a specific table.
pub fn delete_from(table: impl Into<String>) -> DeleteBuilder {
    DeleteBuilder::new().from_table(table)
}

/// TRUNCATE TABLE builder.
#[derive(Debug, Clone)]
pub struct TruncateBuilder {
    table: Option<TableRef>,
}

impl Default for TruncateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TruncateBuilder {
    /// Create a new TRUNCATE builder.
    pub fn new() -> Self {
        Self { table: None }
    }

    /// Set the target table.
    pub fn table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::table(table.into()));
        self
    }

    /// Set the target table with schema.
    pub fn schema_table(mut self, schema: impl Into<String>, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::schema_table(schema, table));
        self
    }
}

impl QueryBuilder for TruncateBuilder {
    fn build(&self) -> MssqlResult<String> {
        if self.table.is_none() {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "TRUNCATE requires a target table".to_string(),
            )));
        }

        Ok(format!("TRUNCATE TABLE {}", self.table.as_ref().unwrap().to_sql()))
    }

    fn build_with_params(&self) -> MssqlResult<(String, Vec<SqlValue>)> {
        Ok((self.build()?, Vec::new()))
    }
}

/// Create a TRUNCATE TABLE builder.
pub fn truncate() -> TruncateBuilder {
    TruncateBuilder::new()
}

/// Create a TRUNCATE TABLE query for a specific table.
pub fn truncate_table(table: impl Into<String>) -> TruncateBuilder {
    TruncateBuilder::new().table(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_delete() {
        let query = delete_from("users")
            .where_clause(col("id").eq(1))
            .build()
            .unwrap();

        assert_eq!(query, "DELETE [users] WHERE ([id] = 1)");
    }

    #[test]
    fn test_delete_with_top() {
        let query = delete_from("logs")
            .top(1000)
            .where_clause(col("created_at").lt("2024-01-01"))
            .build()
            .unwrap();

        assert!(query.contains("DELETE TOP (1000)"));
    }

    #[test]
    fn test_delete_with_output() {
        let query = delete_from("users")
            .output_deleted(["id", "email"])
            .where_clause(col("status").eq("deleted"))
            .build()
            .unwrap();

        assert!(query.contains("OUTPUT DELETED.[id]"));
        assert!(query.contains("DELETED.[email]"));
    }

    #[test]
    fn test_delete_with_join() {
        let query = delete_from("orders")
            .alias("o")
            .inner_join("users", col("o.user_id").eq(Expr::col("users.id")))
            .where_clause(col("users.status").eq("inactive"))
            .build()
            .unwrap();

        // Note: Delete alias is not bracketed since it may be a reference
        assert!(query.contains("DELETE"));
        assert!(query.contains("FROM [orders] AS [o]"));
        assert!(query.contains("INNER JOIN"));
    }

    #[test]
    fn test_truncate() {
        let query = truncate_table("logs").build().unwrap();
        assert_eq!(query, "TRUNCATE TABLE [logs]");
    }
}

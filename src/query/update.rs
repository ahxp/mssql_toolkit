//! UPDATE query builder.

use crate::error::{MssqlResult, QueryError, MssqlError};
use crate::types::SqlValue;

use super::builder::*;
use super::expression::*;
use super::insert::OutputColumn;

/// UPDATE query builder.
#[derive(Debug, Clone)]
pub struct UpdateBuilder {
    /// WITH clause (CTEs).
    with: Vec<CommonTableExpr>,
    /// Target table.
    table: Option<TableRef>,
    /// Table alias.
    alias: Option<String>,
    /// SET clauses.
    sets: Vec<SetClause>,
    /// FROM clause for UPDATE...FROM.
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

/// SET clause for UPDATE.
#[derive(Debug, Clone)]
pub struct SetClause {
    pub column: String,
    pub value: Expr,
}

impl SetClause {
    /// Create a new SET clause.
    pub fn new(column: impl Into<String>, value: impl Into<Expr>) -> Self {
        Self {
            column: column.into(),
            value: value.into(),
        }
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        format!("{} = {}", quote_identifier(&self.column), self.value.to_sql())
    }
}

impl Default for UpdateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateBuilder {
    /// Create a new UPDATE query builder.
    pub fn new() -> Self {
        Self {
            with: Vec::new(),
            table: None,
            alias: None,
            sets: Vec::new(),
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
    pub fn table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::table(table.into()));
        self
    }

    /// Set the target table with schema.
    pub fn schema_table(mut self, schema: impl Into<String>, table: impl Into<String>) -> Self {
        self.table = Some(TableRef::schema_table(schema, table));
        self
    }

    /// Set the target table with a TableRef.
    pub fn table_ref(mut self, table: TableRef) -> Self {
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

    /// Set a column to a value.
    pub fn set<V: Into<Expr>>(mut self, column: impl Into<String>, value: V) -> Self {
        self.sets.push(SetClause::new(column, value));
        self
    }

    /// Set a column to a literal value.
    pub fn set_literal<V: Into<SqlValue>>(mut self, column: impl Into<String>, value: V) -> Self {
        self.sets.push(SetClause::new(column, Expr::Literal(value.into())));
        self
    }

    /// Set a column to NULL.
    pub fn set_null(mut self, column: impl Into<String>) -> Self {
        self.sets.push(SetClause::new(column, Expr::Literal(SqlValue::Null)));
        self
    }

    /// Set a column to DEFAULT.
    pub fn set_default(mut self, column: impl Into<String>) -> Self {
        self.sets.push(SetClause::new(column, Expr::Raw("DEFAULT".to_string())));
        self
    }

    /// Set a column by incrementing its value.
    pub fn increment(mut self, column: impl Into<String>, amount: i64) -> Self {
        let col = column.into();
        let expr = Expr::col(&col).add(Expr::lit(amount));
        self.sets.push(SetClause::new(col, expr));
        self
    }

    /// Set a column by decrementing its value.
    pub fn decrement(mut self, column: impl Into<String>, amount: i64) -> Self {
        let col = column.into();
        let expr = Expr::col(&col).sub(Expr::lit(amount));
        self.sets.push(SetClause::new(col, expr));
        self
    }

    /// Set multiple columns from a map.
    pub fn set_many<I, K, V>(mut self, sets: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<Expr>,
    {
        for (col, val) in sets {
            self.sets.push(SetClause::new(col, val));
        }
        self
    }

    /// Add FROM clause for UPDATE...FROM syntax.
    pub fn from(mut self, table: impl Into<String>) -> Self {
        self.from = Some(TableWithHints::new(TableRef::table(table.into())));
        self
    }

    /// Add FROM clause with a table reference.
    pub fn from_table(mut self, table: TableRef) -> Self {
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

    /// Add a table hint.
    pub fn hint(mut self, hint: QueryHint) -> Self {
        self.hints.push(hint);
        self
    }

    /// Validate the UPDATE query.
    fn validate(&self) -> MssqlResult<()> {
        if self.table.is_none() {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "UPDATE requires a target table".to_string(),
            )));
        }

        if self.sets.is_empty() {
            return Err(MssqlError::Query(QueryError::BuilderError(
                "UPDATE requires at least one SET clause".to_string(),
            )));
        }

        Ok(())
    }
}

impl QueryBuilder for UpdateBuilder {
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

        // UPDATE
        sql.push_str("UPDATE ");

        // TOP
        if let Some(top) = self.top {
            sql.push_str(&format!("TOP ({}) ", top));
        }

        // Table
        sql.push_str(&self.table.as_ref().unwrap().to_sql());

        // Alias
        if let Some(ref alias) = self.alias {
            sql.push_str(&format!(" AS {}", quote_identifier(alias)));
        }

        // Table hints
        if !self.hints.is_empty() {
            let hints: Vec<String> = self.hints.iter().map(|h| h.to_sql()).collect();
            sql.push_str(&format!(" WITH ({})", hints.join(", ")));
        }

        // SET
        let sets: Vec<String> = self.sets.iter().map(|s| s.to_sql()).collect();
        sql.push_str(&format!(" SET {}", sets.join(", ")));

        // OUTPUT
        if !self.output.is_empty() {
            let outputs: Vec<String> = self.output.iter().map(|o| o.to_sql()).collect();
            sql.push_str(&format!(" OUTPUT {}", outputs.join(", ")));
        }

        // FROM
        if let Some(ref from) = self.from {
            sql.push_str(&format!(" FROM {}", from.to_sql()));
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

/// Create a new UPDATE query builder.
pub fn update() -> UpdateBuilder {
    UpdateBuilder::new()
}

/// Create an UPDATE query for a specific table.
pub fn update_table(table: impl Into<String>) -> UpdateBuilder {
    UpdateBuilder::new().table(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_update() {
        let query = update_table("users")
            .set_literal("name", "John")
            .where_clause(col("id").eq(1))
            .build()
            .unwrap();

        assert!(query.contains("UPDATE [users]"));
        assert!(query.contains("SET [name] = 'John'"));
        assert!(query.contains("WHERE"));
    }

    #[test]
    fn test_update_multiple_columns() {
        let query = update_table("users")
            .set_literal("name", "John")
            .set_literal("email", "john@example.com")
            .set_literal("age", 30)
            .where_clause(col("id").eq(1))
            .build()
            .unwrap();

        assert!(query.contains("[name] = 'John'"));
        assert!(query.contains("[email] = 'john@example.com'"));
        assert!(query.contains("[age] = 30"));
    }

    #[test]
    fn test_update_with_increment() {
        let query = update_table("products")
            .increment("view_count", 1)
            .where_clause(col("id").eq(1))
            .build()
            .unwrap();

        assert!(query.contains("[view_count] = ([view_count] + 1)"));
    }

    #[test]
    fn test_update_with_from() {
        let query = update_table("orders")
            .alias("o")
            .set("status", Expr::col("s.new_status"))
            .from("status_updates")
            .where_clause(col("o.id").eq(Expr::col("s.order_id")))
            .build()
            .unwrap();

        assert!(query.contains("UPDATE [orders] AS [o]"));
        assert!(query.contains("FROM [status_updates]"));
    }

    #[test]
    fn test_update_with_output() {
        let query = update_table("users")
            .set_literal("status", "inactive")
            .output_deleted(["id", "status"])
            .output_inserted(["id", "status"])
            .where_clause(col("last_login").lt("2024-01-01"))
            .build()
            .unwrap();

        assert!(query.contains("OUTPUT"));
        assert!(query.contains("DELETED.[id]"));
        assert!(query.contains("INSERTED.[status]"));
    }

    #[test]
    fn test_update_with_top() {
        let query = update_table("messages")
            .top(100)
            .set_literal("read", true)
            .where_clause(col("read").eq(false))
            .build()
            .unwrap();

        assert!(query.contains("UPDATE TOP (100)"));
    }
}

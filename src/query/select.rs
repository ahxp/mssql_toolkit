//! SELECT query builder.

use crate::error::{MssqlResult, QueryError, MssqlError};
use crate::types::SqlValue;

use super::builder::*;
use super::expression::*;

/// SELECT query builder.
#[derive(Debug, Clone)]
pub struct SelectBuilder {
    /// WITH clause (CTEs).
    with: Vec<CommonTableExpr>,
    /// Whether the WITH clause is recursive.
    with_recursive: bool,
    /// DISTINCT keyword.
    distinct: bool,
    /// TOP clause.
    top: Option<TopClause>,
    /// SELECT columns.
    columns: Vec<SelectColumn>,
    /// FROM clause.
    from: Option<TableWithHints>,
    /// JOIN clauses.
    joins: Vec<JoinClause>,
    /// WHERE clause.
    where_clause: Option<Expr>,
    /// GROUP BY clause.
    group_by: Vec<Expr>,
    /// HAVING clause.
    having: Option<Expr>,
    /// ORDER BY clause.
    order_by: Vec<OrderByExpr>,
    /// UNION clauses.
    unions: Vec<UnionClause>,
    /// Pagination (OFFSET/FETCH).
    pagination: Pagination,
    /// FOR clause (FOR XML, FOR JSON).
    for_clause: Option<ForClause>,
    /// Query hints.
    option_hints: Vec<String>,
}

/// TOP clause specification.
#[derive(Debug, Clone)]
pub struct TopClause {
    pub count: u64,
    pub percent: bool,
    pub with_ties: bool,
}

/// SELECT column specification.
#[derive(Debug, Clone)]
pub enum SelectColumn {
    /// All columns (*).
    All,
    /// All columns from a table (table.*).
    AllFrom(String),
    /// A single expression.
    Expr(Expr),
    /// An expression with alias.
    Aliased(AliasedExpr),
}

impl SelectColumn {
    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        match self {
            SelectColumn::All => "*".to_string(),
            SelectColumn::AllFrom(table) => format!("{}.*", quote_identifier(table)),
            SelectColumn::Expr(expr) => expr.to_sql(),
            SelectColumn::Aliased(aliased) => aliased.to_sql(),
        }
    }
}

/// UNION clause.
#[derive(Debug, Clone)]
pub struct UnionClause {
    pub union_type: UnionType,
    pub query: Box<SelectBuilder>,
}

/// UNION type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnionType {
    Union,
    UnionAll,
    Intersect,
    Except,
}

impl std::fmt::Display for UnionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnionType::Union => write!(f, "UNION"),
            UnionType::UnionAll => write!(f, "UNION ALL"),
            UnionType::Intersect => write!(f, "INTERSECT"),
            UnionType::Except => write!(f, "EXCEPT"),
        }
    }
}

/// FOR clause types.
#[derive(Debug, Clone)]
pub enum ForClause {
    /// FOR XML clause.
    Xml(XmlMode),
    /// FOR JSON clause.
    Json(JsonMode),
}

/// XML output modes.
#[derive(Debug, Clone)]
pub enum XmlMode {
    Raw(Option<String>),
    Auto,
    Path(Option<String>),
    Explicit,
}

/// JSON output modes.
#[derive(Debug, Clone)]
pub enum JsonMode {
    Auto,
    Path,
}

impl Default for SelectBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectBuilder {
    /// Create a new SELECT query builder.
    pub fn new() -> Self {
        Self {
            with: Vec::new(),
            with_recursive: false,
            distinct: false,
            top: None,
            columns: Vec::new(),
            from: None,
            joins: Vec::new(),
            where_clause: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            unions: Vec::new(),
            pagination: Pagination::new(),
            for_clause: None,
            option_hints: Vec::new(),
        }
    }

    /// Add a CTE (WITH clause).
    pub fn with_cte(mut self, cte: CommonTableExpr) -> Self {
        self.with.push(cte);
        self
    }

    /// Make the WITH clause recursive.
    pub fn with_recursive(mut self) -> Self {
        self.with_recursive = true;
        self
    }

    /// Add DISTINCT keyword.
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Add TOP clause.
    pub fn top(mut self, count: u64) -> Self {
        self.top = Some(TopClause {
            count,
            percent: false,
            with_ties: false,
        });
        self
    }

    /// Add TOP PERCENT clause.
    pub fn top_percent(mut self, count: u64) -> Self {
        self.top = Some(TopClause {
            count,
            percent: true,
            with_ties: false,
        });
        self
    }

    /// Add TOP WITH TIES clause.
    pub fn top_with_ties(mut self, count: u64) -> Self {
        self.top = Some(TopClause {
            count,
            percent: false,
            with_ties: true,
        });
        self
    }

    /// Select all columns.
    pub fn select_all(mut self) -> Self {
        self.columns.push(SelectColumn::All);
        self
    }

    /// Select specific columns by name.
    pub fn select<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for col in columns {
            self.columns.push(SelectColumn::Expr(Expr::col(col.into())));
        }
        self
    }

    /// Select a single column.
    pub fn select_col(mut self, column: impl Into<String>) -> Self {
        self.columns.push(SelectColumn::Expr(Expr::col(column.into())));
        self
    }

    /// Select an expression.
    pub fn select_expr(mut self, expr: Expr) -> Self {
        self.columns.push(SelectColumn::Expr(expr));
        self
    }

    /// Select an expression with alias.
    pub fn select_as(mut self, expr: Expr, alias: impl Into<String>) -> Self {
        self.columns.push(SelectColumn::Aliased(AliasedExpr {
            expr,
            alias: alias.into(),
        }));
        self
    }

    /// Select all columns from a specific table.
    pub fn select_all_from(mut self, table: impl Into<String>) -> Self {
        self.columns.push(SelectColumn::AllFrom(table.into()));
        self
    }

    /// Set the FROM clause.
    pub fn from(mut self, table: impl Into<String>) -> Self {
        self.from = Some(TableWithHints::new(TableRef::table(table.into())));
        self
    }

    /// Set the FROM clause with a table reference.
    pub fn from_table(mut self, table: TableRef) -> Self {
        self.from = Some(TableWithHints::new(table));
        self
    }

    /// Set the FROM clause with hints.
    pub fn from_with_hints(mut self, table: TableWithHints) -> Self {
        self.from = Some(table);
        self
    }

    /// Set the FROM clause from a subquery.
    pub fn from_subquery(mut self, query: SelectBuilder, alias: impl Into<String>) -> MssqlResult<Self> {
        let sql = query.build()?;
        self.from = Some(TableWithHints::new(TableRef::subquery(sql, alias)));
        Ok(self)
    }

    /// Add an INNER JOIN.
    pub fn inner_join(mut self, table: impl Into<String>, condition: Expr) -> Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Inner,
            table: TableRef::table(table.into()),
            condition: Some(condition),
        });
        self
    }

    /// Add a LEFT JOIN.
    pub fn left_join(mut self, table: impl Into<String>, condition: Expr) -> Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Left,
            table: TableRef::table(table.into()),
            condition: Some(condition),
        });
        self
    }

    /// Add a RIGHT JOIN.
    pub fn right_join(mut self, table: impl Into<String>, condition: Expr) -> Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Right,
            table: TableRef::table(table.into()),
            condition: Some(condition),
        });
        self
    }

    /// Add a FULL OUTER JOIN.
    pub fn full_join(mut self, table: impl Into<String>, condition: Expr) -> Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Full,
            table: TableRef::table(table.into()),
            condition: Some(condition),
        });
        self
    }

    /// Add a CROSS JOIN.
    pub fn cross_join(mut self, table: impl Into<String>) -> Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Cross,
            table: TableRef::table(table.into()),
            condition: None,
        });
        self
    }

    /// Add a JOIN clause.
    pub fn join(mut self, join: JoinClause) -> Self {
        self.joins.push(join);
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

    /// Add a GROUP BY clause.
    pub fn group_by<I>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = Expr>,
    {
        self.group_by.extend(columns);
        self
    }

    /// Add a single column to GROUP BY.
    pub fn group_by_col(mut self, column: impl Into<String>) -> Self {
        self.group_by.push(Expr::col(column.into()));
        self
    }

    /// Set the HAVING clause.
    pub fn having(mut self, condition: Expr) -> Self {
        self.having = Some(condition);
        self
    }

    /// Add ORDER BY clause.
    pub fn order_by(mut self, expr: Expr, direction: OrderDirection) -> Self {
        self.order_by.push(OrderByExpr {
            expr,
            direction,
            nulls: None,
        });
        self
    }

    /// Add ORDER BY ASC.
    pub fn order_by_asc(self, column: impl Into<String>) -> Self {
        self.order_by(Expr::col(column.into()), OrderDirection::Asc)
    }

    /// Add ORDER BY DESC.
    pub fn order_by_desc(self, column: impl Into<String>) -> Self {
        self.order_by(Expr::col(column.into()), OrderDirection::Desc)
    }

    /// Add UNION clause.
    pub fn union(mut self, query: SelectBuilder) -> Self {
        self.unions.push(UnionClause {
            union_type: UnionType::Union,
            query: Box::new(query),
        });
        self
    }

    /// Add UNION ALL clause.
    pub fn union_all(mut self, query: SelectBuilder) -> Self {
        self.unions.push(UnionClause {
            union_type: UnionType::UnionAll,
            query: Box::new(query),
        });
        self
    }

    /// Add INTERSECT clause.
    pub fn intersect(mut self, query: SelectBuilder) -> Self {
        self.unions.push(UnionClause {
            union_type: UnionType::Intersect,
            query: Box::new(query),
        });
        self
    }

    /// Add EXCEPT clause.
    pub fn except(mut self, query: SelectBuilder) -> Self {
        self.unions.push(UnionClause {
            union_type: UnionType::Except,
            query: Box::new(query),
        });
        self
    }

    /// Set the OFFSET.
    pub fn offset(mut self, offset: u64) -> Self {
        self.pagination.offset = Some(offset);
        self
    }

    /// Set the LIMIT (FETCH).
    pub fn limit(mut self, limit: u64) -> Self {
        self.pagination.limit = Some(limit);
        self
    }

    /// Set both OFFSET and LIMIT for pagination.
    pub fn paginate(mut self, page: u64, page_size: u64) -> Self {
        self.pagination.offset = Some(page * page_size);
        self.pagination.limit = Some(page_size);
        self
    }

    /// Add FOR XML clause.
    pub fn for_xml(mut self, mode: XmlMode) -> Self {
        self.for_clause = Some(ForClause::Xml(mode));
        self
    }

    /// Add FOR JSON clause.
    pub fn for_json(mut self, mode: JsonMode) -> Self {
        self.for_clause = Some(ForClause::Json(mode));
        self
    }

    /// Add an OPTION hint.
    pub fn option(mut self, hint: impl Into<String>) -> Self {
        self.option_hints.push(hint.into());
        self
    }
}

impl QueryBuilder for SelectBuilder {
    fn build(&self) -> MssqlResult<String> {
        let mut sql = String::new();

        // WITH clause
        if !self.with.is_empty() {
            sql.push_str("WITH ");
            if self.with_recursive {
                // SQL Server doesn't use RECURSIVE keyword, it's implicit
            }
            let ctes: Vec<String> = self.with.iter().map(|c| c.to_sql()).collect();
            sql.push_str(&ctes.join(", "));
            sql.push(' ');
        }

        // SELECT
        sql.push_str("SELECT ");

        // DISTINCT
        if self.distinct {
            sql.push_str("DISTINCT ");
        }

        // TOP
        if let Some(ref top) = self.top {
            sql.push_str(&format!("TOP ({}) ", top.count));
            if top.percent {
                sql.push_str("PERCENT ");
            }
            if top.with_ties {
                sql.push_str("WITH TIES ");
            }
        }

        // Columns
        if self.columns.is_empty() {
            sql.push_str("*");
        } else {
            let cols: Vec<String> = self.columns.iter().map(|c| c.to_sql()).collect();
            sql.push_str(&cols.join(", "));
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

        // GROUP BY
        if !self.group_by.is_empty() {
            let groups: Vec<String> = self.group_by.iter().map(|e| e.to_sql()).collect();
            sql.push_str(&format!(" GROUP BY {}", groups.join(", ")));
        }

        // HAVING
        if let Some(ref having) = self.having {
            sql.push_str(&format!(" HAVING {}", having.to_sql()));
        }

        // UNION/INTERSECT/EXCEPT
        for union in &self.unions {
            sql.push_str(&format!(" {} {}", union.union_type, union.query.build()?));
        }

        // ORDER BY
        if !self.order_by.is_empty() {
            let orders: Vec<String> = self
                .order_by
                .iter()
                .map(|o| format!("{} {}", o.expr.to_sql(), o.direction))
                .collect();
            sql.push_str(&format!(" ORDER BY {}", orders.join(", ")));
        }

        // OFFSET/FETCH (requires ORDER BY)
        if self.pagination.is_set() {
            if self.order_by.is_empty() {
                return Err(MssqlError::Query(QueryError::BuilderError(
                    "OFFSET/FETCH requires ORDER BY clause".to_string(),
                )));
            }
            sql.push_str(&format!(" {}", self.pagination.to_sql()));
        }

        // FOR clause
        if let Some(ref for_clause) = self.for_clause {
            match for_clause {
                ForClause::Xml(mode) => {
                    sql.push_str(" FOR XML ");
                    match mode {
                        XmlMode::Raw(elem) => {
                            sql.push_str("RAW");
                            if let Some(e) = elem {
                                sql.push_str(&format!("('{}')", e));
                            }
                        }
                        XmlMode::Auto => sql.push_str("AUTO"),
                        XmlMode::Path(elem) => {
                            sql.push_str("PATH");
                            if let Some(e) = elem {
                                sql.push_str(&format!("('{}')", e));
                            }
                        }
                        XmlMode::Explicit => sql.push_str("EXPLICIT"),
                    }
                }
                ForClause::Json(mode) => {
                    sql.push_str(" FOR JSON ");
                    match mode {
                        JsonMode::Auto => sql.push_str("AUTO"),
                        JsonMode::Path => sql.push_str("PATH"),
                    }
                }
            }
        }

        // OPTION hints
        if !self.option_hints.is_empty() {
            sql.push_str(&format!(" OPTION ({})", self.option_hints.join(", ")));
        }

        Ok(sql)
    }

    fn build_with_params(&self) -> MssqlResult<(String, Vec<SqlValue>)> {
        // For now, return the SQL without parameter extraction
        // A full implementation would walk the expression tree and extract parameters
        Ok((self.build()?, Vec::new()))
    }
}

/// Create a new SELECT query builder.
pub fn select() -> SelectBuilder {
    SelectBuilder::new()
}

/// Create a SELECT query with specific columns.
pub fn select_columns<I, S>(columns: I) -> SelectBuilder
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    SelectBuilder::new().select(columns)
}

/// Create a SELECT * query.
pub fn select_all() -> SelectBuilder {
    SelectBuilder::new().select_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_select() {
        let query = select()
            .select(["id", "name"])
            .from("users")
            .build()
            .unwrap();

        assert_eq!(query, "SELECT [id], [name] FROM [users]");
    }

    #[test]
    fn test_select_with_where() {
        let query = select()
            .select(["id", "name"])
            .from("users")
            .where_clause(col("status").eq("active"))
            .build()
            .unwrap();

        assert_eq!(
            query,
            "SELECT [id], [name] FROM [users] WHERE ([status] = 'active')"
        );
    }

    #[test]
    fn test_select_with_join() {
        let query = select()
            .select(["u.id", "u.name", "o.total"])
            .from("users")
            .inner_join(
                "orders",
                col("u.id").eq(Expr::col("o.user_id")),
            )
            .build()
            .unwrap();

        assert!(query.contains("INNER JOIN"));
        assert!(query.contains("ON"));
    }

    #[test]
    fn test_select_with_pagination() {
        let query = select()
            .select(["id", "name"])
            .from("users")
            .order_by_asc("id")
            .paginate(0, 10)
            .build()
            .unwrap();

        assert!(query.contains("ORDER BY"));
        assert!(query.contains("OFFSET"));
        assert!(query.contains("FETCH"));
    }

    #[test]
    fn test_select_with_aggregates() {
        let query = select()
            .select_expr(count_all())
            .select_as(col("price").sum(), "total_price")
            .from("orders")
            .group_by_col("category")
            .build()
            .unwrap();

        assert!(query.contains("COUNT(*)"));
        assert!(query.contains("SUM([price])"));
        assert!(query.contains("GROUP BY"));
    }
}

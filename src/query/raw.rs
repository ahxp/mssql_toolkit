//! Raw SQL query support.

use crate::error::MssqlResult;
use crate::types::SqlValue;

use super::builder::QueryBuilder;
use super::expression::{Expr, quote_qualified_identifier};

/// A raw SQL query with parameter support.
#[derive(Debug, Clone)]
pub struct RawQuery {
    sql: String,
    params: Vec<SqlValue>,
}

impl RawQuery {
    /// Create a new raw query.
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            params: Vec::new(),
        }
    }

    /// Add a parameter to the query.
    pub fn param<V: Into<SqlValue>>(mut self, value: V) -> Self {
        self.params.push(value.into());
        self
    }

    /// Add multiple parameters to the query.
    pub fn params<I, V>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<SqlValue>,
    {
        self.params.extend(values.into_iter().map(|v| v.into()));
        self
    }

    /// Get the parameters.
    pub fn get_params(&self) -> &[SqlValue] {
        &self.params
    }
}

impl QueryBuilder for RawQuery {
    fn build(&self) -> MssqlResult<String> {
        Ok(self.sql.clone())
    }

    fn build_with_params(&self) -> MssqlResult<(String, Vec<SqlValue>)> {
        Ok((self.sql.clone(), self.params.clone()))
    }
}

/// Create a raw SQL query.
pub fn raw_sql(sql: impl Into<String>) -> RawQuery {
    RawQuery::new(sql)
}

/// SQL fragment builder for composing complex queries.
#[derive(Debug, Clone, Default)]
pub struct SqlFragment {
    parts: Vec<String>,
    params: Vec<SqlValue>,
}

impl SqlFragment {
    /// Create a new SQL fragment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append SQL text.
    pub fn sql(mut self, sql: impl Into<String>) -> Self {
        self.parts.push(sql.into());
        self
    }

    /// Append a parameterized value.
    pub fn value<V: Into<SqlValue>>(mut self, value: V) -> Self {
        let index = self.params.len() + 1;
        self.parts.push(format!("@P{}", index));
        self.params.push(value.into());
        self
    }

    /// Append another fragment.
    pub fn fragment(mut self, other: SqlFragment) -> Self {
        let offset = self.params.len();
        // Rewrite parameter references in the other fragment
        for part in other.parts {
            if part.starts_with("@P") {
                if let Ok(idx) = part[2..].parse::<usize>() {
                    self.parts.push(format!("@P{}", idx + offset));
                } else {
                    self.parts.push(part);
                }
            } else {
                self.parts.push(part);
            }
        }
        self.params.extend(other.params);
        self
    }

    /// Append if a condition is true.
    pub fn sql_if(self, condition: bool, sql: impl Into<String>) -> Self {
        if condition {
            self.sql(sql)
        } else {
            self
        }
    }

    /// Append a fragment if a condition is true.
    pub fn fragment_if(self, condition: bool, f: impl FnOnce() -> SqlFragment) -> Self {
        if condition {
            self.fragment(f())
        } else {
            self
        }
    }

    /// Join multiple values with a separator.
    pub fn join_values<I, V>(mut self, values: I, separator: &str) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<SqlValue>,
    {
        let values: Vec<SqlValue> = values.into_iter().map(|v| v.into()).collect();
        let offset = self.params.len();

        for i in 0..values.len() {
            if i > 0 { self.parts.push(separator.to_string()); }
            self.parts.push(format!("@P{}", offset + i + 1));
        }
        self.params.extend(values);
        self
    }

    /// Build the final SQL string.
    pub fn build(&self) -> String {
        self.parts.join("")
    }

    /// Build with parameters.
    pub fn build_with_params(&self) -> (String, Vec<SqlValue>) {
        (self.build(), self.params.clone())
    }
}

/// Create a new SQL fragment.
pub fn fragment() -> SqlFragment {
    SqlFragment::new()
}

/// Template-based SQL query builder.
#[derive(Debug, Clone)]
pub struct SqlTemplate {
    template: String,
    named_params: std::collections::HashMap<String, SqlValue>,
}

impl SqlTemplate {
    /// Create a new SQL template.
    ///
    /// Use `{name}` for named parameters in the template.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
            named_params: std::collections::HashMap::new(),
        }
    }

    /// Set a named parameter.
    pub fn set<V: Into<SqlValue>>(mut self, name: impl Into<String>, value: V) -> Self {
        self.named_params.insert(name.into(), value.into());
        self
    }

    /// Set multiple named parameters.
    pub fn set_many<I, K, V>(mut self, params: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<SqlValue>,
    {
        for (k, v) in params {
            self.named_params.insert(k.into(), v.into());
        }
        self
    }

    /// Build the query, replacing named parameters with positional ones.
    pub fn build(&self) -> MssqlResult<(String, Vec<SqlValue>)> {
        let mut sql = self.template.clone();
        let mut params = Vec::new();

        // Find all {name} patterns and replace with @PN
        for (name, value) in &self.named_params {
            let placeholder = format!("{{{}}}", name);
            let param_ref = format!("@P{}", params.len() + 1);
            sql = sql.replace(&placeholder, &param_ref);
            params.push(value.clone());
        }

        Ok((sql, params))
    }
}

/// Create a new SQL template.
pub fn template(sql: impl Into<String>) -> SqlTemplate {
    SqlTemplate::new(sql)
}

/// Conditional query parts helper.
#[derive(Debug, Clone)]
pub struct ConditionalBuilder {
    base: String,
    conditions: Vec<Expr>,
    order_by: Option<String>,
    pagination: Option<(u64, u64)>,
}

impl ConditionalBuilder {
    /// Create a new conditional builder.
    pub fn new(base_query: impl Into<String>) -> Self {
        Self {
            base: base_query.into(),
            conditions: Vec::new(),
            order_by: None,
            pagination: None,
        }
    }

    /// Add a condition if the value is Some.
    pub fn where_if<V: Into<SqlValue>>(mut self, column: &str, value: Option<V>) -> Self {
        if let Some(value) = value { self.conditions.push(Expr::col(column).eq(Expr::lit(value))); }
        self
    }

    /// Add a LIKE condition if the pattern is present.
    pub fn like_if(mut self, column: &str, pattern: Option<&str>) -> Self {
        if let Some(value) = pattern { self.conditions.push(Expr::col(column).like(Expr::lit(value))); }
        self
    }

    /// Add a range condition for the bounds supplied.
    pub fn between_if<V: Into<SqlValue>>(mut self, column: &str, min: Option<V>, max: Option<V>) -> Self {
        let column = Expr::col(column);
        let condition = match (min, max) {
            (Some(min), Some(max)) => Some(column.between(Expr::lit(min), Expr::lit(max))),
            (Some(min), None) => Some(column.ge(Expr::lit(min))),
            (None, Some(max)) => Some(column.le(Expr::lit(max))),
            (None, None) => None,
        };
        if let Some(condition) = condition { self.conditions.push(condition); }
        self
    }

    /// Add an IN condition if the list is not empty.
    pub fn in_if<I, V>(mut self, column: &str, values: I) -> Self
    where I: IntoIterator<Item = V>, V: Into<SqlValue> {
        let values: Vec<Expr> = values.into_iter().map(Expr::lit).collect();
        if !values.is_empty() { self.conditions.push(Expr::col(column).in_list(values)); }
        self
    }

    /// Set ORDER BY clause using a column identifier.
    pub fn order_by(mut self, column: &str, desc: bool) -> Self {
        let dir = if desc { "DESC" } else { "ASC" };
        self.order_by = Some(format!("ORDER BY {} {}", quote_qualified_identifier(column), dir));
        self
    }

    /// Set pagination.
    pub fn paginate(mut self, page: u64, page_size: u64) -> Self {
        self.pagination = Some((page * page_size, page_size));
        self
    }

    /// Build SQL and the values to bind. Prefer this for user-provided values.
    pub fn build_with_params(&self) -> MssqlResult<(String, Vec<SqlValue>)> {
        let mut query = self.clone();
        let mut params = Vec::new();
        for condition in &mut query.conditions { condition.collect_params(&mut params)?; }
        Ok((query.build(), params))
    }

    /// Build the final query.
    pub fn build(&self) -> String {
        let mut sql = self.base.clone();

        if !self.conditions.is_empty() {
            if sql.to_uppercase().contains("WHERE") {
                sql.push_str(" AND ");
            } else {
                sql.push_str(" WHERE ");
            }
            sql.push_str(&self.conditions.iter().map(Expr::to_sql).collect::<Vec<_>>().join(" AND "));
        }

        if let Some(ref order) = self.order_by {
            sql.push(' ');
            sql.push_str(order);
        }

        if let Some((offset, limit)) = self.pagination {
            if self.order_by.is_none() {
                // SQL Server requires ORDER BY for OFFSET/FETCH
                sql.push_str(" ORDER BY (SELECT NULL)");
            }
            sql.push_str(&format!(" OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, limit));
        }

        sql
    }
}

/// Create a conditional query builder.
pub fn conditional(base_query: impl Into<String>) -> ConditionalBuilder {
    ConditionalBuilder::new(base_query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_binds_untrusted_values_and_quotes_identifiers() {
        let input = "x' OR 1=1 --";
        let query = conditional("SELECT * FROM users")
            .where_if("u.na]me", Some(input))
            .between_if("u.age", Some(18), Some(30))
            .in_if("u.role", ["admin", "user"])
            .order_by("u.name", false);
        let (sql, params) = query.build_with_params().unwrap();
        assert_eq!(sql, "SELECT * FROM users WHERE ([u].[na]]me] = @P1) AND [u].[age] BETWEEN @P2 AND @P3 AND [u].[role] IN (@P4, @P5) ORDER BY [u].[name] ASC");
        assert_eq!(params, vec![SqlValue::String(input.into()), SqlValue::Int(18), SqlValue::Int(30), SqlValue::String("admin".into()), SqlValue::String("user".into())]);
        assert!(!sql.contains(input));
        assert!(query.build().contains("x'' OR 1=1 --"));
    }

    #[test]
    fn composing_fragments_offsets_all_generated_placeholders() {
        let f = fragment().value(10).sql(" IN (").fragment(fragment().join_values([1, 2], ", ")).sql(")");
        let (sql, params) = f.build_with_params();
        assert_eq!(sql, "@P1 IN (@P2, @P3)");
        assert_eq!(params, vec![10.into(), 1.into(), 2.into()]);
    }

    #[test]
    fn test_raw_query() {
        let query = raw_sql("SELECT * FROM users WHERE id = @P1")
            .param(1)
            .build_with_params()
            .unwrap();

        assert_eq!(query.0, "SELECT * FROM users WHERE id = @P1");
        assert_eq!(query.1.len(), 1);
    }

    #[test]
    fn test_sql_fragment() {
        let frag = fragment()
            .sql("SELECT * FROM users WHERE id = ")
            .value(1)
            .sql(" AND status = ")
            .value("active");

        let (sql, params) = frag.build_with_params();
        assert!(sql.contains("@P1"));
        assert!(sql.contains("@P2"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_sql_template() {
        let (sql, params) = template("SELECT * FROM users WHERE id = {id} AND status = {status}")
            .set("id", 1)
            .set("status", "active")
            .build()
            .unwrap();

        assert!(sql.contains("@P"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_conditional_builder() {
        let query = conditional("SELECT * FROM products")
            .where_if("category", Some("electronics"))
            .between_if("price", Some(100), Some(500))
            .order_by("name", false)
            .paginate(0, 10)
            .build();

        assert!(query.contains("WHERE"));
        assert!(query.contains("[category] = 'electronics'"));
        assert!(query.contains("BETWEEN"));
        assert!(query.contains("ORDER BY"));
        assert!(query.contains("OFFSET"));
    }
}

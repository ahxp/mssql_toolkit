//! SQL expressions and conditions for query building.

use crate::types::SqlValue;
use std::fmt;

/// Represents a SQL expression.
#[derive(Debug, Clone)]
pub enum Expr {
    /// A column reference.
    Column(String),
    /// A qualified column reference (table.column).
    QualifiedColumn { table: String, column: String },
    /// A literal value.
    Literal(SqlValue),
    /// A parameter placeholder.
    Param(usize),
    /// A raw SQL expression.
    Raw(String),
    /// A function call.
    Function { name: String, args: Vec<Expr> },
    /// Binary operation.
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    /// Unary operation.
    UnaryOp { op: UnaryOperator, expr: Box<Expr> },
    /// CASE expression.
    Case {
        operand: Option<Box<Expr>>,
        when_clauses: Vec<(Expr, Expr)>,
        else_clause: Option<Box<Expr>>,
    },
    /// Subquery.
    Subquery(String),
    /// IN expression.
    In {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
    /// BETWEEN expression.
    Between {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
        negated: bool,
    },
    /// LIKE expression.
    Like {
        expr: Box<Expr>,
        pattern: Box<Expr>,
        escape: Option<char>,
        negated: bool,
    },
    /// IS NULL expression.
    IsNull { expr: Box<Expr>, negated: bool },
    /// EXISTS expression.
    Exists { subquery: String, negated: bool },
    /// CAST expression.
    Cast { expr: Box<Expr>, data_type: String },
    /// Aggregate function.
    Aggregate {
        func: AggregateFunc,
        expr: Box<Expr>,
        distinct: bool,
    },
    /// Window function.
    Window {
        func: Box<Expr>,
        partition_by: Vec<Expr>,
        order_by: Vec<OrderByExpr>,
    },
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // String
    Concat,
    // Bitwise
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

impl fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOperator::Eq => write!(f, "="),
            BinaryOperator::Ne => write!(f, "<>"),
            BinaryOperator::Lt => write!(f, "<"),
            BinaryOperator::Le => write!(f, "<="),
            BinaryOperator::Gt => write!(f, ">"),
            BinaryOperator::Ge => write!(f, ">="),
            BinaryOperator::And => write!(f, "AND"),
            BinaryOperator::Or => write!(f, "OR"),
            BinaryOperator::Add => write!(f, "+"),
            BinaryOperator::Sub => write!(f, "-"),
            BinaryOperator::Mul => write!(f, "*"),
            BinaryOperator::Div => write!(f, "/"),
            BinaryOperator::Mod => write!(f, "%"),
            BinaryOperator::Concat => write!(f, "+"),
            BinaryOperator::BitwiseAnd => write!(f, "&"),
            BinaryOperator::BitwiseOr => write!(f, "|"),
            BinaryOperator::BitwiseXor => write!(f, "^"),
        }
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Neg,
    BitwiseNot,
}

impl fmt::Display for UnaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOperator::Not => write!(f, "NOT"),
            UnaryOperator::Neg => write!(f, "-"),
            UnaryOperator::BitwiseNot => write!(f, "~"),
        }
    }
}

/// Aggregate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunc {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    StringAgg,
    CountBig,
    StDev,
    StDevP,
    Var,
    VarP,
}

impl fmt::Display for AggregateFunc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AggregateFunc::Count => write!(f, "COUNT"),
            AggregateFunc::Sum => write!(f, "SUM"),
            AggregateFunc::Avg => write!(f, "AVG"),
            AggregateFunc::Min => write!(f, "MIN"),
            AggregateFunc::Max => write!(f, "MAX"),
            AggregateFunc::StringAgg => write!(f, "STRING_AGG"),
            AggregateFunc::CountBig => write!(f, "COUNT_BIG"),
            AggregateFunc::StDev => write!(f, "STDEV"),
            AggregateFunc::StDevP => write!(f, "STDEVP"),
            AggregateFunc::Var => write!(f, "VAR"),
            AggregateFunc::VarP => write!(f, "VARP"),
        }
    }
}

/// Order by expression with direction.
#[derive(Debug, Clone)]
pub struct OrderByExpr {
    pub expr: Expr,
    pub direction: OrderDirection,
    pub nulls: Option<NullsOrder>,
}

/// Order direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderDirection {
    #[default]
    Asc,
    Desc,
}

impl fmt::Display for OrderDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderDirection::Asc => write!(f, "ASC"),
            OrderDirection::Desc => write!(f, "DESC"),
        }
    }
}

/// Nulls ordering in ORDER BY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullsOrder {
    First,
    Last,
}

impl Expr {
    /// Create a column expression.
    pub fn col(name: impl Into<String>) -> Self {
        Expr::Column(name.into())
    }

    /// Create a qualified column expression.
    pub fn qualified_col(table: impl Into<String>, column: impl Into<String>) -> Self {
        Expr::QualifiedColumn {
            table: table.into(),
            column: column.into(),
        }
    }

    /// Create a literal expression.
    pub fn lit<T: Into<SqlValue>>(value: T) -> Self {
        Expr::Literal(value.into())
    }

    /// Create a raw SQL expression.
    pub fn raw(sql: impl Into<String>) -> Self {
        Expr::Raw(sql.into())
    }

    /// Create a parameter placeholder.
    pub fn param(index: usize) -> Self {
        Expr::Param(index)
    }

    /// Create a function call.
    pub fn func(name: impl Into<String>, args: Vec<Expr>) -> Self {
        Expr::Function {
            name: name.into(),
            args,
        }
    }

    // Comparison operators

    /// Equal to (=).
    pub fn eq(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Eq,
            right: Box::new(other.into()),
        }
    }

    /// Not equal to (<>).
    pub fn ne(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Ne,
            right: Box::new(other.into()),
        }
    }

    /// Less than (<).
    pub fn lt(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Lt,
            right: Box::new(other.into()),
        }
    }

    /// Less than or equal (<=).
    pub fn le(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Le,
            right: Box::new(other.into()),
        }
    }

    /// Greater than (>).
    pub fn gt(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Gt,
            right: Box::new(other.into()),
        }
    }

    /// Greater than or equal (>=).
    pub fn ge(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Ge,
            right: Box::new(other.into()),
        }
    }

    // Logical operators

    /// Logical AND.
    pub fn and(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::And,
            right: Box::new(other.into()),
        }
    }

    /// Logical OR.
    pub fn or(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Or,
            right: Box::new(other.into()),
        }
    }

    /// Logical NOT.
    pub fn not(self) -> Self {
        Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr: Box::new(self),
        }
    }

    // Arithmetic operators

    /// Addition (+).
    pub fn add(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Add,
            right: Box::new(other.into()),
        }
    }

    /// Subtraction (-).
    pub fn sub(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Sub,
            right: Box::new(other.into()),
        }
    }

    /// Multiplication (*).
    pub fn mul(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Mul,
            right: Box::new(other.into()),
        }
    }

    /// Division (/).
    pub fn div(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Div,
            right: Box::new(other.into()),
        }
    }

    /// Modulo (%).
    pub fn modulo(self, other: impl Into<Expr>) -> Self {
        Expr::BinaryOp {
            left: Box::new(self),
            op: BinaryOperator::Mod,
            right: Box::new(other.into()),
        }
    }

    // Special expressions

    /// IN expression.
    pub fn in_list(self, list: Vec<Expr>) -> Self {
        Expr::In {
            expr: Box::new(self),
            list,
            negated: false,
        }
    }

    /// NOT IN expression.
    pub fn not_in_list(self, list: Vec<Expr>) -> Self {
        Expr::In {
            expr: Box::new(self),
            list,
            negated: true,
        }
    }

    /// BETWEEN expression.
    pub fn between(self, low: impl Into<Expr>, high: impl Into<Expr>) -> Self {
        Expr::Between {
            expr: Box::new(self),
            low: Box::new(low.into()),
            high: Box::new(high.into()),
            negated: false,
        }
    }

    /// NOT BETWEEN expression.
    pub fn not_between(self, low: impl Into<Expr>, high: impl Into<Expr>) -> Self {
        Expr::Between {
            expr: Box::new(self),
            low: Box::new(low.into()),
            high: Box::new(high.into()),
            negated: true,
        }
    }

    /// LIKE expression.
    pub fn like(self, pattern: impl Into<Expr>) -> Self {
        Expr::Like {
            expr: Box::new(self),
            pattern: Box::new(pattern.into()),
            escape: None,
            negated: false,
        }
    }

    /// NOT LIKE expression.
    pub fn not_like(self, pattern: impl Into<Expr>) -> Self {
        Expr::Like {
            expr: Box::new(self),
            pattern: Box::new(pattern.into()),
            escape: None,
            negated: true,
        }
    }

    /// LIKE with ESCAPE expression.
    pub fn like_escape(self, pattern: impl Into<Expr>, escape: char) -> Self {
        Expr::Like {
            expr: Box::new(self),
            pattern: Box::new(pattern.into()),
            escape: Some(escape),
            negated: false,
        }
    }

    /// IS NULL expression.
    pub fn is_null(self) -> Self {
        Expr::IsNull {
            expr: Box::new(self),
            negated: false,
        }
    }

    /// IS NOT NULL expression.
    pub fn is_not_null(self) -> Self {
        Expr::IsNull {
            expr: Box::new(self),
            negated: true,
        }
    }

    /// CAST expression.
    pub fn cast(self, data_type: impl Into<String>) -> Self {
        Expr::Cast {
            expr: Box::new(self),
            data_type: data_type.into(),
        }
    }

    /// Create an alias for this expression.
    pub fn alias(self, name: impl Into<String>) -> AliasedExpr {
        AliasedExpr {
            expr: self,
            alias: name.into(),
        }
    }

    // Aggregate functions

    /// COUNT aggregate.
    pub fn count(self) -> Self {
        Expr::Aggregate {
            func: AggregateFunc::Count,
            expr: Box::new(self),
            distinct: false,
        }
    }

    /// COUNT DISTINCT aggregate.
    pub fn count_distinct(self) -> Self {
        Expr::Aggregate {
            func: AggregateFunc::Count,
            expr: Box::new(self),
            distinct: true,
        }
    }

    /// SUM aggregate.
    pub fn sum(self) -> Self {
        Expr::Aggregate {
            func: AggregateFunc::Sum,
            expr: Box::new(self),
            distinct: false,
        }
    }

    /// AVG aggregate.
    pub fn avg(self) -> Self {
        Expr::Aggregate {
            func: AggregateFunc::Avg,
            expr: Box::new(self),
            distinct: false,
        }
    }

    /// MIN aggregate.
    pub fn min(self) -> Self {
        Expr::Aggregate {
            func: AggregateFunc::Min,
            expr: Box::new(self),
            distinct: false,
        }
    }

    /// MAX aggregate.
    pub fn max(self) -> Self {
        Expr::Aggregate {
            func: AggregateFunc::Max,
            expr: Box::new(self),
            distinct: false,
        }
    }

    /// Render the expression to SQL.
    pub fn to_sql(&self) -> String {
        match self {
            Expr::Column(name) => quote_identifier(name),
            Expr::QualifiedColumn { table, column } => {
                format!("{}.{}", quote_identifier(table), quote_identifier(column))
            }
            Expr::Literal(value) => value.to_string(),
            Expr::Param(index) => format!("@p{}", index),
            Expr::Raw(sql) => sql.clone(),
            Expr::Function { name, args } => {
                let args_sql: Vec<String> = args.iter().map(|a| a.to_sql()).collect();
                format!("{}({})", name, args_sql.join(", "))
            }
            Expr::BinaryOp { left, op, right } => {
                format!("({} {} {})", left.to_sql(), op, right.to_sql())
            }
            Expr::UnaryOp { op, expr } => {
                format!("{} {}", op, expr.to_sql())
            }
            Expr::Case {
                operand,
                when_clauses,
                else_clause,
            } => {
                let mut sql = "CASE".to_string();
                if let Some(op) = operand {
                    sql.push_str(&format!(" {}", op.to_sql()));
                }
                for (when_expr, then_expr) in when_clauses {
                    sql.push_str(&format!(
                        " WHEN {} THEN {}",
                        when_expr.to_sql(),
                        then_expr.to_sql()
                    ));
                }
                if let Some(else_expr) = else_clause {
                    sql.push_str(&format!(" ELSE {}", else_expr.to_sql()));
                }
                sql.push_str(" END");
                sql
            }
            Expr::Subquery(sql) => format!("({})", sql),
            Expr::In {
                expr,
                list,
                negated,
            } => {
                let not = if *negated { "NOT " } else { "" };
                let values: Vec<String> = list.iter().map(|e| e.to_sql()).collect();
                format!("{} {}IN ({})", expr.to_sql(), not, values.join(", "))
            }
            Expr::Between {
                expr,
                low,
                high,
                negated,
            } => {
                let not = if *negated { "NOT " } else { "" };
                format!(
                    "{} {}BETWEEN {} AND {}",
                    expr.to_sql(),
                    not,
                    low.to_sql(),
                    high.to_sql()
                )
            }
            Expr::Like {
                expr,
                pattern,
                escape,
                negated,
            } => {
                let not = if *negated { "NOT " } else { "" };
                let mut sql = format!("{} {}LIKE {}", expr.to_sql(), not, pattern.to_sql());
                if let Some(esc) = escape {
                    sql.push_str(&format!(" ESCAPE '{}'", esc));
                }
                sql
            }
            Expr::IsNull { expr, negated } => {
                let not = if *negated { " NOT" } else { "" };
                format!("{} IS{} NULL", expr.to_sql(), not)
            }
            Expr::Exists { subquery, negated } => {
                let not = if *negated { "NOT " } else { "" };
                format!("{}EXISTS ({})", not, subquery)
            }
            Expr::Cast { expr, data_type } => {
                format!("CAST({} AS {})", expr.to_sql(), data_type)
            }
            Expr::Aggregate {
                func,
                expr,
                distinct,
            } => {
                let dist = if *distinct { "DISTINCT " } else { "" };
                format!("{}({}{})", func, dist, expr.to_sql())
            }
            Expr::Window {
                func,
                partition_by,
                order_by,
            } => {
                let mut sql = func.to_sql();
                sql.push_str(" OVER (");

                if !partition_by.is_empty() {
                    let parts: Vec<String> = partition_by.iter().map(|e| e.to_sql()).collect();
                    sql.push_str(&format!("PARTITION BY {}", parts.join(", ")));
                }

                if !order_by.is_empty() {
                    if !partition_by.is_empty() {
                        sql.push(' ');
                    }
                    let orders: Vec<String> = order_by
                        .iter()
                        .map(|o| format!("{} {}", o.expr.to_sql(), o.direction))
                        .collect();
                    sql.push_str(&format!("ORDER BY {}", orders.join(", ")));
                }

                sql.push(')');
                sql
            }
        }
    }
}

/// An expression with an alias.
#[derive(Debug, Clone)]
pub struct AliasedExpr {
    pub expr: Expr,
    pub alias: String,
}

impl AliasedExpr {
    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        format!("{} AS {}", self.expr.to_sql(), quote_identifier(&self.alias))
    }
}

// Conversion implementations

impl From<&str> for Expr {
    fn from(s: &str) -> Self {
        Expr::Literal(SqlValue::String(s.to_string()))
    }
}

impl From<String> for Expr {
    fn from(s: String) -> Self {
        Expr::Literal(SqlValue::String(s))
    }
}

impl From<i32> for Expr {
    fn from(v: i32) -> Self {
        Expr::Literal(SqlValue::Int(v))
    }
}

impl From<i64> for Expr {
    fn from(v: i64) -> Self {
        Expr::Literal(SqlValue::BigInt(v))
    }
}

impl From<f64> for Expr {
    fn from(v: f64) -> Self {
        Expr::Literal(SqlValue::Double(v))
    }
}

impl From<bool> for Expr {
    fn from(v: bool) -> Self {
        Expr::Literal(SqlValue::Bool(v))
    }
}

impl From<SqlValue> for Expr {
    fn from(v: SqlValue) -> Self {
        Expr::Literal(v)
    }
}

// Helper functions

/// Quote an identifier for SQL Server.
/// Always uses brackets for safety and consistency.
pub fn quote_identifier(name: &str) -> String {
    // Always quote identifiers in SQL Server for safety
    format!("[{}]", name.replace(']', "]]"))
}

/// Check if a word is a SQL Server reserved word.
fn is_reserved_word(word: &str) -> bool {
    const RESERVED: &[&str] = &[
        "SELECT", "FROM", "WHERE", "INSERT", "UPDATE", "DELETE", "CREATE", "DROP", "ALTER",
        "TABLE", "INDEX", "VIEW", "DATABASE", "SCHEMA", "ORDER", "GROUP", "BY", "AS", "ON",
        "AND", "OR", "NOT", "IN", "IS", "NULL", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER",
        "FULL", "CROSS", "UNION", "INTERSECT", "EXCEPT", "HAVING", "DISTINCT", "TOP", "LIMIT",
        "OFFSET", "FETCH", "CASE", "WHEN", "THEN", "ELSE", "END", "EXISTS", "BETWEEN", "LIKE",
        "ALL", "ANY", "SOME", "INTO", "VALUES", "SET", "DEFAULT", "PRIMARY", "KEY", "FOREIGN",
        "REFERENCES", "CONSTRAINT", "UNIQUE", "CHECK", "ASC", "DESC", "NULLS", "FIRST", "LAST",
        "USER", "ROLE", "GRANT", "REVOKE", "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION",
    ];
    RESERVED.iter().any(|r| r.eq_ignore_ascii_case(word))
}

/// Convenience function to create a column expression.
pub fn col(name: impl Into<String>) -> Expr {
    Expr::col(name)
}

/// Convenience function to create a literal expression.
pub fn lit<T: Into<SqlValue>>(value: T) -> Expr {
    Expr::lit(value)
}

/// Convenience function to create a raw SQL expression.
pub fn raw(sql: impl Into<String>) -> Expr {
    Expr::raw(sql)
}

/// Convenience function for COUNT(*).
pub fn count_all() -> Expr {
    Expr::Raw("COUNT(*)".to_string())
}

/// Build a CASE expression.
pub fn case() -> CaseBuilder {
    CaseBuilder::new()
}

/// Builder for CASE expressions.
#[derive(Debug, Default)]
pub struct CaseBuilder {
    operand: Option<Expr>,
    when_clauses: Vec<(Expr, Expr)>,
    else_clause: Option<Expr>,
}

impl CaseBuilder {
    /// Create a new CASE builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the operand for simple CASE expression.
    pub fn operand(mut self, expr: impl Into<Expr>) -> Self {
        self.operand = Some(expr.into());
        self
    }

    /// Add a WHEN clause.
    pub fn when(mut self, condition: impl Into<Expr>, result: impl Into<Expr>) -> Self {
        self.when_clauses.push((condition.into(), result.into()));
        self
    }

    /// Set the ELSE clause.
    pub fn otherwise(mut self, result: impl Into<Expr>) -> Self {
        self.else_clause = Some(result.into());
        self
    }

    /// Build the CASE expression.
    pub fn build(self) -> Expr {
        Expr::Case {
            operand: self.operand.map(Box::new),
            when_clauses: self.when_clauses,
            else_clause: self.else_clause.map(Box::new),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expression() {
        let expr = col("name").eq("John");
        assert_eq!(expr.to_sql(), "([name] = 'John')");
    }

    #[test]
    fn test_complex_expression() {
        let expr = col("age").ge(18).and(col("status").eq("active"));
        assert_eq!(expr.to_sql(), "(([age] >= 18) AND ([status] = 'active'))");
    }

    #[test]
    fn test_between_expression() {
        let expr = col("price").between(lit(10), lit(100));
        assert_eq!(expr.to_sql(), "[price] BETWEEN 10 AND 100");
    }

    #[test]
    fn test_aggregate_expression() {
        let expr = col("amount").sum();
        assert_eq!(expr.to_sql(), "SUM([amount])");
    }

    #[test]
    fn test_case_expression() {
        let expr = case()
            .when(col("status").eq("active"), lit(1))
            .when(col("status").eq("inactive"), lit(0))
            .otherwise(lit(-1))
            .build();

        assert!(expr.to_sql().contains("CASE"));
        assert!(expr.to_sql().contains("WHEN"));
    }
}

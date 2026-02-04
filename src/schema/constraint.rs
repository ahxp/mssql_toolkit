//! Constraint definitions for tables.

use crate::query::expression::quote_identifier;

/// Table constraint definition.
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Primary key constraint.
    PrimaryKey(PrimaryKeyConstraint),
    /// Unique constraint.
    Unique(UniqueConstraint),
    /// Foreign key constraint.
    ForeignKey(ForeignKeyConstraint),
    /// Check constraint.
    Check(CheckConstraint),
    /// Default constraint.
    Default(DefaultConstraint),
}

impl Constraint {
    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        match self {
            Constraint::PrimaryKey(pk) => pk.to_sql(),
            Constraint::Unique(uq) => uq.to_sql(),
            Constraint::ForeignKey(fk) => fk.to_sql(),
            Constraint::Check(ck) => ck.to_sql(),
            Constraint::Default(df) => df.to_sql(),
        }
    }

    /// Get the constraint name.
    pub fn name(&self) -> Option<&str> {
        match self {
            Constraint::PrimaryKey(pk) => pk.name.as_deref(),
            Constraint::Unique(uq) => uq.name.as_deref(),
            Constraint::ForeignKey(fk) => fk.name.as_deref(),
            Constraint::Check(ck) => ck.name.as_deref(),
            Constraint::Default(df) => df.name.as_deref(),
        }
    }
}

/// Primary key constraint.
#[derive(Debug, Clone)]
pub struct PrimaryKeyConstraint {
    pub name: Option<String>,
    pub columns: Vec<IndexColumn>,
    pub clustered: bool,
    pub fill_factor: Option<u8>,
    pub with_options: Vec<String>,
}

impl PrimaryKeyConstraint {
    /// Create a new primary key constraint.
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            name: None,
            columns: columns.into_iter().map(IndexColumn::from).collect(),
            clustered: true,
            fill_factor: None,
            with_options: Vec::new(),
        }
    }

    /// Set the constraint name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set as non-clustered.
    pub fn nonclustered(mut self) -> Self {
        self.clustered = false;
        self
    }

    /// Set the fill factor.
    pub fn fill_factor(mut self, factor: u8) -> Self {
        self.fill_factor = Some(factor);
        self
    }

    /// Add a WITH option.
    pub fn with_option(mut self, option: impl Into<String>) -> Self {
        self.with_options.push(option.into());
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut sql = String::new();

        if let Some(ref name) = self.name {
            sql.push_str(&format!("CONSTRAINT {} ", quote_identifier(name)));
        }

        sql.push_str("PRIMARY KEY ");

        if self.clustered {
            sql.push_str("CLUSTERED ");
        } else {
            sql.push_str("NONCLUSTERED ");
        }

        let cols: Vec<String> = self.columns.iter().map(|c| c.to_sql()).collect();
        sql.push_str(&format!("({})", cols.join(", ")));

        let mut options = self.with_options.clone();
        if let Some(ff) = self.fill_factor {
            options.push(format!("FILLFACTOR = {}", ff));
        }

        if !options.is_empty() {
            sql.push_str(&format!(" WITH ({})", options.join(", ")));
        }

        sql
    }
}

/// Unique constraint.
#[derive(Debug, Clone)]
pub struct UniqueConstraint {
    pub name: Option<String>,
    pub columns: Vec<IndexColumn>,
    pub clustered: bool,
    pub fill_factor: Option<u8>,
    pub where_clause: Option<String>,
}

impl UniqueConstraint {
    /// Create a new unique constraint.
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            name: None,
            columns: columns.into_iter().map(IndexColumn::from).collect(),
            clustered: false,
            fill_factor: None,
            where_clause: None,
        }
    }

    /// Set the constraint name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set as clustered.
    pub fn clustered(mut self) -> Self {
        self.clustered = true;
        self
    }

    /// Set the fill factor.
    pub fn fill_factor(mut self, factor: u8) -> Self {
        self.fill_factor = Some(factor);
        self
    }

    /// Add a WHERE clause (filtered unique).
    pub fn where_clause(mut self, expr: impl Into<String>) -> Self {
        self.where_clause = Some(expr.into());
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut sql = String::new();

        if let Some(ref name) = self.name {
            sql.push_str(&format!("CONSTRAINT {} ", quote_identifier(name)));
        }

        sql.push_str("UNIQUE ");

        if self.clustered {
            sql.push_str("CLUSTERED ");
        } else {
            sql.push_str("NONCLUSTERED ");
        }

        let cols: Vec<String> = self.columns.iter().map(|c| c.to_sql()).collect();
        sql.push_str(&format!("({})", cols.join(", ")));

        if let Some(ff) = self.fill_factor {
            sql.push_str(&format!(" WITH (FILLFACTOR = {})", ff));
        }

        if let Some(ref where_clause) = self.where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }

        sql
    }
}

/// Foreign key constraint.
#[derive(Debug, Clone)]
pub struct ForeignKeyConstraint {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_schema: Option<String>,
    pub ref_columns: Vec<String>,
    pub on_delete: Option<ReferentialAction>,
    pub on_update: Option<ReferentialAction>,
    pub not_for_replication: bool,
}

/// Referential action for foreign keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferentialAction {
    NoAction,
    Cascade,
    SetNull,
    SetDefault,
}

impl std::fmt::Display for ReferentialAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferentialAction::NoAction => write!(f, "NO ACTION"),
            ReferentialAction::Cascade => write!(f, "CASCADE"),
            ReferentialAction::SetNull => write!(f, "SET NULL"),
            ReferentialAction::SetDefault => write!(f, "SET DEFAULT"),
        }
    }
}

impl ForeignKeyConstraint {
    /// Create a new foreign key constraint.
    pub fn new(columns: Vec<String>, ref_table: impl Into<String>, ref_columns: Vec<String>) -> Self {
        Self {
            name: None,
            columns,
            ref_table: ref_table.into(),
            ref_schema: None,
            ref_columns,
            on_delete: None,
            on_update: None,
            not_for_replication: false,
        }
    }

    /// Set the constraint name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the reference table schema.
    pub fn ref_schema(mut self, schema: impl Into<String>) -> Self {
        self.ref_schema = Some(schema.into());
        self
    }

    /// Set ON DELETE action.
    pub fn on_delete(mut self, action: ReferentialAction) -> Self {
        self.on_delete = Some(action);
        self
    }

    /// Set ON UPDATE action.
    pub fn on_update(mut self, action: ReferentialAction) -> Self {
        self.on_update = Some(action);
        self
    }

    /// Set NOT FOR REPLICATION.
    pub fn not_for_replication(mut self) -> Self {
        self.not_for_replication = true;
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut sql = String::new();

        if let Some(ref name) = self.name {
            sql.push_str(&format!("CONSTRAINT {} ", quote_identifier(name)));
        }

        let cols: Vec<String> = self.columns.iter().map(|c| quote_identifier(c)).collect();
        sql.push_str(&format!("FOREIGN KEY ({})", cols.join(", ")));

        let ref_table = match &self.ref_schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.ref_table)),
            None => quote_identifier(&self.ref_table),
        };

        let ref_cols: Vec<String> = self.ref_columns.iter().map(|c| quote_identifier(c)).collect();
        sql.push_str(&format!(" REFERENCES {} ({})", ref_table, ref_cols.join(", ")));

        if let Some(ref action) = self.on_delete {
            sql.push_str(&format!(" ON DELETE {}", action));
        }

        if let Some(ref action) = self.on_update {
            sql.push_str(&format!(" ON UPDATE {}", action));
        }

        if self.not_for_replication {
            sql.push_str(" NOT FOR REPLICATION");
        }

        sql
    }
}

/// Check constraint.
#[derive(Debug, Clone)]
pub struct CheckConstraint {
    pub name: Option<String>,
    pub expression: String,
    pub not_for_replication: bool,
}

impl CheckConstraint {
    /// Create a new check constraint.
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            name: None,
            expression: expression.into(),
            not_for_replication: false,
        }
    }

    /// Set the constraint name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set NOT FOR REPLICATION.
    pub fn not_for_replication(mut self) -> Self {
        self.not_for_replication = true;
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut sql = String::new();

        if let Some(ref name) = self.name {
            sql.push_str(&format!("CONSTRAINT {} ", quote_identifier(name)));
        }

        sql.push_str(&format!("CHECK ({})", self.expression));

        if self.not_for_replication {
            sql.push_str(" NOT FOR REPLICATION");
        }

        sql
    }
}

/// Default constraint.
#[derive(Debug, Clone)]
pub struct DefaultConstraint {
    pub name: Option<String>,
    pub column: String,
    pub expression: String,
}

impl DefaultConstraint {
    /// Create a new default constraint.
    pub fn new(column: impl Into<String>, expression: impl Into<String>) -> Self {
        Self {
            name: None,
            column: column.into(),
            expression: expression.into(),
        }
    }

    /// Set the constraint name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut sql = String::new();

        if let Some(ref name) = self.name {
            sql.push_str(&format!("CONSTRAINT {} ", quote_identifier(name)));
        }

        sql.push_str(&format!(
            "DEFAULT {} FOR {}",
            self.expression,
            quote_identifier(&self.column)
        ));

        sql
    }
}

/// Index column specification.
#[derive(Debug, Clone)]
pub struct IndexColumn {
    pub name: String,
    pub descending: bool,
}

impl From<String> for IndexColumn {
    fn from(name: String) -> Self {
        Self {
            name,
            descending: false,
        }
    }
}

impl From<&str> for IndexColumn {
    fn from(name: &str) -> Self {
        Self {
            name: name.to_string(),
            descending: false,
        }
    }
}

impl IndexColumn {
    /// Create a new index column.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            descending: false,
        }
    }

    /// Set descending order.
    pub fn desc(mut self) -> Self {
        self.descending = true;
        self
    }

    /// Set ascending order.
    pub fn asc(mut self) -> Self {
        self.descending = false;
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        if self.descending {
            format!("{} DESC", quote_identifier(&self.name))
        } else {
            quote_identifier(&self.name)
        }
    }
}

// Builder functions

/// Create a primary key constraint.
pub fn primary_key<I, S>(columns: I) -> PrimaryKeyConstraint
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    PrimaryKeyConstraint::new(columns.into_iter().map(|c| c.into()).collect())
}

/// Create a unique constraint.
pub fn unique<I, S>(columns: I) -> UniqueConstraint
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    UniqueConstraint::new(columns.into_iter().map(|c| c.into()).collect())
}

/// Create a foreign key constraint.
pub fn foreign_key<I1, I2, S1, S2>(
    columns: I1,
    ref_table: impl Into<String>,
    ref_columns: I2,
) -> ForeignKeyConstraint
where
    I1: IntoIterator<Item = S1>,
    I2: IntoIterator<Item = S2>,
    S1: Into<String>,
    S2: Into<String>,
{
    ForeignKeyConstraint::new(
        columns.into_iter().map(|c| c.into()).collect(),
        ref_table,
        ref_columns.into_iter().map(|c| c.into()).collect(),
    )
}

/// Create a check constraint.
pub fn check(expression: impl Into<String>) -> CheckConstraint {
    CheckConstraint::new(expression)
}

/// Create a default constraint.
pub fn default_constraint(column: impl Into<String>, expression: impl Into<String>) -> DefaultConstraint {
    DefaultConstraint::new(column, expression)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primary_key() {
        let pk = primary_key(["id"]).name("PK_users");
        assert!(pk.to_sql().contains("CONSTRAINT [PK_users]"));
        assert!(pk.to_sql().contains("PRIMARY KEY CLUSTERED"));
    }

    #[test]
    fn test_foreign_key() {
        let fk = foreign_key(["user_id"], "users", ["id"])
            .name("FK_orders_users")
            .on_delete(ReferentialAction::Cascade);

        assert!(fk.to_sql().contains("FOREIGN KEY"));
        assert!(fk.to_sql().contains("REFERENCES [users]"));
        assert!(fk.to_sql().contains("ON DELETE CASCADE"));
    }

    #[test]
    fn test_check_constraint() {
        let ck = check("age >= 0 AND age <= 150").name("CK_users_age");
        assert!(ck.to_sql().contains("CHECK (age >= 0 AND age <= 150)"));
    }

    #[test]
    fn test_unique_constraint() {
        let uq = unique(["email"]).name("UQ_users_email");
        assert!(uq.to_sql().contains("UNIQUE NONCLUSTERED"));
    }
}

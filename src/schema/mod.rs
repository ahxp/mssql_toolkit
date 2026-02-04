//! Schema management for creating and modifying database objects.
//!
//! This module provides builders for creating tables, indexes, constraints,
//! and other database schema objects.

mod table;
mod column;
mod index;
mod constraint;
mod alter;

pub use table::*;
pub use column::*;
pub use index::*;
pub use constraint::*;
pub use alter::*;

use crate::error::MssqlResult;

/// Trait for schema objects that can be rendered to SQL.
pub trait SchemaObject {
    /// Generate the CREATE statement.
    fn to_create_sql(&self) -> MssqlResult<String>;

    /// Generate the DROP statement.
    fn to_drop_sql(&self) -> MssqlResult<String>;
}

/// Database object types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Table,
    View,
    Index,
    Procedure,
    Function,
    Trigger,
    Schema,
    Constraint,
    Sequence,
    Type,
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectType::Table => write!(f, "TABLE"),
            ObjectType::View => write!(f, "VIEW"),
            ObjectType::Index => write!(f, "INDEX"),
            ObjectType::Procedure => write!(f, "PROCEDURE"),
            ObjectType::Function => write!(f, "FUNCTION"),
            ObjectType::Trigger => write!(f, "TRIGGER"),
            ObjectType::Schema => write!(f, "SCHEMA"),
            ObjectType::Constraint => write!(f, "CONSTRAINT"),
            ObjectType::Sequence => write!(f, "SEQUENCE"),
            ObjectType::Type => write!(f, "TYPE"),
        }
    }
}

/// Check if an object exists in the database.
pub fn check_exists_sql(object_type: ObjectType, schema: Option<&str>, name: &str) -> String {
    match object_type {
        ObjectType::Table => {
            let full_name = match schema {
                Some(s) => format!("{}.{}", s, name),
                None => name.to_string(),
            };
            format!(
                "SELECT CASE WHEN OBJECT_ID(N'{}', N'U') IS NOT NULL THEN 1 ELSE 0 END",
                full_name
            )
        }
        ObjectType::View => {
            let full_name = match schema {
                Some(s) => format!("{}.{}", s, name),
                None => name.to_string(),
            };
            format!(
                "SELECT CASE WHEN OBJECT_ID(N'{}', N'V') IS NOT NULL THEN 1 ELSE 0 END",
                full_name
            )
        }
        ObjectType::Index => {
            format!(
                "SELECT CASE WHEN EXISTS (SELECT 1 FROM sys.indexes WHERE name = N'{}') THEN 1 ELSE 0 END",
                name
            )
        }
        ObjectType::Procedure => {
            let full_name = match schema {
                Some(s) => format!("{}.{}", s, name),
                None => name.to_string(),
            };
            format!(
                "SELECT CASE WHEN OBJECT_ID(N'{}', N'P') IS NOT NULL THEN 1 ELSE 0 END",
                full_name
            )
        }
        ObjectType::Function => {
            let full_name = match schema {
                Some(s) => format!("{}.{}", s, name),
                None => name.to_string(),
            };
            format!(
                "SELECT CASE WHEN OBJECT_ID(N'{}', N'FN') IS NOT NULL OR OBJECT_ID(N'{}', N'TF') IS NOT NULL OR OBJECT_ID(N'{}', N'IF') IS NOT NULL THEN 1 ELSE 0 END",
                full_name, full_name, full_name
            )
        }
        ObjectType::Trigger => {
            format!(
                "SELECT CASE WHEN EXISTS (SELECT 1 FROM sys.triggers WHERE name = N'{}') THEN 1 ELSE 0 END",
                name
            )
        }
        ObjectType::Schema => {
            format!(
                "SELECT CASE WHEN EXISTS (SELECT 1 FROM sys.schemas WHERE name = N'{}') THEN 1 ELSE 0 END",
                name
            )
        }
        ObjectType::Constraint => {
            format!(
                "SELECT CASE WHEN EXISTS (SELECT 1 FROM sys.objects WHERE name = N'{}' AND type IN ('C', 'D', 'F', 'PK', 'UQ')) THEN 1 ELSE 0 END",
                name
            )
        }
        ObjectType::Sequence => {
            let full_name = match schema {
                Some(s) => format!("{}.{}", s, name),
                None => name.to_string(),
            };
            format!(
                "SELECT CASE WHEN OBJECT_ID(N'{}', N'SO') IS NOT NULL THEN 1 ELSE 0 END",
                full_name
            )
        }
        ObjectType::Type => {
            format!(
                "SELECT CASE WHEN EXISTS (SELECT 1 FROM sys.types WHERE name = N'{}') THEN 1 ELSE 0 END",
                name
            )
        }
    }
}

/// Generate DROP IF EXISTS statement.
pub fn drop_if_exists_sql(object_type: ObjectType, schema: Option<&str>, name: &str) -> String {
    let full_name = match schema {
        Some(s) => format!("[{}].[{}]", s, name),
        None => format!("[{}]", name),
    };

    match object_type {
        ObjectType::Table => format!("DROP TABLE IF EXISTS {}", full_name),
        ObjectType::View => format!("DROP VIEW IF EXISTS {}", full_name),
        ObjectType::Index => {
            // Index requires table name which we don't have here
            format!("-- DROP INDEX {} requires table name", name)
        }
        ObjectType::Procedure => format!("DROP PROCEDURE IF EXISTS {}", full_name),
        ObjectType::Function => format!("DROP FUNCTION IF EXISTS {}", full_name),
        ObjectType::Trigger => format!("DROP TRIGGER IF EXISTS {}", full_name),
        ObjectType::Schema => format!("DROP SCHEMA IF EXISTS [{}]", name),
        ObjectType::Sequence => format!("DROP SEQUENCE IF EXISTS {}", full_name),
        ObjectType::Type => format!("DROP TYPE IF EXISTS {}", full_name),
        ObjectType::Constraint => {
            // Constraint requires table name
            format!("-- DROP CONSTRAINT {} requires table name", name)
        }
    }
}

/// Schema builder for creating a database schema.
#[derive(Debug, Clone)]
pub struct CreateSchemaBuilder {
    name: String,
    authorization: Option<String>,
}

impl CreateSchemaBuilder {
    /// Create a new schema builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            authorization: None,
        }
    }

    /// Set the schema owner.
    pub fn authorization(mut self, owner: impl Into<String>) -> Self {
        self.authorization = Some(owner.into());
        self
    }

    /// Build the CREATE SCHEMA statement.
    pub fn build(&self) -> String {
        let mut sql = format!("CREATE SCHEMA [{}]", self.name);
        if let Some(ref auth) = self.authorization {
            sql.push_str(&format!(" AUTHORIZATION [{}]", auth));
        }
        sql
    }

    /// Build with IF NOT EXISTS check.
    pub fn build_if_not_exists(&self) -> String {
        format!(
            "IF NOT EXISTS (SELECT 1 FROM sys.schemas WHERE name = N'{}')\nBEGIN\n    {}\nEND",
            self.name,
            self.build()
        )
    }
}

/// Create a schema builder.
pub fn create_schema(name: impl Into<String>) -> CreateSchemaBuilder {
    CreateSchemaBuilder::new(name)
}

/// Sequence builder.
#[derive(Debug, Clone)]
pub struct CreateSequenceBuilder {
    schema: Option<String>,
    name: String,
    data_type: String,
    start: i64,
    increment: i64,
    min_value: Option<i64>,
    max_value: Option<i64>,
    cycle: bool,
    cache: Option<i64>,
}

impl CreateSequenceBuilder {
    /// Create a new sequence builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
            data_type: "BIGINT".to_string(),
            start: 1,
            increment: 1,
            min_value: None,
            max_value: None,
            cycle: false,
            cache: None,
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Set the data type.
    pub fn data_type(mut self, data_type: impl Into<String>) -> Self {
        self.data_type = data_type.into();
        self
    }

    /// Set the start value.
    pub fn start(mut self, start: i64) -> Self {
        self.start = start;
        self
    }

    /// Set the increment.
    pub fn increment(mut self, increment: i64) -> Self {
        self.increment = increment;
        self
    }

    /// Set the minimum value.
    pub fn min_value(mut self, min: i64) -> Self {
        self.min_value = Some(min);
        self
    }

    /// Set the maximum value.
    pub fn max_value(mut self, max: i64) -> Self {
        self.max_value = Some(max);
        self
    }

    /// Enable cycling.
    pub fn cycle(mut self) -> Self {
        self.cycle = true;
        self
    }

    /// Set cache size.
    pub fn cache(mut self, size: i64) -> Self {
        self.cache = Some(size);
        self
    }

    /// Build the CREATE SEQUENCE statement.
    pub fn build(&self) -> String {
        let full_name = match &self.schema {
            Some(s) => format!("[{}].[{}]", s, self.name),
            None => format!("[{}]", self.name),
        };

        let mut sql = format!(
            "CREATE SEQUENCE {} AS {} START WITH {} INCREMENT BY {}",
            full_name, self.data_type, self.start, self.increment
        );

        if let Some(min) = self.min_value {
            sql.push_str(&format!(" MINVALUE {}", min));
        }

        if let Some(max) = self.max_value {
            sql.push_str(&format!(" MAXVALUE {}", max));
        }

        if self.cycle {
            sql.push_str(" CYCLE");
        } else {
            sql.push_str(" NO CYCLE");
        }

        if let Some(cache) = self.cache {
            sql.push_str(&format!(" CACHE {}", cache));
        }

        sql
    }
}

/// Create a sequence builder.
pub fn create_sequence(name: impl Into<String>) -> CreateSequenceBuilder {
    CreateSequenceBuilder::new(name)
}

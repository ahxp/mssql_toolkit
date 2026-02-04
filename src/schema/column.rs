//! Column definition builder.

use crate::types::{SqlType, VarCharLength};

/// Column definition for table creation.
#[derive(Debug, Clone)]
pub struct ColumnDefinition {
    /// Column name.
    pub name: String,
    /// Data type.
    pub data_type: SqlType,
    /// Whether the column allows NULL.
    pub nullable: bool,
    /// Default value expression.
    pub default: Option<String>,
    /// Identity specification.
    pub identity: Option<IdentitySpec>,
    /// Computed column expression.
    pub computed: Option<ComputedSpec>,
    /// Collation.
    pub collation: Option<String>,
    /// Row GUID column.
    pub rowguidcol: bool,
    /// Sparse column.
    pub sparse: bool,
    /// Column description/comment.
    pub description: Option<String>,
}

/// Identity column specification.
#[derive(Debug, Clone)]
pub struct IdentitySpec {
    pub seed: i64,
    pub increment: i64,
}

impl Default for IdentitySpec {
    fn default() -> Self {
        Self {
            seed: 1,
            increment: 1,
        }
    }
}

/// Computed column specification.
#[derive(Debug, Clone)]
pub struct ComputedSpec {
    pub expression: String,
    pub persisted: bool,
}

impl ColumnDefinition {
    /// Create a new column definition.
    pub fn new(name: impl Into<String>, data_type: SqlType) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable: true,
            default: None,
            identity: None,
            computed: None,
            collation: None,
            rowguidcol: false,
            sparse: false,
            description: None,
        }
    }

    /// Set the column as NOT NULL.
    pub fn not_null(mut self) -> Self {
        self.nullable = false;
        self
    }

    /// Set the column as NULL.
    pub fn null(mut self) -> Self {
        self.nullable = true;
        self
    }

    /// Set the default value.
    pub fn default(mut self, expr: impl Into<String>) -> Self {
        self.default = Some(expr.into());
        self
    }

    /// Set the default value to a literal string.
    pub fn default_string(mut self, value: impl Into<String>) -> Self {
        self.default = Some(format!("'{}'", value.into().replace('\'', "''")));
        self
    }

    /// Set the default value to a literal number.
    pub fn default_number<N: std::fmt::Display>(mut self, value: N) -> Self {
        self.default = Some(value.to_string());
        self
    }

    /// Set the default to GETDATE().
    pub fn default_now(mut self) -> Self {
        self.default = Some("GETDATE()".to_string());
        self
    }

    /// Set the default to GETUTCDATE().
    pub fn default_utc_now(mut self) -> Self {
        self.default = Some("GETUTCDATE()".to_string());
        self
    }

    /// Set the default to NEWID().
    pub fn default_newid(mut self) -> Self {
        self.default = Some("NEWID()".to_string());
        self
    }

    /// Set the default to NEWSEQUENTIALID().
    pub fn default_sequential_id(mut self) -> Self {
        self.default = Some("NEWSEQUENTIALID()".to_string());
        self
    }

    /// Make this an identity column.
    pub fn identity(mut self) -> Self {
        self.identity = Some(IdentitySpec::default());
        self
    }

    /// Make this an identity column with custom seed and increment.
    pub fn identity_with(mut self, seed: i64, increment: i64) -> Self {
        self.identity = Some(IdentitySpec { seed, increment });
        self
    }

    /// Make this a computed column.
    pub fn computed(mut self, expression: impl Into<String>) -> Self {
        self.computed = Some(ComputedSpec {
            expression: expression.into(),
            persisted: false,
        });
        self
    }

    /// Make this a persisted computed column.
    pub fn computed_persisted(mut self, expression: impl Into<String>) -> Self {
        self.computed = Some(ComputedSpec {
            expression: expression.into(),
            persisted: true,
        });
        self
    }

    /// Set the collation.
    pub fn collation(mut self, collation: impl Into<String>) -> Self {
        self.collation = Some(collation.into());
        self
    }

    /// Mark as ROWGUIDCOL.
    pub fn rowguidcol(mut self) -> Self {
        self.rowguidcol = true;
        self
    }

    /// Mark as SPARSE.
    pub fn sparse(mut self) -> Self {
        self.sparse = true;
        self
    }

    /// Set the column description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Render to SQL.
    pub fn to_sql(&self) -> String {
        let mut parts = Vec::new();

        // Column name
        parts.push(format!("[{}]", self.name));

        // Computed column
        if let Some(ref computed) = self.computed {
            parts.push(format!("AS ({})", computed.expression));
            if computed.persisted {
                parts.push("PERSISTED".to_string());
            }
        } else {
            // Data type
            parts.push(self.data_type.to_sql());

            // Collation
            if let Some(ref coll) = self.collation {
                parts.push(format!("COLLATE {}", coll));
            }

            // Sparse
            if self.sparse {
                parts.push("SPARSE".to_string());
            }

            // Identity
            if let Some(ref identity) = self.identity {
                parts.push(format!(
                    "IDENTITY({}, {})",
                    identity.seed, identity.increment
                ));
            }

            // ROWGUIDCOL
            if self.rowguidcol {
                parts.push("ROWGUIDCOL".to_string());
            }

            // NULL/NOT NULL
            if self.nullable {
                parts.push("NULL".to_string());
            } else {
                parts.push("NOT NULL".to_string());
            }

            // Default
            if let Some(ref default) = self.default {
                parts.push(format!("DEFAULT {}", default));
            }
        }

        parts.join(" ")
    }
}

/// Column builder for fluent API.
#[derive(Debug)]
pub struct ColumnBuilder {
    name: String,
}

impl ColumnBuilder {
    /// Create a new column builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    // Integer types

    /// Create a BIGINT column.
    pub fn bigint(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::BigInt)
    }

    /// Create an INT column.
    pub fn int(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Int)
    }

    /// Create a SMALLINT column.
    pub fn smallint(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::SmallInt)
    }

    /// Create a TINYINT column.
    pub fn tinyint(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::TinyInt)
    }

    /// Create a BIT column.
    pub fn bit(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Bit)
    }

    // Decimal types

    /// Create a DECIMAL column.
    pub fn decimal(self, precision: u8, scale: u8) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Decimal { precision, scale })
    }

    /// Create a NUMERIC column.
    pub fn numeric(self, precision: u8, scale: u8) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Numeric { precision, scale })
    }

    /// Create a MONEY column.
    pub fn money(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Money)
    }

    /// Create a SMALLMONEY column.
    pub fn smallmoney(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::SmallMoney)
    }

    // Floating point types

    /// Create a FLOAT column.
    pub fn float(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Float { precision: None })
    }

    /// Create a FLOAT column with precision.
    pub fn float_with_precision(self, precision: u8) -> ColumnDefinition {
        ColumnDefinition::new(
            self.name,
            SqlType::Float {
                precision: Some(precision),
            },
        )
    }

    /// Create a REAL column.
    pub fn real(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Real)
    }

    // String types

    /// Create a CHAR column.
    pub fn char(self, length: u16) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Char { length })
    }

    /// Create a VARCHAR column.
    pub fn varchar(self, length: u16) -> ColumnDefinition {
        ColumnDefinition::new(
            self.name,
            SqlType::VarChar {
                length: VarCharLength::Length(length),
            },
        )
    }

    /// Create a VARCHAR(MAX) column.
    pub fn varchar_max(self) -> ColumnDefinition {
        ColumnDefinition::new(
            self.name,
            SqlType::VarChar {
                length: VarCharLength::Max,
            },
        )
    }

    /// Create a TEXT column.
    pub fn text(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Text)
    }

    // Unicode string types

    /// Create an NCHAR column.
    pub fn nchar(self, length: u16) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::NChar { length })
    }

    /// Create an NVARCHAR column.
    pub fn nvarchar(self, length: u16) -> ColumnDefinition {
        ColumnDefinition::new(
            self.name,
            SqlType::NVarChar {
                length: VarCharLength::Length(length),
            },
        )
    }

    /// Create an NVARCHAR(MAX) column.
    pub fn nvarchar_max(self) -> ColumnDefinition {
        ColumnDefinition::new(
            self.name,
            SqlType::NVarChar {
                length: VarCharLength::Max,
            },
        )
    }

    /// Create an NTEXT column.
    pub fn ntext(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::NText)
    }

    // Binary types

    /// Create a BINARY column.
    pub fn binary(self, length: u16) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Binary { length })
    }

    /// Create a VARBINARY column.
    pub fn varbinary(self, length: u16) -> ColumnDefinition {
        ColumnDefinition::new(
            self.name,
            SqlType::VarBinary {
                length: VarCharLength::Length(length),
            },
        )
    }

    /// Create a VARBINARY(MAX) column.
    pub fn varbinary_max(self) -> ColumnDefinition {
        ColumnDefinition::new(
            self.name,
            SqlType::VarBinary {
                length: VarCharLength::Max,
            },
        )
    }

    /// Create an IMAGE column.
    pub fn image(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Image)
    }

    // Date/Time types

    /// Create a DATE column.
    pub fn date(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Date)
    }

    /// Create a TIME column.
    pub fn time(self, precision: u8) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Time { precision })
    }

    /// Create a DATETIME column.
    pub fn datetime(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::DateTime)
    }

    /// Create a DATETIME2 column.
    pub fn datetime2(self, precision: u8) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::DateTime2 { precision })
    }

    /// Create a SMALLDATETIME column.
    pub fn smalldatetime(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::SmallDateTime)
    }

    /// Create a DATETIMEOFFSET column.
    pub fn datetimeoffset(self, precision: u8) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::DateTimeOffset { precision })
    }

    // Other types

    /// Create a UNIQUEIDENTIFIER column.
    pub fn uniqueidentifier(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::UniqueIdentifier)
    }

    /// Create an XML column.
    pub fn xml(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Xml)
    }

    /// Create a ROWVERSION column.
    pub fn rowversion(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::RowVersion)
    }

    /// Create a HIERARCHYID column.
    pub fn hierarchyid(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::HierarchyId)
    }

    /// Create a GEOMETRY column.
    pub fn geometry(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Geometry)
    }

    /// Create a GEOGRAPHY column.
    pub fn geography(self) -> ColumnDefinition {
        ColumnDefinition::new(self.name, SqlType::Geography)
    }

    /// Create a column with a custom SQL type.
    pub fn custom_type(self, sql_type: SqlType) -> ColumnDefinition {
        ColumnDefinition::new(self.name, sql_type)
    }
}

/// Create a column builder.
pub fn column(name: impl Into<String>) -> ColumnBuilder {
    ColumnBuilder::new(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_column() {
        let col = column("id").int().not_null();
        assert_eq!(col.to_sql(), "[id] INT NOT NULL");
    }

    #[test]
    fn test_identity_column() {
        let col = column("id").int().identity().not_null();
        assert!(col.to_sql().contains("IDENTITY(1, 1)"));
    }

    #[test]
    fn test_varchar_with_default() {
        let col = column("name").nvarchar(100).not_null().default_string("Unknown");
        assert!(col.to_sql().contains("NVARCHAR(100)"));
        assert!(col.to_sql().contains("DEFAULT 'Unknown'"));
    }

    #[test]
    fn test_datetime_with_default() {
        let col = column("created_at").datetime2(7).not_null().default_now();
        assert!(col.to_sql().contains("DATETIME2(7)"));
        assert!(col.to_sql().contains("DEFAULT GETDATE()"));
    }

    #[test]
    fn test_computed_column() {
        let col = column("full_name")
            .nvarchar(200)
            .computed_persisted("first_name + ' ' + last_name");
        assert!(col.to_sql().contains("AS (first_name + ' ' + last_name)"));
        assert!(col.to_sql().contains("PERSISTED"));
    }
}

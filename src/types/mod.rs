//! SQL data types and type conversion utilities.
//!
//! This module provides Rust representations of SQL Server data types
//! and utilities for converting between Rust and SQL types.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::error::{MssqlError, MssqlResult, TypeConversionError};

/// Represents all SQL Server data types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlType {
    // Exact numerics
    BigInt,
    Int,
    SmallInt,
    TinyInt,
    Bit,
    Decimal { precision: u8, scale: u8 },
    Numeric { precision: u8, scale: u8 },
    Money,
    SmallMoney,

    // Approximate numerics
    Float { precision: Option<u8> },
    Real,

    // Date and time
    Date,
    Time { precision: u8 },
    DateTime,
    DateTime2 { precision: u8 },
    SmallDateTime,
    DateTimeOffset { precision: u8 },

    // Character strings
    Char { length: u16 },
    VarChar { length: VarCharLength },
    Text,

    // Unicode character strings
    NChar { length: u16 },
    NVarChar { length: VarCharLength },
    NText,

    // Binary strings
    Binary { length: u16 },
    VarBinary { length: VarCharLength },
    Image,

    // Other data types
    UniqueIdentifier,
    Xml,
    Json,

    // SQL Server specific
    RowVersion,
    HierarchyId,
    Geometry,
    Geography,
}

/// Variable character length specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarCharLength {
    /// Fixed length (1-8000 for varchar, 1-4000 for nvarchar)
    Length(u16),
    /// Maximum length (varchar(max), nvarchar(max), varbinary(max))
    Max,
}

impl fmt::Display for VarCharLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarCharLength::Length(len) => write!(f, "{}", len),
            VarCharLength::Max => write!(f, "MAX"),
        }
    }
}

impl SqlType {
    /// Convert the SQL type to its SQL Server representation.
    pub fn to_sql(&self) -> String {
        match self {
            SqlType::BigInt => "BIGINT".to_string(),
            SqlType::Int => "INT".to_string(),
            SqlType::SmallInt => "SMALLINT".to_string(),
            SqlType::TinyInt => "TINYINT".to_string(),
            SqlType::Bit => "BIT".to_string(),
            SqlType::Decimal { precision, scale } => format!("DECIMAL({}, {})", precision, scale),
            SqlType::Numeric { precision, scale } => format!("NUMERIC({}, {})", precision, scale),
            SqlType::Money => "MONEY".to_string(),
            SqlType::SmallMoney => "SMALLMONEY".to_string(),
            SqlType::Float { precision } => {
                if let Some(p) = precision {
                    format!("FLOAT({})", p)
                } else {
                    "FLOAT".to_string()
                }
            }
            SqlType::Real => "REAL".to_string(),
            SqlType::Date => "DATE".to_string(),
            SqlType::Time { precision } => format!("TIME({})", precision),
            SqlType::DateTime => "DATETIME".to_string(),
            SqlType::DateTime2 { precision } => format!("DATETIME2({})", precision),
            SqlType::SmallDateTime => "SMALLDATETIME".to_string(),
            SqlType::DateTimeOffset { precision } => format!("DATETIMEOFFSET({})", precision),
            SqlType::Char { length } => format!("CHAR({})", length),
            SqlType::VarChar { length } => format!("VARCHAR({})", length),
            SqlType::Text => "TEXT".to_string(),
            SqlType::NChar { length } => format!("NCHAR({})", length),
            SqlType::NVarChar { length } => format!("NVARCHAR({})", length),
            SqlType::NText => "NTEXT".to_string(),
            SqlType::Binary { length } => format!("BINARY({})", length),
            SqlType::VarBinary { length } => format!("VARBINARY({})", length),
            SqlType::Image => "IMAGE".to_string(),
            SqlType::UniqueIdentifier => "UNIQUEIDENTIFIER".to_string(),
            SqlType::Xml => "XML".to_string(),
            SqlType::Json => "NVARCHAR(MAX)".to_string(), // SQL Server stores JSON as NVARCHAR
            SqlType::RowVersion => "ROWVERSION".to_string(),
            SqlType::HierarchyId => "HIERARCHYID".to_string(),
            SqlType::Geometry => "GEOMETRY".to_string(),
            SqlType::Geography => "GEOGRAPHY".to_string(),
        }
    }

    /// Check if the type is nullable by default.
    pub fn is_nullable_by_default(&self) -> bool {
        true // In SQL Server, columns are nullable by default
    }

    /// Get the default value expression for this type.
    pub fn default_value(&self) -> Option<String> {
        match self {
            SqlType::BigInt | SqlType::Int | SqlType::SmallInt | SqlType::TinyInt => {
                Some("0".to_string())
            }
            SqlType::Bit => Some("0".to_string()),
            SqlType::Decimal { .. } | SqlType::Numeric { .. } => Some("0".to_string()),
            SqlType::Float { .. } | SqlType::Real => Some("0.0".to_string()),
            SqlType::Date => Some("GETDATE()".to_string()),
            SqlType::DateTime | SqlType::DateTime2 { .. } | SqlType::SmallDateTime => {
                Some("GETDATE()".to_string())
            }
            SqlType::UniqueIdentifier => Some("NEWID()".to_string()),
            _ => None,
        }
    }

    /// Check if this type supports identity/auto-increment.
    pub fn supports_identity(&self) -> bool {
        matches!(
            self,
            SqlType::BigInt | SqlType::Int | SqlType::SmallInt | SqlType::TinyInt
        )
    }

    /// Parse a SQL type string into a SqlType.
    pub fn parse(s: &str) -> MssqlResult<Self> {
        let s = s.trim().to_uppercase();

        if s == "BIGINT" {
            return Ok(SqlType::BigInt);
        }
        if s == "INT" || s == "INTEGER" {
            return Ok(SqlType::Int);
        }
        if s == "SMALLINT" {
            return Ok(SqlType::SmallInt);
        }
        if s == "TINYINT" {
            return Ok(SqlType::TinyInt);
        }
        if s == "BIT" {
            return Ok(SqlType::Bit);
        }
        if s == "MONEY" {
            return Ok(SqlType::Money);
        }
        if s == "SMALLMONEY" {
            return Ok(SqlType::SmallMoney);
        }
        if s == "REAL" {
            return Ok(SqlType::Real);
        }
        if s == "DATE" {
            return Ok(SqlType::Date);
        }
        if s == "DATETIME" {
            return Ok(SqlType::DateTime);
        }
        if s == "SMALLDATETIME" {
            return Ok(SqlType::SmallDateTime);
        }
        if s == "TEXT" {
            return Ok(SqlType::Text);
        }
        if s == "NTEXT" {
            return Ok(SqlType::NText);
        }
        if s == "IMAGE" {
            return Ok(SqlType::Image);
        }
        if s == "UNIQUEIDENTIFIER" {
            return Ok(SqlType::UniqueIdentifier);
        }
        if s == "XML" {
            return Ok(SqlType::Xml);
        }
        if s == "ROWVERSION" || s == "TIMESTAMP" {
            return Ok(SqlType::RowVersion);
        }
        if s == "HIERARCHYID" {
            return Ok(SqlType::HierarchyId);
        }
        if s == "GEOMETRY" {
            return Ok(SqlType::Geometry);
        }
        if s == "GEOGRAPHY" {
            return Ok(SqlType::Geography);
        }
        if s == "FLOAT" {
            return Ok(SqlType::Float { precision: None });
        }

        // Parse parameterized types
        if let Some(inner) = extract_params(&s, "DECIMAL") {
            let (p, sc) = parse_precision_scale(&inner)?;
            return Ok(SqlType::Decimal {
                precision: p,
                scale: sc,
            });
        }
        if let Some(inner) = extract_params(&s, "NUMERIC") {
            let (p, sc) = parse_precision_scale(&inner)?;
            return Ok(SqlType::Numeric {
                precision: p,
                scale: sc,
            });
        }
        if let Some(inner) = extract_params(&s, "FLOAT") {
            let p: u8 = inner.parse().map_err(|_| {
                MssqlError::TypeConversion(TypeConversionError::InvalidFormat {
                    target_type: "FLOAT precision".to_string(),
                    value: inner.clone(),
                })
            })?;
            return Ok(SqlType::Float { precision: Some(p) });
        }
        if let Some(inner) = extract_params(&s, "TIME") {
            let p: u8 = inner.parse().unwrap_or(7);
            return Ok(SqlType::Time { precision: p });
        }
        if let Some(inner) = extract_params(&s, "DATETIME2") {
            let p: u8 = inner.parse().unwrap_or(7);
            return Ok(SqlType::DateTime2 { precision: p });
        }
        if let Some(inner) = extract_params(&s, "DATETIMEOFFSET") {
            let p: u8 = inner.parse().unwrap_or(7);
            return Ok(SqlType::DateTimeOffset { precision: p });
        }
        if let Some(inner) = extract_params(&s, "CHAR") {
            let len: u16 = inner.parse().map_err(|_| {
                MssqlError::TypeConversion(TypeConversionError::InvalidFormat {
                    target_type: "CHAR length".to_string(),
                    value: inner.clone(),
                })
            })?;
            return Ok(SqlType::Char { length: len });
        }
        if let Some(inner) = extract_params(&s, "VARCHAR") {
            let length = parse_varchar_length(&inner)?;
            return Ok(SqlType::VarChar { length });
        }
        if let Some(inner) = extract_params(&s, "NCHAR") {
            let len: u16 = inner.parse().map_err(|_| {
                MssqlError::TypeConversion(TypeConversionError::InvalidFormat {
                    target_type: "NCHAR length".to_string(),
                    value: inner.clone(),
                })
            })?;
            return Ok(SqlType::NChar { length: len });
        }
        if let Some(inner) = extract_params(&s, "NVARCHAR") {
            let length = parse_varchar_length(&inner)?;
            return Ok(SqlType::NVarChar { length });
        }
        if let Some(inner) = extract_params(&s, "BINARY") {
            let len: u16 = inner.parse().map_err(|_| {
                MssqlError::TypeConversion(TypeConversionError::InvalidFormat {
                    target_type: "BINARY length".to_string(),
                    value: inner.clone(),
                })
            })?;
            return Ok(SqlType::Binary { length: len });
        }
        if let Some(inner) = extract_params(&s, "VARBINARY") {
            let length = parse_varchar_length(&inner)?;
            return Ok(SqlType::VarBinary { length });
        }

        Err(MssqlError::TypeConversion(
            TypeConversionError::InvalidFormat {
                target_type: "SqlType".to_string(),
                value: s,
            },
        ))
    }
}

fn extract_params(s: &str, prefix: &str) -> Option<String> {
    if s.starts_with(prefix) && s.contains('(') && s.ends_with(')') {
        let start = s.find('(')?;
        let inner = &s[start + 1..s.len() - 1];
        Some(inner.trim().to_string())
    } else {
        None
    }
}

fn parse_precision_scale(s: &str) -> MssqlResult<(u8, u8)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(MssqlError::TypeConversion(
            TypeConversionError::InvalidFormat {
                target_type: "precision,scale".to_string(),
                value: s.to_string(),
            },
        ));
    }
    let precision: u8 = parts[0].trim().parse().map_err(|_| {
        MssqlError::TypeConversion(TypeConversionError::InvalidFormat {
            target_type: "precision".to_string(),
            value: parts[0].to_string(),
        })
    })?;
    let scale: u8 = parts[1].trim().parse().map_err(|_| {
        MssqlError::TypeConversion(TypeConversionError::InvalidFormat {
            target_type: "scale".to_string(),
            value: parts[1].to_string(),
        })
    })?;
    Ok((precision, scale))
}

fn parse_varchar_length(s: &str) -> MssqlResult<VarCharLength> {
    let s = s.trim().to_uppercase();
    if s == "MAX" {
        Ok(VarCharLength::Max)
    } else {
        let len: u16 = s.parse().map_err(|_| {
            MssqlError::TypeConversion(TypeConversionError::InvalidFormat {
                target_type: "varchar length".to_string(),
                value: s.clone(),
            })
        })?;
        Ok(VarCharLength::Length(len))
    }
}

/// A dynamic SQL value that can hold any SQL-compatible type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlValue {
    Null,
    Bool(bool),
    TinyInt(u8),
    SmallInt(i16),
    Int(i32),
    BigInt(i64),
    Float(f32),
    Double(f64),
    Decimal(Decimal),
    String(String),
    Binary(Vec<u8>),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(NaiveDateTime),
    DateTimeUtc(DateTime<Utc>),
    Uuid(Uuid),
    Json(serde_json::Value),
}

impl SqlValue {
    /// Check if the value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, SqlValue::Null)
    }

    /// Try to convert to a specific type.
    pub fn try_into_i32(&self) -> MssqlResult<i32> {
        match self {
            SqlValue::Int(v) => Ok(*v),
            SqlValue::SmallInt(v) => Ok(*v as i32),
            SqlValue::TinyInt(v) => Ok(*v as i32),
            SqlValue::BigInt(v) => {
                if *v >= i32::MIN as i64 && *v <= i32::MAX as i64 {
                    Ok(*v as i32)
                } else {
                    Err(MssqlError::TypeConversion(TypeConversionError::Overflow {
                        target_type: "i32".to_string(),
                    }))
                }
            }
            _ => Err(MssqlError::TypeConversion(
                TypeConversionError::ConversionFailed {
                    from: format!("{:?}", self),
                    to: "i32".to_string(),
                },
            )),
        }
    }

    /// Try to convert to i64.
    pub fn try_into_i64(&self) -> MssqlResult<i64> {
        match self {
            SqlValue::BigInt(v) => Ok(*v),
            SqlValue::Int(v) => Ok(*v as i64),
            SqlValue::SmallInt(v) => Ok(*v as i64),
            SqlValue::TinyInt(v) => Ok(*v as i64),
            _ => Err(MssqlError::TypeConversion(
                TypeConversionError::ConversionFailed {
                    from: format!("{:?}", self),
                    to: "i64".to_string(),
                },
            )),
        }
    }

    /// Try to convert to String.
    pub fn try_into_string(&self) -> MssqlResult<String> {
        match self {
            SqlValue::String(v) => Ok(v.clone()),
            SqlValue::Int(v) => Ok(v.to_string()),
            SqlValue::BigInt(v) => Ok(v.to_string()),
            SqlValue::Bool(v) => Ok(v.to_string()),
            SqlValue::Uuid(v) => Ok(v.to_string()),
            _ => Err(MssqlError::TypeConversion(
                TypeConversionError::ConversionFailed {
                    from: format!("{:?}", self),
                    to: "String".to_string(),
                },
            )),
        }
    }

    /// Try to convert to bool.
    pub fn try_into_bool(&self) -> MssqlResult<bool> {
        match self {
            SqlValue::Bool(v) => Ok(*v),
            SqlValue::Int(v) => Ok(*v != 0),
            SqlValue::TinyInt(v) => Ok(*v != 0),
            _ => Err(MssqlError::TypeConversion(
                TypeConversionError::ConversionFailed {
                    from: format!("{:?}", self),
                    to: "bool".to_string(),
                },
            )),
        }
    }

    /// Try to convert to Decimal.
    pub fn try_into_decimal(&self) -> MssqlResult<Decimal> {
        match self {
            SqlValue::Decimal(v) => Ok(*v),
            SqlValue::Int(v) => Ok(Decimal::from(*v)),
            SqlValue::BigInt(v) => Ok(Decimal::from(*v)),
            SqlValue::Float(v) => Decimal::try_from(*v).map_err(|_| {
                MssqlError::TypeConversion(TypeConversionError::InvalidDecimal(v.to_string()))
            }),
            SqlValue::Double(v) => Decimal::try_from(*v).map_err(|_| {
                MssqlError::TypeConversion(TypeConversionError::InvalidDecimal(v.to_string()))
            }),
            _ => Err(MssqlError::TypeConversion(
                TypeConversionError::ConversionFailed {
                    from: format!("{:?}", self),
                    to: "Decimal".to_string(),
                },
            )),
        }
    }
}

impl fmt::Display for SqlValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlValue::Null => write!(f, "NULL"),
            SqlValue::Bool(v) => write!(f, "{}", if *v { "1" } else { "0" }),
            SqlValue::TinyInt(v) => write!(f, "{}", v),
            SqlValue::SmallInt(v) => write!(f, "{}", v),
            SqlValue::Int(v) => write!(f, "{}", v),
            SqlValue::BigInt(v) => write!(f, "{}", v),
            SqlValue::Float(v) => write!(f, "{}", v),
            SqlValue::Double(v) => write!(f, "{}", v),
            SqlValue::Decimal(v) => write!(f, "{}", v),
            SqlValue::String(v) => write!(f, "'{}'", v.replace('\'', "''")),
            SqlValue::Binary(v) => {
                write!(f, "0x")?;
                for byte in v {
                    write!(f, "{:02X}", byte)?;
                }
                Ok(())
            }
            SqlValue::Date(v) => write!(f, "'{}'", v.format("%Y-%m-%d")),
            SqlValue::Time(v) => write!(f, "'{}'", v.format("%H:%M:%S%.f")),
            SqlValue::DateTime(v) => write!(f, "'{}'", v.format("%Y-%m-%d %H:%M:%S%.f")),
            SqlValue::DateTimeUtc(v) => write!(f, "'{}'", v.format("%Y-%m-%d %H:%M:%S%.f")),
            SqlValue::Uuid(v) => write!(f, "'{}'", v),
            SqlValue::Json(v) => write!(f, "'{}'", v.to_string().replace('\'', "''")),
        }
    }
}

// Conversion implementations
impl From<bool> for SqlValue {
    fn from(v: bool) -> Self {
        SqlValue::Bool(v)
    }
}

impl From<i32> for SqlValue {
    fn from(v: i32) -> Self {
        SqlValue::Int(v)
    }
}

impl From<i64> for SqlValue {
    fn from(v: i64) -> Self {
        SqlValue::BigInt(v)
    }
}

impl From<f32> for SqlValue {
    fn from(v: f32) -> Self {
        SqlValue::Float(v)
    }
}

impl From<f64> for SqlValue {
    fn from(v: f64) -> Self {
        SqlValue::Double(v)
    }
}

impl From<String> for SqlValue {
    fn from(v: String) -> Self {
        SqlValue::String(v)
    }
}

impl From<&str> for SqlValue {
    fn from(v: &str) -> Self {
        SqlValue::String(v.to_string())
    }
}

impl From<Uuid> for SqlValue {
    fn from(v: Uuid) -> Self {
        SqlValue::Uuid(v)
    }
}

impl From<Decimal> for SqlValue {
    fn from(v: Decimal) -> Self {
        SqlValue::Decimal(v)
    }
}

impl From<NaiveDate> for SqlValue {
    fn from(v: NaiveDate) -> Self {
        SqlValue::Date(v)
    }
}

impl From<NaiveDateTime> for SqlValue {
    fn from(v: NaiveDateTime) -> Self {
        SqlValue::DateTime(v)
    }
}

impl From<DateTime<Utc>> for SqlValue {
    fn from(v: DateTime<Utc>) -> Self {
        SqlValue::DateTimeUtc(v)
    }
}

impl<T: Into<SqlValue>> From<Option<T>> for SqlValue {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(val) => val.into(),
            None => SqlValue::Null,
        }
    }
}

impl From<serde_json::Value> for SqlValue {
    fn from(v: serde_json::Value) -> Self {
        SqlValue::Json(v)
    }
}

impl From<Vec<u8>> for SqlValue {
    fn from(v: Vec<u8>) -> Self {
        SqlValue::Binary(v)
    }
}

/// Trait for types that can be converted to SQL values.
pub trait ToSqlValue {
    fn to_sql_value(&self) -> SqlValue;
}

impl<T: Clone + Into<SqlValue>> ToSqlValue for T {
    fn to_sql_value(&self) -> SqlValue {
        self.clone().into()
    }
}

/// Trait for types that can be converted from SQL values.
pub trait FromSqlValue: Sized {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self>;
}

impl FromSqlValue for i32 {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_i32()
    }
}

impl FromSqlValue for i64 {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_i64()
    }
}

impl FromSqlValue for String {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_string()
    }
}

impl FromSqlValue for bool {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_bool()
    }
}

impl FromSqlValue for Decimal {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        value.try_into_decimal()
    }
}

impl<T: FromSqlValue> FromSqlValue for Option<T> {
    fn from_sql_value(value: SqlValue) -> MssqlResult<Self> {
        if value.is_null() {
            Ok(None)
        } else {
            T::from_sql_value(value).map(Some)
        }
    }
}

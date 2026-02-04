//! Stored procedure support.
//!
//! This module provides builders for creating, executing, and managing
//! stored procedures in SQL Server.

use crate::connection::{MssqlConnection, MssqlRow};
use crate::error::{MssqlResult, QueryError, MssqlError};
use crate::query::expression::quote_identifier;
use crate::types::SqlValue;

/// Parameter direction for stored procedure parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParameterDirection {
    #[default]
    Input,
    Output,
    InputOutput,
    ReturnValue,
}

/// Stored procedure parameter.
#[derive(Debug, Clone)]
pub struct ProcedureParameter {
    pub name: String,
    pub data_type: String,
    pub direction: ParameterDirection,
    pub default_value: Option<String>,
    pub value: Option<SqlValue>,
}

impl ProcedureParameter {
    /// Create a new input parameter.
    pub fn input(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            direction: ParameterDirection::Input,
            default_value: None,
            value: None,
        }
    }

    /// Create a new output parameter.
    pub fn output(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            direction: ParameterDirection::Output,
            default_value: None,
            value: None,
        }
    }

    /// Create a new input/output parameter.
    pub fn input_output(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            direction: ParameterDirection::InputOutput,
            default_value: None,
            value: None,
        }
    }

    /// Set a default value.
    pub fn default(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    /// Set the parameter value.
    pub fn value<V: Into<SqlValue>>(mut self, value: V) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Render for CREATE PROCEDURE.
    pub fn to_definition_sql(&self) -> String {
        let mut sql = format!("@{} {}", self.name, self.data_type);

        if matches!(
            self.direction,
            ParameterDirection::Output | ParameterDirection::InputOutput
        ) {
            sql.push_str(" OUTPUT");
        }

        if let Some(ref default) = self.default_value {
            sql.push_str(&format!(" = {}", default));
        }

        sql
    }

    /// Render for EXEC call.
    pub fn to_exec_sql(&self) -> String {
        let value_str = match &self.value {
            Some(v) => v.to_string(),
            None => "NULL".to_string(),
        };

        match self.direction {
            ParameterDirection::Input => {
                format!("@{} = {}", self.name, value_str)
            }
            ParameterDirection::Output | ParameterDirection::InputOutput => {
                format!("@{} = @{} OUTPUT", self.name, self.name)
            }
            ParameterDirection::ReturnValue => String::new(),
        }
    }
}

/// Builder for creating stored procedures.
#[derive(Debug, Clone)]
pub struct CreateProcedureBuilder {
    schema: Option<String>,
    name: String,
    parameters: Vec<ProcedureParameter>,
    body: String,
    with_options: Vec<String>,
    or_alter: bool,
}

impl CreateProcedureBuilder {
    /// Create a new procedure builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
            parameters: Vec::new(),
            body: String::new(),
            with_options: Vec::new(),
            or_alter: false,
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Add a parameter.
    pub fn parameter(mut self, param: ProcedureParameter) -> Self {
        self.parameters.push(param);
        self
    }

    /// Add an input parameter.
    pub fn input_param(mut self, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        self.parameters
            .push(ProcedureParameter::input(name, data_type));
        self
    }

    /// Add an output parameter.
    pub fn output_param(mut self, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        self.parameters
            .push(ProcedureParameter::output(name, data_type));
        self
    }

    /// Set the procedure body.
    pub fn body(mut self, sql: impl Into<String>) -> Self {
        self.body = sql.into();
        self
    }

    /// Add WITH RECOMPILE option.
    pub fn with_recompile(mut self) -> Self {
        self.with_options.push("RECOMPILE".to_string());
        self
    }

    /// Add WITH ENCRYPTION option.
    pub fn with_encryption(mut self) -> Self {
        self.with_options.push("ENCRYPTION".to_string());
        self
    }

    /// Add WITH EXECUTE AS option.
    pub fn execute_as(mut self, user: impl Into<String>) -> Self {
        self.with_options
            .push(format!("EXECUTE AS '{}'", user.into()));
        self
    }

    /// Use CREATE OR ALTER instead of CREATE.
    pub fn or_alter(mut self) -> Self {
        self.or_alter = true;
        self
    }

    /// Get the full procedure name.
    fn full_name(&self) -> String {
        match &self.schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.name)),
            None => quote_identifier(&self.name),
        }
    }

    /// Build the CREATE PROCEDURE statement.
    pub fn build(&self) -> String {
        let mut sql = String::new();

        if self.or_alter {
            sql.push_str("CREATE OR ALTER PROCEDURE ");
        } else {
            sql.push_str("CREATE PROCEDURE ");
        }

        sql.push_str(&self.full_name());

        // Parameters
        if !self.parameters.is_empty() {
            sql.push_str("\n");
            let params: Vec<String> = self
                .parameters
                .iter()
                .map(|p| format!("    {}", p.to_definition_sql()))
                .collect();
            sql.push_str(&params.join(",\n"));
            sql.push_str("\n");
        }

        // WITH options
        if !self.with_options.is_empty() {
            sql.push_str(&format!("WITH {}\n", self.with_options.join(", ")));
        }

        sql.push_str("AS\nBEGIN\n");
        sql.push_str("    SET NOCOUNT ON;\n");

        // Body
        for line in self.body.lines() {
            sql.push_str(&format!("    {}\n", line));
        }

        sql.push_str("END");

        sql
    }

    /// Build with IF NOT EXISTS check.
    pub fn build_if_not_exists(&self) -> String {
        let full_name = self.full_name();
        format!(
            "IF NOT EXISTS (SELECT 1 FROM sys.procedures WHERE object_id = OBJECT_ID(N'{}''))\nBEGIN\n    EXEC('{}')\nEND",
            full_name.replace('[', "").replace(']', ""),
            self.build().replace('\'', "''")
        )
    }
}

/// Create a stored procedure builder.
pub fn create_procedure(name: impl Into<String>) -> CreateProcedureBuilder {
    CreateProcedureBuilder::new(name)
}

/// Builder for executing stored procedures.
#[derive(Debug)]
pub struct ExecProcedureBuilder<'a> {
    connection: &'a MssqlConnection,
    schema: Option<String>,
    name: String,
    parameters: Vec<ProcedureParameter>,
    timeout: Option<u32>,
}

impl<'a> ExecProcedureBuilder<'a> {
    /// Create a new exec builder.
    pub fn new(connection: &'a MssqlConnection, name: impl Into<String>) -> Self {
        Self {
            connection,
            schema: None,
            name: name.into(),
            parameters: Vec::new(),
            timeout: None,
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Add a parameter with value.
    pub fn param<V: Into<SqlValue>>(mut self, name: impl Into<String>, value: V) -> Self {
        let mut param = ProcedureParameter::input(name, "");
        param.value = Some(value.into());
        self.parameters.push(param);
        self
    }

    /// Add an output parameter.
    pub fn output_param(mut self, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        self.parameters
            .push(ProcedureParameter::output(name, data_type));
        self
    }

    /// Set execution timeout.
    pub fn timeout(mut self, seconds: u32) -> Self {
        self.timeout = Some(seconds);
        self
    }

    /// Get the full procedure name.
    fn full_name(&self) -> String {
        match &self.schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.name)),
            None => quote_identifier(&self.name),
        }
    }

    /// Build the EXEC statement.
    pub fn build(&self) -> String {
        let mut sql = String::new();

        // Declare output variables
        let output_params: Vec<_> = self
            .parameters
            .iter()
            .filter(|p| {
                matches!(
                    p.direction,
                    ParameterDirection::Output | ParameterDirection::InputOutput
                )
            })
            .collect();

        for param in &output_params {
            sql.push_str(&format!("DECLARE @{} {};\n", param.name, param.data_type));
        }

        // EXEC statement
        sql.push_str(&format!("EXEC {}", self.full_name()));

        if !self.parameters.is_empty() {
            let params: Vec<String> = self
                .parameters
                .iter()
                .filter_map(|p| {
                    let sql = p.to_exec_sql();
                    if sql.is_empty() {
                        None
                    } else {
                        Some(sql)
                    }
                })
                .collect();

            if !params.is_empty() {
                sql.push_str(&format!(" {}", params.join(", ")));
            }
        }

        sql.push(';');

        // Select output variables
        if !output_params.is_empty() {
            let selects: Vec<String> = output_params
                .iter()
                .map(|p| format!("@{} AS [{}]", p.name, p.name))
                .collect();
            sql.push_str(&format!("\nSELECT {};", selects.join(", ")));
        }

        sql
    }

    /// Execute the procedure and return result rows.
    pub async fn execute(&self) -> MssqlResult<Vec<MssqlRow>> {
        let sql = self.build();
        self.connection.query(&sql, &[]).await
    }

    /// Execute the procedure and return scalar result.
    pub async fn execute_scalar<T: crate::connection::FromSqlRow>(&self) -> MssqlResult<T> {
        let rows = self.execute().await?;
        if rows.is_empty() {
            return Err(MssqlError::Query(QueryError::NoRowsAffected));
        }
        T::from_row(&rows[0])
    }
}

/// Extension trait for executing stored procedures.
pub trait StoredProcedureExt {
    /// Execute a stored procedure.
    fn exec_procedure(&self, name: impl Into<String>) -> ExecProcedureBuilder<'_>;
}

impl StoredProcedureExt for MssqlConnection {
    fn exec_procedure(&self, name: impl Into<String>) -> ExecProcedureBuilder<'_> {
        ExecProcedureBuilder::new(self, name)
    }
}

/// Drop procedure builder.
#[derive(Debug, Clone)]
pub struct DropProcedureBuilder {
    schema: Option<String>,
    name: String,
    if_exists: bool,
}

impl DropProcedureBuilder {
    /// Create a new drop procedure builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
            if_exists: false,
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Set IF EXISTS behavior.
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }

    /// Build the DROP PROCEDURE statement.
    pub fn build(&self) -> String {
        let full_name = match &self.schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.name)),
            None => quote_identifier(&self.name),
        };

        if self.if_exists {
            format!("DROP PROCEDURE IF EXISTS {}", full_name)
        } else {
            format!("DROP PROCEDURE {}", full_name)
        }
    }
}

/// Create a drop procedure builder.
pub fn drop_procedure(name: impl Into<String>) -> DropProcedureBuilder {
    DropProcedureBuilder::new(name)
}

/// User-defined function builder.
#[derive(Debug, Clone)]
pub struct CreateFunctionBuilder {
    schema: Option<String>,
    name: String,
    parameters: Vec<ProcedureParameter>,
    return_type: String,
    body: String,
    is_table_valued: bool,
    with_options: Vec<String>,
    or_alter: bool,
}

impl CreateFunctionBuilder {
    /// Create a scalar function builder.
    pub fn scalar(name: impl Into<String>, return_type: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
            parameters: Vec::new(),
            return_type: return_type.into(),
            body: String::new(),
            is_table_valued: false,
            with_options: Vec::new(),
            or_alter: false,
        }
    }

    /// Create an inline table-valued function builder.
    pub fn table_valued(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
            parameters: Vec::new(),
            return_type: "TABLE".to_string(),
            body: String::new(),
            is_table_valued: true,
            with_options: Vec::new(),
            or_alter: false,
        }
    }

    /// Set the schema.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Add a parameter.
    pub fn parameter(mut self, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        self.parameters
            .push(ProcedureParameter::input(name, data_type));
        self
    }

    /// Set the function body.
    pub fn body(mut self, sql: impl Into<String>) -> Self {
        self.body = sql.into();
        self
    }

    /// Add WITH SCHEMABINDING option.
    pub fn with_schemabinding(mut self) -> Self {
        self.with_options.push("SCHEMABINDING".to_string());
        self
    }

    /// Use CREATE OR ALTER.
    pub fn or_alter(mut self) -> Self {
        self.or_alter = true;
        self
    }

    /// Get the full function name.
    fn full_name(&self) -> String {
        match &self.schema {
            Some(s) => format!("{}.{}", quote_identifier(s), quote_identifier(&self.name)),
            None => quote_identifier(&self.name),
        }
    }

    /// Build the CREATE FUNCTION statement.
    pub fn build(&self) -> String {
        let mut sql = String::new();

        if self.or_alter {
            sql.push_str("CREATE OR ALTER FUNCTION ");
        } else {
            sql.push_str("CREATE FUNCTION ");
        }

        sql.push_str(&self.full_name());

        // Parameters
        sql.push('(');
        if !self.parameters.is_empty() {
            let params: Vec<String> = self
                .parameters
                .iter()
                .map(|p| format!("@{} {}", p.name, p.data_type))
                .collect();
            sql.push_str(&params.join(", "));
        }
        sql.push_str(")\n");

        // Return type
        sql.push_str(&format!("RETURNS {}\n", self.return_type));

        // WITH options
        if !self.with_options.is_empty() {
            sql.push_str(&format!("WITH {}\n", self.with_options.join(", ")));
        }

        if self.is_table_valued {
            sql.push_str("AS\n");
            sql.push_str("RETURN (\n");
            sql.push_str(&self.body);
            sql.push_str("\n)");
        } else {
            sql.push_str("AS\nBEGIN\n");
            for line in self.body.lines() {
                sql.push_str(&format!("    {}\n", line));
            }
            sql.push_str("END");
        }

        sql
    }
}

/// Create a scalar function builder.
pub fn create_scalar_function(
    name: impl Into<String>,
    return_type: impl Into<String>,
) -> CreateFunctionBuilder {
    CreateFunctionBuilder::scalar(name, return_type)
}

/// Create a table-valued function builder.
pub fn create_table_function(name: impl Into<String>) -> CreateFunctionBuilder {
    CreateFunctionBuilder::table_valued(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_procedure() {
        let sql = create_procedure("usp_GetUser")
            .schema("dbo")
            .input_param("UserId", "INT")
            .body(
                r#"
SELECT * FROM Users WHERE Id = @UserId;
"#,
            )
            .build();

        assert!(sql.contains("CREATE PROCEDURE [dbo].[usp_GetUser]"));
        assert!(sql.contains("@UserId INT"));
        assert!(sql.contains("SET NOCOUNT ON"));
    }

    #[test]
    fn test_create_procedure_with_output() {
        let sql = create_procedure("usp_CreateUser")
            .input_param("Name", "NVARCHAR(100)")
            .output_param("NewId", "INT")
            .body(
                r#"
INSERT INTO Users (Name) VALUES (@Name);
SET @NewId = SCOPE_IDENTITY();
"#,
            )
            .build();

        assert!(sql.contains("@NewId INT OUTPUT"));
    }

    #[test]
    fn test_create_scalar_function() {
        let sql = create_scalar_function("fn_GetFullName", "NVARCHAR(200)")
            .parameter("FirstName", "NVARCHAR(100)")
            .parameter("LastName", "NVARCHAR(100)")
            .body(
                r#"
DECLARE @FullName NVARCHAR(200);
SET @FullName = @FirstName + ' ' + @LastName;
RETURN @FullName;
"#,
            )
            .build();

        assert!(sql.contains("CREATE FUNCTION"));
        assert!(sql.contains("RETURNS NVARCHAR(200)"));
    }

    #[test]
    fn test_create_table_function() {
        let sql = create_table_function("fn_GetUserOrders")
            .parameter("UserId", "INT")
            .body("SELECT * FROM Orders WHERE UserId = @UserId")
            .build();

        assert!(sql.contains("RETURNS TABLE"));
        assert!(sql.contains("RETURN ("));
    }

    #[test]
    fn test_drop_procedure() {
        let sql = drop_procedure("usp_GetUser").if_exists().build();
        assert_eq!(sql, "DROP PROCEDURE IF EXISTS [usp_GetUser]");
    }
}

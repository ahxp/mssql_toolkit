//! Connection configuration for MSSQL databases.

use crate::error::{ConnectionError, MssqlError, MssqlResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for connecting to an MSSQL database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Server hostname or IP address.
    pub host: String,

    /// Server port (default: 1433).
    pub port: u16,

    /// Database name.
    pub database: String,

    /// Username for authentication.
    pub username: String,

    /// Password for authentication.
    #[serde(skip_serializing)]
    pub password: String,

    /// Application name shown in SQL Server.
    pub application_name: Option<String>,

    /// Connection timeout.
    pub connect_timeout: Duration,

    /// Command/query timeout.
    pub command_timeout: Duration,

    /// Whether to use TLS encryption.
    pub encrypt: EncryptionMode,

    /// Trust server certificate without validation.
    pub trust_server_certificate: bool,

    /// SQL Server instance name (for named instances).
    pub instance: Option<String>,

    /// Maximum packet size.
    pub packet_size: Option<u32>,

    /// Whether to enable MARS (Multiple Active Result Sets).
    pub mars: bool,

    /// Authentication mode.
    pub auth_mode: AuthMode,
}

/// Encryption mode for the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EncryptionMode {
    /// No encryption.
    Off,
    /// Encryption enabled but optional.
    #[default]
    On,
    /// Encryption required.
    Required,
}

/// Authentication mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AuthMode {
    /// SQL Server authentication with username and password.
    #[default]
    SqlServer,
    /// Windows authentication (NTLM/Kerberos).
    Windows,
    /// Azure Active Directory authentication.
    AzureActiveDirectory {
        tenant_id: Option<String>,
        client_id: Option<String>,
    },
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 1433,
            database: "master".to_string(),
            username: String::new(),
            password: String::new(),
            application_name: Some("mssql_toolkit".to_string()),
            connect_timeout: Duration::from_secs(30),
            command_timeout: Duration::from_secs(30),
            encrypt: EncryptionMode::On,
            trust_server_certificate: false,
            instance: None,
            packet_size: None,
            mars: false,
            auth_mode: AuthMode::SqlServer,
        }
    }
}

impl ConnectionConfig {
    /// Create a new connection configuration with required parameters.
    pub fn new(host: impl Into<String>, database: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            database: database.into(),
            ..Default::default()
        }
    }

    /// Set the server port.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the username.
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = username.into();
        self
    }

    /// Set the password.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = password.into();
        self
    }

    /// Set the application name.
    pub fn application_name(mut self, name: impl Into<String>) -> Self {
        self.application_name = Some(name.into());
        self
    }

    /// Set the connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the command timeout.
    pub fn command_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = timeout;
        self
    }

    /// Set encryption mode.
    pub fn encrypt(mut self, mode: EncryptionMode) -> Self {
        self.encrypt = mode;
        self
    }

    /// Set whether to trust the server certificate.
    pub fn trust_server_certificate(mut self, trust: bool) -> Self {
        self.trust_server_certificate = trust;
        self
    }

    /// Set the instance name for named instances.
    pub fn instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Set the packet size.
    pub fn packet_size(mut self, size: u32) -> Self {
        self.packet_size = Some(size);
        self
    }

    /// Enable or disable MARS.
    pub fn mars(mut self, enabled: bool) -> Self {
        self.mars = enabled;
        self
    }

    /// Set authentication mode.
    pub fn auth_mode(mut self, mode: AuthMode) -> Self {
        self.auth_mode = mode;
        self
    }

    /// Parse a connection string into configuration.
    ///
    /// Supported format:
    /// `Server=host,port;Database=db;User Id=user;Password=pass;...`
    pub fn from_connection_string(conn_str: &str) -> MssqlResult<Self> {
        let mut config = Self::default();

        for part in conn_str.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            let (key, value) = part.split_once('=').ok_or_else(|| {
                MssqlError::Connection(ConnectionError::InvalidConnectionString(format!(
                    "Invalid key-value pair: {}",
                    part
                )))
            })?;

            let key = key.trim().to_lowercase();
            let value = value.trim();

            match key.as_str() {
                "server" | "data source" | "host" => {
                    // Handle Server=host,port format
                    if let Some((host, port)) = value.split_once(',') {
                        config.host = host.trim().to_string();
                        config.port = port.trim().parse().map_err(|_| {
                            MssqlError::Connection(ConnectionError::InvalidConnectionString(
                                format!("Invalid port: {}", port),
                            ))
                        })?;
                    } else if let Some((host, instance)) = value.split_once('\\') {
                        config.host = host.trim().to_string();
                        config.instance = Some(instance.trim().to_string());
                    } else {
                        config.host = value.to_string();
                    }
                }
                "port" => {
                    config.port = value.parse().map_err(|_| {
                        MssqlError::Connection(ConnectionError::InvalidConnectionString(format!(
                            "Invalid port: {}",
                            value
                        )))
                    })?;
                }
                "database" | "initial catalog" => {
                    config.database = value.to_string();
                }
                "user id" | "uid" | "username" | "user" => {
                    config.username = value.to_string();
                }
                "password" | "pwd" => {
                    config.password = value.to_string();
                }
                "application name" | "app" => {
                    config.application_name = Some(value.to_string());
                }
                "connect timeout" | "connection timeout" | "timeout" => {
                    let secs: u64 = value.parse().map_err(|_| {
                        MssqlError::Connection(ConnectionError::InvalidConnectionString(format!(
                            "Invalid timeout: {}",
                            value
                        )))
                    })?;
                    config.connect_timeout = Duration::from_secs(secs);
                }
                "command timeout" => {
                    let secs: u64 = value.parse().map_err(|_| {
                        MssqlError::Connection(ConnectionError::InvalidConnectionString(format!(
                            "Invalid command timeout: {}",
                            value
                        )))
                    })?;
                    config.command_timeout = Duration::from_secs(secs);
                }
                "encrypt" => {
                    config.encrypt = match value.to_lowercase().as_str() {
                        "true" | "yes" | "1" | "on" => EncryptionMode::On,
                        "false" | "no" | "0" | "off" => EncryptionMode::Off,
                        "strict" | "required" => EncryptionMode::Required,
                        _ => {
                            return Err(MssqlError::Connection(
                                ConnectionError::InvalidConnectionString(format!(
                                    "Invalid encrypt value: {}",
                                    value
                                )),
                            ))
                        }
                    };
                }
                "trustservercertificate" | "trust server certificate" => {
                    config.trust_server_certificate = matches!(
                        value.to_lowercase().as_str(),
                        "true" | "yes" | "1"
                    );
                }
                "multipleactiveresultsets" | "mars" => {
                    config.mars = matches!(value.to_lowercase().as_str(), "true" | "yes" | "1");
                }
                "packet size" => {
                    config.packet_size = Some(value.parse().map_err(|_| {
                        MssqlError::Connection(ConnectionError::InvalidConnectionString(format!(
                            "Invalid packet size: {}",
                            value
                        )))
                    })?);
                }
                "integrated security" | "trusted_connection" => {
                    if matches!(value.to_lowercase().as_str(), "true" | "yes" | "sspi" | "1") {
                        config.auth_mode = AuthMode::Windows;
                    }
                }
                _ => {
                    // Ignore unknown keys for forward compatibility
                    tracing::debug!("Ignoring unknown connection string key: {}", key);
                }
            }
        }

        config.validate()?;
        Ok(config)
    }

    /// Build a connection string from this configuration.
    pub fn to_connection_string(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref instance) = self.instance {
            parts.push(format!("Server={}\\{}", self.host, instance));
        } else {
            parts.push(format!("Server={},{}", self.host, self.port));
        }

        parts.push(format!("Database={}", self.database));

        match &self.auth_mode {
            AuthMode::SqlServer => {
                parts.push(format!("User Id={}", self.username));
                parts.push(format!("Password={}", self.password));
            }
            AuthMode::Windows => {
                parts.push("Integrated Security=SSPI".to_string());
            }
            AuthMode::AzureActiveDirectory { .. } => {
                parts.push("Authentication=Active Directory Default".to_string());
            }
        }

        if let Some(ref app_name) = self.application_name {
            parts.push(format!("Application Name={}", app_name));
        }

        parts.push(format!(
            "Connect Timeout={}",
            self.connect_timeout.as_secs()
        ));

        let encrypt_str = match self.encrypt {
            EncryptionMode::Off => "False",
            EncryptionMode::On => "True",
            EncryptionMode::Required => "Strict",
        };
        parts.push(format!("Encrypt={}", encrypt_str));

        if self.trust_server_certificate {
            parts.push("TrustServerCertificate=True".to_string());
        }

        if self.mars {
            parts.push("MultipleActiveResultSets=True".to_string());
        }

        if let Some(packet_size) = self.packet_size {
            parts.push(format!("Packet Size={}", packet_size));
        }

        parts.join(";")
    }

    /// Validate the configuration.
    pub fn validate(&self) -> MssqlResult<()> {
        if self.host.is_empty() {
            return Err(MssqlError::Configuration(
                "Host cannot be empty".to_string(),
            ));
        }

        if self.database.is_empty() {
            return Err(MssqlError::Configuration(
                "Database cannot be empty".to_string(),
            ));
        }

        if matches!(self.auth_mode, AuthMode::SqlServer) {
            if self.username.is_empty() {
                return Err(MssqlError::Configuration(
                    "Username is required for SQL Server authentication".to_string(),
                ));
            }
        }

        if let Some(packet_size) = self.packet_size {
            if !(512..=32767).contains(&packet_size) {
                return Err(MssqlError::Configuration(
                    "Packet size must be between 512 and 32767".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Create Tiberius config from this configuration.
    pub fn to_tiberius_config(&self) -> MssqlResult<tiberius::Config> {
        let mut config = tiberius::Config::new();

        config.host(&self.host);
        config.port(self.port);
        config.database(&self.database);

        if let Some(ref instance) = self.instance {
            config.instance_name(instance);
        }

        match &self.auth_mode {
            AuthMode::SqlServer => {
                config.authentication(tiberius::AuthMethod::sql_server(
                    &self.username,
                    &self.password,
                ));
            }
            AuthMode::Windows => {
                #[cfg(windows)]
                {
                    config.authentication(tiberius::AuthMethod::Integrated);
                }
                #[cfg(not(windows))]
                {
                    return Err(MssqlError::Configuration(
                        "Windows authentication is only supported on Windows".to_string(),
                    ));
                }
            }
            AuthMode::AzureActiveDirectory { .. } => {
                return Err(MssqlError::Configuration(
                    "Azure AD authentication requires additional setup".to_string(),
                ));
            }
        }

        if let Some(ref app_name) = self.application_name {
            config.application_name(app_name);
        }

        match self.encrypt {
            EncryptionMode::Off => {
                config.encryption(tiberius::EncryptionLevel::Off);
            }
            EncryptionMode::On => {
                config.encryption(tiberius::EncryptionLevel::On);
            }
            EncryptionMode::Required => {
                config.encryption(tiberius::EncryptionLevel::Required);
            }
        }

        if self.trust_server_certificate {
            config.trust_cert();
        }

        Ok(config)
    }
}

/// Builder pattern for ConnectionConfig.
#[derive(Debug, Default)]
pub struct ConnectionConfigBuilder {
    config: ConnectionConfig,
}

impl ConnectionConfigBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the host.
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.config.host = host.into();
        self
    }

    /// Set the port.
    pub fn port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    /// Set the database.
    pub fn database(mut self, database: impl Into<String>) -> Self {
        self.config.database = database.into();
        self
    }

    /// Set the username.
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.config.username = username.into();
        self
    }

    /// Set the password.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.config.password = password.into();
        self
    }

    /// Set the application name.
    pub fn application_name(mut self, name: impl Into<String>) -> Self {
        self.config.application_name = Some(name.into());
        self
    }

    /// Set connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set command timeout.
    pub fn command_timeout(mut self, timeout: Duration) -> Self {
        self.config.command_timeout = timeout;
        self
    }

    /// Set encryption mode.
    pub fn encrypt(mut self, mode: EncryptionMode) -> Self {
        self.config.encrypt = mode;
        self
    }

    /// Set trust server certificate.
    pub fn trust_server_certificate(mut self, trust: bool) -> Self {
        self.config.trust_server_certificate = trust;
        self
    }

    /// Set instance name.
    pub fn instance(mut self, instance: impl Into<String>) -> Self {
        self.config.instance = Some(instance.into());
        self
    }

    /// Set packet size.
    pub fn packet_size(mut self, size: u32) -> Self {
        self.config.packet_size = Some(size);
        self
    }

    /// Enable MARS.
    pub fn mars(mut self, enabled: bool) -> Self {
        self.config.mars = enabled;
        self
    }

    /// Set authentication mode.
    pub fn auth_mode(mut self, mode: AuthMode) -> Self {
        self.config.auth_mode = mode;
        self
    }

    /// Build the configuration.
    pub fn build(self) -> MssqlResult<ConnectionConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_string_parsing() {
        let conn_str =
            "Server=localhost,1433;Database=testdb;User Id=sa;Password=secret;Encrypt=true";
        let config = ConnectionConfig::from_connection_string(conn_str).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 1433);
        assert_eq!(config.database, "testdb");
        assert_eq!(config.username, "sa");
        assert_eq!(config.password, "secret");
    }

    #[test]
    fn test_connection_string_with_instance() {
        let conn_str = "Server=localhost\\SQLEXPRESS;Database=testdb;User Id=sa;Password=secret";
        let config = ConnectionConfig::from_connection_string(conn_str).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.instance, Some("SQLEXPRESS".to_string()));
    }

    #[test]
    fn test_builder_pattern() {
        let config = ConnectionConfigBuilder::new()
            .host("localhost")
            .port(1433)
            .database("testdb")
            .username("sa")
            .password("secret")
            .build()
            .unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.database, "testdb");
    }
}

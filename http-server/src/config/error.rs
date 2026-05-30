use std::fmt;

/// All errors that can arise during config parsing or validation.
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    /// File could not be read.
    IoError(String),
    /// Unexpected token or structure.
    ParseError { line: usize, message: String },

    MissingField { block: &'static str, field: &'static str },

    InvalidValue { field: &'static str, value: String, reason: &'static str },

    /// Two server blocks share the same host:port with the same server_name.
    ConflictingPort { host: String, port: u16 },

    PathNotFound { field: &'static str, path: String },

    InvalidBodySize { value: String },

    NoRoutes { server: String },

    RootNotDirectory { path: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::IoError(msg) =>
                write!(f, "IO error: {}", msg),
            ConfigError::ParseError { line, message } =>
                write!(f, "Parse error at line {}: {}", line, message),
            ConfigError::MissingField { block, field } =>
                write!(f, "Missing required field '{}' in {} block", field, block),
            ConfigError::InvalidValue { field, value, reason } =>
                write!(f, "Invalid value '{}' for field '{}': {}", value, field, reason),
            ConfigError::ConflictingPort { host, port } =>
                write!(f, "Conflicting server_name for {}:{} — only one default (unnamed) server allowed per host:port", host, port),
            ConfigError::PathNotFound { field, path } =>
                write!(f, "Path not found for '{}': '{}'", field, path),
            ConfigError::InvalidBodySize { value } =>
                write!(f, "Invalid client_max_body_size '{}': must be a positive integer (bytes) or 0 for unlimited", value),
            ConfigError::NoRoutes { server } =>
                write!(f, "Server '{}' has no routes defined", server),
            ConfigError::RootNotDirectory { path } =>
                write!(f, "Route root is not a directory: '{}'", path),
        }
    }
}

impl std::error::Error for ConfigError {}

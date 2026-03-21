use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum AppError {
    #[error("Invalid input: {0}")]
    #[diagnostic(
        code(zinc::invalid_input),
        help("Check your command arguments and try again.")
    )]
    Invalid(String),

    #[error("Configuration error: {0}")]
    #[diagnostic(
        code(zinc::config_error),
        help("Ensure your config file exists and is valid. Run 'zinc setup' to reconfigure.")
    )]
    Config(String),

    #[error("Internal error: {0}")]
    #[diagnostic(code(zinc::internal_error))]
    Internal(String),

    #[error("IO error: {0}")]
    #[diagnostic(code(zinc::io_error))]
    Io(String),

    #[error("Not found: {0}")]
    #[diagnostic(code(zinc::not_found))]
    NotFound(String),

    #[error("Auth error: {0}")]
    #[diagnostic(code(zinc::auth_error))]
    Auth(String),

    #[error("Network error: {0}")]
    #[diagnostic(code(zinc::network_error))]
    Network(String),

    #[error("Insufficient funds: {0}")]
    #[diagnostic(code(zinc::insufficient_funds))]
    InsufficientFunds(String),

    #[error("Policy error: {0}")]
    #[diagnostic(code(zinc::policy_error))]
    Policy(String),
}

impl AppError {
    pub fn tag(&self) -> &str {
        match self {
            Self::Invalid(_) => "invalid",
            Self::Config(_) => "config",
            // Keep IO failures in the documented config/storage bucket for v1.
            Self::Io(_) => "config",
            Self::NotFound(_) => "not_found",
            Self::Auth(_) => "auth",
            Self::Network(_) => "network",
            Self::InsufficientFunds(_) => "insufficient_funds",
            Self::Policy(_) => "policy",
            Self::Internal(_) => "internal",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Invalid(_) => 2,
            Self::Config(_) => 10,
            Self::Io(_) => 10,
            Self::NotFound(_) => 15,
            Self::Auth(_) => 11,
            Self::Network(_) => 12,
            Self::InsufficientFunds(_) => 13,
            Self::Policy(_) => 14,
            Self::Internal(_) => 1,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        Self::Internal(msg)
    }
}

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

    #[error("Capability error: {0}")]
    #[diagnostic(
        code(zinc::capability_error),
        help("This command requires a 'Seed' profile. Your current profile is 'Watch' only.")
    )]
    Capability(String),
}

impl AppError {
    pub fn tag(&self) -> &str {
        match self {
            Self::Invalid(_) => "invalid",
            Self::Config(_) | Self::Io(_) => "config",
            Self::NotFound(_) => "not_found",
            Self::Auth(_) => "auth",
            Self::Network(_) => "network",
            Self::InsufficientFunds(_) => "insufficient_funds",
            Self::Policy(_) => "policy",
            Self::Capability(_) => "capability",
            Self::Internal(_) => "internal",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Invalid(_) => 2,
            Self::Config(_) | Self::Io(_) => 10,
            Self::NotFound(_) => 15,
            Self::Auth(_) => 11,
            Self::Network(_) => 12,
            Self::InsufficientFunds(_) => 13,
            Self::Policy(_) => 14,
            Self::Capability(_) => 16,
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

#[cfg(test)]
mod tests {
    use super::AppError;

    fn all_variants() -> Vec<AppError> {
        vec![
            AppError::Invalid("x".into()),
            AppError::Config("x".into()),
            AppError::Internal("x".into()),
            AppError::Io("x".into()),
            AppError::NotFound("x".into()),
            AppError::Auth("x".into()),
            AppError::Network("x".into()),
            AppError::InsufficientFunds("x".into()),
            AppError::Policy("x".into()),
            AppError::Capability("x".into()),
        ]
    }

    #[test]
    fn tag_matches_each_variant() {
        assert_eq!(AppError::Invalid("a".into()).tag(), "invalid");
        assert_eq!(AppError::Config("a".into()).tag(), "config");
        assert_eq!(AppError::Io("a".into()).tag(), "config");
        assert_eq!(AppError::NotFound("a".into()).tag(), "not_found");
        assert_eq!(AppError::Auth("a".into()).tag(), "auth");
        assert_eq!(AppError::Network("a".into()).tag(), "network");
        assert_eq!(
            AppError::InsufficientFunds("a".into()).tag(),
            "insufficient_funds"
        );
        assert_eq!(AppError::Policy("a".into()).tag(), "policy");
        assert_eq!(AppError::Capability("a".into()).tag(), "capability");
        assert_eq!(AppError::Internal("a".into()).tag(), "internal");
    }

    #[test]
    fn exit_code_matches_each_variant() {
        assert_eq!(AppError::Invalid("a".into()).exit_code(), 2);
        assert_eq!(AppError::Config("a".into()).exit_code(), 10);
        assert_eq!(AppError::Io("a".into()).exit_code(), 10);
        assert_eq!(AppError::NotFound("a".into()).exit_code(), 15);
        assert_eq!(AppError::Auth("a".into()).exit_code(), 11);
        assert_eq!(AppError::Network("a".into()).exit_code(), 12);
        assert_eq!(AppError::InsufficientFunds("a".into()).exit_code(), 13);
        assert_eq!(AppError::Policy("a".into()).exit_code(), 14);
        assert_eq!(AppError::Capability("a".into()).exit_code(), 16);
        assert_eq!(AppError::Internal("a".into()).exit_code(), 1);
    }

    #[test]
    fn exit_codes_are_nonzero_and_tags_are_stable() {
        for err in all_variants() {
            assert_ne!(err.exit_code(), 0, "exit code must be non-zero");
            assert!(!err.tag().is_empty(), "tag must be non-empty");
        }
    }

    #[test]
    fn display_includes_message_payload() {
        assert_eq!(
            AppError::Invalid("bad arg".into()).to_string(),
            "Invalid input: bad arg"
        );
        assert_eq!(
            AppError::Network("timeout".into()).to_string(),
            "Network error: timeout"
        );
        assert_eq!(
            AppError::Config("missing".into()).to_string(),
            "Configuration error: missing"
        );
    }

    #[test]
    fn from_io_error_maps_to_io_variant() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let err: AppError = io.into();
        assert_eq!(err.tag(), "config");
        assert!(matches!(err, AppError::Io(_)));
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn from_string_maps_to_internal_variant() {
        let err: AppError = String::from("boom").into();
        assert_eq!(err.tag(), "internal");
        assert_eq!(err.exit_code(), 1);
        assert!(matches!(err, AppError::Internal(_)));
    }
}

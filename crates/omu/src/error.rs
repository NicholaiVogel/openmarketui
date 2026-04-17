use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("invalid daemon URL: {0}")]
    InvalidDaemonUrl(String),

    #[error("could not connect to daemon at {url}: {source}")]
    DaemonUnavailable {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("daemon returned {status} for {path}: {body}")]
    DaemonStatus {
        status: reqwest::StatusCode,
        path: String,
        body: String,
    },

    #[error("failed to decode daemon response from {path}: {source}")]
    Decode {
        path: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("{resource} not found: {id}")]
    NotFound { resource: String, id: String },

    #[error("confirmation required for {action}; re-run with --yes")]
    ConfirmationRequired { action: String },

    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("policy blocked {action}: {reason}")]
    PolicyBlocked { action: String, reason: String },

    #[error("{message}")]
    Process { message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

impl CliError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidDaemonUrl(_) => "INVALID_DAEMON_URL",
            Self::DaemonUnavailable { .. } => "DAEMON_UNAVAILABLE",
            Self::DaemonStatus { status, .. } if *status == reqwest::StatusCode::NOT_FOUND => {
                "NOT_FOUND"
            }
            Self::DaemonStatus { .. } => "DAEMON_ERROR",
            Self::Decode { .. } => "DECODE_ERROR",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::ConfirmationRequired { .. } => "CONFIRMATION_REQUIRED",
            Self::InvalidArgument { .. } => "INVALID_ARGUMENT",
            Self::PolicyBlocked { .. } => "POLICY_BLOCKED",
            Self::Process { .. } => "PROCESS_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "JSON_ERROR",
            Self::TomlDe(_) => "CONFIG_PARSE_ERROR",
            Self::TomlSer(_) => "CONFIG_SERIALIZE_ERROR",
            Self::Http(_) => "HTTP_ERROR",
        }
    }

    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::DaemonUnavailable { .. } => {
                Some("start the daemon with `cargo run --release -p pm-kalshi -- paper --config config.toml`, or set --daemon-url")
            }
            Self::ConfirmationRequired { .. } => Some("re-run with --yes after reviewing the command"),
            Self::InvalidArgument { .. } => Some("check the command arguments and try again"),
            Self::PolicyBlocked { .. } => {
                Some("review the selected profile policy and the command safety gates")
            }
            Self::Process { .. } => Some("check `omu daemon logs` for details"),
            Self::DaemonStatus { status, .. } if *status == reqwest::StatusCode::NOT_FOUND => {
                Some("the daemon may be older than this CLI; rebuild and restart it")
            }
            Self::TomlDe(_) => Some("check the syntax in the omu config file"),
            _ => None,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ConfirmationRequired { .. } => 2,
            Self::InvalidArgument { .. } => 2,
            Self::PolicyBlocked { .. } => 6,
            Self::NotFound { .. } => 4,
            Self::DaemonUnavailable { .. } => 7,
            Self::Process { .. } => 5,
            _ => 1,
        }
    }
}

//! Engine state definitions

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineState {
    Starting,
    Recovering,
    Running,
    Paused(String),
    ShuttingDown,
}

impl std::fmt::Display for EngineState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Recovering => write!(f, "recovering"),
            Self::Running => write!(f, "running"),
            Self::Paused(reason) => write!(f, "paused: {}", reason),
            Self::ShuttingDown => write!(f, "shutting_down"),
        }
    }
}

//! Ripple's own page runtime protocol and ChatGPT subscription transport.

pub mod document;
pub mod oauth;
pub mod provider;
pub mod store;
pub mod workspace;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("OpenAI request failed during {operation} (HTTP {status}).")]
    Http {
        operation: &'static str,
        status: u16,
    },
    #[error("OpenAI could not be reached. Try again.")]
    Network,
    #[error("The model did not finish successfully.")]
    Incomplete,
    #[error("OpenAI model response failed ({0}).")]
    Model(String),
    #[error("Another Ripple process is using this account store.")]
    StoreLocked,
    #[error("Project validation failed: {0}")]
    Validation(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<reqwest::Error> for Error {
    fn from(_: reqwest::Error) -> Self {
        // Do not surface token bodies, authorization URLs, or request headers.
        Self::Network
    }
}

pub type Result<T> = std::result::Result<T, Error>;

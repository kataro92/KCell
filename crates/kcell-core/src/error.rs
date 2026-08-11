use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("validation: {0}")]
    Validation(String),

    #[error("lifecycle: {0}")]
    Lifecycle(String),

    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("binding: {0}")]
    Binding(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("timeout: {0}")]
    Timeout(String),
}

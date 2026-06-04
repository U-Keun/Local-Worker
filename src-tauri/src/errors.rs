use std::io;

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

impl From<WorkerError> for String {
    fn from(value: WorkerError) -> Self {
        value.to_string()
    }
}

pub type WorkerResult<T> = Result<T, WorkerError>;

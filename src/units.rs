use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Search error: {0}")]
    SearchError(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Image not found: {0}")]
    ImageNotFound(String),
    #[error("Invalid coordinates: {0}")]
    InvalidCoordinates(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),
    #[error("Database error: {0}")]
    Database(#[from] redb::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Cache error: {0}")]
    Cache(String),
}

impl From<std::io::Error> for ClError {
    fn from(e: std::io::Error) -> Self {
        ClError::IoError(e.to_string())
    }
}

impl From<std::path::PathBuf> for ClError {
    fn from(e: std::path::PathBuf) -> Self {
        ClError::DirectoryNotFound(e.display().to_string())
    }
}
// Implement From for redb error types
impl From<redb::TransactionError> for ClError {
    fn from(e: redb::TransactionError) -> Self {
        ClError::Database(redb::Error::from(e))
    }
}

impl From<redb::TableError> for ClError {
    fn from(e: redb::TableError) -> Self {
        ClError::Database(redb::Error::from(e))
    }
}

impl From<redb::StorageError> for ClError {
    fn from(e: redb::StorageError) -> Self {
        ClError::Database(redb::Error::from(e))
    }
}

impl From<redb::CommitError> for ClError {
    fn from(e: redb::CommitError) -> Self {
        ClError::Database(redb::Error::from(e))
    }
}

// impl From<std::io::Error> for ClError {
//     fn from(e: std::io::Error) -> Self {
//         ClError::IoError(e.to_string())
//     }
// }

pub type Result<T> = std::result::Result<T, ClError>;

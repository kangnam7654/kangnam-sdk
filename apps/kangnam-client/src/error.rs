use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Auth error: {0}")]
    Auth(String),
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("MCP error: {0}")]
    Mcp(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<AppError> for String {
    fn from(e: AppError) -> String {
        e.to_string()
    }
}

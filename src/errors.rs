use thiserror::Error;

#[derive(Error, Debug)]
pub enum MuxiError {
    #[error("Authentication error ({code}): {message}")]
    Authentication { code: String, message: String, status: u16 },
    
    #[error("Authorization error ({code}): {message}")]
    Authorization { code: String, message: String, status: u16 },
    
    #[error("Not found ({code}): {message}")]
    NotFound { code: String, message: String, status: u16 },
    
    #[error("Conflict ({code}): {message}")]
    Conflict { code: String, message: String, status: u16 },
    
    #[error("Validation error ({code}): {message}")]
    Validation { code: String, message: String, status: u16 },
    
    #[error("Rate limited: {message} (retry after {retry_after:?}s)")]
    RateLimit { message: String, status: u16, retry_after: Option<u32> },
    
    #[error("Server error ({code}): {message}")]
    Server { code: String, message: String, status: u16 },
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Unknown error ({code}): {message}")]
    Unknown { code: String, message: String, status: u16 },
}

impl MuxiError {
    pub fn from_response(status: u16, code: Option<String>, message: String, retry_after: Option<u32>) -> Self {
        let code = code.unwrap_or_default();
        match status {
            401 => MuxiError::Authentication { code: if code.is_empty() { "UNAUTHORIZED".to_string() } else { code }, message, status },
            403 => MuxiError::Authorization { code: if code.is_empty() { "FORBIDDEN".to_string() } else { code }, message, status },
            404 => MuxiError::NotFound { code: if code.is_empty() { "NOT_FOUND".to_string() } else { code }, message, status },
            409 => MuxiError::Conflict { code: if code.is_empty() { "CONFLICT".to_string() } else { code }, message, status },
            422 => MuxiError::Validation { code: if code.is_empty() { "VALIDATION_ERROR".to_string() } else { code }, message, status },
            429 => MuxiError::RateLimit { message, status, retry_after },
            500..=504 => MuxiError::Server { code: if code.is_empty() { "SERVER_ERROR".to_string() } else { code }, message, status },
            _ => MuxiError::Unknown { code: if code.is_empty() { "ERROR".to_string() } else { code }, message, status },
        }
    }
}

pub type Result<T> = std::result::Result<T, MuxiError>;

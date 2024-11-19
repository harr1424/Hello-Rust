#[derive(Debug)]
pub enum SearchError {
    IoError(std::io::Error),
    SendError(String),
    ThreadError(String),
    PathError(String),
}

impl From<std::io::Error> for SearchError {
    fn from(err: std::io::Error) -> Self {
        SearchError::IoError(err)
    }
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::IoError(e) => write!(f, "IO error: {}", e),
            SearchError::SendError(e) => write!(f, "Send error: {}", e),
            SearchError::ThreadError(e) => write!(f, "Thread error: {}", e),
            SearchError::PathError(e) => write!(f, "Path error: {}", e),
        }
    }
}

impl std::error::Error for SearchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SearchError::IoError(e) => Some(e),
            _ => None,
        }
    }
}
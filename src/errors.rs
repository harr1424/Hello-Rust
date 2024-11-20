/// Error type representing various failures that can occur during search operations.
///
/// This enum encapsulates different types of errors that might occur during file analysis or
/// directory traversal, including I/O errors, thread communication errors,
/// and filepath-related errors.
#[derive(Debug)]
pub enum SearchError {
    /// Represents underlying I/O errors from the standard library.
    ///
    /// This variant wraps [`std::io::Error`] and is commonly used for file system
    /// operations that fail, such as reading directories or files.
    IoError(std::io::Error),

    /// Represents errors that occur when sending data between threads.
    ///
    /// Contains a string description of what went wrong during the send operation.
    SendError(String),

    /// Represents errors related to thread operation failures.
    ///
    /// Contains a string description of what went wrong with thread handling,
    /// such as join handle errors or thread panic information.
    ThreadError(String),

    /// Represents errors related to invalid or problematic file paths.
    ///
    /// Contains a string description of what went wrong with the path,
    /// such as invalid characters or path syntax errors.
    PathError(String),
}

impl From<std::io::Error> for SearchError {
    /// Converts a [`std::io::Error`] into a [`SearchError`].
    ///
    /// This implementation allows for easy conversion of standard I/O errors
    /// into our custom error type using the `?` operator.
    ///
    /// # Examples
    /// ```
    /// use std::fs::File;
    /// use ferris_files::errors::SearchError;
    ///
    /// fn read_file() -> Result<(), SearchError> {
    ///     let _file = File::open("nonexistent.txt")?; // Will convert io::Error to SearchError
    ///     Ok(())
    /// }
    /// assert!(matches!(read_file(), Err(SearchError)));
    /// ```
    fn from(err: std::io::Error) -> Self {
        SearchError::IoError(err)
    }
}

impl std::fmt::Display for SearchError {
    /// Formats the error for display purposes.
    ///
    /// Provides a human-readable error message that includes both the error type
    /// and its associated details.
    ///
    /// # Examples
    /// ```
    /// use ferris_files::errors::SearchError;
    /// let err = SearchError::PathError("Invalid path character".to_string());
    /// assert_eq!(format!("{}", err), "Path error: Invalid path character");
    /// ```
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
    /// Returns the lower-level source of this error, if any.
    ///
    /// Currently only returns a source for [`SearchError::IoError`], as it's the only
    /// variant that wraps another error type implementing [`std::error::Error`].
    ///
    /// # Returns
    /// - `Some(&std::io::Error)` for `IoError` variant
    /// - `None` for all other variants
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SearchError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

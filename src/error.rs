use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrunerError {
    MathError(String),
    MalformedTopology(String),
}

impl fmt::Display for PrunerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrunerError::MathError(msg) => write!(f, "Mathematical solver failure: {}", msg),
            PrunerError::MalformedTopology(msg) => {
                write!(f, "Invalid topology constraints: {}", msg)
            }
        }
    }
}

impl Error for PrunerError {}

pub type Result<T> = std::result::Result<T, PrunerError>;

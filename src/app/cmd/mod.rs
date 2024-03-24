mod transform;

use clap::Subcommand;

use super::{input, output};

#[derive(Debug)]
pub enum Error {
    Csv(csv::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
}

impl From<csv::Error> for Error {
    fn from(e: csv::Error) -> Self {
        Self::Csv(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Csv(e) => write!(f, "CSV error: {}", e),
            Self::Json(e) => write!(f, "JSON error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    Transform(transform::Transform),
}

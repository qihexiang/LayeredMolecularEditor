use lme::workspace::LayerStorageError;
use std::io;

use crate::runner::substituent::SubstituentError;

#[derive(Debug)]
pub enum WorkflowError {
    WindowsNameConflict(String),
    WindowNotFound(String),
    SubstituentError(SubstituentError),
    SerdeError(serde_json::Error),
    IoError(io::Error),
    StackIdOutOfRange(usize),
    LayerError(LayerStorageError),
    FilePatternError(glob::PatternError),
    GlobError(glob::GlobError),
}

impl From<SubstituentError> for WorkflowError {
    fn from(value: SubstituentError) -> Self {
        Self::SubstituentError(value)
    }
}

impl From<glob::GlobError> for WorkflowError {
    fn from(value: glob::GlobError) -> Self {
        Self::GlobError(value)
    }
}

impl From<glob::PatternError> for WorkflowError {
    fn from(value: glob::PatternError) -> Self {
        Self::FilePatternError(value)
    }
}

impl From<io::Error> for WorkflowError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<serde_json::Error> for WorkflowError {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeError(value)
    }
}

impl From<LayerStorageError> for WorkflowError {
    fn from(value: LayerStorageError) -> Self {
        Self::LayerError(value)
    }
}
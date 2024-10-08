use lme::workspace::LayerStorageError;
use std::{io, path::PathBuf, process::ExitStatus};

use crate::runner::substituent::SubstituentError;

#[derive(Debug)]
#[allow(dead_code, reason = "only use for error output")]
pub enum WorkflowError {
    WindowNotFound(String),
    SubstituentError(SubstituentError),
    SerdeJSONError(serde_json::Error),
    SerdeYAMLError(serde_yaml::Error),
    TempDirCreateError(io::Error),
    FileWriteError((PathBuf, io::Error)),
    FileReadError((PathBuf, io::Error)),
    CommandOutputLengthNotMatchInputLength((usize, usize)),
    CommandExecutionFail((String, Vec<String>, io::Error)),
    CommandExitStatus(ExitStatus),
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

impl From<serde_json::Error> for WorkflowError {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeJSONError(value)
    }
}

impl From<serde_yaml::Error> for WorkflowError {
    fn from(value: serde_yaml::Error) -> Self {
        Self::SerdeYAMLError(value)
    }
}

impl From<LayerStorageError> for WorkflowError {
    fn from(value: LayerStorageError) -> Self {
        Self::LayerError(value)
    }
}

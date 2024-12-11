use std::path::PathBuf;

use lme::sparse_molecule::SparseMolecule;
use serde::{Deserialize, Serialize};

use crate::{step::Steps, workflow_data::WorkflowData};

#[derive(Deserialize, Default, Debug)]
pub struct WorkflowInput {
    #[serde(default)]
    pub binaries: Vec<PathBuf>,
    #[serde(default)]
    pub no_checkpoint: bool,
    #[serde(default)]
    pub base: SparseMolecule,
    pub steps: Steps,
}

#[derive(Deserialize, Serialize)]
pub struct WorkflowCheckPoint {
    pub skip: usize,
    pub workflow_data: WorkflowData,
}

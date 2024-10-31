use lme::sparse_molecule::SparseMolecule;
use serde::{Deserialize, Serialize};

use crate::{step::Step, workflow_data::WorkflowData};

#[derive(Deserialize, Default, Debug)]
pub struct WorkflowInput {
    #[serde(default)]
    pub no_checkpoint: bool,
    #[serde(default)]
    pub base: SparseMolecule,
    pub steps: Vec<Step>,
}

#[derive(Deserialize, Serialize)]
pub struct WorkflowCheckPoint {
    pub skip: usize,
    pub workflow_data: WorkflowData,
}

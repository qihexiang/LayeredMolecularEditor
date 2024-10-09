use lme::molecule_layer::MoleculeLayer;
use serde::{Deserialize, Serialize};

use crate::{step::Step, workflow_data::WorkflowData};

#[derive(Deserialize, Default)]
pub struct WorkflowInput {
    #[serde(default)]
    pub no_checkpoint: bool,
    #[serde(default)]
    pub base: MoleculeLayer,
    pub steps: Vec<Step>,
}

#[derive(Deserialize, Serialize)]
pub struct WorkflowCheckPoint {
    pub skip: usize,
    pub workflow_data: WorkflowData,
}

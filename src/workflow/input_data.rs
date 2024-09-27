use lme::{chemistry::MoleculeLayer, workspace::Workspace};
use serde::{Deserialize, Serialize};

use crate::{step::Step, workflow_data::WorkflowData};

#[derive(Deserialize)]
pub struct WorkflowInput {
    pub base: MoleculeLayer,
    pub steps: Vec<Step>,
}

#[derive(Deserialize, Serialize)]
pub struct WorkflowCheckPoint {
    pub skip: usize,
    pub workflow_data: WorkflowData,
}
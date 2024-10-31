use std::{cell::RefCell, collections::BTreeMap, ops::Range};

use anyhow::{Context, Result};
use lme::sparse_molecule::SparseMolecule;
use serde::{Deserialize, Serialize};

use crate::workspace::LayerStorage;

#[derive(Deserialize, Serialize, Clone)]
pub struct WorkflowData {
    pub base: SparseMolecule,
    pub layers: RefCell<LayerStorage>,
    pub stacks: Vec<Vec<usize>>,
    pub windows: BTreeMap<String, Range<usize>>,
    pub current_window: Range<usize>,
}

impl Default for WorkflowData {
    fn default() -> Self {
        let base = Default::default();
        let layers = Default::default();
        let stacks = vec![vec![]];
        let current_window = 0..1;
        let windows = BTreeMap::from([("base".to_string(), 0..1)]);
        Self {
            base,
            layers,
            stacks,
            windows,
            current_window,
        }
    }
}

impl WorkflowData {
    pub fn new(base: SparseMolecule) -> Self {
        let mut workflow_data = Self::default();
        workflow_data.base = base;
        workflow_data
    }

    pub fn current_window_stacks(&self) -> Result<Vec<&Vec<usize>>> {
        self.current_window
            .clone()
            .map(|index| {
                self.stacks
                    .get(index)
                    .with_context(|| format!("Failed to load stack with index: {}", index))
            })
            .collect()
    }
}

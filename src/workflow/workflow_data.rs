use std::{
    cell::RefCell,
    collections::BTreeMap,
    ops::Range,
    sync::{Arc, RwLock},
};

use lme::{
    molecule_layer::MoleculeLayer,
    workspace::{LayerStorage, StackCache},
};
use serde::{Deserialize, Serialize};

use crate::error::WorkflowError;

#[derive(Deserialize, Serialize, Clone)]
pub struct WorkflowData {
    pub base: MoleculeLayer,
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
    pub fn new(base: MoleculeLayer) -> Self {
        let mut workflow_data = Self::default();
        workflow_data.base = base;
        workflow_data
    }

    pub fn current_window_stacks(&self) -> Result<Vec<&Vec<usize>>, WorkflowError> {
        self.current_window
            .clone()
            .map(|index| {
                self.stacks
                    .get(index)
                    .ok_or(WorkflowError::StackIdOutOfRange(index))
            })
            .collect()
    }

    pub fn read_stack(
        &self,
        path: &[usize],
        cache: Arc<RwLock<StackCache>>,
    ) -> Result<MoleculeLayer, WorkflowError> {
        let cached = cache
            .read()
            .expect("cache error, please check error log and retry.")
            .read_cache(path)
            .cloned();
        if let Some(cached) = cached {
            Ok(cached)
        } else {
            let data = self.layers.borrow().read_stack(path, self.base.clone())?;
            let mut writable_cache = cache
                .write()
                .expect("cache error, please check error log and retry.");
            writable_cache.write_cache(path, data);
            Ok(writable_cache
                .read_cache(path)
                .cloned()
                .expect("should be able to get here."))
        }
    }
}

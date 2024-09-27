use std::{
    collections::BTreeMap,
    ops::Range,
    sync::{Arc, RwLock},
};

use lme::{
    chemistry::MoleculeLayer,
    workspace::{StackCache, Workspace},
};
use serde::{Deserialize, Serialize};

use crate::error::WorkflowError;

#[derive(Deserialize, Serialize, Clone)]
pub struct WorkflowData {
    base: MoleculeLayer,
    pub workspace: Workspace,
    pub windows: BTreeMap<String, Range<usize>>,
    pub current_window: Range<usize>,
}

impl Default for WorkflowData {
    fn default() -> Self {
        let base = MoleculeLayer::default();
        let mut workspace = Workspace::default();
        workspace.stacks.push(vec![]);
        let windows = BTreeMap::default();
        let current_window = 0..1;
        Self {base, workspace, windows, current_window}
    }
}

impl WorkflowData {
    pub fn new(base: MoleculeLayer) -> Self {
        let mut workflow_data = Self::default();
        workflow_data.base = base;
        workflow_data
    }

    pub fn read_stack(
        &self,
        path: &[usize],
        cache: Arc<RwLock<StackCache>>,
    ) -> Result<MoleculeLayer, WorkflowError> {
        if let Some(cached) = cache
            .read()
            .expect("cache error, please check error log and retry.")
            .read_cache(path)
            .cloned()
        {
            Ok(cached)
        } else {
            let data = self.workspace.layers.read_stack(path, self.base.clone())?;
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

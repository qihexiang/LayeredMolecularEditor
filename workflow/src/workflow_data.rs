use std::{cell::RefCell, collections::BTreeMap, ops::Range};

use anyhow::Result;
use lme::{
    layer::{Layer, SelectOne},
    sparse_molecule::SparseMolecule,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct WorkflowData {
    pub base: SparseMolecule,
    pub layers: RefCell<LayerStorage>,
    pub windows: BTreeMap<String, BTreeMap<String, Vec<usize>>>,
    pub current_window: BTreeMap<String, Vec<usize>>,
}

impl Default for WorkflowData {
    fn default() -> Self {
        let base = Default::default();
        let layers = Default::default();
        let current_window = BTreeMap::from([("".to_string(), vec![])]);
        let windows = BTreeMap::from([("base".to_string(), current_window.clone())]);
        Self {
            base,
            layers,
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
}

#[derive(Default, Deserialize, Serialize, Clone)]
pub struct LayerStorage {
    base: SparseMolecule,
    layers: BTreeMap<usize, Layer>,
}

#[derive(Serialize, Debug, Clone)]
pub enum LayerStorageError {
    NoSuchLayer(usize),
    FilterError(SelectOne),
}

impl std::fmt::Display for LayerStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self)
    }
}

impl std::error::Error for LayerStorageError {}

impl LayerStorage {
    fn next_layer_id(&self) -> usize {
        self.layers.keys().max().copied().unwrap_or_default() + 1
    }

    pub fn layer_ids(&self) -> impl Iterator<Item = &usize> {
        self.layers.keys()
    }

    pub fn create_layers<I>(&mut self, layers: I) -> Range<usize>
    where
        I: IntoIterator<Item = Layer>,
    {
        let start_id = self.next_layer_id();
        for (idx, layer) in layers.into_iter().enumerate() {
            self.layers.insert(start_id + idx, layer);
        }
        start_id..self.next_layer_id()
    }

    pub fn read_layer(&self, layer_id: &usize) -> Option<&Layer> {
        self.layers.get(layer_id)
    }

    pub fn write_layer(&mut self, layer_id: &usize) -> Option<&mut Layer> {
        self.layers.get_mut(layer_id)
    }

    pub fn remove_layer(&mut self, layer_id: &usize) -> Option<Layer> {
        self.layers.remove(layer_id)
    }

    pub fn read_stack(
        &self,
        stack_path: &[usize],
        mut base: SparseMolecule,
    ) -> Result<SparseMolecule, LayerStorageError> {
        for layer_id in stack_path {
            base = self
                .layers
                .get(layer_id)
                .ok_or(LayerStorageError::NoSuchLayer(*layer_id))
                .and_then(|layer| {
                    layer
                        .filter(base)
                        .map_err(|select| LayerStorageError::FilterError(select))
                })?;
        }
        Ok(base)
    }
}

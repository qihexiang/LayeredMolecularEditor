use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

use crate::{chemistry::MoleculeLayer, layer::{Layer, SelectOne}};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StackStorage {
    cache: Option<MoleculeLayer>,
    children: Box<BTreeMap<String, StackStorage>>,
}

impl StackStorage {
    pub fn read_stack(&self, path: Vec<String>) -> Option<&MoleculeLayer> {
        if let Some((head, nexts)) = path.split_first() {
            let next_stack = self.children.get(head)?;
            next_stack.read_stack(nexts.to_vec())
        } else {
            self.cache.as_ref()
        }
    }

    pub fn write_stack(&mut self, path: Vec<String>, stack_data: MoleculeLayer) {
        if let Some((head, nexts)) = path.split_first() {
            if let Some(next_stack) = self.children.get_mut(head) {
                next_stack.write_stack(nexts.to_vec(), stack_data);
            } else {
                let mut next_stack = StackStorage::default();
                next_stack.write_stack(nexts.to_vec(), stack_data);
                self.children.insert(head.to_string(), next_stack);
            }
        } else {
            self.cache = Some(stack_data);
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workspace {
    layers: BTreeMap<String, Layer>,
    stacks: StackStorage,
}

pub enum WorkspaceError {
    NoSuchLayer(String),
    SelectError(SelectOne),
}

impl Workspace {
    pub fn export_stack(&self, stack: Vec<&String>) -> Result<MoleculeLayer, WorkspaceError> {
        let mut molecule_layer = MoleculeLayer::default();
        for layer in stack {
            let layer = self
                .layers
                .get(layer)
                .ok_or(WorkspaceError::NoSuchLayer(layer.to_string()))?;
            molecule_layer = layer
                .filter(molecule_layer)
                .map_err(|select| WorkspaceError::SelectError(select))?;
        }
        Ok(molecule_layer)
    }
}

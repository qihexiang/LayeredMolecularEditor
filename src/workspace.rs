use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ops::Range};

use crate::{
    layer::{Layer, SelectOne},
    molecule_layer::MoleculeLayer,
};

#[derive(Debug, Clone, Default)]
pub struct StackCache {
    cache: Option<MoleculeLayer>,
    children: BTreeMap<usize, StackCache>,
}

impl StackCache {
    pub fn read_cache(&self, path: &[usize]) -> Option<&MoleculeLayer> {
        if let Some((head, nexts)) = path.split_first() {
            let next_stack = self.children.get(head)?;
            next_stack.read_cache(nexts)
        } else {
            self.cache.as_ref()
        }
    }

    pub fn write_cache(&mut self, path: &[usize], stack_data: MoleculeLayer) {
        if let Some((head, nexts)) = path.split_first() {
            if let Some(next_stack) = self.children.get_mut(head) {
                next_stack.write_cache(nexts, stack_data);
            } else {
                let mut next_stack = StackCache::default();
                next_stack.write_cache(nexts, stack_data);
                self.children.insert(*head, next_stack);
            }
        } else {
            self.cache = Some(stack_data);
        }
    }
}

#[derive(Default, Deserialize, Serialize, Clone)]
pub struct LayerStorage {
    base: MoleculeLayer,
    layers: BTreeMap<usize, Layer>,
}

#[derive(Serialize, Debug)]
pub enum LayerStorageError {
    NoSuchLayer(usize),
    FilterError(SelectOne),
}

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
        mut base: MoleculeLayer,
    ) -> Result<MoleculeLayer, LayerStorageError> {
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

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub layers: LayerStorage,
    pub stacks: Vec<Vec<usize>>,
}

impl Workspace {
    pub fn add_layers_on_stack<I>(&mut self, mut base: Vec<usize>, layers: I) -> usize
    where
        I: Iterator<Item = Layer>,
    {
        let layer_ids = self.layers.create_layers(layers);
        base.extend(layer_ids);
        self.stacks.push(base);
        self.stacks.len()
    }
}

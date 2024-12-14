use lmers::{
    layer::{Layer, SelectOne},
    sparse_molecule::SparseMolecule,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use redb::{Database, ReadableTableMetadata, TableDefinition};
use std::{collections::BTreeMap, ops::Range, path::PathBuf};

const LAYER_TABLE: TableDefinition<u64, Layer> = TableDefinition::new("layer_table");

use serde::{Deserialize, Serialize};

pub type Window = BTreeMap<String, Vec<u64>>;

#[derive(Deserialize, Serialize)]
pub struct WorkflowData {
    pub base: SparseMolecule,
    pub layers: LayerStorage,
    pub windows: BTreeMap<String, Window>,
    pub current_window: Window,
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

#[derive(Deserialize, Serialize)]
#[serde(try_from = "LayerStorageConfig")]
pub struct LayerStorage {
    base: SparseMolecule,
    db_path: PathBuf,
    #[serde(skip)]
    db: Database,
}

impl Default for LayerStorage {
    fn default() -> Self {
        Self {
            base: Default::default(),
            db_path: PathBuf::from(".layer_storage.db"),
            db: Database::create(".layer_storage.db")
                .or(Database::open(".layer_storage.db"))
                .unwrap(),
        }
    }
}

impl LayerStorage {
    pub fn get_config(&self) -> LayerStorageConfig {
        LayerStorageConfig {
            base: self.base.clone(),
            db_path: self.db_path.clone(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct LayerStorageConfig {
    base: SparseMolecule,
    db_path: PathBuf,
}

impl TryFrom<LayerStorageConfig> for LayerStorage {
    type Error = anyhow::Error;
    fn try_from(value: LayerStorageConfig) -> Result<Self, Self::Error> {
        let db = Database::create(&value.db_path).or(Database::open(&value.db_path))?;
        Ok(Self {
            base: value.base,
            db_path: value.db_path,
            db,
        })
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum LayerStorageError {
    NoSuchLayer(u64),
    FilterError(SelectOne),
}

impl std::fmt::Display for LayerStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self)
    }
}

impl std::error::Error for LayerStorageError {}

impl LayerStorage {
    fn next_layer_id(&self) -> u64 {
        let read_txn = self.db.begin_read().unwrap();
        if let Ok(table) = read_txn.open_table(LAYER_TABLE) {
            table.len().unwrap()
        } else {
            0
        }
    }

    pub fn create_layers(&self, layers: &[Layer]) -> Range<u64> {
        let start_id = self.next_layer_id();
        let write_txn = self.db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(LAYER_TABLE).unwrap();
            for (idx, layer) in layers.into_iter().enumerate() {
                table.insert(start_id + idx as u64, layer.clone()).unwrap();
            }
        }
        write_txn.commit().unwrap();
        start_id..self.next_layer_id()
    }

    pub fn read_layer(&self, layer_id: u64) -> Option<Layer> {
        self.db
            .begin_read()
            .unwrap()
            .open_table(LAYER_TABLE)
            .unwrap()
            .get(layer_id)
            .unwrap()
            .map(|acc| acc.value())
    }
}

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use lme::layer::{Layer, SelectOne};
use lme::molecule_layer::MoleculeLayer;
use lme::workspace::{LayerStorage, LayerStorageError};
use serde::Deserialize;
use substituent::{Substituent, SubstituentError};
use tempfile::tempdir;

use crate::error::WorkflowError;

use glob::glob;
use rayon::prelude::*;

pub mod substituent;

#[derive(Deserialize)]
pub enum Runner {
    AddLayers(Vec<Layer>),
    Substituent {
        entry: SelectOne,
        target: SelectOne,
        file_pattern: String,
    },
    Function {
        command: String,
        arguments: Vec<String>,
    },
}

#[derive(Deserialize)]
pub enum RunnerOutput {
    Serial(Vec<Vec<usize>>),
    Named(BTreeMap<String, Vec<Vec<usize>>>),
    None,
}

impl Runner {
    pub fn execute<'a>(
        self,
        base: &MoleculeLayer,
        current_window: Vec<&Vec<usize>>,
        layer_storage: &mut LayerStorage,
    ) -> Result<RunnerOutput, WorkflowError> {
        match self {
            Self::AddLayers(layers) => {
                let layer_ids = layer_storage.create_layers(layers);
                Ok(RunnerOutput::Serial(
                    current_window
                        .into_iter()
                        .map(|current| {
                            let mut current = current.clone();
                            current.extend(layer_ids.clone());
                            current
                        })
                        .collect(),
                ))
            }
            Self::Function { command, arguments } => {
                let input = current_window
                    .into_par_iter()
                    .map(|stack_path| layer_storage.read_stack(stack_path, base.clone()))
                    .collect::<Result<Vec<_>, _>>()?;
                let input =
                    serde_json::to_string(&input).map_err(|err| WorkflowError::SerdeError(err))?;
                let temp_directory =
                    tempdir().map_err(|err| WorkflowError::TempDirCreateError(err))?;
                let filepath = temp_directory.path().join("stacks.json");
                let mut file = File::create(&filepath)
                    .map_err(|err| WorkflowError::FileWriteError((filepath.clone(), err)))?;
                file.write_all(input.as_bytes())
                    .map_err(|err| WorkflowError::FileWriteError((filepath, err)))?;
                let exit_status = Command::new(&command)
                    .args(&arguments)
                    .current_dir(&temp_directory)
                    .status()
                    .map_err(|err| {
                        WorkflowError::CommandExecutionFail((
                            command.to_string(),
                            arguments.clone(),
                            err,
                        ))
                    })?;
                if !exit_status.success() {
                    Err(WorkflowError::CommandExitStatus(exit_status))?;
                }
                let filepath = temp_directory.path().join("output.json");
                let file = File::open(&filepath)
                    .map_err(|err| WorkflowError::FileReadError((filepath, err)))?;
                let output: RunnerOutput = serde_json::from_reader(file)?;
                Ok(output)
            }
            Self::Substituent {
                entry,
                target,
                file_pattern,
            } => {
                let matched_files = glob(&file_pattern)?.collect::<Result<Vec<_>, _>>()?;
                let matched_files = matched_files
                    .into_par_iter()
                    .map(|path| {
                        Ok((
                            path.clone(),
                            File::open(&path)
                                .map_err(|err| WorkflowError::FileReadError((path, err)))?,
                        ))
                    })
                    .collect::<Result<Vec<_>, WorkflowError>>()?;
                let substituents = matched_files
                    .into_par_iter()
                    .map(|(path, file)| Ok((path, serde_json::from_reader(file)?)))
                    .collect::<Result<Vec<(PathBuf, Substituent)>, WorkflowError>>()?;
                let current_structures = current_window
                    .iter()
                    .map(|stack_path| layer_storage.read_stack(stack_path, base.clone()))
                    .collect::<Result<Vec<_>, LayerStorageError>>()?;
                let mut result = BTreeMap::new();
                for (path, substituent) in substituents {
                    let path = path.to_string_lossy().to_string();
                    let new_layers = current_structures
                        .par_iter()
                        .map(|base| substituent.generate_layer(base, entry.clone(), target.clone()))
                        .collect::<Result<Vec<_>, SubstituentError>>()?;
                    let layer_ids = layer_storage
                        .create_layers(new_layers.into_iter().map(|ml| Layer::Fill(ml)));
                    let new_stacks = layer_ids
                        .enumerate()
                        .map(|(index, layer_id)| {
                            let mut stack_path = current_window[index].clone();
                            stack_path.push(layer_id);
                            stack_path
                        })
                        .collect::<Vec<_>>();
                    result.insert(path, new_stacks);
                }
                Ok(RunnerOutput::Named(result))
            }
        }
    }
}

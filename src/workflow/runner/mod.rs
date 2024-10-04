use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::ops::Range;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, RwLock};

use lme::molecule_layer::MoleculeLayer;
use lme::{
    layer::{Layer, SelectOne},
    workspace::StackCache,
};
use serde::Deserialize;
use substituent::Substituent;
use tempfile::tempdir;

use crate::{error::WorkflowError, workflow_data::WorkflowData};

use glob::glob;
use rayon::prelude::*;

pub mod substituent;

#[derive(Deserialize)]
pub enum Runner {
    AddLayer(Layer),
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

impl Runner {
    pub fn execute(
        &self,
        workflow_data: &mut WorkflowData,
        cache: Arc<RwLock<StackCache>>,
    ) -> Result<Option<BTreeMap<String, Range<usize>>>, WorkflowError> {
        match self {
            Self::AddLayer(layer) => {
                let layers = workflow_data
                    .workspace
                    .layers
                    .create_layers([layer].into_iter().cloned());
                let new_stack = workflow_data
                    .current_window
                    .clone()
                    .par_bridge()
                    .map(|stack_id| {
                        let mut stack = workflow_data
                            .workspace
                            .stacks
                            .get(stack_id)
                            .cloned()
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?;
                        stack.extend(layers.clone());
                        Ok(stack)
                    })
                    .collect::<Result<Vec<_>, WorkflowError>>()?;
                workflow_data.workspace.stacks.extend(new_stack);
                Ok(None)
            }
            Self::Function { command, arguments } => {
                let stacks = workflow_data
                    .current_window
                    .clone()
                    .par_bridge()
                    .map(|stack_id| {
                        workflow_data
                            .workspace
                            .stacks
                            .get(stack_id)
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))
                    })
                    .collect::<Result<Vec<&Vec<usize>>, _>>()?;
                let input = stacks
                    .into_par_iter()
                    .map(|stack_path| workflow_data.read_stack(stack_path, cache.clone()))
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
                let exit_status = Command::new(command)
                    .args(arguments)
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
                // here, output should have same length with input, as the current window.
                let output: Vec<Vec<MoleculeLayer>> = serde_json::from_reader(file)?;
                if output.len() != workflow_data.current_window.len() {
                    Err(WorkflowError::CommandOutputLengthNotMatchInputLength((
                        output.len(),
                        workflow_data.current_window.len(),
                    )))?;
                }
                let current_and_layers = output
                    .into_par_iter()
                    .enumerate()
                    .map(|(stack_id, generated_layers)| {
                        let stack_id = stack_id + workflow_data.current_window.start;
                        let stack_path = workflow_data
                            .workspace
                            .stacks
                            .get(stack_id)
                            .cloned()
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?;
                        Ok((stack_path, generated_layers))
                    })
                    .collect::<Result<Vec<_>, WorkflowError>>()?;
                for (stack_path, generated_layers) in current_and_layers {
                    for layer_id in workflow_data
                        .workspace
                        .layers
                        .create_layers(generated_layers.into_iter().map(|ml| Layer::Fill(ml)))
                    {
                        let mut stack_path = stack_path.clone();
                        stack_path.push(layer_id);
                        workflow_data.workspace.stacks.push(stack_path);
                    }
                }
                Ok(None)
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
                let stack_and_layers = substituents
                    .into_par_iter()
                    .map(|(path, subsitituent)| {
                        Ok((
                            path,
                            workflow_data
                                .current_window
                                .clone()
                                .map(|stack_id| {
                                    let stack_path = workflow_data
                                        .workspace
                                        .stacks
                                        .get(stack_id)
                                        .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?
                                        .clone();
                                    let stack_data =
                                        workflow_data.read_stack(&stack_path, cache.clone())?;
                                    let generated_layer = subsitituent.generate_layer(
                                        stack_data.clone(),
                                        entry.clone(),
                                        target.clone(),
                                    )?;
                                    Ok((stack_path, generated_layer))
                                })
                                .collect::<Result<Vec<_>, WorkflowError>>()?,
                        ))
                    })
                    .collect::<Result<Vec<_>, WorkflowError>>()?;
                let mut additional_windows = BTreeMap::new();
                for (path, generated_layers) in stack_and_layers {
                    let path = path.to_string_lossy().to_string();
                    let start = workflow_data.workspace.stacks.len();
                    for (base, layer) in generated_layers {
                        workflow_data
                            .workspace
                            .add_layers_on_stack(base.clone(), [Layer::Fill(layer)].into_iter());
                    }
                    let stop = workflow_data.workspace.stacks.len();
                    additional_windows.insert(path, start..stop);
                }
                Ok(Some(additional_windows))
            }
        }
    }
}

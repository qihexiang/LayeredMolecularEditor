use std::fs::File;
use std::io::Write;
use std::ops::Range;
use std::process::Command;
use std::sync::{Arc, RwLock};

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
    AddLayers {
        layers: Vec<Layer>,
        in_place: bool,
    },
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
    ) -> Result<Range<usize>, WorkflowError> {
        match self {
            Self::AddLayers { layers, in_place } => {
                let layers = workflow_data
                    .workspace
                    .layers
                    .create_layers(layers.iter().cloned());
                Ok(if *in_place {
                    for stack_id in workflow_data.current_window.clone() {
                        workflow_data
                            .workspace
                            .stacks
                            .get_mut(stack_id)
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?
                            .extend(layers.clone());
                    }
                    workflow_data.current_window.clone()
                } else {
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
                    workflow_data.current_window.end
                        ..workflow_data.current_window.end
                            + (workflow_data.current_window.end
                                - workflow_data.current_window.start)
                })
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
                let stacks = stacks
                    .into_par_iter()
                    .map(|stack_path| workflow_data.read_stack(stack_path, cache.clone()))
                    .collect::<Result<Vec<_>, _>>()?;
                let data =
                    serde_json::to_string(&stacks).map_err(|err| WorkflowError::SerdeError(err))?;
                let temp_directory =
                    tempdir().map_err(|err| WorkflowError::TempDirCreateError(err))?;
                let filepath = temp_directory.path().join("stacks.json");
                let mut file = File::create(&filepath)
                    .map_err(|err| WorkflowError::FileWriteError((filepath.clone(), err)))?;
                file.write_all(data.as_bytes())
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
                let output: Vec<Vec<Vec<Layer>>> = serde_json::from_reader(file)?;
                let output_window_length =
                    output.iter().flatten().flatten().collect::<Vec<_>>().len();
                let current_start = workflow_data.current_window.start;
                let current_and_layers = output
                    .into_par_iter()
                    .enumerate()
                    .map(|(stack_id, generated_layers)| {
                        let stack_id = stack_id + current_start;
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
                    for layers_to_stack in generated_layers {
                        let mut stack_path = stack_path.clone();
                        let layer_ids = workflow_data
                            .workspace
                            .layers
                            .create_layers(layers_to_stack.into_iter());
                        stack_path.extend(layer_ids);
                        workflow_data.workspace.stacks.push(stack_path);
                    }
                }
                Ok(workflow_data.current_window.end
                    ..workflow_data.current_window.end + output_window_length)
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
                        File::open(&path).map_err(|err| WorkflowError::FileReadError((path, err)))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let substituents = matched_files
                    .into_par_iter()
                    .map(|file| serde_json::from_reader(file))
                    .collect::<Result<Vec<Substituent>, _>>()?;
                let stack_and_layers = workflow_data
                    .current_window
                    .clone()
                    .par_bridge()
                    .map(|stack_id| {
                        let path = workflow_data
                            .workspace
                            .stacks
                            .get(stack_id)
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?;
                        let stack = workflow_data.read_stack(path, cache.clone())?;
                        let generated_layers = substituents
                            .iter()
                            .map(|sub| {
                                sub.generate_path(stack.clone(), entry.clone(), target.clone())
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok((path.clone(), generated_layers))
                    })
                    .collect::<Result<Vec<_>, WorkflowError>>()?;
                let window_start = workflow_data.workspace.stacks.len();
                for (path, generated_layers) in stack_and_layers {
                    let created_layers = workflow_data.workspace.layers.create_layers(
                        generated_layers
                            .into_iter()
                            .map(|molecule| Layer::Fill(molecule)),
                    );
                    let modified_stacks = created_layers
                        .par_bridge()
                        .map(|layer_id| vec![path.clone(), vec![layer_id]].concat())
                        .collect::<Vec<_>>();
                    workflow_data.workspace.stacks.extend(modified_stacks);
                }
                let window_stop = workflow_data.workspace.stacks.len();
                Ok(window_start..window_stop)
            }
        }
    }
}

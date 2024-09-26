use error::WorkflowError;
use glob::glob;
use lme::{
    chemistry::MoleculeLayer,
    layer::{Layer, SelectOne},
    workspace::{StackCache, Workspace},
};
use rayon::prelude::*;
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    ops::Range,
    process::Command,
    sync::{Arc, RwLock},
};
use substituent::Substituent;
use tempfile::tempdir;

mod error;
mod substituent;

pub struct WorkflowData {
    base: MoleculeLayer,
    workspace: Workspace,
    windows: BTreeMap<String, Range<usize>>,
    current_window: Range<usize>,
}

pub struct Workflow {
    base: MoleculeLayer,
    steps: Vec<Step>,
}

pub struct Step {
    from: Option<String>,
    name: Option<String>,
    operation: Operation,
}

pub enum Operation {
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

impl WorkflowData {
    fn read_stack(
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

impl Operation {
    fn execute(
        &self,
        workflow: &mut WorkflowData,
        cache: Arc<RwLock<StackCache>>,
    ) -> Result<Range<usize>, WorkflowError> {
        match self {
            Self::AddLayers { layers, in_place } => {
                let layers = workflow
                    .workspace
                    .layers
                    .create_layers(layers.iter().cloned());
                Ok(if *in_place {
                    for stack_id in workflow.current_window.clone() {
                        workflow
                            .workspace
                            .stacks
                            .get_mut(stack_id)
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?
                            .extend(layers.clone());
                    }
                    workflow.current_window.clone()
                } else {
                    let new_stack = workflow
                        .current_window
                        .clone()
                        .par_bridge()
                        .map(|stack_id| {
                            let mut stack = workflow
                                .workspace
                                .stacks
                                .get(stack_id)
                                .cloned()
                                .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?;
                            stack.extend(layers.clone());
                            Ok(stack)
                        })
                        .collect::<Result<Vec<_>, WorkflowError>>()?;
                    workflow.workspace.stacks.extend(new_stack);
                    workflow.current_window.end
                        ..workflow.current_window.end
                            + (workflow.current_window.end - workflow.current_window.start)
                })
            }
            Self::Function { command, arguments } => {
                let stacks = workflow
                    .current_window
                    .clone()
                    .par_bridge()
                    .map(|stack_id| {
                        workflow
                            .workspace
                            .stacks
                            .get(stack_id)
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))
                    })
                    .collect::<Result<Vec<&Vec<usize>>, _>>()?;
                let stacks = stacks
                    .into_par_iter()
                    .map(|stack_path| workflow.read_stack(stack_path, cache.clone()))
                    .collect::<Result<Vec<_>, _>>()?;
                let data =
                    serde_json::to_string(&stacks).map_err(|err| WorkflowError::SerdeError(err))?;
                let temp_directory = tempdir()?;
                let filepath = temp_directory.path().join("stacks.json");
                let mut file = File::create(filepath)?;
                file.write_all(data.as_bytes())?;
                Command::new(command)
                    .args(arguments)
                    .current_dir(&temp_directory)
                    .spawn()?;
                let filepath = temp_directory.path().join("output.json");
                let file = File::create(filepath)?;
                let output: Vec<Vec<Vec<Layer>>> = serde_json::from_reader(file)?;
                let output_window_length =
                    output.iter().flatten().flatten().collect::<Vec<_>>().len();
                let current_start = workflow.current_window.start;
                let current_and_layers = output
                    .into_par_iter()
                    .enumerate()
                    .map(|(stack_id, generated_layers)| {
                        let stack_id = stack_id + current_start;
                        let stack_path = workflow
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
                        let layer_ids = workflow
                            .workspace
                            .layers
                            .create_layers(layers_to_stack.into_iter());
                        stack_path.extend(layer_ids);
                        workflow.workspace.stacks.push(stack_path);
                    }
                }
                Ok(workflow.current_window.end..workflow.current_window.end + output_window_length)
            }
            Self::Substituent {
                entry,
                target,
                file_pattern,
            } => {
                let matched_files = glob(&file_pattern)?.collect::<Result<Vec<_>, _>>()?;
                let matched_files = matched_files
                    .into_par_iter()
                    .map(|path| File::open(path))
                    .collect::<Result<Vec<_>, _>>()?;
                let substituents = matched_files
                    .into_par_iter()
                    .map(|file| serde_json::from_reader(file))
                    .collect::<Result<Vec<Substituent>, _>>()?;
                let stack_and_layers = workflow
                    .current_window
                    .clone()
                    .par_bridge()
                    .map(|stack_id| {
                        let path = workflow
                            .workspace
                            .stacks
                            .get(stack_id)
                            .ok_or(WorkflowError::StackIdOutOfRange(stack_id))?;
                        let stack = workflow.read_stack(path, cache.clone())?;
                        let generated_layers = substituents
                            .iter()
                            .map(|sub| {
                                sub.generate_path(stack.clone(), entry.clone(), target.clone())
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok((path.clone(), generated_layers))
                    })
                    .collect::<Result<Vec<_>, WorkflowError>>()?;
                let window_start = workflow.workspace.stacks.len();
                for (path, generated_layers) in stack_and_layers {
                    let created_layers = workflow.workspace.layers.create_layers(generated_layers.into_iter().map(|molecule| Layer::Fill(molecule)));
                    let modified_stacks = created_layers.par_bridge().map(|layer_id| vec![path.clone(), vec![layer_id]].concat()).collect::<Vec<_>>();
                    workflow.workspace.stacks.extend(modified_stacks);
                }
                let window_stop = workflow.workspace.stacks.len();
                Ok(window_start..window_stop)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("started.")
}

use anyhow::{anyhow, Context, Result};
use cached::{proc_macro::cached, UnboundCache};
use nalgebra::Vector3;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use std::{collections::BTreeMap, io::Write};

use lme::{
    layer::{Layer, SelectOne},
    sparse_molecule::SparseMolecule,
};
use serde::Deserialize;
use tempfile::tempdir;

use glob::glob;
use rayon::prelude::*;

use crate::io::BasicIOMolecule;
use crate::workflow_data::{LayerStorage, LayerStorageError};

#[derive(Debug, Deserialize)]
pub enum Runner {
    AppendLayers(Vec<Layer>),
    Substituent {
        center: SelectOne,
        replace: SelectOne,
        file_pattern: String,
    },
    Function {
        command: String,
        arguments: Vec<String>,
    },
    Output {
        #[serde(default)]
        prefix: String,
        #[serde(default)]
        suffix: String,
        #[serde(default)]
        target_directory: PathBuf,
        target_format: String,
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
        &self,
        base: &SparseMolecule,
        current_window: Vec<&Vec<usize>>,
        layer_storage: &mut LayerStorage,
    ) -> Result<RunnerOutput> {
        match self {
            Self::AppendLayers(layers) => {
                let layer_ids = layer_storage.create_layers(layers.clone());
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
                    .map(|stack_path| cached_read_stack(base, &layer_storage, &stack_path))
                    .collect::<Result<Vec<_>, _>>()?;
                let input = serde_json::to_string(&input)?;
                let temp_directory =
                    tempdir().with_context(|| "Unable to create temp directory")?;
                let filepath = temp_directory.path().join("stacks.json");
                let mut file = File::create(&filepath).with_context(|| {
                    format!(
                        "Unable to create file {:?} as input for external function.",
                        filepath
                    )
                })?;
                file.write_all(input.as_bytes()).with_context(|| {
                    format!(
                        "Unable to write to file {:?} as input for external function.",
                        filepath
                    )
                })?;
                let exit_status = Command::new(&command)
                    .args(arguments)
                    .current_dir(&temp_directory)
                    .status()
                    .with_context(|| format!("Failed to start external program for {:#?}", self))?;
                if !exit_status.success() {
                    Err(anyhow!(
                        "External process exited with non-zero code {}",
                        exit_status.code().unwrap_or_default()
                    ))?;
                }
                let filepath = temp_directory.path().join("output.json");
                let file = File::open(&filepath).with_context(|| {
                    format!(
                        "Unable to read file {:#?} as output from external program",
                        filepath
                    )
                })?;
                let output: RunnerOutput = serde_json::from_reader(file).with_context(|| {
                    format!("Failed to deserialize output file in {:?}", filepath)
                })?;
                Ok(output)
            }
            Self::Substituent {
                center,
                replace,
                file_pattern,
            } => {
                let matched_files = glob(&file_pattern)?.collect::<Result<Vec<_>, _>>()?;
                let substituents = matched_files
                    .into_par_iter()
                    .map(|path| {
                        let file = File::open(&path).with_context(|| {
                            format!("Unbale to open and deserialize matched file {:#?}", path)
                        })?;
                        serde_yaml::from_reader(file).with_context(|| {
                            format!("Unable to deserialize matched file {:#?}", path)
                        })
                    })
                    .collect::<Result<Vec<SparseMolecule>, _>>()?;
                let current_structures = current_window
                    .into_iter()
                    .map(|stack_path| {
                        Ok((
                            stack_path.clone(),
                            cached_read_stack(base, &layer_storage, &stack_path)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, LayerStorageError>>()?;
                let center_layer = Layer::SetCenter {
                    select: center.clone(),
                    center: Default::default(),
                };
                let align_layer = Layer::DirectionAlgin {
                    select: replace.clone(),
                    direction: Vector3::x(),
                };
                let align_layers = layer_storage.create_layers([center_layer, align_layer]);
                let mut result = BTreeMap::new();
                for substituent in substituents {
                    let replace_atom =
                        SelectOne::Index(1)
                            .get_atom(&substituent)
                            .with_context(|| {
                                format!(
                                "Substitutuent must have at least 2 atoms, substituent title: {}",
                                substituent.title
                            )
                            })?;
                    let mut updated_stacks = Vec::with_capacity(current_structures.len());
                    for (stack_path, current_structure) in &current_structures {
                        let mut substituent = substituent.clone();
                        SelectOne::Index(0).set_atom(&mut substituent, None);
                        SelectOne::Index(1).set_atom(&mut substituent, None);
                        let offset = current_structure.atoms.len();
                        let mut substituent = substituent.offset(offset);
                        substituent.ids = current_structure.ids.clone();
                        replace
                            .set_atom(&mut substituent, Some(replace_atom))
                            .with_context(|| {
                                format!(
                                    "The replace selector {:?} in {:#?} is not validated",
                                    replace, substituent
                                )
                            })?;
                        let replaced_index = replace.to_index(&substituent).unwrap();
                        let updated_bonds = substituent
                            .bonds
                            .get_neighbors(offset + 1)
                            .unwrap()
                            .enumerate()
                            .map(|(index, bond)| (replaced_index, index, bond.clone()))
                            .collect::<Vec<_>>();
                        for (a, b, bond) in updated_bonds {
                            substituent.bonds.set_bond(a, b, bond);
                        }
                        substituent.title =
                            vec![current_structure.title.clone(), substituent.title].join("_");
                        let mut updated_stack_path = stack_path.clone();
                        updated_stack_path.extend(align_layers.clone());
                        updated_stack_path
                            .extend(layer_storage.create_layers([Layer::Fill(substituent)]));
                        updated_stacks.push(updated_stack_path);
                    }
                    result.insert(substituent.title, updated_stacks);
                }
                Ok(RunnerOutput::Named(result))
            }
            Self::Output {
                prefix,
                suffix,
                target_directory,
                target_format,
            } => {
                let outputs = current_window
                    .into_par_iter()
                    .map(|stack_path| {
                        let data = cached_read_stack(base, &layer_storage, &stack_path)?;
                        let bonds = data.bonds.to_continous_list(&data.atoms);
                        Ok(BasicIOMolecule::new(data.title, data.atoms.into(), bonds))
                    })
                    .collect::<Result<Vec<_>, LayerStorageError>>()?;
                for output in outputs {
                    let mut path = target_directory.clone().join(&output.title);
                    let content = match target_format.as_str() {
                        "xyz" => output.output_to_xyz(),
                        "mol2" => output.output_to_mol2(),
                        format => Err(anyhow!("Unsupported output format: {}", format)),
                    }?;
                    let content = [prefix.clone(), content, suffix.clone()]
                        .into_iter()
                        .filter(|part| part != "")
                        .collect::<Vec<_>>()
                        .join("\n");
                    path.set_extension(target_format.as_str());
                    let mut file = File::create_new(&path)
                        .with_context(|| format!("Unable to create output file at {:?}", path))?;
                    file.write_all(content.as_bytes())
                        .with_context(|| format!("Unable to write to output file at {:?}", path))?;
                }
                Ok(RunnerOutput::None)
            }
        }
    }
}

/// In a workflow, the base and existed layers will not be modified or deleted,
/// so the result of read_stack function is in fact only dependent on the path
/// parameter so create a cached function here is reasonable.
///
/// The read_stack function may return an Err(LayerStorageError), which
/// means there might be something wrong in program or input file, and the workflow
/// will exit, so the cache of error result will never be accessed in practice.
#[cached(
    ty = "UnboundCache<String, Result<SparseMolecule, LayerStorageError>>",
    create = "{ UnboundCache::new() }",
    convert = r#"{ stack_path.iter().map(|item| item.to_string()).collect::<Vec<_>>().join("/") }"#
)]
fn cached_read_stack(
    base: &SparseMolecule,
    layer_storage: &LayerStorage,
    stack_path: &[usize],
) -> Result<SparseMolecule, LayerStorageError> {
    if let Some((last, heads)) = stack_path.split_last() {
        let layer = layer_storage
            .read_layer(last)
            .ok_or(LayerStorageError::NoSuchLayer(*last))?;
        let lower_result = cached_read_stack(base, layer_storage, heads)?;
        layer
            .filter(lower_result)
            .map_err(|err| LayerStorageError::FilterError(err))
    } else {
        Ok(base.clone())
    }
}

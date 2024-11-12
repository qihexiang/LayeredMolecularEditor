use anyhow::{anyhow, Context, Result};
use cached::{proc_macro::cached, UnboundCache};
use nalgebra::Vector3;
use sedregex::find_and_replace;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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
    Command {
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
        #[serde(default)]
        openbabel: bool
    },
    Rename {
        #[serde(default)]
        prefix: Option<String>,
        #[serde(default)]
        suffix: Option<String>,
        #[serde(default)]
        replace: Option<(String, String)>,
        #[serde(default)]
        regex: Vec<String>,
    },
    Calculation {
        working_directory: PathBuf,
        pre_format: FormatOptions,
        pre_filename: String,
        #[serde(default)]
        stdin: bool,
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        envs: BTreeMap<String, String>,
        post_format: String,
        #[serde(default)]
        post_filename: String,
        #[serde(default)]
        ignore_failed: bool,
        #[serde(default)]
        stdout: Option<String>,
        #[serde(default)]
        stderr: Option<String>
    },
}

#[derive(Deserialize, Debug)]
pub struct FormatOptions {
    format: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    regex: Vec<String>,
}

#[derive(Deserialize)]
pub enum RunnerOutput {
    SingleWindow(BTreeMap<String, Vec<usize>>),
    MultiWindow(BTreeMap<String, BTreeMap<String, Vec<usize>>>),
    None,
}

impl Runner {
    pub fn execute<'a>(
        &self,
        base: &SparseMolecule,
        current_window: &BTreeMap<String, Vec<usize>>,
        layer_storage: &mut LayerStorage,
    ) -> Result<RunnerOutput> {
        match self {
            Self::AppendLayers(layers) => {
                let layer_ids = layer_storage.create_layers(layers.clone());
                Ok(RunnerOutput::SingleWindow(
                    current_window
                        .into_iter()
                        .map(|(title, current)| {
                            let mut current = current.clone();
                            current.extend(layer_ids.clone());
                            (title.to_string(), current)
                        })
                        .collect(),
                ))
            }
            Self::Command { command, arguments } => {
                let input = current_window
                    .into_par_iter()
                    .map(|(title, stack_path)| {
                        Ok((title, cached_read_stack(base, &layer_storage, &stack_path)?))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()?;
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
            Self::Calculation {
                working_directory,
                pre_format,
                pre_filename,
                stdin,
                program,
                args,
                envs,
                post_format,
                post_filename,
                ignore_failed,
                stdout, stderr,
            } => {
                std::fs::create_dir_all(&working_directory).with_context(|| {
                    format!("Unable to create directory at {:?}", working_directory)
                })?;
                let outputs = current_window.par_iter().map(|(title, stack_path)| {
                    let working_directory = working_directory.join(title);
                    std::fs::create_dir_all(&working_directory).with_context(|| {
                        format!(
                            "Unable to create directory at {:?} for structure titled {}",
                            working_directory, title
                        )
                    })?;
                    let structure = cached_read_stack(base, layer_storage, stack_path)?;
                    let bonds = structure.bonds.clone().to_continuous_list(&structure.atoms);
                    let atoms = structure.atoms.clone().into();
                    let basic_molecule = BasicIOMolecule::new(title.to_string(), atoms, bonds);
                    let pre_content = basic_molecule.output(&pre_format.format)?;
                    let pre_content =
                        find_and_replace(&pre_content, &pre_format.regex)?.to_string();
                    let pre_content = [
                        pre_format.prefix.to_string(),
                        pre_content,
                        pre_format.suffix.to_string(),
                    ]
                    .join("\n");
                    let pre_path = working_directory.join(pre_filename);
                    File::create(&pre_path)
                        .with_context(|| {
                            format!(
                                "Unable to create pre-file for calculation at {:?}",
                                pre_path
                            )
                        })?
                        .write_all(pre_content.as_bytes())
                        .with_context(|| {
                            format!(
                                "Unable to write to pre-file for calculation at {:?}",
                                pre_path
                            )
                        })?;
                    let mut command = Command::new(program);
                    command.current_dir(&working_directory).args(args).envs(envs);
                    if *stdin {
                        let stdin = Stdio::from(File::open(&pre_path).with_context(|| {
                            format!("Unable to open created pre-file at {:?}", pre_content)
                        })?);
                        command.stdin(stdin);
                    }
                    if let Some(stdout) = stdout {
                        let stdout_path = working_directory.join(stdout);
                        let stdout_file = File::create(&stdout_path).with_context(|| format!("Unable to create stdout file at {:?} for structure titled {}", stdout_path, title))?;
                        command.stdout(Stdio::from(stdout_file));
                    } else {
                        command.stdout(Stdio::null());
                    }

                    if let Some(stderr) = stderr {
                        let stderr_path = working_directory.join(stderr);
                        let stderr_file = File::create(&stderr_path).with_context(|| format!("Unable to create stdout file at {:?} for structure titled {}", stderr_path, title))?;
                        command.stderr(Stdio::from(stderr_file));
                    } else {
                        command.stderr(Stdio::null());
                    }

                    let mut child = command.spawn().with_context(|| format!("Failed to start process for structure {}, process detail: {:#?}", title, command))?;
                    let result = child.wait().with_context(|| format!("Unable to wait the process handling structure {}, process detail: {:#?}", title, child))?;
                    
                    if !result.success() {
                        Err(anyhow!("Handling process for structure {} failed. Error code {:?}", title, result.code()))?;
                    }

                    let post_path = working_directory.join(post_filename);
                    let post_file = File::open(&post_path).with_context(|| format!("Failed to open post-calculation file at {:?} for structure {}", post_path, title))?;
                    let post_content = BasicIOMolecule::input(&post_format, post_file)?;
                    let updated_atoms = structure.atoms.update_from_continuous_list(&post_content.atoms).with_context(|| format!("Failed to import atoms from calculated result for structure {}", title))?;
                    let updated_bonds = post_content.bonds.into_iter()
                        .map(|(a, b, bond)| Some((structure.atoms.from_continuous_index(a)?, structure.atoms.from_continuous_index(b)?, bond)))
                        .collect::<Option<Vec<_>>>().with_context(|| format!("Failed to import bonds from calculated results for structure {}", title))?;
                    let mut structure = structure;
                    structure.atoms.migrate(&updated_atoms);
                    for (a, b, bond) in updated_bonds {
                        structure.bonds.set_bond(a, b, Some(bond));
                    }
                    Ok::<_, anyhow::Error>((title, stack_path, structure))
                });
                let results = if *ignore_failed {
                    outputs.filter_map(|item| item.ok()).collect::<Vec<_>>()
                } else {
                    outputs.collect::<Result<Vec<_>>>()?
                };
                let mut window = BTreeMap::new();
                for (title, stack_path, updated) in results {
                    let updated_layer = layer_storage.create_layers([Layer::Fill(updated)]);
                    let mut stack_path = stack_path.clone();
                    stack_path.extend(updated_layer);
                    window.insert(title.to_string(), stack_path);
                }
                Ok(RunnerOutput::SingleWindow(window))
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
                            format!("Unable to open and deserialize matched file {:#?}", path)
                        })?;
                        let substituent_name = path
                            .file_stem()
                            .with_context(|| {
                                format!("Unable to get file name from path {:?}", path)
                            })?
                            .to_string_lossy()
                            .to_string();
                        Ok((
                            substituent_name,
                            serde_yaml::from_reader(file).with_context(|| {
                                format!("Unable to deserialize matched file {:?}", path)
                            })?,
                        ))
                    })
                    .collect::<Result<BTreeMap<String, SparseMolecule>>>()?;
                let current_structures = current_window
                    .into_iter()
                    .map(|(title, stack_path)| {
                        Ok((
                            title.to_string(),
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
                for (substituent_name, substituent) in substituents {
                    let replace_atom =
                        SelectOne::Index(1)
                            .get_atom(&substituent)
                            .with_context(|| {
                                format!(
                                    "Substituent must have at least 2 atoms, substituent title: {}",
                                    substituent_name
                                )
                            })?;
                    let mut updated_stacks = BTreeMap::new();
                    for (current_title, stack_path, current_structure) in &current_structures {
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
                        let title = format!("{}_{}", current_title, substituent_name);
                        let mut updated_stack_path = stack_path.clone();
                        updated_stack_path.extend(align_layers.clone());
                        updated_stack_path
                            .extend(layer_storage.create_layers([Layer::Fill(substituent)]));
                        updated_stacks.insert(title, updated_stack_path);
                    }
                    result.insert(substituent_name, updated_stacks);
                }
                Ok(RunnerOutput::MultiWindow(result))
            }
            Self::Rename {
                prefix,
                suffix,
                replace,
                regex,
            } => Ok(RunnerOutput::SingleWindow(
                current_window
                    .iter()
                    .map(|(title, stack_path)| {
                        let mut title = String::from(title);
                        if let Some((from, to)) = replace {
                            title = title.replace(from, to)
                        }
                        title = find_and_replace(&title, regex)?.to_string();
                        if let Some(prefix) = prefix {
                            title = [prefix.to_string(), title].join("_")
                        }
                        if let Some(suffix) = suffix {
                            title = [title, suffix.to_string()].join("_")
                        }
                        Ok((title, stack_path.clone()))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()?,
            )),
            Self::Output {
                prefix,
                suffix,
                target_directory,
                target_format,
                openbabel
            } => {
                std::fs::create_dir_all(target_directory).with_context(|| format!("Unable to create directory at {:?}", target_directory))?;
                let outputs = current_window
                    .into_par_iter()
                    .map(|(title, stack_path)| {
                        let data = cached_read_stack(base, &layer_storage, &stack_path)?;
                        let bonds = data.bonds.to_continuous_list(&data.atoms);
                        Ok(BasicIOMolecule::new(
                            title.to_string(),
                            data.atoms.into(),
                            bonds,
                        ))
                    })
                    .collect::<Result<Vec<_>, LayerStorageError>>()?;
                outputs.into_par_iter()
                    .map(|output| {
                        let mut path = target_directory.clone().join(&output.title);
                        let content = output.output(&target_format)?;
                        let content = [prefix.clone(), content, suffix.clone()]
                            .into_iter()
                            .filter(|part| part != "")
                            .collect::<Vec<_>>()
                            .join("\n");
                        path.set_extension(target_format.as_str());
                        let mut file = File::create(&path)
                            .with_context(|| format!("Unable to create output file at {:?}", path))?;
                        file.write_all(content.as_bytes())
                            .with_context(|| format!("Unable to write to output file at {:?}", path))?;
                        if *openbabel {
                            let path_for_arguments = path.to_string_lossy().to_string();
                            let mut command =  Command::new("obabel");
                            let child = command
                                .args([format!("{}", path_for_arguments), format!("-O{}", path_for_arguments)])
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped());
                            let result = child
                                .spawn()
                                .with_context(|| format!("Failed to start openbabel process for handling file at {:?}", path_for_arguments))?
                                .wait_with_output()
                                .with_context(|| format!("Failed to wait openbabel process for handling file at {:?}", path_for_arguments))?;
                            if !result.status.success() {
                                let mut error_log = path.clone();
                                error_log.set_extension("err_log");
                                let mut out_log = path.clone();
                                out_log.set_extension("out_log");
                                File::create(&error_log).with_context(|| format!("Failed to create error log file at {:?}", error_log))?
                                    .write_all(&result.stderr).with_context(|| format!("Failed to write error log file at {:?}", error_log))?;
                                File::create(&out_log).with_context(|| format!("Failed to create output log file at {:?}", out_log))?
                                    .write_all(&result.stderr).with_context(|| format!("Failed to write output log file at {:?}", out_log))?;
                                Err(anyhow!("Failed to handle file {:?} with openbabel, exit status {:?}, stderr and stdout logged.", path, result.status.code()))?;
                            };
                        };
                        Ok(())
                    })
                    .collect::<Result<Vec<()>>>()?;
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

use anyhow::{anyhow, Context, Result};
use cached::{proc_macro::cached, SizedCache};
use lmers::utils::fs::copy_skeleton;
use nalgebra::Vector3;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{collections::BTreeMap, io::Write};

use lmers::{
    external::{obabel::obabel, regexsed::regex_sed},
    io::{BasicIOMolecule, NamespaceMapping},
    layer::{Layer, SelectOne},
    sparse_molecule::SparseMolecule,
};
use serde::Deserialize;
use tempfile::tempdir;

use glob::glob;
use rayon::prelude::*;

use super::workflow_data::{LayerStorage, LayerStorageError, Window};

#[derive(Debug, Deserialize)]
pub struct RenameOptions {
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    suffix: Option<String>,
    #[serde(default)]
    replace: Option<(String, String)>,
    #[serde(default)]
    regex: Vec<String>,
}

impl RenameOptions {
    fn rename(&self, title: &str) -> anyhow::Result<String> {
        let mut title = String::from(title);
        if let Some((from, to)) = &self.replace {
            title = title.replace(from, to)
        }
        title = regex_sed(&title, &self.regex.join("; "))?;
        if let Some(prefix) = &self.prefix {
            title = [prefix.to_string(), title].join("_")
        }
        if let Some(suffix) = &self.suffix {
            title = [title, suffix.to_string()].join("_")
        }
        Ok(title)
    }
}

#[derive(Deserialize, Debug)]
pub struct FormatOptions {
    format: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    openbabel: bool,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    export_map: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "runner", content = "options")]
pub enum Runner {
    AppendLayers(Vec<Layer>),
    Substituent {
        address: Vec<(SelectOne, SelectOne)>,
        file_pattern: String,
    },
    Command {
        command: String,
        arguments: Vec<String>,
    },
    Rename(RenameOptions),
    Calculation {
        working_directory: PathBuf,
        pre_format: FormatOptions,
        pre_filename: String,
        #[serde(default)]
        serial_mode: bool,
        #[serde(default)]
        skeleton: Option<PathBuf>,
        #[serde(default)]
        redirect_to: Option<RenameOptions>,
        #[serde(default)]
        stdin: bool,
        #[serde(default)]
        program: Option<String>,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        envs: BTreeMap<String, String>,
        #[serde(default)]
        post_file: Option<(String, String)>,
        #[serde(default)]
        ignore_failed: bool,
        #[serde(default)]
        stdout: Option<String>,
        #[serde(default)]
        stderr: Option<String>,
    },
    CheckPoint,
}

#[derive(Deserialize, Debug)]
pub enum RunnerOutput {
    SingleWindow(Window),
    MultiWindow(BTreeMap<String, Window>),
    None,
}

impl Runner {
    pub fn execute<'a>(
        &self,
        base: &SparseMolecule,
        current_window: &'a Window,
        layer_storage: &LayerStorage,
    ) -> Result<RunnerOutput> {
        match self {
            Self::CheckPoint => Ok(RunnerOutput::None),
            Self::AppendLayers(layers) => {
                let layer_ids = layer_storage.create_layers(layers);
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
                serial_mode,
                pre_format,
                pre_filename,
                skeleton,
                stdin,
                program,
                args,
                envs,
                post_file,
                ignore_failed,
                stdout,
                stderr,
                redirect_to,
            } => {
                std::fs::create_dir_all(&working_directory).with_context(|| {
                    format!("Unable to create directory at {:?}", working_directory)
                })?;
                let handler = |(title, stack_path): (&'a String, &'a Vec<u64>)| {
                    let title = if let Some(redirect_to) = redirect_to {
                        redirect_to.rename(title)?
                    } else {
                        title.to_string()
                    };
                    let working_directory = working_directory.join(&title);
                    std::fs::create_dir_all(&working_directory).with_context(|| {
                        format!(
                            "Unable to create directory at {:?} for structure titled {}",
                            working_directory, &title
                        )
                    })?;
                    if let Some(skeleton) = skeleton {
                        copy_skeleton(skeleton, &working_directory).with_context(|| format!("Unable to copy skeleton folder from {:?} to {:?}", skeleton, working_directory))?
                    }
                    let structure = cached_read_stack(base, &layer_storage, stack_path)?;
                    let bonds = structure.bonds.clone().to_continuous_list(&structure.atoms);
                    let atoms = structure.atoms.clone().into();
                    let basic_molecule = BasicIOMolecule::new(title.to_string(), atoms, bonds);
                    let pre_content = basic_molecule.output(&pre_format.format)?;
                    let pre_content = if pre_format.openbabel {
                        obabel(&pre_content, &pre_format.format, &pre_format.format)?
                    } else {
                        pre_content
                    };
                    let mut pre_content = regex_sed(&pre_content, &pre_format.regex.join("; "))?;

                    if pre_format.prefix.len() > 0 {
                        pre_content = format!("{}\n{}", pre_format.prefix, pre_content)
                    }
                    if pre_format.suffix.len() > 0 {
                        pre_content = format!("{}\n{}", pre_content, pre_format.suffix)
                    }

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
                    if pre_format.export_map {
                        let map_file_path = working_directory.join("input.map.json");
                        let content = NamespaceMapping::from(structure.clone());
                        let file = File::create(&map_file_path).with_context(|| {
                            format!("Unable to create map file at {:?}", map_file_path)
                        })?;
                        serde_json::to_writer(file, &content).with_context(|| {
                            format!(
                                "Unable to serialize map file at {:?}, content: {:#?}",
                                map_file_path, content
                            )
                        })?;
                    }
                    if let Some(program) = program {
                        let mut command = Command::new(program);
                        command
                            .current_dir(&working_directory)
                            .args(args)
                            .envs(envs);
                        if *stdin {
                            let stdin = Stdio::from(File::open(&pre_path).with_context(|| {
                                format!("Unable to open created pre-file at {:?}", pre_content)
                            })?);
                            command.stdin(stdin);
                        }
                        if let Some(stdout) = stdout {
                            let stdout_path = working_directory.join(stdout);
                            let stdout_file = File::create(&stdout_path).with_context(|| {
                                format!(
                                    "Unable to create stdout file at {:?} for structure titled {}",
                                    stdout_path, title
                                )
                            })?;
                            command.stdout(Stdio::from(stdout_file));
                        } else {
                            command.stdout(Stdio::null());
                        }

                        if let Some(stderr) = stderr {
                            let stderr_path = working_directory.join(stderr);
                            let stderr_file = File::create(&stderr_path).with_context(|| {
                                format!(
                                    "Unable to create stdout file at {:?} for structure titled {}",
                                    stderr_path, title
                                )
                            })?;
                            command.stderr(Stdio::from(stderr_file));
                        } else {
                            command.stderr(Stdio::null());
                        }

                        let mut child = command.spawn().with_context(|| {
                            format!(
                                "Failed to start process for structure {}, process detail: {:#?}",
                                title, command
                            )
                        })?;
                        let result = child.wait().with_context(|| format!("Unable to wait the process handling structure {}, process detail: {:#?}", title, child))?;

                        if !result.success() {
                            Err(anyhow!(
                                "Handling process for structure {} failed. Error code {:?}",
                                title,
                                result.code()
                            ))?;
                        }
                        if let Some((post_format, post_filename)) = post_file {
                            let post_path = working_directory.join(post_filename);
                            let post_file = File::open(&post_path).with_context(|| {
                                format!(
                                    "Failed to open post-calculation file at {:?} for structure {}",
                                    post_path, title
                                )
                            })?;
                            let post_content = BasicIOMolecule::input(&post_format, post_file)?;
                            let updated_atoms = structure
                                .atoms
                                .update_from_continuous_list(&post_content.atoms)
                                .with_context(|| {
                                    format!(
                                        "Failed to import atoms from calculated result for structure {}",
                                        title
                                    )
                                })?;
                            let updated_bonds = post_content
                                .bonds
                                .into_iter()
                                .map(|(a, b, bond)| {
                                    Some((
                                        structure.atoms.from_continuous_index(a)?,
                                        structure.atoms.from_continuous_index(b)?,
                                        bond,
                                    ))
                                })
                                .collect::<Option<Vec<_>>>()
                                .with_context(|| {
                                    format!(
                                        "Failed to import bonds from calculated results for structure {}",
                                        title
                                    )
                                })?;
                            let mut structure = SparseMolecule::default();
                            structure.extend_to(structure.len());
                            structure.atoms.migrate(updated_atoms);
                            for (a, b, bond) in updated_bonds {
                                structure.bonds.set_bond(a, b, Some(bond));
                            }
                            Ok::<_, anyhow::Error>((title, stack_path, structure))
                        } else {
                            Ok((title, stack_path, SparseMolecule::default()))
                        }
                    } else {
                        Ok((title, stack_path, SparseMolecule::default()))
                    }
                };
                let results = if *serial_mode {
                    let outputs = current_window.iter().map(handler);
                    if *ignore_failed {
                        outputs.filter_map(|item| item.ok()).collect::<Vec<_>>()
                    } else {
                        outputs.collect::<Result<Vec<_>>>()?
                    }
                } else {
                    let outputs = current_window.par_iter().map(handler);
                    if *ignore_failed {
                        outputs.filter_map(|item| item.ok()).collect::<Vec<_>>()
                    } else {
                        outputs.collect::<Result<Vec<_>>>()?
                    }
                };
                if post_file.is_some() {
                    let mut window = BTreeMap::new();
                    for (title, stack_path, updated) in results {
                        let updated_layer = layer_storage.create_layers(&[Layer::Fill(updated)]);
                        let mut stack_path = stack_path.clone();
                        stack_path.extend(updated_layer);
                        window.insert(title.to_string(), stack_path);
                    }
                    Ok(RunnerOutput::SingleWindow(window))
                } else {
                    Ok(RunnerOutput::None)
                }
            }
            Self::Substituent {
                address,
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
                    for (current_title, stack_path) in current_window {
                        // println!("{:?}", stack_path);
                        let title = format!("{}_{}", current_title, substituent_name);
                        let mut stack_path = stack_path.clone();
                        for (center, replace) in address {
                            let current_structure =
                                cached_read_stack(base, &layer_storage, &stack_path)?;
                            let center_layer = Layer::SetCenter {
                                select: center.clone(),
                                center: Default::default(),
                            };
                            let align_layer = Layer::DirectionAlign {
                                select: replace.clone(),
                                direction: Vector3::x(),
                            };
                            let align_layers =
                                layer_storage.create_layers(&[center_layer, align_layer]);
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
                                        "The replace selector {:?} in {:?} is not validated",
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
                            stack_path.extend(align_layers);
                            stack_path
                                .extend(layer_storage.create_layers(&[Layer::Fill(substituent)]));
                        }
                        updated_stacks.insert(title, stack_path);
                    }
                    result.insert(substituent_name, updated_stacks);
                }
                Ok(RunnerOutput::MultiWindow(result))
            }
            Self::Rename(options) => Ok(RunnerOutput::SingleWindow(
                current_window
                    .iter()
                    .map(|(title, stack_path)| {
                        let title = options.rename(title)?;
                        Ok((title, stack_path.clone()))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()?,
            )),
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
    ty = "SizedCache<String, Result<SparseMolecule, LayerStorageError>>",
    create = "{ SizedCache::with_size(std::env::var(\"LME_CACHE_SIZE\").unwrap_or(\"5000\".to_string()).parse().unwrap()) }",
    convert = r#"{ stack_path.iter().map(|item| item.to_string()).collect::<Vec<_>>().join("/") }"#
)]
fn cached_read_stack(
    base: &SparseMolecule,
    layer_storage: &LayerStorage,
    stack_path: &[u64],
) -> Result<SparseMolecule, LayerStorageError> {
    if let Some((last, heads)) = stack_path.split_last() {
        let layer = layer_storage
            .read_layer(*last)
            .ok_or(LayerStorageError::NoSuchLayer(*last))?;
        let lower_result = cached_read_stack(base, layer_storage, heads)?;
        layer
            .filter(lower_result)
            .map_err(|err| LayerStorageError::FilterError(err))
    } else {
        Ok(base.clone())
    }
}

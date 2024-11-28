use std::{
    collections::BTreeMap,
    fs::File,
    path::PathBuf,
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use workflow::io::SparseMoleculeMap;

#[derive(Parser)]
struct Args {
    input: String,
    plain: bool,
    #[command(subcommand)]
    task: TaskType,
}

#[derive(Subcommand)]
enum TaskType {
    ConvertIndex { index: usize },
    Mapping,
    Ids,
    Groups,
    Size,
    IdToIndex { id: String },
    GroupToIndex { group: String },
}

fn main() {
    let args = Args::parse();
    let input_path = PathBuf::from(args.input);
    let input = File::open(&input_path)
        .with_context(|| format!("Failed to read input file {:?}", input_path))
        .unwrap();
    let input: SparseMoleculeMap = serde_json::from_reader(input)
        .with_context(|| format!("Failed to parse input file {:?}", input_path))
        .unwrap();
    let mapping = input
        .atoms
        .into_iter()
        .enumerate()
        .filter_map(|(sparse_index, filled)| if filled { Some(sparse_index) } else { None })
        .enumerate()
        .map(|(continuous_index, sparse_index)| (sparse_index, continuous_index))
        .collect::<BTreeMap<usize, usize>>();
    println!(
        "{}",
        match args.task {
            TaskType::ConvertIndex { index } => {
                let value = mapping.get(&index);
                serde_json::to_string(&value).unwrap()
            }
            TaskType::Size => {
                serde_json::to_string(&mapping.len()).unwrap()
            }
            TaskType::Mapping => {
                if args.plain {
                    mapping
                        .into_iter()
                        .map(|(s, c)| format!("{} {}", s, c))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    serde_json::to_string(&mapping).unwrap()
                }
            }
            TaskType::Ids => {
                let result = input
                    .ids
                    .into_iter()
                    .filter_map(|(name, index)| mapping.get(&index).map(|index| (name, index)));
                if args.plain {
                    result
                        .map(|(name, idx)| format!("{} {}", name, idx))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    serde_json::to_string(&result.collect::<BTreeMap<_, _>>()).unwrap()
                }
            }
            TaskType::Groups => {
                let groups = input.groups.get_lefts().into_iter().map(|group_name| {
                    (
                        group_name,
                        input
                            .groups
                            .get_left(group_name)
                            .filter_map(|index| mapping.get(index)),
                    )
                });
                if args.plain {
                    groups
                        .map(|(group_name, index)| {
                            format!(
                                "{} {}",
                                group_name,
                                index
                                    .map(|value| value.to_string())
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    serde_json::to_string(
                        &groups
                            .map(|(group_name, indexes)| (group_name, indexes.collect::<Vec<_>>()))
                            .collect::<BTreeMap<_, _>>(),
                    )
                    .unwrap()
                }
            }
            TaskType::IdToIndex { id } => {
                let index = input.ids.get(&id).and_then(|idx| mapping.get(idx));
                serde_json::to_string(&index).unwrap()
            }
            TaskType::GroupToIndex { group } => {
                let result = input
                    .groups
                    .get_left(&group)
                    .filter_map(|index| mapping.get(index));
                if args.plain {
                    result
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    serde_json::to_string(&result.collect::<Vec<_>>()).unwrap()
                }
            }
        }
    );
}

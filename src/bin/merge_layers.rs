use std::fs::File;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use lmers::{layer::Layer, sparse_molecule::SparseMolecule};

#[derive(Parser)]
/// Merge given layers on the given base SparseMolecular
struct Args {
    /// Specify the layers file (one file, YAML format)
    #[clap(long, short)]
    layers: String,
    /// Specify the base SparseMolecular file, ignore this to use an empty SparseMolecular 
    #[clap(long, short)]
    base: Option<String>,
    /// Specify the output file, ignore this to output to stdout
    #[clap(long, short)]
    output: Option<String>
}

fn merge_layers(layers: String, base: Option<String>) -> Result<SparseMolecule> {
    let layers_file = File::open(&layers).with_context(|| format!("Failed to open layers file at {}", layers))?;
    let layers: Vec<Layer> = serde_yaml::from_reader(layers_file).with_context(|| format!("Failed to read or parse layers file at {}", layers))?;
    let mut base = if let Some(base_file_path) = base {
        let base_file = File::open(&base_file_path).with_context(|| format!("Failed to open base file at {}", base_file_path))?;
        let base: SparseMolecule = serde_yaml::from_reader(base_file).with_context(|| format!("Failed to read or parse base file at {}", base_file_path))?;
        base
    } else {
        Default::default()
    };
    for (idx, layer) in layers.into_iter().enumerate() {
        base = layer.filter(base).map_err(|select| anyhow!("Unable to find select target {:?} used in layer {}", select, idx))?;
    }
    Ok(base)
}

fn main() {
    let Args { layers, base, output } = Args::parse();
    let result = merge_layers(layers, base).unwrap();
    if let Some(output) = output {
        let output_file = File::create(&output).with_context(|| format!("Failed to create output file at {}", output)).unwrap();
        serde_json::to_writer(output_file, &result).with_context(|| format!("Failed to write or serialize processed sparse molecule")).unwrap();
    } else {
        serde_json::to_writer(std::io::stdout(), &result).with_context(|| format!("Failed to write or serialize processed sparse molecule")).unwrap();
    }
}
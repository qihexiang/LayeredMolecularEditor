use std::fs::File;

use clap::Parser;
use glob::glob;
use lmers::{
    layer::{Layer, SelectOne},
    sparse_molecule::SparseMolecule,
};
use nalgebra::Vector3;

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// Prepare SparseMolecule file as subsittuent. This program will put the first atom at (0, 0, 0)
/// and rotate the molecule to make the second atom on (1, 0, 0) axis.
struct Arguments {
    /// Give the global file match pattern, for example:
    ///
    /// - "./*.ml.json" matches all ml.json files in current working directory
    ///
    /// - "./abc-*.ml.json" matches all ml.json files starts with abc- in current working directory
    ///
    /// - "./**/*.ml.json" matches all ml.json files can be found recursively in current working directory
    #[arg(short, long)]
    input: String,
}

fn main() {
    let arg = Arguments::parse();
    let matched_paths = glob(&arg.input).unwrap();
    for path in matched_paths {
        let path = path.unwrap();
        println!("Handling file {:?}", path);
        let file = File::open(&path).unwrap();
        let structure: SparseMolecule = serde_yaml::from_reader(file).unwrap();
        let set_center_layer = Layer::SetCenter {
            select: SelectOne::Index(0),
            center: Default::default(),
        };
        let align_layer = Layer::DirectionAlign {
            select: SelectOne::Index(1),
            direction: Vector3::x(),
        };
        let structure = set_center_layer.filter(structure).unwrap();
        let structure = align_layer.filter(structure).unwrap();
        let file = File::create(path).unwrap();
        serde_yaml::to_writer(file, &structure).unwrap();
    }
}

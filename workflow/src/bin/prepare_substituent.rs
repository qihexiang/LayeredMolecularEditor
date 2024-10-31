use std::fs::File;

use clap::Parser;
use glob::glob;
use lme::{
    layer::{Layer, SelectOne},
    sparse_molecule::SparseMolecule,
};
use nalgebra::Vector3;

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// Convert ml.json files to MoleculeLayer data in JSON(.ml.json) or YAML(.ml.yaml) format.
///
/// If neither -j/--json nor -y/--yaml is set, nothing will be output but check the ml.json files could be convert.
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
        let mut path = path.unwrap();
        println!("Handling file {:?}", path);
        let file = File::open(&path).unwrap();
        let structure: SparseMolecule = serde_yaml::from_reader(file).unwrap();
        let set_center_layer = Layer::SetCenter {
            select: SelectOne::Index(0),
            center: Default::default(),
        };
        let align_layer = Layer::DirectionAlgin {
            select: SelectOne::Index(1),
            direction: Vector3::x(),
        };
        let structure = set_center_layer.filter(structure).unwrap();
        let structure = align_layer.filter(structure).unwrap();
        path.set_extension("substituent.yaml");
        let file = File::create_new(path).unwrap();
        serde_yaml::to_writer(file, &structure).unwrap();
    }
}

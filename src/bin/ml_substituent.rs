use std::{collections::HashMap, fs::File, io::{Read, Write}};

use clap::Parser;
use glob::glob;
use lme::{
    chemistry::element_symbol_to_num,
    molecule_layer::{Atom3D, Atom3DList, BondMatrix, MoleculeLayer},
    n_to_n::NtoN, substituent::Substituent,
};
use nalgebra::Point3;

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
    /// Generate output MoleculeLayer file in JSON format.
    #[arg(short, long)]
    json: bool,
    /// Generate output MoleculeLayer file in YAML format.
    #[arg(short, long)]
    yaml: bool,
}

fn main() {
    let arg = Arguments::parse();
    let matched_paths = glob(&arg.input).unwrap();
    for path in matched_paths {
        let mut path = path.unwrap();
        println!("Handling file {:#?}", path);
        let file = File::open(&path).unwrap();
        let structure: MoleculeLayer = serde_yaml::from_reader(file).unwrap();
        let substituent_name = structure.title.clone();
        let substituent = Substituent::new(lme::layer::SelectOne::Index(0), lme::layer::SelectOne::Index(1), structure, substituent_name);
        path.set_extension("substituent.yaml");
        let file = File::create_new(path).unwrap();
        serde_yaml::to_writer(file, &substituent).unwrap();
    }
}

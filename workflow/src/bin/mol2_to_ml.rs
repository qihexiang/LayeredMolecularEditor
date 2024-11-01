use std::{collections::HashMap, fs::File};

use clap::Parser;
use glob::glob;
use lme::sparse_molecule::{SparseAtomList, SparseBondMatrix, SparseMolecule};
use n_to_n::NtoN;
use workflow::io::BasicIOMolecule;

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// Convert mol2 files to SparseMolecule data in JSON(.ml.json) or YAML(.ml.yaml) format.
///
/// If neither -j/--json nor -y/--yaml is set, nothing will be output but check the mol2 files could be convert.
struct Arguments {
    /// Give the global file match pattern, for example:
    ///
    /// - "./*.mol2" matches all mol2 files in current working directory
    ///
    /// - "./abc-*.mol2" matches all mol2 files starts with abc- in current working directory
    ///
    /// - "./**/*.mol2" matches all mol2 files can be found recursively in current working directory
    #[arg(short, long)]
    input: String,
    /// Generate output SparseMolecule file in JSON format.
    #[arg(short, long)]
    json: bool,
    /// Generate output SparseMolecule file in YAML format.
    #[arg(short, long)]
    yaml: bool,
}

fn main() {
    let arg = Arguments::parse();
    let matched_paths = glob(&arg.input).unwrap();
    for path in matched_paths {
        let path = path.unwrap();
        let content = {
            println!("Read file {:#?}", path);
            let file = File::open(&path).unwrap();
            let structure = BasicIOMolecule::input_from_mol2(file).unwrap();
            let atoms = SparseAtomList::from(structure.atoms);
            let mut bonds = SparseBondMatrix::new(atoms.len());
            for (a, b, bond) in structure.bonds {
                bonds.set_bond(a, b, Some(bond));
            }
            SparseMolecule {
                title: structure.title,
                atoms,
                bonds,
                ids: HashMap::new(),
                groups: NtoN::new(),
            }
        };

        if arg.json {
            let mut ml_path = path.clone();
            ml_path.set_extension("ml.json");
            let ml_file = File::create(ml_path).unwrap();
            serde_json::to_writer(ml_file, &content).unwrap();
        }

        if arg.yaml {
            let mut ml_path = path.clone();
            ml_path.set_extension("ml.yaml");
            let ml_file = File::create(ml_path).unwrap();
            serde_yaml::to_writer(ml_file, &content).unwrap();
        }
    }
}

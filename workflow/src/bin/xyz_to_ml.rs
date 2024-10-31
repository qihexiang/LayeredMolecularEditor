use std::{collections::HashMap, fs::File, io::Read};

use clap::Parser;
use glob::glob;
use lme::{chemistry::{element_symbol_to_num, Atom3D}, sparse_molecule::{SparseAtomList, SparseBondMatrix, SparseMolecule}};
use n_to_n::NtoN;
use nalgebra::Point3;

struct XYZContent {
    title: String,
    atoms: Vec<Atom3D>,
}

impl XYZContent {
    fn len(&self) -> usize {
        self.atoms.len()
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// Convert XYZ files to SparseMolecule data in JSON(.ml.json) or YAML(.ml.yaml) format.
///
/// If neither -j/--json nor -y/--yaml is set, nothing will be output but check the XYZ files could be convert.
struct Arguments {
    /// Give the global file match pattern, for example:
    ///
    /// - "./*.xyz" matches all xyz files in current working directory
    ///
    /// - "./abc-*.xyz" matches all xyz files starts with abc- in current working directory
    ///
    /// - "./**/*.xyz" matches all xyz files can be found recursively in current working directory
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
            let mut file = File::open(&path).unwrap();
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();
            let lines = content.lines();
            let mut lines = lines.filter(|line| line.len() != 0);
            let amount: usize = lines.next().unwrap().parse().unwrap();
            let title = lines.next().unwrap();
            let atoms: Vec<_> = lines
                .chain(std::iter::empty())
                .map(|line| {
                    let items = line
                        .split(" ")
                        .filter(|item| item.len() != 0)
                        .collect::<Vec<_>>();
                    let element = items[0];
                    let element = element_symbol_to_num(element).unwrap();
                    let position = items[1..4]
                        .into_iter()
                        .map(|item| -> f64 { item.parse().unwrap() })
                        .collect::<Vec<_>>();
                    let position: Point3<f64> = Point3::new(position[0], position[1], position[2]);
                    Atom3D { element, position }
                })
                .collect();
            if amount != atoms.len() {
                panic!("Invalid number and atoms")
            } else {
                XYZContent {
                    title: title.to_string(),
                    atoms,
                }
            }
        };

        let size = content.len();

        let molecule_layer = SparseMolecule {
            title: content.title,
            atoms: SparseAtomList::from(content.atoms),
            bonds: SparseBondMatrix::new(size),
            ids: HashMap::new(),
            groups: NtoN::new(),
        };

        if arg.json {
            let mut ml_path = path.clone();
            ml_path.set_extension("ml.json");
            let ml_file = File::create(ml_path).unwrap();
            serde_json::to_writer(ml_file, &molecule_layer).unwrap();
        }

        if arg.yaml {
            let mut ml_path = path.clone();
            ml_path.set_extension("ml.yaml");
            let ml_file = File::create(ml_path).unwrap();
            serde_yaml::to_writer(ml_file, &molecule_layer).unwrap();
        }
    }
}

use std::{collections::HashMap, fs::File, io::Read};

use clap::Parser;
use glob::glob;
use lme::{
    chemistry::element_symbol_to_num,
    molecule_layer::{Atom3D, Atom3DList, BondMatrix, MoleculeLayer},
    n_to_n::NtoN,
};
use nalgebra::Point3;

struct Mol2Content {
    title: String,
    atoms: Vec<Atom3D>,
    bonds: HashMap<(usize, usize), f64>,
}

impl Mol2Content {
    fn len(&self) -> usize {
        self.atoms.len()
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// Convert mol2 files to MoleculeLayer data in JSON(.ml.json) or YAML(.ml.yaml) format.
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
        let path = path.unwrap();
        let content = {
            println!("Read file {:#?}", path);
            let mut content = String::new();
            File::open(&path)
                .unwrap()
                .read_to_string(&mut content)
                .unwrap();
            let lines = content.lines();
            let lines = lines.filter(|line| line.len() != 0 || line.starts_with("#"));
            let mut molecule_block = lines
                .clone()
                .skip_while(|line| line != &"@<TRIPOS>MOLECULE")
                .skip(1)
                .take_while(|line| !line.starts_with("@<TRIPOS>"))
                .filter(|line| line != &"");
            let atom_block = lines
                .clone()
                .skip_while(|line| line != &"@<TRIPOS>ATOM")
                .skip(1)
                .take_while(|line| !line.starts_with("@<TRIPOS>"))
                .filter(|line| line != &"");
            let bond_block = lines
                .skip_while(|line| line != &"@<TRIPOS>BOND")
                .skip(1)
                .take_while(|line| !line.starts_with("@<TRIPOS>"))
                .filter(|line| line != &"");
            let title = molecule_block.next().unwrap();
            let atoms = atom_block
                .map(|line| {
                    let mut line_items = line.split(" ").filter(|item| item != &"").skip(1);
                    let element = line_items.next().unwrap();
                    let x = line_items.next().unwrap();
                    let y = line_items.next().unwrap();
                    let z = line_items.next().unwrap();
                    let element = element_symbol_to_num(element).unwrap();
                    let [x, y, z] = [x, y, z].map(|item| -> f64 { item.parse().unwrap() });
                    Atom3D {
                        element,
                        position: Point3::new(x, y, z),
                    }
                })
                .collect::<Vec<_>>();
            let bonds = bond_block
                .map(|line| {
                    let mut line_items = line.split(" ").filter(|item| item != &"").skip(1);
                    let a = line_items.next().unwrap();
                    let b = line_items.next().unwrap();
                    let bond = line_items.next().unwrap();
                    let [a, b] = [a, b]
                        .map(|item| -> usize { item.parse().unwrap() })
                        .map(|item| item - 1);
                    let bond = match bond {
                        "ar" | "Ar" | "AR" => 1.5,
                        value => value.parse().unwrap(),
                    };
                    ((a, b), bond)
                })
                .collect::<HashMap<_, _>>();
            Mol2Content {
                title: title.to_string(),
                atoms,
                bonds,
            }
        };

        let size = content.len();

        let mut molecule_layer = MoleculeLayer {
            title: content.title,
            atoms: Atom3DList::from(content.atoms),
            bonds: BondMatrix::new(size),
            ids: HashMap::new(),
            groups: NtoN::new(),
        };

        for ((a, b), bond) in content.bonds {
            molecule_layer.bonds.set_bond(a, b, Some(bond));
        }

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

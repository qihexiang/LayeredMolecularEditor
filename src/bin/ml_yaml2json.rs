use std::fs::File;

use clap::Parser;
use glob::glob;
use lmers::sparse_molecule::SparseMolecule;

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// Convert ml.json files to SparseMolecule data in JSON(.ml.json)。
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
        println!("Handling file {:#?}", path);
        let file = File::open(&path).unwrap();
        let structure: SparseMolecule = serde_yaml::from_reader(file).unwrap();
        path.set_extension("json");
        let file = File::create_new(path).unwrap();
        serde_json::to_writer(file, &structure).unwrap();
    }
}

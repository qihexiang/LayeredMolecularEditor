use std::{fs::File, io::{Cursor, Read, Write}};

use clap::Parser;
use lmers::{external::obabel::obabel, io::BasicIOMolecule, layer::{Layer, SelectOne}, sparse_molecule::SparseMolecule, utils::sterimol::{self, auto_connect_bonds, get_molecular_graph, RadiisTable}};
use nalgebra::Vector3;
use rayon::prelude::*;
use glob::glob;
use anyhow::{anyhow, Context, Result};

#[derive(Parser)]
enum Operation {
    /// Import commom molecular files to LME
    Import {
        /// Input filepath pattern
        #[clap(short='i')]
        input_filepath: String,
        /// Input file format
        #[clap(short='I')]
        input_format: String,
        #[clap(short='g')]
        gen3d: bool,
        /// Prepare generate file as substituents
        #[clap(short='s')]
        as_substituent: bool,
        #[clap(short='S')]
        sterimol: Option<String>
    },
    /// Export LME files to common formats
    Export {
        /// Input LME files
        #[clap(short)]
        input_filepath: String,
        /// Output file format
        #[clap(short)]
        output_format: String,
    }
}

impl Operation {
    fn operate(self) -> Result<()> {
        match self {
            Self::Import { input_filepath, input_format, gen3d, as_substituent, sterimol } => {
                let matched_paths = glob(&input_filepath).with_context(|| format!("Invalid file match pattern: {}", input_filepath))?;
                let set_center_layer = Layer::SetCenter {
                    select: SelectOne::Index(0),
                    center: Default::default(),
                };
                let align_layer = Layer::DirectionAlign {
                    select: SelectOne::Index(1),
                    direction: Vector3::x(),
                };
                let radiis_table = if let Some(radiis_path) = sterimol {
                    let file = File::open(&radiis_path).with_context(|| format!("Failed to open speicified radiis table {}", radiis_path))?;
                    let table: RadiisTable = serde_json::from_reader(file).with_context(|| "Unable to parse given radiis table")?;
                    Some(table)
                } else {
                    None
                };
                let _ = matched_paths.par_bridge()
                    .map(|entry| {
                        let mut input = entry.with_context(|| format!("Unable to read path matched"))?;
                        let mut input_content = String::new();
                        File::open(&input).with_context(|| format!("Failed to open matched file {:?}", input))?
                            .read_to_string(&mut input_content)
                            .with_context(|| format!("Failed to read matched file {:?}", input))?;
                        let mol2 = obabel(&input_content, &input_format, "mol2", true, gen3d)?;
                        let mut molecule = SparseMolecule::from(BasicIOMolecule::input("mol2", Cursor::new(mol2))?);
                        if as_substituent {
                            molecule = align_layer.filter(set_center_layer.filter(molecule).map_err(|_| anyhow!("Substituent require at least 2 atoms"))?).map_err(|_| anyhow!("Substituent require at least 2 atoms"))?;
                        }
                        input.set_extension("lme");
                        serde_json::to_writer(File::create(&input).with_context(|| format!("Unable to create output file at {:?}", input))?, &molecule)?;
                        if let Some(radiis_table) = &radiis_table {
                            let bonds = molecule.bonds.to_continuous_list(&molecule.atoms);
                            let atoms = molecule.atoms.into();
                            let bonds = if bonds.len() == 0 {
                                auto_connect_bonds(&atoms, radiis_table)?
                            } else {
                                bonds
                            };
                            let molecular_graph = get_molecular_graph(&atoms, &bonds);
                            let (l, b1, b5) = sterimol::sterimol(&molecular_graph, radiis_table)?;
                            let tca = sterimol::tolman_cone_angle(&molecular_graph)?;
                            input.set_extension("sterimol");
                            File::create(&input).with_context(|| format!("Unable to create sterimol file at {:?}", input))?
                                .write_all(format!("{l},{b1},{b5},{tca}").as_bytes())
                                .with_context(|| format!("Unable to write sterimol file at {:?}", input))?;
                        }
                        Ok(())
                    })
                    .collect::<Result<Vec<()>>>()?;
                Ok(())
            },
            Self::Export { input_filepath, output_format } => {
                let matched_paths = glob(&input_filepath).with_context(|| format!("Invalid file match pattern: {}", input_filepath))?;
                let _ = matched_paths.par_bridge()
                    .map(|entry| {
                        let mut input = entry.with_context(|| format!("Unable to read path matched"))?;
                        let structure: SparseMolecule = serde_yaml::from_reader(File::open(&input).with_context(|| format!("Failed to open matched file {:?}", input))?)?;
                        let mol2 = BasicIOMolecule::from((structure, input.file_stem().map(|stem| stem.to_string_lossy().to_string()).unwrap_or_default())).output("mol2").with_context(|| format!("Failed to convert to intermediate format {:?}", input))?;
                        let output = obabel(&mol2, "mol2", &output_format, true, false)?;
                        input.set_extension(output_format.clone());
                        File::create(&input).with_context(|| format!("Failed to create output file {:?}", input))?
                            .write_all(output.as_bytes())
                            .with_context(|| format!("Failed to write to output file {:?}", input))?;
                        Ok(())
                    })
                    .collect::<Result<Vec<()>>>()?;
                Ok(())
            }
        }
    }
}

fn main() {
    let operation = Operation::parse();
    operation.operate().unwrap();
}
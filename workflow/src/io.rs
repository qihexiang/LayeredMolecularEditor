use std::io::Read;

use anyhow::{anyhow, Context, Error, Ok, Result};
use lme::chemistry::{element_num_to_symbol, element_symbol_to_num, Atom3D};
use nalgebra::Point3;
use rayon::prelude::*;

pub struct BasicIOMolecule {
    pub atoms: Vec<Atom3D>,
    pub bonds: Vec<(usize, usize, f64)>,
    pub title: String,
}

impl BasicIOMolecule {
    pub fn new(title: String, atoms: Vec<Atom3D>, bonds: Vec<(usize, usize, f64)>) -> Self {
        Self {
            title,
            atoms,
            bonds,
        }
    }

    pub fn output(&self, format: &str) -> Result<String> {
        match format {
            "xyz" => self.output_to_xyz(),
            "mol2" => self.output_to_mol2(),
            format => Err(anyhow!("Unsupported format {format}")),
        }
    }

    pub fn input<R: Read>(format: &str, r: R) -> Result<Self> {
        match format {
            "xyz" => Self::input_from_xyz(r),
            "mol2" => Self::input_from_mol2(r),
            format => Err(anyhow!("Unsupported format {format}")),
        }
    }

    fn input_from_xyz<R: Read>(mut r: R) -> Result<Self> {
        let mut content = String::new();
        r.read_to_string(&mut content)?;
        let lines = content.lines();
        let mut lines = lines.filter(|line| line.len() != 0);
        let amount: usize = lines
            .next()
            .with_context(|| "Unable to read count line of XYZ file")?
            .parse()
            .with_context(|| "Count line is not a integer")?;
        let title = lines
            .next()
            .with_context(|| "Unable to read title line of XYZ file")?;
        let atoms: Vec<_> = lines
            .chain(std::iter::empty())
            .map(|line| {
                let items = line
                    .split(" ")
                    .filter(|item| item.len() != 0)
                    .collect::<Vec<_>>();
                let element = items.get(0).with_context(|| {
                    format!("Invalid atom line {line} in XYZ file, no element token found")
                })?;
                let element = element_symbol_to_num(element)
                    .with_context(|| format!("Invalid element token in {line}"))?;
                let x = items
                    .get(1)
                    .with_context(|| {
                        format!("Invalid atom line {line} in XYZ file, no x token found")
                    })?
                    .parse()
                    .with_context(|| format!("Unable to parse x token in line {line}"))?;
                let y = items
                    .get(2)
                    .with_context(|| {
                        format!("Invalid atom line {line} in XYZ file, no y token found")
                    })?
                    .parse()
                    .with_context(|| format!("Unable to parse y token in line {line}"))?;
                let z = items
                    .get(3)
                    .with_context(|| {
                        format!("Invalid atom line {line} in XYZ file, no z token found")
                    })?
                    .parse()
                    .with_context(|| format!("Unable to parse z token in line {line}"))?;
                let position = Point3::new(x, y, z);
                Ok(Atom3D { element, position })
            })
            .collect::<Result<Vec<_>>>()?;
        if amount != atoms.len() {
            Err(anyhow!(
                "Count of atom lines is not matched to count line: {} vs. {}",
                atoms.len(),
                amount
            ))
        } else {
            Ok(Self {
                title: title.to_string(),
                atoms,
                bonds: vec![],
            })
        }
    }

    fn input_from_mol2<R: Read>(mut r: R) -> Result<Self> {
        let mut content = String::new();
        r.read_to_string(&mut content)?;
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
                let element = line_items.next().with_context(|| {
                    format!("Unable to read element token of atom in line {line}")
                })?;
                let x = line_items
                    .next()
                    .with_context(|| format!("Unable to read x token of atom in line {line}"))?
                    .parse()?;
                let y = line_items
                    .next()
                    .with_context(|| format!("Unable to read y token of atom in line {line}"))?
                    .parse()?;
                let z = line_items
                    .next()
                    .with_context(|| format!("Unable to read z token of atom in line {line}"))?
                    .parse()?;
                let element = element_symbol_to_num(element).unwrap();
                Ok(Atom3D {
                    element,
                    position: Point3::new(x, y, z),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bonds = bond_block
            .map(|line| {
                let mut line_items = line.split(" ").filter(|item| item != &"").skip(1);
                let a: usize = line_items
                    .next()
                    .with_context(|| format!("Unable to read atom token 0 of bond in line {line}"))?
                    .parse()?;
                let b: usize = line_items
                    .next()
                    .with_context(|| format!("Unable to read atom token 1 of bond in line {line}"))?
                    .parse()?;
                let bond = line_items
                    .next()
                    .with_context(|| format!("Unable to read bond token of bond in line {line}"))?;
                let bond = match bond {
                    "ar" | "Ar" | "AR" => 1.5,
                    value => value.parse().unwrap(),
                };
                Ok((a - 1, b - 1, bond))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            title: title.to_string(),
            atoms,
            bonds,
        })
    }

    fn output_to_xyz(&self) -> Result<String> {
        let title = self.title.clone();
        let count = self.atoms.len().to_string();
        let xyz = self
            .atoms
            .iter()
            .map(|atom| {
                Ok(format!(
                    "{} {} {} {}",
                    element_num_to_symbol(&atom.element).with_context(|| format!(
                        "Invalid element number found {}",
                        atom.element
                    ))?,
                    atom.position.x,
                    atom.position.y,
                    atom.position.z
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok([vec![count, title], xyz].concat().join("\n"))
    }

    fn output_to_mol2(&self) -> Result<String> {
        let title = self.title.clone();
        let atom_count = self.atoms.len().to_string();
        let bond_count = self.bonds.len();
        let atoms = self
            .atoms
            .iter()
            .enumerate()
            .map(|(index, atom)| {
                let element_symbol = element_num_to_symbol(&atom.element)
                    .with_context(|| format!("Invalid element number found {}", atom.element))?;
                Ok(format!(
                    "{} {} {} {} {} {}",
                    index,
                    element_symbol,
                    atom.position.x,
                    atom.position.y,
                    atom.position.z,
                    element_symbol
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let bonds = self
            .bonds
            .par_iter()
            .enumerate()
            .map(|(index, (a, b, bond))| {
                let bond = if bond == &1.5 {
                    "ar".to_string()
                } else {
                    bond.to_string()
                };
                format!("{} {} {} {}", index + 1, a + 1, b + 1, bond)
            })
            .collect::<Vec<_>>();
        let content = vec![
            vec![
                "@<TRIPOS>MOLECULE".to_string(),
                title,
                format!("{} {} 0 0 0", atom_count, bond_count),
                "SMALL".to_string(),
                "GASTEIGER".to_string(),
                "".to_string(),
                "@<TRIPOS>ATOM".to_string(),
            ],
            atoms,
            vec!["@<TRIPOS>BOND".to_string()],
            bonds,
        ]
        .concat()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
        Ok(content)
    }
}

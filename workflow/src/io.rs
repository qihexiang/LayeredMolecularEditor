use anyhow::{Context, Error, Result};
use lme::chemistry::{element_num_to_symbol, Atom3D};
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

    pub fn output_to_xyz(&self) -> Result<String> {
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

    pub fn output_to_mol2(&self) -> Result<String> {
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

use std::collections::{BTreeMap, HashSet};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    chemistry::{element_num_to_symbol, validated_element_num},
    molecule_layer::{Atom3D, Atom3DList, MoleculeLayer},
    n_to_n::NtoN,
};

#[derive(Deserialize, Serialize)]
pub struct AtomListMap(Vec<Option<bool>>);

impl AtomListMap {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn to_compacted_idx(&self, index: usize) -> Option<usize> {
        if self.0.get(index).copied().flatten()? {
            Some(
                self.0
                    .iter()
                    .take(index + 1)
                    .filter(|value| value.unwrap_or_default())
                    .count()
                    - 1,
            )
        } else {
            None
        }
    }
}

impl From<&Atom3DList> for AtomListMap {
    fn from(value: &Atom3DList) -> Self {
        Self(
            value
                .data()
                .par_iter()
                .map(|atom| atom.map(|atom| validated_element_num(&atom.element)))
                .collect(),
        )
    }
}

#[derive(Debug)]
pub enum CompactedMoleculeError {
    OutputInvalidAtomSymbol(usize),
    UnsupportedFormat(String)
}

pub struct CompactedMolecule {
    pub atoms: Vec<Atom3D>,
    pub bonds: Vec<(usize, usize, f64)>,
    pub title: String,
    pub ids: BTreeMap<String, usize>,
    pub groups: NtoN<String, usize>,
    pub atom_map: AtomListMap,
}

impl CompactedMolecule {
    pub fn output_to_xyz(&self) -> Result<String, CompactedMoleculeError> {
        let title = self.title.clone();
        let count = self.atoms.len().to_string();
        let xyz = self
            .atoms
            .iter()
            .map(|atom| {
                Ok(format!(
                    "{} {} {} {}",
                    element_num_to_symbol(&atom.element).ok_or(
                        CompactedMoleculeError::OutputInvalidAtomSymbol(atom.element)
                    )?,
                    atom.position.x,
                    atom.position.y,
                    atom.position.z
                ))
            })
            .collect::<Result<Vec<_>, CompactedMoleculeError>>()?;
        Ok([vec![count, title], xyz].concat().join("\n"))
    }

    pub fn output_to_mol2(&self) -> Result<String, CompactedMoleculeError> {
        let title = self.title.clone();
        let atom_count = self.atoms.len().to_string();
        let bond_count = self.bonds.len();
        let atoms = self
            .atoms
            .iter()
            .enumerate()
            .map(|(index, atom)| {
                let element_symbol = element_num_to_symbol(&atom.element).ok_or(
                    CompactedMoleculeError::OutputInvalidAtomSymbol(atom.element),
                )?;
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
            .collect::<Result<Vec<_>, _>>()?;
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

impl From<MoleculeLayer> for CompactedMolecule {
    fn from(value: MoleculeLayer) -> Self {
        let atom_map = AtomListMap::from(&value.atoms);
        let atoms: Vec<Atom3D> = value.atoms.into();
        let mut bonds = Vec::with_capacity(atom_map.len().pow(2));
        for row_idx in 0..value.bonds.len() {
            for col_idx in row_idx..value.bonds.len() {
                match (
                    atom_map.to_compacted_idx(row_idx),
                    atom_map.to_compacted_idx(col_idx),
                    value.bonds.read_bond(row_idx, col_idx),
                ) {
                    (Some(a), Some(b), Some(bond)) => {
                        if bond != 0. {
                            bonds.push((a, b, bond))
                        }
                    }
                    _ => {}
                }
            }
        }
        let ids = value
            .ids
            .into_iter()
            .filter_map(|(id, index)| atom_map.to_compacted_idx(index).map(|index| (id, index)))
            .collect::<BTreeMap<_, _>>();
        let groups = NtoN::from(
            value
                .groups
                .into_iter()
                .filter_map(|(group_name, index)| {
                    atom_map
                        .to_compacted_idx(index)
                        .map(|index| (group_name, index))
                })
                .collect::<HashSet<_>>(),
        );
        let title = value.title;
        Self {
            title,
            atom_map,
            atoms,
            bonds,
            ids,
            groups,
        }
    }
}

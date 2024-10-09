use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use nalgebra::{Isometry3, Point3};
use serde::{Deserialize, Serialize};

use crate::{chemistry::validated_element_num, io::AtomListMap, n_to_n::NtoN};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Atom3D {
    pub element: usize,
    pub position: Point3<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Atom3DList(Vec<Option<Atom3D>>);

impl From<Vec<Atom3D>> for Atom3DList {
    fn from(value: Vec<Atom3D>) -> Self {
        Self(value.into_iter().map(|atom| Some(atom)).collect())
    }
}

impl Into<Vec<Atom3D>> for Atom3DList {
    fn into(self) -> Vec<Atom3D> {
        self.0
            .into_iter()
            .filter_map(|atom| {
                atom.and_then(|atom| {
                    if validated_element_num(&atom.element) {
                        Some(atom)
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}

impl Atom3DList {
    pub fn new(capacity: usize) -> Self {
        Self(vec![Default::default(); capacity])
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    fn extend_to(&mut self, capacity: usize) {
        let current_capacity = self.len();
        if current_capacity < capacity {
            self.0
                .extend_from_slice(&vec![Default::default(); capacity - current_capacity]);
        }
    }

    pub fn offset(self, offset: usize) -> Self {
        Self(vec![vec![Default::default(); offset], self.0].concat())
    }

    pub fn read_atom(&self, index: usize) -> Option<Atom3D> {
        self.0.get(index).copied().unwrap_or_default()
    }

    pub fn set_atoms(&mut self, offset: usize, atoms: Vec<Option<Atom3D>>) {
        let len_after_set = (offset + atoms.len() - 1).max(self.len());
        self.extend_to(len_after_set);
        for (idx, atom) in atoms.into_iter().enumerate() {
            self.0[idx + offset] = atom
        }
    }

    pub fn isometry(&mut self, isometry: Isometry3<f64>, select: &BTreeSet<usize>) {
        self.0
            .iter_mut()
            .enumerate()
            .filter(|(idx, _)| select.contains(idx))
            .for_each(|(_, atom)| {
                if let Some(atom) = atom {
                    atom.position = isometry * atom.position
                }
            })
    }

    pub fn migrate(&mut self, other: &Self) {
        let capacity = self.len().max(other.len());
        self.extend_to(capacity);
        self.0
            .iter_mut()
            .enumerate()
            .for_each(|(index, atom)| *atom = other.read_atom(index).or(*atom))
    }

    pub fn data(&self) -> &Vec<Option<Atom3D>> {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BondMatrix(Vec<Vec<Option<f64>>>);

impl BondMatrix {
    pub fn new(capacity: usize) -> Self {
        Self(vec![vec![None; capacity]; capacity])
    }

    pub fn new_filled(capacity: usize) -> Self {
        Self(vec![vec![Some(0.); capacity]; capacity])
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn extend_to(&mut self, capacity: usize) {
        if self.len() < capacity {
            let current_capacity = self.len();
            self.0
                .iter_mut()
                .for_each(|row| row.extend(&vec![None; capacity - current_capacity]));
            self.0
                .append(&mut vec![vec![None; capacity]; capacity - current_capacity]);
        }
    }

    pub fn offset(self, offset: usize) -> Self {
        let current_capacity = self.len();
        let prepend_rows = vec![vec![None; offset + current_capacity]; offset];
        let current_rows = self
            .0
            .into_iter()
            .map(|row| vec![vec![None; offset], row].concat())
            .collect();
        Self(vec![prepend_rows, current_rows].concat())
    }

    pub fn read_bond(&self, a: usize, b: usize) -> Option<f64> {
        self.0.get(a)?.get(b).copied().flatten()
    }

    pub fn get_neighbors(&self, center: usize) -> Option<impl Iterator<Item = &Option<f64>>> {
        Some(self.0.get(center)?.iter())
    }

    pub fn set_bond(&mut self, a: usize, b: usize, bond: Option<f64>) -> bool {
        let max_index = a.max(b);
        if max_index >= self.len() {
            false
        } else {
            self.0[a][b] = bond;
            self.0[b][a] = bond;
            true
        }
    }

    pub fn migrate(&mut self, other: &Self) {
        let capacity = self.len().max(other.len());
        self.extend_to(capacity);
        for (row_idx, row) in self.0.iter_mut().enumerate() {
            for (col_idx, cell) in row.iter_mut().enumerate() {
                *cell = other.read_bond(row_idx, col_idx).or(*cell);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MoleculeLayer {
    pub title: String,
    pub atoms: Atom3DList,
    pub bonds: BondMatrix,
    pub ids: HashMap<String, usize>,
    pub groups: NtoN<String, usize>,
}

impl MoleculeLayer {
    pub fn migrate(&mut self, other: &Self) {
        self.title = other.title.to_string();
        self.atoms.migrate(&other.atoms);
        self.bonds.migrate(&other.bonds);
        self.ids.extend(other.ids.clone());
        self.groups.extend(other.groups.clone());
    }

    pub fn offset(self, offset: usize) -> Self {
        let atoms = self.atoms.offset(offset);
        let bonds = self.bonds.offset(offset);
        let ids: HashMap<String, usize> = self
            .ids
            .into_iter()
            .map(|(id, idx)| (id, idx + offset))
            .collect();
        let groups: NtoN<String, usize> = NtoN::from(
            self.groups
                .into_iter()
                .map(|(group_name, idx)| (group_name, idx + offset))
                .collect::<HashSet<_>>(),
        );
        Self {
            title: self.title,
            atoms,
            bonds,
            ids,
            groups,
        }
    }
}

pub struct CompactedMolecule {
    pub atoms: Vec<Atom3D>,
    pub bonds: Vec<(usize, usize, f64)>,
    pub title: String,
    pub ids: BTreeMap<String, usize>,
    pub groups: NtoN<String, usize>,
    pub atom_map: AtomListMap,
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

use std::{
    collections::{BTreeSet, HashMap},
    ops::Div,
};

use n_to_n::NtoN;
use nalgebra::Isometry3;
use serde::{Deserialize, Serialize};

use crate::chemistry::{validated_element_num, Atom3D};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SparseAtomList(Vec<Option<Atom3D>>);

impl From<Vec<Atom3D>> for SparseAtomList {
    fn from(value: Vec<Atom3D>) -> Self {
        Self(value.into_iter().map(|atom| Some(atom)).collect())
    }
}

impl Into<Vec<Atom3D>> for SparseAtomList {
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

impl SparseAtomList {
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

    pub fn update_from_continous_list(&self, list: &Vec<Atom3D>) -> Option<Self> {
        let mut sparse_list = self.clone();
        let mut wait_to_update = list.iter();
        for item in sparse_list.0.iter_mut() {
            if item
                .map(|atom| validated_element_num(atom.element))
                .unwrap_or_default()
            {
                *item = Some(*wait_to_update.next()?);
            }
        }
        Some(sparse_list)
    }

    pub fn to_continous_index(&self, index: usize) -> Option<usize> {
        if self
            .read_atom(index)
            .map(|atom| validated_element_num(atom.element))
            .unwrap_or_default()
        {
            Some(
                self.0
                    .iter()
                    .take(index)
                    .filter(|item| {
                        item.map(|item| validated_element_num(item.element))
                            .unwrap_or_default()
                    })
                    .count(),
            )
        } else {
            None
        }
    }

    pub fn from_countious_index(&self, index: usize) -> Option<usize> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, atom)| {
                atom.map(|atom| validated_element_num(atom.element))
                    .unwrap_or_default()
            })
            .take(index + 1)
            .last()
            .map(|(index, _)| index)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SparseBondMatrix(Vec<Vec<Option<f64>>>);

impl SparseBondMatrix {
    pub fn new(capacity: usize) -> Self {
        Self(vec![vec![None; capacity]; capacity])
    }

    pub fn new_filled(capacity: usize) -> Self {
        Self(vec![vec![Some(0.); capacity]; capacity])
    }

    pub fn len(&self) -> usize {
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

    pub fn set_bond(&mut self, a: usize, b: usize, bond: Option<f64>) {
        self.extend_to(a.max(b) + 1);
        self.0[a][b] = bond;
        self.0[b][a] = bond;
    }

    pub fn migrate(&mut self, other: &Self) {
        for row_idx in 0..other.len() {
            for col_idx in row_idx..other.len() {
                let bond = other
                    .read_bond(row_idx, col_idx)
                    .or(self.read_bond(row_idx, col_idx));
                self.set_bond(row_idx, col_idx, bond);
            }
        }
    }

    pub fn to_continous_list(&self, atom_list: &SparseAtomList) -> Vec<(usize, usize, f64)> {
        let mut continous_list = Vec::with_capacity(atom_list.len().pow(2).div(2));
        for row_idx in 0..self.len() {
            for col_idx in row_idx..self.len() {
                match (
                    atom_list.to_continous_index(row_idx),
                    atom_list.to_continous_index(col_idx),
                    self.read_bond(row_idx, col_idx),
                ) {
                    (Some(row_idx), Some(col_idx), Some(bond)) => {
                        continous_list.push((row_idx, col_idx, bond));
                    }
                    _ => {}
                }
            }
        }
        continous_list
    }
}

impl<T: Clone + Iterator<Item = ((usize, usize), f64)>> From<T> for SparseBondMatrix {
    fn from(value: T) -> Self {
        let capacity = value
            .clone()
            .map(|((a, b), _)| a.max(b))
            .max()
            .unwrap_or_default();
        let mut bond_matrix = Self::new(capacity);
        for ((a, b), bond) in value {
            bond_matrix.set_bond(a, b, Some(bond));
        }
        bond_matrix
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SparseMolecule {
    pub atoms: SparseAtomList,
    pub bonds: SparseBondMatrix,
    pub ids: HashMap<String, usize>,
    pub groups: NtoN<String, usize>,
}

impl SparseMolecule {
    pub fn migrate(&mut self, other: &Self) {
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
                .map(|(group_name, idx)| (group_name, idx + offset)),
        );
        Self {
            atoms,
            bonds,
            ids,
            groups,
        }
    }
}

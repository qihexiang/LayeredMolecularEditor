use std::collections::{BTreeSet, HashMap};

use nalgebra::{Isometry3, Point3, Translation3, Vector3};
use serde::{Serialize, Deserialize};
use crate::{chemistry::{Atom3D, MoleculeLayer}, n_to_n::NtoN};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Layer {
    Fill {
        data: MoleculeLayer,
    },
    Plugin {
        plugin_name: String,
        arguments: Vec<String>,
        data: MoleculeLayer,
    },
    IdMap {
        data: HashMap<String, usize>,
    },
    GroupMap {
        data: NtoN<String, usize>,
    },
    SetCenter {
        select: SelectOne,
    },
    Translation {
        select: SelectMany,
        vector: Vector3<f64>,
    },
    Rotation {
        select: SelectMany,
        center: SelectOne,
        axis: Vector3<f64>,
        angle: f64,
    },
    Isometry {
        select: SelectMany,
        isometry: Isometry3<f64>,
    },
    RemoveAtoms {
        select: SelectMany,
    },
}

impl Layer {
    pub fn filter(&self, mut current: MoleculeLayer) -> Result<MoleculeLayer, SelectOne> {
        match self {
            Self::Fill { data } => current.migrate(data),
            Self::IdMap { data } => current.ids.extend(data.clone()),
            Self::GroupMap { data } => current.groups.extend(data.clone()),
            Self::Plugin { data, .. } => current.migrate(data),
            Self::SetCenter { select } => {
                let target_atom = select
                    .to_index(&current)
                    .and_then(|index| current.atoms.read_atom(index));
                if let Some(target_atom) = target_atom {
                    let translation = Point3::origin() - target_atom.position;
                    let translation =
                        Isometry3::translation(translation.x, translation.y, translation.z);
                    current
                        .atoms
                        .isometry(translation, &SelectMany::All.to_indexes(&current));
                } else {
                    Err(select.clone())?
                }
            }
            Self::Translation { select, vector } => {
                let translation = Isometry3::translation(vector.x, vector.y, vector.z);
                current
                    .atoms
                    .isometry(translation, &select.to_indexes(&current));
            }
            Self::Rotation {
                select,
                center,
                axis,
                angle,
            } => {
                let center_atom = center
                    .to_index(&current)
                    .and_then(|index| current.atoms.read_atom(index));
                if let Some(center) = center_atom {
                    let move_to_origin = Point3::origin() - center.position;
                    let move_to_origin =
                        Translation3::new(move_to_origin.x, move_to_origin.y, move_to_origin.z);
                    let move_back = move_to_origin.inverse();
                    current
                        .atoms
                        .isometry(move_to_origin.into(), &select.to_indexes(&current));
                    current.atoms.isometry(
                        Isometry3::rotation(*axis * *angle),
                        &select.to_indexes(&current),
                    );
                    current
                        .atoms
                        .isometry(move_back.into(), &select.to_indexes(&current));
                } else {
                    Err(center.clone())?
                }
            }
            Self::Isometry { select, isometry } => {
                current
                    .atoms
                    .isometry(*isometry, &select.to_indexes(&current));
            }
            Self::RemoveAtoms { select } => {
                let selected = select.to_indexes(&current);
                let atoms = (0..current.atoms.len())
                    .map(|index| {
                        if selected.contains(&index) {
                            Some(Atom3D::default())
                        } else {
                            None
                        }
                    })
                    .collect();
                current.atoms.set_atoms(0, atoms);
            }
        }
        Ok(current)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectOne {
    Index(usize),
    IdName(String),
}

impl SelectOne {
    pub fn to_index(&self, layer: &MoleculeLayer) -> Option<usize> {
        match self {
            Self::Index(index) => Some(*index),
            Self::IdName(id_name) => layer.ids.get(id_name).copied(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectMany {
    All,
    Number(usize),
    Indexes(BTreeSet<usize>),
    GroupName(String),
}

impl SelectMany {
    fn to_indexes(&self, layer: &MoleculeLayer) -> BTreeSet<usize> {
        match self {
            Self::All => (0..layer.atoms.len()).collect(),
            Self::Number(number) => (0..layer.atoms.len())
                .filter(|index| {
                    if let Some(atom) = layer.atoms.read_atom(*index) {
                        atom.element == *number
                    } else {
                        false
                    }
                })
                .collect(),
            Self::GroupName(group_name) => layer
                .groups
                .get_left(group_name)
                .into_iter()
                .copied()
                .collect(),
            Self::Indexes(indexes) => indexes.clone(),
        }
    }
}

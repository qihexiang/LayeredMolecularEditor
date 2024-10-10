use std::collections::{BTreeSet, HashMap};

use crate::{
    molecule_layer::{Atom3D, MoleculeLayer},
    n_to_n::NtoN,
    serde_default::default_x_axis,
    substituent::axis_angle_for_b2a,
};
use nalgebra::{Isometry3, Point3, Translation3, Vector3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Layer {
    Fill(MoleculeLayer),
    TitleModification {
        #[serde(default)]
        prefix: String,
        #[serde(default)]
        suffix: String,
        #[serde(default)]
        replace: Vec<(String, String)>,
    },
    SetBond(Vec<(usize, usize, f64)>),
    Plugin {
        plugin_name: String,
        arguments: Vec<String>,
        data: MoleculeLayer,
    },
    IdMap(HashMap<String, usize>),
    GroupMap(NtoN<String, usize>),
    SetCenter(SelectOne),
    DirectionAlgin {
        select: SelectOne,
        #[serde(default = "default_x_axis")]
        direction: Vector3<f64>,
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
    RemoveAtoms(SelectMany),
}

impl Default for Layer {
    fn default() -> Self {
        Self::Fill(Default::default())
    }
}

impl Layer {
    pub fn filter(&self, mut current: MoleculeLayer) -> Result<MoleculeLayer, SelectOne> {
        match self {
            Self::Fill(data) => current.migrate(data),
            Self::SetBond(bonds) => {
                for (a, b, bond) in bonds {
                    current.bonds.set_bond(*a, *b, Some(*bond));
                }
            }
            Self::TitleModification {
                prefix,
                suffix,
                replace,
            } => {
                let mut title = current.title.clone();
                for (from, to) in replace {
                    title = title.replace(from, to);
                }
                current.title = [prefix, &title, suffix]
                    .into_iter()
                    .filter(|item| item != &"")
                    .map(|item| item.clone())
                    .collect::<Vec<_>>()
                    .join("_")
            }
            Self::IdMap(data) => current.ids.extend(data.clone()),
            Self::GroupMap(data) => current.groups.extend(data.clone()),
            Self::Plugin { data, .. } => current.migrate(data),
            Self::SetCenter(select) => {
                let target_atom = select.get_atom(&current);
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
            Self::DirectionAlgin { select, direction } => {
                let target_atom = select.get_atom(&current).ok_or(select.clone())?;
                let current_direction = target_atom.position - Point3::default();
                let (axis, angle) = axis_angle_for_b2a(*direction, current_direction);
                let rotation = Isometry3::rotation(*axis * angle);
                current
                    .atoms
                    .isometry(rotation, &SelectMany::All.to_indexes(&current));
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
                let center_atom = center.get_atom(&current);
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
            Self::RemoveAtoms(select) => {
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

    pub fn get_atom(&self, layer: &MoleculeLayer) -> Option<Atom3D> {
        self.to_index(layer)
            .and_then(|index| layer.atoms.read_atom(index))
    }

    pub fn set_atom(&self, layer: &mut MoleculeLayer, atom: Option<Atom3D>) -> Option<()> {
        self.to_index(layer)
            .and_then(|index| Some(layer.atoms.set_atoms(index, vec![atom])))
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
    pub fn to_indexes(&self, layer: &MoleculeLayer) -> BTreeSet<usize> {
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

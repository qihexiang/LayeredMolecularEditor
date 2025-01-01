use std::{
    collections::{BTreeMap, BTreeSet},
    ops::RangeInclusive,
};

use bincode::{Encode, Decode};
use nalgebra::{Isometry3, Point3, Translation3, Vector3};
use redb::Value;
use serde::{Deserialize, Serialize};

use crate::{
    chemistry::Atom3D,
    group_name::GroupName,
    sparse_molecule::{SparseAtomList, SparseMolecule},
    utils::geometric::axis_angle_for_b2a,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(tag = "layer", content = "arguments")]
pub enum Layer {
    Fill(SparseMolecule),
    Insert(usize, SparseMolecule),
    Append(String, SparseMolecule),
    SetAtom(Vec<(SelectOne, Option<Atom3D>)>),
    UpdateFormalCharge(Vec<(SelectOne, f64)>),
    AppendAtoms(Vec<Atom3D>),
    SetBond(Vec<(SelectOne, SelectOne, f64)>),
    IdMap(BTreeMap<String, usize>),
    GroupMap(Vec<(String, SelectMany)>),
    SetCenter {
        select: SelectOne,
        #[serde(default)]
        #[bincode(with_serde)]
        center: Point3<f64>,
    },
    DirectionAlign {
        select: SelectOne,
        #[serde(default = "Vector3::x")]
        #[bincode(with_serde)]
        direction: Vector3<f64>,
    },
    XYAlign {
        o: SelectOne,
        x: SelectOne,
        y: SelectOne,
        #[serde(default)]
        select: SelectMany
    },
    Translation {
        select: SelectMany,
        #[bincode(with_serde)]
        vector: Vector3<f64>,
    },
    TranslationTo {
        select: SelectMany,
        target: SelectOne,
        #[serde(default)]
        #[bincode(with_serde)]
        position: Point3<f64>,
    },
    RotationTo {
        a: SelectOne,
        b: SelectOne,
        select: SelectMany,
        #[serde(default = "Vector3::x")]
        #[bincode(with_serde)]
        direction: Vector3<f64>,
    },
    Rotation {
        select: SelectMany,
        #[bincode(with_serde)]
        #[serde(default)]
        center: Point3<f64>,
        #[bincode(with_serde)]
        #[serde(default="Vector3::x")]
        axis: Vector3<f64>,
        angle: f64,
    },
    Isometry {
        select: SelectMany,
        #[bincode(with_serde)]
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
    pub fn filter(&self, mut current: SparseMolecule) -> Result<SparseMolecule, SelectOne> {
        match self {
            Self::Fill(data) => current.migrate(data.clone()),
            Self::Insert(offset, molecule) => {
                current.migrate(molecule.clone().offset(*offset));
            }
            Self::Append(prefix, molecule) => {
                let mut molecule = molecule.clone();
                molecule.ids = molecule.ids.map(|ids| {
                    ids.into_iter()
                        .map(|(name, index)| (format!("{}_{}", prefix, name), index))
                        .collect()
                });
                molecule.groups =
                    molecule.groups.map(|groups| {
                        GroupName::from_iter(groups.into_iter().map(|(group_name, index)| {
                            (format!("{}_{}", prefix, group_name), index)
                        }))
                    });
                let molecule = Layer::GroupMap(vec![(prefix.to_string(), SelectMany::All)])
                    .filter(molecule)?;
                let molecule = molecule.offset(current.len());
                current.migrate(molecule);
            }
            Self::SetBond(bonds) => {
                for (a, b, bond) in bonds {
                    let a = a.to_index(&current).ok_or(a.clone())?;
                    let b = b.to_index(&current).ok_or(b.clone())?;
                    current.bonds.set_bond(a, b, Some(*bond));
                }
            }
            Self::SetAtom(updates) => {
                for (select, atom) in updates {
                    select.set_atom(&mut current, atom.clone());
                }
            },
            Self::UpdateFormalCharge(updates) => {
                for (select, charge) in updates {
                    let mut current_atom = select.get_atom(&current).ok_or(select.clone())?;
                    current_atom.formal_charge = *charge;
                    select.set_atom(&mut current, Some(current_atom));
                }
            }
            Self::AppendAtoms(atoms) => {
                current.atoms.set_atoms(
                    current.atoms.len(),
                    atoms.iter().map(|atom| Some(*atom)).collect(),
                );
            }
            Self::IdMap(data) => {
                if let Some(current_ids) = &mut current.ids {
                    current_ids.extend(data.clone());
                } else {
                    current.ids = Some(data.clone());
                }
            }
            Self::GroupMap(data) => {
                for (name, selects) in data {
                    let selects = selects
                        .to_indexes(&current)
                        .into_iter()
                        .map(|index| (name.to_string(), index));
                    if let Some(current_groups) = &mut current.groups {
                        current_groups.extend(selects);
                    } else {
                        current.groups =
                            Some(GroupName::from_iter(selects.collect::<BTreeSet<_>>()));
                    }
                }
            }
            Self::XYAlign { o, x, y, select } => {
                let o_position = o.get_atom(&current).ok_or(o.clone())?.position;
                let move_to_origin = Point3::origin() - o_position;
                current = Self::Translation { select: select.clone(), vector: move_to_origin }.filter(current)?;
                let x_position = x.get_atom(&current).ok_or(x.clone())?.position;
                let ox = (x_position - Point3::origin()).normalize();
                let (ox_rt_axis, ox_rt_angle) = axis_angle_for_b2a(Vector3::x(), ox);
                current = Self::Rotation { select: select.clone(), center: Point3::origin(), axis: *ox_rt_axis, angle: ox_rt_angle }.filter(current)?;
                let y_position = y.get_atom(&current).ok_or(y.clone())?.position;
                let oy = y_position - Point3::origin();
                let oy = (oy - oy.dot(&Vector3::x()) * Vector3::x()).normalize();
                let (oy_rt_axis, oy_rt_angle) = axis_angle_for_b2a(Vector3::y(), oy);
                current = Self::Rotation { select: select.clone(), center: Default::default(), axis: *oy_rt_axis, angle: oy_rt_angle }.filter(current)?;
            }
            Self::SetCenter { select, center } => {
                let target_atom = select.get_atom(&current);
                if let Some(target_atom) = target_atom {
                    let translation = center - target_atom.position;
                    let translation =
                        Isometry3::translation(translation.x, translation.y, translation.z);
                    current
                        .atoms
                        .isometry(translation, &SelectMany::All.to_indexes(&current));
                } else {
                    Err(select.clone())?
                }
            }
            Self::DirectionAlign { select, direction } => {
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
            Self::TranslationTo {
                select,
                target,
                position,
            } => {
                let target_atom = target.get_atom(&current).ok_or(target.clone())?;
                let vector = *position - target_atom.position;
                current = Self::Translation {
                    select: select.clone(),
                    vector,
                }
                .filter(current)?;
            }
            Self::RotationTo {
                a,
                b,
                select,
                direction,
            } => {
                let center_atom = a.get_atom(&current).ok_or(a.clone())?;
                let target_atom = b.get_atom(&current).ok_or(b.clone())?;
                let current_direction = target_atom.position - center_atom.position;
                let (axis, angle) = axis_angle_for_b2a(*direction, current_direction);
                current = Self::Rotation {
                    select: select.clone(),
                    center: center_atom.position,
                    axis: *axis,
                    angle,
                }
                .filter(current)?;
            }
            Self::Rotation {
                select,
                center,
                axis,
                angle,
            } => {
                let move_to_origin = Point3::origin() - center;
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
            }
            Self::Isometry { select, isometry } => {
                current
                    .atoms
                    .isometry(*isometry, &select.to_indexes(&current));
            }
            Self::RemoveAtoms(select) => {
                let selected = select.to_indexes(&current);
                let atoms = SparseAtomList::from(
                    (0..current.atoms.len())
                        .map(|index| {
                            if selected.contains(&index) {
                                Some(Atom3D::default())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                );
                current.atoms.migrate(atoms);
            }
        }
        Ok(current)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, PartialOrd, Ord, Eq, Encode, Decode)]
#[serde(untagged)]
pub enum SelectOne {
    Index(usize),
    IdName(String),
}

impl SelectOne {
    pub fn to_index(&self, layer: &SparseMolecule) -> Option<usize> {
        match self {
            Self::Index(index) => Some(*index),
            Self::IdName(id_name) => layer.ids.as_ref()?.get(id_name).copied(),
        }
    }

    pub fn get_atom(&self, layer: &SparseMolecule) -> Option<Atom3D> {
        self.to_index(layer)
            .and_then(|index| layer.atoms.read_atom(index))
    }

    pub fn set_atom(&self, layer: &mut SparseMolecule, atom: Option<Atom3D>) -> Option<()> {
        self.to_index(layer)
            .and_then(|index| Some(layer.atoms.set_atoms(index, vec![atom])))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode, Default)]
#[serde(untagged)]
pub enum SelectMany {
    #[default]
    All,
    Complex {
        includes: Vec<SelectMany>,
        #[serde(default)]
        excludes: Vec<SelectMany>,
    },
    Element(usize),
    Indexes(BTreeSet<SelectOne>),
    Range(RangeInclusive<usize>),
    GroupName(String),
}

impl SelectMany {
    pub fn to_indexes(&self, layer: &SparseMolecule) -> BTreeSet<usize> {
        match self {
            Self::All => (0..layer.atoms.len()).collect(),
            Self::Element(number) => (0..layer.atoms.len())
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
                .as_ref()
                .map(|groups| groups.get_left(group_name).into_iter().copied().collect())
                .unwrap_or_default(),
            Self::Indexes(indexes) => indexes
                .iter()
                .filter_map(|select| select.to_index(layer))
                .collect(),
            Self::Range(range) => range.clone().collect(),
            Self::Complex { includes, excludes } => {
                let mut selected = BTreeSet::new();
                for include in includes {
                    selected.extend(include.to_indexes(layer));
                }
                for exclude in excludes {
                    let exclude = exclude.to_indexes(layer);
                    selected.retain(|index| !exclude.contains(index));
                }
                selected
            }
        }
    }
}

impl Value for Layer {
    type AsBytes<'a> = Vec<u8>;
    type SelfType<'a> = Layer;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a {
        bincode::decode_from_slice(data, bincode::config::standard()).unwrap().0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b {
        bincode::encode_to_vec(value, bincode::config::standard()).unwrap()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("layer_table")
    }
}

use std::{collections::{HashMap, HashSet}, f64::consts::PI};

use lme::{
    chemistry::MoleculeLayer,
    layer::{SelectMany, SelectOne}, n_to_n::NtoN,
};
use nalgebra::{Isometry3, Translation3, Unit, UnitQuaternion, Vector3};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Substituent {
    entry: SelectOne,
    target: SelectOne,
    structure: MoleculeLayer,
    group_prefix: String
}

#[derive(Debug)]
pub enum SubstituentError {
    EntryAtomNotFoundInTarget(SelectOne),
    ExitAtomNotFoundInTarget(SelectOne),
    EntryAtomNotFoundInSubstituent(SelectOne),
    ExitAtomNotFoundInSusbstituent(SelectOne),
}

impl Substituent {
    pub fn generate_path(
        &self,
        base: MoleculeLayer,
        entry: SelectOne,
        target: SelectOne,
    ) -> Result<MoleculeLayer, SubstituentError> {
        let target_entry = entry
            .get_atom(&base)
            .ok_or(SubstituentError::EntryAtomNotFoundInTarget(entry.clone()))?;
        let target_exit = target
            .get_atom(&base)
            .ok_or(SubstituentError::ExitAtomNotFoundInTarget(target.clone()))?;
        let a = target_exit.position - target_entry.position;
        let substituent_entry = self.entry.get_atom(&self.structure).ok_or(
            SubstituentError::EntryAtomNotFoundInSubstituent(self.entry.clone()),
        )?;
        let substituent_exit = self.target.get_atom(&self.structure).ok_or(
            SubstituentError::ExitAtomNotFoundInSusbstituent(self.target.clone()),
        )?;
        let b = substituent_exit.position - substituent_entry.position;
        let axis = a.cross(&b);
        let axis = Unit::new_normalize(if axis.norm() == 0. {
            Vector3::x()
        } else {
            axis
        });
        let angle = a.dot(&b) / (a.norm() * b.norm());
        let angle = if angle.is_nan() { PI } else { angle };
        let translation = Translation3::from(target_entry.position - substituent_entry.position);
        let rotation = UnitQuaternion::new(angle * *axis);
        let rotation = Isometry3::from_parts(Translation3::from(Vector3::zeros()), rotation);
        let mut substituent = self.structure.clone();
        let select = SelectMany::All.to_indexes(&substituent);
        let pre_translation = Translation3::from(- substituent_entry.position);
        let post_translation = pre_translation.inverse();
        substituent.atoms.isometry(pre_translation.into(), &select);
        substituent.atoms.isometry(rotation, &select);
        substituent.atoms.isometry(post_translation.into(), &select);
        substituent.atoms.isometry(translation.into(), &select);
        self.entry.set_atom(&mut substituent, None);
        let exit_atom = self.target.get_atom(&substituent).expect("unable to get exit atom in substituent");
        self.target.set_atom(&mut substituent, None);
        let offset = base.atoms.len();
        let mut substituent = substituent.offset(offset); 
        substituent.groups = NtoN::from(substituent.groups.get_lefts().into_iter().map(|current_name| {
            let mut updated_name = self.group_prefix.clone();
            updated_name.push_str(&current_name);
            substituent.groups.get_left(current_name).map(move |index| (updated_name.clone(), *index))
        }).flatten().collect::<HashSet<_>>());
        substituent.ids = HashMap::new();
        entry.set_atom(&mut substituent, Some(target_entry));
        target.set_atom(&mut substituent, Some(exit_atom));
        Ok(substituent)
    }
}

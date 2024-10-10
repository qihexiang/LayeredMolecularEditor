use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{chemistry::validated_element_num, molecule_layer::Atom3DList};

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

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{chemistry::validated_element_num, molecule_layer::Atom3DList};

#[derive(Deserialize, Serialize)]
pub struct AtomListMap(Vec<Option<bool>>);

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

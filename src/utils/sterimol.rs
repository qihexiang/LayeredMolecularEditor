use anyhow::{Context, Result};
use serde::Deserialize;

use crate::chemistry::Atom3D;

#[derive(Deserialize)]
pub struct RadiisItem {
    symbol: String,
    value: f64,
}

pub type RadiisTable = Vec<RadiisItem>;

pub fn sterimol(atoms: Vec<Atom3D>, table: &RadiisTable) -> Result<(f64, f64, f64)> {
    let a = atoms
        .get(0)
        .with_context(|| "First atom of substituent group not found, require at least 2 atoms")?;
    let b = atoms
        .get(1)
        .with_context(|| "Second atom of subsitutent group not found, require at least 2 atoms")?;
    let ab = b.position - a.position;
    let axis = ab.normalize();
    let mean_width = ab.norm()
        + table
            .get(b.element)
            .with_context(|| {
                format!(
                    "Failed to found the radiis for the second atom element {}",
                    b.element
                )
            })?
            .value;
    let l = atoms
        .iter()
        .skip(1)
        .map(|atom| {
            let radii = table
                .get(atom.element)
                .with_context(|| format!("Failed to read radiis of element {}", atom.element))?
                .value;
            let projection = (atom.position - a.position).dot(&axis);
            Ok::<f64, anyhow::Error>(projection + radii)
        })
        .collect::<Result<Vec<f64>>>()?
        .into_iter()
        .reduce(|acc, next| if acc > next { acc } else { next })
        .unwrap_or(mean_width);
    let b1 = atoms
        .iter()
        .skip(2)
        .map(|atom| {
            Ok::<f64, anyhow::Error>((atom.position - b.position).norm()
                + table
                    .get(atom.element)
                    .with_context(|| format!("Failed to read radiis of element {}", atom.element))?
                    .value)
        })
        .collect::<Result<Vec<f64>>>()?
        .into_iter()
        .reduce(|acc, next| if acc > next { acc } else { next })
        .unwrap_or(mean_width);
    let b5 = atoms
        .iter()
        .skip(2)
        .map(|atom| {
            Ok::<f64, anyhow::Error>((atom.position - b.position).norm()
                + table
                    .get(atom.element)
                    .with_context(|| format!("Failed to read radiis of element {}", atom.element))?
                    .value)
        })
        .collect::<Result<Vec<f64>>>()?
        .into_iter()
        .reduce(|acc, next| if acc > next { acc } else { next })
        .unwrap_or(mean_width);
    Ok((l, b1, b5))
}

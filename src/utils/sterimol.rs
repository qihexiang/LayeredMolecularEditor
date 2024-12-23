use anyhow::{Context, Result};
use petgraph::{csr::IndexType, prelude::StableUnGraph};
use serde::Deserialize;

use crate::chemistry::Atom3D;

#[derive(Deserialize)]
pub struct RadiisItem {
    symbol: String,
    value: f64,
}

pub fn auto_connect_bonds(
    atoms: &Vec<Atom3D>,
    r_cov_table: &RadiisTable,
) -> Result<Vec<(usize, usize, f64)>> {
    let mut bonds = vec![];
    for (a_idx, atom) in atoms.iter().enumerate() {
        let r_a = r_cov_table
            .get(atom.element)
            .with_context(|| {
                format!(
                    "Failed to found the radiis for the second atom element {}",
                    atom.element
                )
            })?
            .value;
        let p_a = atom.position;
        for (b_idx, atom) in atoms.iter().enumerate().skip(a_idx + 1) {
            let r_b = r_cov_table
                .get(atom.element)
                .with_context(|| {
                    format!(
                        "Failed to found the radiis for the second atom element {}",
                        atom.element
                    )
                })?
                .value;
            let distance = (atom.position - p_a).norm();
            if distance <= r_a + r_b {
                bonds.push((a_idx, b_idx, 1.0))
            }
        }
    }
    Ok(bonds)
}

pub fn molecular_graph_walk(
    graph: &StableUnGraph<Atom3D, f64, usize>,
    entry: usize,
    current_depth: usize,
    limit_depth: usize,
    excludes: Vec<usize>,
) -> Result<Vec<(usize, Atom3D)>> {
    let current_position = graph
        .node_weight(entry.into())
        .with_context(|| format!("Failed to get atom information with index {}", entry))?;
    let neighbors = graph
        .neighbors(entry.into())
        .filter(|neighbor| !excludes.contains(&neighbor.index()))
        .collect::<Vec<_>>();
    if current_depth == limit_depth || neighbors.len() == 0 {
        Ok(vec![(entry, *current_position)])
    } else {
        let sub_find_results = neighbors
            .into_iter()
            .map(|index| {
                molecular_graph_walk(
                    graph,
                    index.index(),
                    current_depth + 1,
                    limit_depth,
                    vec![vec![entry.index()], excludes.clone()].concat(),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(sub_find_results.concat())
    }
}

pub type RadiisTable = Vec<RadiisItem>;

pub fn sterimol(
    atoms: Vec<Atom3D>,
    bonds: Vec<(usize, usize, f64)>,
    table: &RadiisTable,
) -> Result<(f64, f64, f64)> {
    let mut molecular_graph: StableUnGraph<Atom3D, f64, usize> = StableUnGraph::default();
    for atom in atoms {
        molecular_graph.add_node(atom);
    }
    molecular_graph.extend_with_edges(bonds);
    let a = molecular_graph
        .node_weight(0.into())
        .with_context(|| "First atom of substituent group not found, require at least 2 atoms")?;
    let b = molecular_graph
        .node_weight(1.into())
        .with_context(|| "Second atom of subsitutent group not found, require at least 2 atoms")?;
    let b_radii = table
        .get(b.element)
        .with_context(|| format!("Unable to get radii from table for element {}", b.element))?
        .value;
    let ab = b.position - a.position;
    let axis = ab.normalize();
    let l = molecular_graph
        .node_indices()
        .skip(2)
        .map(|idx| molecular_graph.node_weight(idx).unwrap())
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
        .unwrap_or(ab.norm() + b_radii);
    let branches = molecular_graph_walk(&molecular_graph, 1, 0, 1, vec![0])?
        .into_iter()
        .map(|(idx, _)| {
            Ok(
                molecular_graph_walk(&molecular_graph, idx, 1, 0, vec![0, 1])?
                    .into_iter()
                    .map(|(_, atom)| atom)
                    .map(|atom| {
                        Ok((atom.position - b.position).norm()
                            + table
                                .get(atom.element)
                                .with_context(|| {
                                    format!("Failed to read radiis of element {}", atom.element)
                                })?
                                .value)
                    })
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .reduce(|acc, next| if acc > next { acc } else { next })
                    .expect("At least one value in each branch here"),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let b1 = branches
        .iter()
        .copied()
        .reduce(|acc, next| if acc < next { acc } else { next })
        .unwrap_or(b_radii);
    let b5 = branches
        .into_iter()
        .reduce(|acc, next| if acc > next { acc } else { next })
        .unwrap_or(b_radii);
    // let b1 = atoms
    //     .iter()
    //     .skip(2)
    //     .map(|atom| {
    //         Ok::<f64, anyhow::Error>((atom.position - b.position).norm()
    //             + table
    //                 .get(atom.element)
    //                 .with_context(|| format!("Failed to read radiis of element {}", atom.element))?
    //                 .value)
    //     })
    //     .collect::<Result<Vec<f64>>>()?
    //     .into_iter()
    //     .reduce(|acc, next| if acc > next { acc } else { next })
    //     .unwrap_or(mean_width);
    // let b5 = atoms
    //     .iter()
    //     .skip(2)
    //     .map(|atom| {
    //         Ok::<f64, anyhow::Error>((atom.position - b.position).norm()
    //             + table
    //                 .get(atom.element)
    //                 .with_context(|| format!("Failed to read radiis of element {}", atom.element))?
    //                 .value)
    //     })
    //     .collect::<Result<Vec<f64>>>()?
    //     .into_iter()
    //     .reduce(|acc, next| if acc > next { acc } else { next })
    //     .unwrap_or(mean_width);
    Ok((l, b1, b5))
}

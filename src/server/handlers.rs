use std::{
    collections::BTreeSet,
    ops::{Deref, Range},
    sync::Arc,
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use lme::{
    molecule_layer::{Atom3D, MoleculeLayer},
    layer::{Layer, SelectOne},
    workspace::{LayerStorage, LayerStorageError, StackCache},
};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{AppState, WorkspaceName};

#[derive(Deserialize)]
pub struct WorkspaceCreation {
    name: String,
    import: Option<(LayerStorage, Vec<Vec<usize>>)>,
}

pub async fn create_workspace(
    State(state): State<AppState>,
    Json(workspace): Json<WorkspaceCreation>,
) -> Response {
    let name_confilct = state.read().await.contains_key(&workspace.name);
    if name_confilct {
        (
            StatusCode::CONFLICT,
            "Unable to create workspace, name already used.",
        )
            .into_response()
    } else if let Some((layers, stacks)) = workspace.import {
        state.write().await.insert(
            workspace.name,
            (
                Arc::new(RwLock::new(layers)),
                Arc::new(RwLock::new(stacks)),
                Default::default(),
            ),
        );
        StatusCode::OK.into_response()
    } else {
        state
            .write()
            .await
            .insert(workspace.name, Default::default());
        StatusCode::OK.into_response()
    }
}

pub async fn export_workspace(
    Extension(layers): Extension<Arc<RwLock<LayerStorage>>>,
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
) -> Json<(LayerStorage, Vec<Vec<usize>>)> {
    let layers = layers.read().await;
    let stacks = stacks.read().await;
    Json((layers.deref().clone(), stacks.deref().clone()))
}

pub async fn remove_workspace(
    State(state): State<AppState>,
    Path(workspace): Path<WorkspaceName>,
) -> Response {
    let mut workspaces = state.write().await;
    if workspaces.remove(&workspace.ws_name).is_some() {
        StatusCode::OK.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

pub async fn get_layers(
    Extension(layers): Extension<Arc<RwLock<LayerStorage>>>,
) -> Json<BTreeSet<usize>> {
    Json(layers.read().await.layer_ids().copied().collect())
}

#[derive(Serialize, Debug)]
pub enum WorkspaceError {
    LayerInUse(usize),
    NotFillLayer(usize),
    NoSuchStack(usize),
    NoSuchLayer(usize),
    NoSuchAtom(SelectOne),
}

impl From<LayerStorageError> for WorkspaceError {
    fn from(value: LayerStorageError) -> Self {
        match value {
            LayerStorageError::NoSuchLayer(err_layer_id) => Self::NoSuchLayer(err_layer_id),
            LayerStorageError::FilterError(err_select_info) => Self::NoSuchAtom(err_select_info),
        }
    }
}

#[derive(Deserialize)]
pub struct LayerID {
    layer_id: usize,
}

#[derive(Deserialize)]
pub struct StackId {
    stack_id: usize,
}

pub async fn read_stack(
    Extension(layers_storage): Extension<Arc<RwLock<LayerStorage>>>,
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
    Extension(stack_cache): Extension<Arc<RwLock<StackCache>>>,
    Path(StackId { stack_id }): Path<StackId>,
) -> Result<Json<MoleculeLayer>, Json<WorkspaceError>> {
    let stacks = stacks.read().await;
    let stack_path = stacks
        .get(stack_id)
        .ok_or(Json(WorkspaceError::NoSuchStack(stack_id)))?;
    let cache = stack_cache.read().await.read_cache(&stack_path).cloned();
    if let Some(cached) = cache {
        Ok(Json(cached))
    } else {
        let data = layers_storage
            .read()
            .await
            .read_stack(&stack_path, Default::default())
            .map_err(|err| Json(WorkspaceError::from(err)))?;
        stack_cache
            .write()
            .await
            .write_cache(&stack_path, data.clone());
        Ok(Json(data))
    }
}

pub async fn create_layers(
    Extension(layers_storage): Extension<Arc<RwLock<LayerStorage>>>,
    Json(layers): Json<Vec<Layer>>,
) -> Json<Range<usize>> {
    Json(
        layers_storage
            .write()
            .await
            .create_layers(layers.into_iter()),
    )
}

pub async fn create_stack(
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
    Json(create_stack): Json<Vec<usize>>,
) -> Json<usize> {
    let mut stacks = stacks.write().await;
    let stack_id = stacks.len();
    stacks.push(create_stack);
    Json(stack_id)
}

pub async fn clone_stacks(
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
    Path(StackId { stack_id }): Path<StackId>,
    Json(copies): Json<usize>,
) -> Response {
    let mut stacks = stacks.write().await;
    if let Some(target) = stacks.get(stack_id).cloned() {
        let start = stacks.len();
        stacks.extend((0..copies).map(|_| target.clone()));
        Json(start..stacks.len()).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchStack(stack_id)),
        )
            .into_response()
    }
}

pub async fn slice_stack(
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
    Path(StackId { stack_id }): Path<StackId>,
    Json((start, end)): Json<(usize, usize)>,
) -> Response {
    if let Some(stack) = stacks.write().await.get_mut(stack_id) {
        *stack = stack[start..end].to_vec();
        StatusCode::OK.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchStack(stack_id)),
        )
            .into_response()
    }
}

pub async fn add_layers(
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
    Path(stack_id): Path<usize>,
    Json(layers): Json<Vec<usize>>,
) -> Response {
    if let Some(stack) = stacks.write().await.get_mut(stack_id) {
        stack.extend(layers);
        StatusCode::OK.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchStack(stack_id)),
        )
            .into_response()
    }
}

pub async fn read_layer(
    Extension(layers): Extension<Arc<RwLock<LayerStorage>>>,
    Path(LayerID { layer_id }): Path<LayerID>,
) -> Response {
    if let Some(layer) = layers.read().await.read_layer(&layer_id).cloned() {
        Json(layer).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchLayer(layer_id)),
        )
            .into_response()
    }
}

pub async fn remove_unused_layers(
    Extension(layers): Extension<Arc<RwLock<LayerStorage>>>,
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
) -> Response {
    let layers_in_use = stacks
        .read()
        .await
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let layers_unused = layers
        .read()
        .await
        .layer_ids()
        .filter(|id| !layers_in_use.contains(id))
        .copied()
        .collect::<Vec<_>>();
    let mut layers = layers.write().await;
    for layer in layers_unused {
        layers.remove_layer(&layer);
    }
    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct SetAtoms {
    offset: usize,
    atoms: Vec<Option<Atom3D>>,
}

pub async fn layer_set_atoms(
    Extension(layers): Extension<Arc<RwLock<LayerStorage>>>,
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
    Path(LayerID { layer_id }): Path<LayerID>,
    Json(set_atoms): Json<SetAtoms>,
) -> Response {
    if stacks
        .read()
        .await
        .iter()
        .any(|stack| stack.contains(&layer_id))
    {
        (
            StatusCode::BAD_REQUEST,
            Json(WorkspaceError::LayerInUse(layer_id)),
        )
            .into_response()
    } else if let Some(layer) = layers.write().await.write_layer(&layer_id) {
        match layer {
            Layer::Fill(molecule_layer) => {
                molecule_layer
                    .atoms
                    .set_atoms(set_atoms.offset, set_atoms.atoms);
                StatusCode::OK.into_response()
            }
            _ => (
                StatusCode::BAD_REQUEST,
                Json(WorkspaceError::NotFillLayer(layer_id)),
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchLayer(layer_id)),
        )
            .into_response()
    }
}

pub async fn layer_set_bonds(
    Extension(layers): Extension<Arc<RwLock<LayerStorage>>>,
    Extension(stacks): Extension<Arc<RwLock<Vec<Vec<usize>>>>>,
    Path(LayerID { layer_id }): Path<LayerID>,
    Json(set_bonds): Json<Vec<(usize, usize, Option<f64>)>>,
) -> Response {
    if stacks
        .read()
        .await
        .iter()
        .any(|stack| stack.contains(&layer_id))
    {
        (
            StatusCode::BAD_REQUEST,
            Json(WorkspaceError::LayerInUse(layer_id)),
        )
            .into_response()
    } else if let Some(layer) = layers.write().await.write_layer(&layer_id) {
        match layer {
            Layer::Fill(molecule_layer) => {
                for (a, b, bond) in set_bonds {
                    molecule_layer.bonds.set_bond(a, b, bond);
                }
                StatusCode::OK.into_response()
            }
            _ => (
                StatusCode::BAD_REQUEST,
                Json(WorkspaceError::NotFillLayer(layer_id)),
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchLayer(layer_id)),
        )
            .into_response()
    }
}

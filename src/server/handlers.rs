use std::{
    collections::{BTreeMap, BTreeSet},
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
    chemistry::MoleculeLayer,
    layer::{Layer, SelectOne},
    workspace::{LayerStorage, LayerStorageError, StackCache},
};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{AppState, WorkspaceName};

#[derive(Deserialize)]
pub struct WorkspaceCreation {
    name: String,
    import: Option<(LayerStorage, BTreeMap<String, Vec<usize>>)>,
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
    Extension(stacks): Extension<Arc<RwLock<BTreeMap<String, Vec<usize>>>>>,
) -> Json<(LayerStorage, BTreeMap<String, Vec<usize>>)> {
    let layers = layers.read().await;
    let stacks = stacks.read().await;
    Json((layers.deref().clone(), stacks.deref().clone()))
}

pub async fn remove_workspace(
    State(state): State<AppState>,
    Path(workspace): Path<WorkspaceName>,
) -> Response {
    let mut workspaces = state.write().await;
    if workspaces.remove(&workspace.name).is_some() {
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

#[derive(Deserialize)]
pub struct StackName {
    stack_name: String,
}

#[derive(Serialize, Debug)]
pub enum WorkspaceError {
    StackNameConflicted(String),
    NoSuchStack(String),
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

pub async fn read_stack(
    Extension(layers_storage): Extension<Arc<RwLock<LayerStorage>>>,
    Extension(stacks): Extension<Arc<RwLock<BTreeMap<String, Vec<usize>>>>>,
    Extension(stack_cache): Extension<Arc<RwLock<StackCache>>>,
    Path(stack): Path<StackName>,
) -> Result<Json<MoleculeLayer>, Json<WorkspaceError>> {
    let stacks = stacks.read().await;
    let stack_path = stacks
        .get(&stack.stack_name)
        .ok_or(Json(WorkspaceError::NoSuchStack(
            stack.stack_name.to_string(),
        )))?;
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

#[derive(Deserialize)]
pub struct CreateStack {
    name: String,
    path: Vec<usize>,
}

pub async fn create_stack(
    Extension(stacks): Extension<Arc<RwLock<BTreeMap<String, Vec<usize>>>>>,
    Json(create_stack): Json<CreateStack>,
) -> Response {
    let mut stacks = stacks.write().await;
    if stacks.contains_key(&create_stack.name) {
        (
            StatusCode::CONFLICT,
            Json(WorkspaceError::StackNameConflicted(create_stack.name)),
        )
            .into_response()
    } else {
        stacks.insert(create_stack.name, create_stack.path);
        StatusCode::OK.into_response()
    }
}

pub async fn clone_stacks(
    Extension(stacks): Extension<Arc<RwLock<BTreeMap<String, Vec<usize>>>>>,
    Path(stack_name): Path<String>,
    Json(copy_names): Json<Vec<String>>,
) -> Response {
    let mut stacks = stacks.write().await;
    if let Some(conflicted_name) = copy_names.iter().find(|name| stacks.contains_key(*name)) {
        (
            StatusCode::CONFLICT,
            Json(WorkspaceError::StackNameConflicted(
                conflicted_name.to_string(),
            )),
        )
            .into_response()
    } else if let Some(target_stack) = stacks.get(&stack_name).cloned() {
        stacks.extend(
            copy_names
                .into_iter()
                .map(|name| (name, target_stack.clone())),
        );
        StatusCode::OK.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchStack(stack_name)),
        )
            .into_response()
    }
}

pub async fn slice_stack(
    Extension(stacks): Extension<Arc<RwLock<BTreeMap<String, Vec<usize>>>>>,
    Path(stack_name): Path<String>,
    Json((start, end)): Json<(usize, usize)>,
) -> Response {
    if let Some(stack) = stacks.write().await.get_mut(&stack_name) {
        *stack = stack[start..end].to_vec();
        StatusCode::OK.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchStack(stack_name)),
        )
            .into_response()
    }
}

pub async fn add_layers(
    Extension(stacks): Extension<Arc<RwLock<BTreeMap<String, Vec<usize>>>>>,
    Path(stack_name): Path<String>,
    Json(layers): Json<Vec<usize>>,
) -> Response {
    if let Some(stack) = stacks.write().await.get_mut(&stack_name) {
        stack.extend(layers);
        StatusCode::OK.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(WorkspaceError::NoSuchStack(stack_name)),
        )
            .into_response()
    }
}

pub async fn read_layer(
    Extension(layers): Extension<Arc<RwLock<LayerStorage>>>,
    Path(layer_id): Path<usize>,
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
    Extension(stacks): Extension<Arc<RwLock<BTreeMap<String, Vec<usize>>>>>,
) -> Response {
    let layers_in_use = stacks
        .read()
        .await
        .values()
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

#[tokio::test]
async fn is_able_to_unlock() {
    let layers: Arc<RwLock<LayerStorage>> = Default::default();
    layers
        .write()
        .await
        .create_layers(vec![Default::default()].into_iter());
    let stacks: Arc<RwLock<BTreeMap<String, Vec<usize>>>> = Default::default();
    stacks.write().await.insert("example".to_string(), vec![1]);
    let caches: Arc<RwLock<StackCache>> = Default::default();
    println!("Generate response");
    let response = read_stack(
        Extension(layers),
        Extension(stacks),
        Extension(caches),
        Path(StackName {
            stack_name: "example".to_string(),
        }),
    )
    .await;
    println!("{:#?}", response.into_response());
}

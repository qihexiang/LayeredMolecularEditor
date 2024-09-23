use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
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

pub async fn create_workspace(
    State(state): State<AppState>,
    Json(workspace): Json<WorkspaceName>,
) -> Response {
    let name_confilct = state.read().await.contains_key(&workspace.name);
    if name_confilct {
        (
            StatusCode::CONFLICT,
            "Unable to create workspace, name already used.",
        )
            .into_response()
    } else {
        state
            .write()
            .await
            .insert(workspace.name, Default::default());
        StatusCode::OK.into_response()
    }
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
) -> StatusCode {
    let mut stacks = stacks.write().await;
    if stacks.contains_key(&create_stack.name) {
        StatusCode::CONFLICT
    } else {
        stacks.insert(create_stack.name, create_stack.path);
        StatusCode::OK
    }
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

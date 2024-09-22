use std::{collections::BTreeSet, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use lme::{
    chemistry::MoleculeLayer,
    workspace::{Workspace, WorkspaceError},
};

use serde::Deserialize;
use tokio::sync::RwLock;

use crate::{AppState, WorkspaceName};

pub async fn create_workspace(
    State(state): State<AppState>,
    Json(workspace): Json<WorkspaceName>,
) -> Response {
    let workspaces = state.read().await;
    if workspaces.contains_key(&workspace.name) {
        (
            StatusCode::CONFLICT,
            "Unable to create workspace, name already used.",
        )
            .into_response()
    } else {
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
    Extension(workspace): Extension<Arc<RwLock<Workspace>>>,
) -> Json<BTreeSet<String>> {
    Json(workspace.read().await.layers.keys().cloned().collect())
}

#[derive(Clone, Debug, Deserialize)]
pub struct StackName {
    name: String
}

pub async fn read_stack(
    Extension(workspace): Extension<Arc<RwLock<Workspace>>>,
    Path(stack): Path<StackName>
) -> Result<Json<MoleculeLayer>, Json<WorkspaceError>> {
    let mut workspace = workspace.write().await;
    let stack = workspace.read_stack(&stack.name).cloned().ok_or(err)
}

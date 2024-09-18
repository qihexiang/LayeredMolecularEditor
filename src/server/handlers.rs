use axum::{extract::{Path, State}, http::StatusCode, response::{IntoResponse, Response}, Json};

use crate::{AppState, WorkspaceName};

pub async fn create_workspace(State(state): State<AppState>, Json(workspace): Json<WorkspaceName>) -> Response {
    let workspaces = state.read().await;
    if workspaces.contains_key(&workspace.name) {
        (StatusCode::CONFLICT, "Unable to create workspace, name already used.").into_response()
    } else {
        StatusCode::OK.into_response()
    }
}

pub async fn remove_workspace(State(state): State<AppState>, Path(workspace): Path<WorkspaceName>) -> Response {
    let mut workspaces = state.write().await;
    if workspaces.remove(&workspace.name).is_some() {
        StatusCode::OK.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

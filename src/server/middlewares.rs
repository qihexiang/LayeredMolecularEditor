use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{AppState, WorkspaceName};

pub async fn workspace_middleware(
    State(state): State<AppState>,
    Path(workspace): Path<WorkspaceName>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some((layers, stacks, stack_cache)) = state.read().await.get(&workspace.name) {
        request.extensions_mut().insert(layers.clone());
        request.extensions_mut().insert(stacks.clone());
        request.extensions_mut().insert(stack_cache.clone());
        next.run(request).await
    } else {
        (StatusCode::NOT_FOUND, "No such workspace").into_response()
    }
}

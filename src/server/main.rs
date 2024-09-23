use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use handlers::{
    create_layers, create_stack, create_workspace, get_layers, read_stack, remove_workspace,
};
use lme::workspace::{LayerStorage, StackCache};
use middlewares::workspace_middleware;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;

mod handlers;
mod middlewares;

pub type AppState = Arc<
    RwLock<
        BTreeMap<
            String,
            (
                Arc<RwLock<LayerStorage>>,
                Arc<RwLock<BTreeMap<String, Vec<usize>>>>,
                Arc<RwLock<StackCache>>,
            ),
        >,
    >,
>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkspaceName {
    name: String,
}

#[tokio::main]
async fn main() {
    let server_state: AppState = Default::default();
    let workspace_router = Router::new()
        .route("/stacks/new", post(create_stack))
        .route("/layers/new", post(create_layers))
        .route("/layers", get(get_layers))
        .route("/stacks/:name", get(read_stack))
        .layer(middleware::from_fn_with_state(
            server_state.clone(),
            workspace_middleware,
        ));
    let app = Router::new()
        .nest("/workspace/:name", workspace_router)
        .route("/workspace", post(create_workspace))
        .route("/workspace/:name", delete(remove_workspace))
        .with_state(server_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

use axum::{
    middleware, routing::{delete, post}, Router
};
use handlers::{create_workspace, remove_workspace};
use lme::workspace::Workspace;
use middlewares::workspace_middleware;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::sync::RwLock;

mod handlers;
mod middlewares;

pub type AppState = Arc<RwLock<BTreeMap<String, Arc<Mutex<Workspace>>>>>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkspaceName {
    name: String
}

#[tokio::main]
async fn main() {
    let server_state: AppState = Default::default();
    let workspace_router = Router::new()
        .layer(middleware::from_fn_with_state(server_state.clone(), workspace_middleware));
    let app = Router::new()
        .nest("/workspace/:name", workspace_router)
        .route("/create_workspace", post(create_workspace))
        .route("/remove_workspace/:name", delete(remove_workspace))
        .with_state(server_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

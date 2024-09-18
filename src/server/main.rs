use axum::{routing::get, Json, Router};
use lme::chemistry::Atom3D;
use nalgebra::Point3;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "hello, world" }))
        .route(
            "/example",
            get(|| async {
                Json(Atom3D {
                    element: 1,
                    position: Point3::origin(),
                })
            }),
        );
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

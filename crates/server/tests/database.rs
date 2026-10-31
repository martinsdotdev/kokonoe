//! The server against a real Postgres. Each test starts its own in Docker through Testcontainers,
//! so `cargo test` needs Docker running, and the container goes when the test ends.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::postgres;
use sqlx::PgPool;
use tower::ServiceExt;

async fn health(pool: PgPool) -> StatusCode {
    let (app, _root) = common::app(pool);
    let request = Request::get("/healthz").body(Body::empty()).unwrap();
    app.oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn migrations_run_on_a_fresh_database_and_again_on_a_migrated_one() {
    let (_container, pool) = postgres().await;
    let citext: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT FROM pg_extension WHERE extname = 'citext')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(citext, "the first migration turns on citext");
    kokonoe_server::MIGRATOR
        .run(&pool)
        .await
        .expect("a restart runs nothing twice");
}

#[tokio::test]
async fn the_health_check_answers_while_the_database_does() {
    let (container, pool) = postgres().await;
    assert_eq!(health(pool.clone()).await, StatusCode::OK);
    container.stop().await.unwrap();
    assert_eq!(health(pool).await, StatusCode::SERVICE_UNAVAILABLE);
}

//! The server against a real Postgres. Each test starts its own in Docker through Testcontainers,
//! so `cargo test` needs Docker running, and the container goes when the test ends.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tower::ServiceExt;

/// A fresh Postgres, the one Fly runs, migrated as the server migrates at startup.
async fn postgres() -> (ContainerAsync<Postgres>, PgPool) {
    let container = Postgres::default()
        .with_tag("18-alpine")
        .start()
        .await
        .expect("the server's database tests start Postgres in Docker: is Docker running?");
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = kokonoe_server::connect(&url).await.unwrap();
    kokonoe_server::MIGRATOR.run(&pool).await.unwrap();
    (container, pool)
}

async fn health(pool: PgPool) -> StatusCode {
    let root = common::web_root(&[]);
    let app = kokonoe_server::app(pool, root.path()).unwrap();
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

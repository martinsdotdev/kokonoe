//! What the server's tests share: a stand-in for the built SPA, the site's origin, a pool that
//! never connects, and a real Postgres.

// Each test crate compiles its own copy and uses only part of it.
#![allow(dead_code)]

use std::fs;

use axum::http::HeaderValue;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tempfile::TempDir;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

/// The fallback page every stand-in build has.
pub const INDEX: &str =
    "<!doctype html><html><head><title>Kokonoe</title></head><body></body></html>";

/// Where the tests' pages are served from.
pub const ORIGIN: &str = "https://kokonoe.test";

/// [`ORIGIN`], as the app takes it.
pub fn origin() -> HeaderValue {
    HeaderValue::from_static(ORIGIN)
}

/// A build holding the fallback page and `files`, each a path and its text.
pub fn web_root(files: &[(&str, &[u8])]) -> TempDir {
    let root = tempfile::tempdir().unwrap();
    for (path, text) in [("200.html", INDEX.as_bytes())].iter().chain(files) {
        let file = root.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
    root
}

/// The app over `pool` and a build with only the fallback page, served from [`ORIGIN`].
pub fn app(pool: PgPool) -> (axum::Router, TempDir) {
    let root = web_root(&[]);
    let app = kokonoe_server::app(pool, root.path(), origin()).unwrap();
    (app, root)
}

/// A pool that connects only if something queries it.
pub fn lazy_pool() -> PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://nobody@localhost/nothing")
        .unwrap()
}

/// A fresh Postgres in Docker, the one Fly runs, migrated as the server migrates at startup. It
/// goes when the container is dropped, so `cargo test` needs Docker running.
pub async fn postgres() -> (ContainerAsync<Postgres>, PgPool) {
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

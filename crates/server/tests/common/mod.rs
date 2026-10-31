//! What the server's tests share: a stand-in for the built SPA, and a pool that never connects.

// Each test crate compiles its own copy and uses only part of it.
#![allow(dead_code)]

use std::fs;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tempfile::TempDir;

/// The fallback page every stand-in build has.
pub const INDEX: &str =
    "<!doctype html><html><head><title>Kokonoe</title></head><body></body></html>";

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

/// A pool that connects only if something queries it, which only the health check does.
pub fn lazy_pool() -> PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://nobody@localhost/nothing")
        .unwrap()
}

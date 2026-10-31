//! Runs the server: `PORT` (8080 unless set), `DATABASE_URL`, `WEB_ROOT`, the built SPA
//! (`web/build` unless set), and `ORIGIN`, where its pages are served from, such as
//! `https://kokonoe.app` (`http://localhost:<PORT>` unless set). It runs the migrations before it
//! serves.

use std::env;
use std::error::Error;
use std::path::Path;

use axum::http::HeaderValue;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let port: u16 = match env::var("PORT") {
        Ok(port) => port
            .parse()
            .map_err(|_| format!("PORT is {port}, not a port"))?,
        Err(_) => 8080,
    };
    let url = env::var("DATABASE_URL").map_err(|_| "DATABASE_URL isn't set")?;
    let web_root = env::var("WEB_ROOT").unwrap_or_else(|_| "web/build".to_string());
    let origin = env::var("ORIGIN").unwrap_or_else(|_| format!("http://localhost:{port}"));
    // A browser's `Origin` never ends in a slash.
    let origin = origin.trim_end_matches('/');
    if !origin.starts_with("http://") && !origin.starts_with("https://") {
        return Err(format!("ORIGIN is {origin}, not an http:// or https:// origin").into());
    }

    let pool = kokonoe_server::connect(&url).await?;
    kokonoe_server::MIGRATOR.run(&pool).await?;
    let app = kokonoe_server::app(pool, Path::new(&web_root), HeaderValue::from_str(origin)?)?;
    // Every interface, as a container's port mapping needs.
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("serving {web_root} for {origin} on port {port}");
    axum::serve(listener, app).await?;
    Ok(())
}

//! The Kokonoe server (spec §8). It serves the built SPA, so the app and its API share one origin
//! and need no CORS: every page, a link preview for invite Rooms, and a health check.

use std::path::Path;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::handler::HandlerWithoutStateExt;
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tower_http::services::ServeDir;

/// The schema, run at startup: `migrations/`, built into the binary.
pub static MIGRATOR: Migrator = sqlx::migrate!();

/// How long a query waits for a connection. sqlx's 30 s would hold a health check past the
/// platform's own deadline when the database is down.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);

/// How long the health check waits for the database to answer once connected.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);

/// A pool for `url`, connected once to check it answers.
pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect(url)
        .await
}

/// What a link to an invite Room shows where it's shared: a plain invitation, which names the
/// game once Rooms exist (KOK-56).
const PREVIEW: &str = r#"<meta property="og:type" content="website" />
<meta property="og:site_name" content="Kokonoe" />
<meta property="og:title" content="Join a game of Kokonoe" />
<meta property="og:description" content="You’re invited to a game of Ultimate Tic-Tac-Toe." />
<meta name="twitter:card" content="summary" />
"#;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    /// The SPA's fallback page with the Room preview in its head.
    room_page: Bytes,
}

/// The app: the SPA from `web_root`, as adapter-static built it, precompressed. Every path that
/// names no file gets the fallback page `200.html`, so deep links resolve; a missing file gets a
/// 404, never the page in its place.
pub fn app(pool: PgPool, web_root: &Path) -> std::io::Result<Router> {
    let fallback = web_root.join("200.html");
    let unreadable = |why: &dyn std::fmt::Display| {
        std::io::Error::other(format!("{}: {why}", fallback.display()))
    };
    let index = std::fs::read_to_string(&fallback).map_err(|error| unreadable(&error))?;
    let Some(head) = index.find("</head>") else {
        return Err(unreadable(&"it has no </head>"));
    };
    let room_page = format!("{}{PREVIEW}{}", &index[..head], &index[head..]).into();
    let page = Bytes::from(index);
    let pages = move |uri: Uri| async move { page_or_missing(&uri, page) };
    let spa = ServeDir::new(web_root)
        .precompressed_br()
        .precompressed_gzip()
        .fallback(pages.into_service());
    Ok(Router::new()
        .route("/healthz", get(health))
        .route("/r/{room}", get(room))
        .fallback_service(spa)
        .layer(middleware::from_fn(cache))
        .with_state(AppState { pool, room_page }))
}

/// The fallback page for a page's path, or a 404 for a file's: anything under `/_app/`, or with
/// an extension. A stale tab asking for a chunk a deploy removed must fail, not get the page as
/// the chunk, which a browser would then keep.
fn page_or_missing(uri: &Uri, page: Bytes) -> Response {
    let path = uri.path();
    let file = path.starts_with("/_app/")
        || path
            .rsplit('/')
            .next()
            .is_some_and(|last| last.contains('.'));
    if file {
        let never = [(header::CACHE_CONTROL, "no-store")];
        return (StatusCode::NOT_FOUND, never, "not found").into_response();
    }
    Html(page).into_response()
}

/// 200 while the database answers, else 503.
async fn health(State(state): State<AppState>) -> (StatusCode, &'static str) {
    let answer = tokio::time::timeout(HEALTH_TIMEOUT, sqlx::query("SELECT 1").execute(&state.pool));
    match answer.await {
        Ok(Ok(_)) => (StatusCode::OK, "ok"),
        Ok(Err(error)) => {
            tracing::warn!(%error, "the database didn't answer the health check");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "the database is unreachable",
            )
        }
        Err(_) => {
            tracing::warn!("the database took too long to answer the health check");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "the database is unreachable",
            )
        }
    }
}

/// An invite Room's link: the SPA, which opens the Room, with the preview for where it's shared.
async fn room(State(state): State<AppState>) -> Html<Bytes> {
    Html(state.room_page)
}

/// How long a browser keeps what it's sent. Vite's hashed files never change, so they're kept for
/// a year. The pages, the service worker and the build's version are checked every time, so a
/// deploy reaches the next load.
async fn cache(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let mut response = next.run(request).await;
    let status = response.status();
    if !status.is_success() && status != StatusCode::NOT_MODIFIED {
        return response;
    }
    let html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|kind| kind.as_bytes().starts_with(b"text/html"));
    let policy = if html || path == "/service-worker.js" || path == "/_app/version.json" {
        "no-cache"
    } else if path.starts_with("/_app/immutable/") {
        "public, max-age=31536000, immutable"
    } else {
        return response;
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    response
}

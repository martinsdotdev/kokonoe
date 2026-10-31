//! The Kokonoe server (spec §8). It serves the built SPA, so the app and its API share one origin
//! and need no CORS: every page, a link preview for invite Rooms, a health check, Guest sessions
//! and the game socket.

mod session;

use std::path::Path;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::handler::HandlerWithoutStateExt;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::{AppendHeaders, Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use kokonoe_protocol::CloseCode;
use serde::Serialize;
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
    /// The site's own origin, such as `https://kokonoe.app`: the one `Origin` that a request
    /// changing something, or opening the socket, may come from.
    origin: HeaderValue,
}

/// The app: the SPA from `web_root`, as adapter-static built it, precompressed. Every path that
/// names no file gets the fallback page `200.html`, so deep links resolve; a missing file gets a
/// 404, never the page in its place. `origin` is where the pages are served from.
pub fn app(pool: PgPool, web_root: &Path, origin: HeaderValue) -> std::io::Result<Router> {
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
    let state = AppState {
        pool,
        room_page,
        origin,
    };
    Ok(Router::new()
        .route("/healthz", get(health))
        .route("/r/{room}", get(room))
        .route("/api/session", post(session))
        .route("/api/socket", get(socket))
        .fallback_service(spa)
        .layer(middleware::from_fn_with_state(state.clone(), same_origin))
        .layer(middleware::from_fn(cache))
        .with_state(state))
}

/// The fallback page for a page's path, or a 404 for a file's: anything under `/_app/` or
/// `/api/`, or with an extension. A stale tab asking for a chunk a deploy removed must fail, not
/// get the page as the chunk, which a browser would then keep.
fn page_or_missing(uri: &Uri, page: Bytes) -> Response {
    let path = uri.path();
    let file = path.starts_with("/_app/")
        || path.starts_with("/api/")
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
            UNREACHABLE
        }
        Err(_) => {
            tracing::warn!("the database took too long to answer the health check");
            UNREACHABLE
        }
    }
}

/// An invite Room's link: the SPA, which opens the Room, with the preview for where it's shared.
async fn room(State(state): State<AppState>) -> Html<Bytes> {
    Html(state.room_page)
}

/// What the SPA learns about whoever's playing.
#[derive(Serialize)]
struct Me {
    handle: String,
}

/// Starts a session, or resumes the one the cookie names. Without a live one, a new Guest
/// Account is made for it, so a cleared or expired cookie starts a new Guest. The cookie is sent
/// again only when the session's expiry moves.
async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Unavailable> {
    let (current, send) = match session::find(&state.pool, &headers).await? {
        Some(found) if found.due => {
            session::roll(&state.pool, &found).await?;
            (found, true)
        }
        Some(found) => (found, false),
        None => (session::start(&state.pool).await?, true),
    };
    let cookie = send.then(|| (header::SET_COOKIE, current.cookie()));
    Ok((
        AppendHeaders(cookie),
        Json(Me {
            handle: current.handle,
        }),
    ))
}

/// The game socket. One opened from another site, or without saying where it's from, is refused
/// outright. One without a session is accepted and then closed with 4003, since a page can't read
/// why a refused upgrade failed but can read a close code. One with a session stays open,
/// answering pings, until Rooms give it something to say (KOK-56). It doesn't move the session's
/// expiry: a socket has no cookie to send back.
async fn socket(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response, Unavailable> {
    if !from_here(&headers, &state.origin) {
        return Ok(not_from_here());
    }
    let found = session::find(&state.pool, &headers).await?;
    Ok(upgrade.on_upgrade(move |socket| async move {
        match found {
            Some(_) => hold(socket).await,
            None => forbid(socket).await,
        }
    }))
}

async fn hold(mut socket: WebSocket) {
    while let Some(Ok(_)) = socket.recv().await {}
}

async fn forbid(mut socket: WebSocket) {
    let close = CloseFrame {
        code: CloseCode::Forbidden.code(),
        reason: "no session".into(),
    };
    let _ = socket.send(Message::Close(Some(close))).await;
}

/// Refuses a request that could change something unless it comes from the site's own pages.
/// SameSite=Lax keeps the cookie off most cross-site requests but not all, so anything but a GET
/// or HEAD from elsewhere, or one that doesn't say where it's from, is refused before it runs.
/// It covers every route, so a new one can't forget it.
async fn same_origin(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let reads = matches!(*request.method(), Method::GET | Method::HEAD);
    if !reads && !from_here(request.headers(), &state.origin) {
        return not_from_here();
    }
    next.run(request).await
}

fn from_here(headers: &HeaderMap, origin: &HeaderValue) -> bool {
    headers.get(header::ORIGIN) == Some(origin)
}

fn not_from_here() -> Response {
    (StatusCode::FORBIDDEN, "not from this site").into_response()
}

/// The site's origin as a browser sends it in `Origin`, from `ORIGIN`: `http` or `https` and a
/// host, maybe with a port, and nothing after it. Browsers send the scheme and host in lowercase
/// and leave out a default port, so they're written that way here; anything else would match no
/// request, and every POST would be refused.
pub fn origin(text: &str) -> Result<HeaderValue, String> {
    let refused = || format!("{text:?} isn't an origin such as https://kokonoe.app");
    let uri: Uri = text.parse().map_err(|_| refused())?;
    let (Some(scheme), Some(authority)) = (uri.scheme_str(), uri.authority()) else {
        return Err(refused());
    };
    let default_port = match scheme {
        "http" => 80,
        "https" => 443,
        _ => return Err(refused()),
    };
    if authority.as_str().contains('@') || uri.path() != "/" || uri.query().is_some() {
        return Err(refused());
    }
    let host = authority.host().to_ascii_lowercase();
    let origin = match authority.port_u16() {
        Some(port) if port != default_port => format!("{scheme}://{host}:{port}"),
        _ => format!("{scheme}://{host}"),
    };
    HeaderValue::try_from(origin).map_err(|_| refused())
}

/// What a request gets while the database doesn't answer.
const UNREACHABLE: (StatusCode, &str) = (
    StatusCode::SERVICE_UNAVAILABLE,
    "the database is unreachable",
);

/// The database failing a request: a 503, logged.
struct Unavailable(sqlx::Error);

impl From<sqlx::Error> for Unavailable {
    fn from(error: sqlx::Error) -> Unavailable {
        Unavailable(error)
    }
}

impl IntoResponse for Unavailable {
    fn into_response(self) -> Response {
        tracing::warn!(error = %self.0, "the database failed a request");
        UNREACHABLE.into_response()
    }
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

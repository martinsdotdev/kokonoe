//! What the server serves, on a stand-in for the built SPA. Nothing here reaches a database: the
//! pool is lazy, and only the health check would connect.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{INDEX, lazy_pool, web_root};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

/// The brotli copy adapter-static writes beside a file, standing in for real brotli.
const BROTLI: &[u8] = b"brotli bytes";

/// The app over a build with a file of each kind it serves.
fn app() -> (Router, TempDir) {
    let root = web_root(&[
        ("_app/immutable/chunks/app.B1x2.js", b"export {};"),
        ("_app/immutable/chunks/app.B1x2.js.br", BROTLI),
        ("_app/immutable/assets/kokonoe_wasm_bg.C3d4.wasm", b"\0asm"),
        ("_app/version.json", br#"{"version":"1"}"#),
        ("service-worker.js", b"self;"),
        ("robots.txt", b"User-agent: *"),
    ]);
    (kokonoe_server::app(lazy_pool(), root.path()).unwrap(), root)
}

struct Answer {
    status: StatusCode,
    kind: Option<String>,
    cache: Option<String>,
    encoding: Option<String>,
    body: Vec<u8>,
}

impl Answer {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

async fn get(app: &Router, uri: &str) -> Answer {
    fetch(app, Request::get(uri).body(Body::empty()).unwrap()).await
}

async fn fetch(app: &Router, request: Request<Body>) -> Answer {
    let response = app.clone().oneshot(request).await.unwrap();
    let header = |name| {
        response
            .headers()
            .get(name)
            .map(|value: &axum::http::HeaderValue| value.to_str().unwrap().to_string())
    };
    let (status, kind, cache, encoding) = (
        response.status(),
        header(header::CONTENT_TYPE),
        header(header::CACHE_CONTROL),
        header(header::CONTENT_ENCODING),
    );
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        kind,
        cache,
        encoding,
        body,
    }
}

#[tokio::test]
async fn a_deep_link_gets_the_spa() {
    let (app, _root) = app();
    for uri in ["/", "/play?rules=d3", "/replay", "/u/someone/games"] {
        let answer = get(&app, uri).await;
        assert_eq!(answer.status, StatusCode::OK, "{uri}");
        assert_eq!(answer.text(), INDEX, "{uri}");
        assert_eq!(answer.cache.as_deref(), Some("no-cache"), "{uri}");
    }
}

#[tokio::test]
async fn a_missing_file_is_missing_and_never_kept() {
    let (app, _root) = app();
    // A stale tab asking for a chunk a deploy removed, and a file no build has.
    for uri in [
        "/_app/immutable/chunks/old.Z9y8.js",
        "/favicon.png",
        "/_app/",
    ] {
        let answer = get(&app, uri).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(answer.cache.as_deref(), Some("no-store"), "{uri}");
        assert_ne!(answer.text(), INDEX, "{uri}: not the page in its place");
    }
}

#[tokio::test]
async fn a_room_link_has_a_preview_for_where_it_is_shared() {
    let (app, _root) = app();
    let answer = get(&app, "/r/abc").await;
    let page = answer.text();
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.kind.unwrap().starts_with("text/html"));
    assert!(page.contains(r#"<meta property="og:title" content="Join a game of Kokonoe" />"#));
    assert!(page.contains(r#"<meta name="twitter:card" content="summary" />"#));
    assert!(
        page.find("og:title").unwrap() < page.find("</head>").unwrap(),
        "in the head"
    );
    assert_eq!(answer.cache.as_deref(), Some("no-cache"));

    let plain = get(&app, "/play").await;
    assert!(
        !plain.text().contains("og:title"),
        "only Room links have it"
    );
}

#[tokio::test]
async fn hashed_files_are_kept_a_year_and_the_rest_checked() {
    let (app, _root) = app();
    let script = get(&app, "/_app/immutable/chunks/app.B1x2.js").await;
    assert_eq!(script.status, StatusCode::OK);
    assert_eq!(
        script.cache.as_deref(),
        Some("public, max-age=31536000, immutable")
    );
    for uri in ["/service-worker.js", "/_app/version.json"] {
        assert_eq!(
            get(&app, uri).await.cache.as_deref(),
            Some("no-cache"),
            "{uri}"
        );
    }
    assert_eq!(get(&app, "/robots.txt").await.cache, None);
}

#[tokio::test]
async fn a_browser_that_takes_brotli_gets_the_build_s_copy() {
    let (app, _root) = app();
    let request = Request::get("/_app/immutable/chunks/app.B1x2.js")
        .header(header::ACCEPT_ENCODING, "br, gzip")
        .body(Body::empty())
        .unwrap();
    let answer = fetch(&app, request).await;
    assert_eq!(answer.encoding.as_deref(), Some("br"));
    assert_eq!(answer.body, BROTLI);
    assert!(
        answer.kind.unwrap().contains("javascript"),
        "still typed as the script"
    );
}

#[tokio::test]
async fn the_engine_goes_out_as_wasm() {
    let (app, _root) = app();
    let wasm = get(&app, "/_app/immutable/assets/kokonoe_wasm_bg.C3d4.wasm").await;
    assert_eq!(wasm.status, StatusCode::OK);
    // The browser compiles it as it streams only with this type.
    assert_eq!(wasm.kind.as_deref(), Some("application/wasm"));
}

#[tokio::test]
async fn a_build_without_the_fallback_page_is_refused_by_name() {
    let root = tempfile::tempdir().unwrap();
    let error = kokonoe_server::app(lazy_pool(), root.path()).unwrap_err();
    assert!(error.to_string().contains("200.html"), "{error}");
}

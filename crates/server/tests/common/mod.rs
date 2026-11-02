//! What the server's tests share: a stand-in for the built SPA, the site's origin, a pool that
//! never connects, a real Postgres, Guests, a server on its own runtime, and its sockets.

// Each test crate compiles its own copy and uses only part of it.
#![allow(dead_code)]

pub mod table;

use std::fs;
use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, header};
use futures_util::{SinkExt, StreamExt};
use kokonoe_protocol::{ClientMsg, ServerMsg};
use kokonoe_server::Timing;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tempfile::TempDir;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tokio::runtime::Runtime;
use tokio_tungstenite::tungstenite::{self, Message, client::IntoClientRequest};
use tower::ServiceExt;

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
    let app = kokonoe_server::app(pool, root.path(), origin(), Timing::default()).unwrap();
    (app, root)
}

/// A pool that connects only if something queries it, to a port nothing listens on, so a query
/// fails within half a second.
pub fn lazy_pool() -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(500))
        .connect_lazy("postgres://nobody@127.0.0.1:1/nothing")
        .unwrap()
}

/// A Postgres in Docker, gone when dropped.
pub type Container = ContainerAsync<Postgres>;

/// A fresh Postgres in Docker, the one Fly runs, migrated as the server migrates at startup. It
/// goes when the container is dropped, so `cargo test` needs Docker running.
pub async fn postgres() -> (Container, PgPool) {
    let (container, url) = database().await;
    (container, kokonoe_server::connect(&url).await.unwrap())
}

/// [`postgres`], as the URL a server started on its own connects to.
pub async fn database() -> (Container, String) {
    let container = Postgres::default()
        .with_tag("18-alpine")
        .start()
        .await
        .expect("the server's database tests start Postgres in Docker: is Docker running?");
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = kokonoe_server::connect(&url).await.unwrap();
    kokonoe_server::MIGRATOR.run(&pool).await.unwrap();
    pool.close().await;
    (container, url)
}

/// A new Guest's session token, the `id.secret` its cookie holds.
pub async fn guest(app: &Router) -> String {
    let request = Request::post("/api/session")
        .header(header::ORIGIN, ORIGIN)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let set = response.headers()[header::SET_COOKIE].to_str().unwrap();
    let value = set.strip_prefix("__Host-session=").unwrap();
    value.split(';').next().unwrap().to_owned()
}

/// How long things take on a test server. Its offers wait long enough to answer, short enough to
/// watch lapse, and apart, so a takeback can't wait as a rematch does. Its time controls run in
/// a fifth of a second a minute and 40 ms a second, so 3+2 is a 600 ms bank and an 80 ms
/// increment, and a timed Room's first move has 500 ms.
pub const TIMING: Timing = Timing {
    takeback: Duration::from_millis(400),
    rematch: Duration::from_millis(900),
    abort: Duration::from_millis(500),
    minute: Duration::from_millis(200),
    second: Duration::from_millis(40),
};

/// The server, listening on its own runtime as a deployed one would, so [`Server::kill`] can stop
/// everything it runs at once, as a crash does.
pub struct Server {
    pub address: SocketAddr,
    runtime: Option<Runtime>,
    _root: TempDir,
}

impl Server {
    pub async fn start(url: &str) -> Server {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let root = web_root(&[]);
        let (url, path) = (url.to_owned(), root.path().to_owned());
        let address = runtime
            .spawn(async move {
                let pool = kokonoe_server::connect(&url).await.unwrap();
                let app = kokonoe_server::app(pool, &path, origin(), TIMING).unwrap();
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
                address
            })
            .await
            .unwrap();
        Server {
            address,
            runtime: Some(runtime),
            _root: root,
        }
    }

    /// Stops every task the server runs, its Rooms and sockets included, without a goodbye.
    pub fn kill(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.kill();
    }
}

pub type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The `Cookie` header holding the session `token`.
pub fn cookie(token: &str) -> String {
    format!("__Host-session={token}")
}

/// The id of the session `token` holds, as `sessions` keeps it.
pub fn session_id(token: &str) -> &str {
    token.split_once('.').unwrap().0
}

/// A socket to `address`, from `origin` and holding the session `token`, if they're given.
pub async fn try_open(
    address: SocketAddr,
    origin: Option<&str>,
    token: Option<&str>,
) -> Result<Socket, tungstenite::Error> {
    let mut request = format!("ws://{address}/api/socket")
        .into_client_request()
        .unwrap();
    let headers = request.headers_mut();
    if let Some(origin) = origin {
        headers.insert(header::ORIGIN, origin.parse().unwrap());
    }
    if let Some(token) = token {
        headers.insert(header::COOKIE, cookie(token).parse().unwrap());
    }
    Ok(tokio_tungstenite::connect_async(request).await?.0)
}

/// A socket to `address` from the site's own page, holding the session `token`.
pub async fn open(address: SocketAddr, token: &str) -> Socket {
    try_open(address, Some(ORIGIN), Some(token)).await.unwrap()
}

/// Sends `message` as the page would.
pub async fn say(socket: &mut Socket, message: &ClientMsg) {
    let text = serde_json::to_string(message).unwrap();
    socket.send(Message::Text(text.into())).await.unwrap();
}

/// The next frame, within 5 s.
pub async fn next(socket: &mut Socket) -> Message {
    let next = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .expect("the server answers in time");
    next.unwrap().unwrap()
}

/// The next frame that isn't a ping or a pong.
pub async fn frame(socket: &mut Socket) -> Message {
    loop {
        match next(socket).await {
            Message::Ping(_) | Message::Pong(_) => {}
            frame => return frame,
        }
    }
}

/// The next message the server sends.
pub async fn hear(socket: &mut Socket) -> ServerMsg {
    match frame(socket).await {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("not a message: {other:?}"),
    }
}

/// The code the server closes the socket with next.
pub async fn closed(socket: &mut Socket) -> u16 {
    match frame(socket).await {
        Message::Close(Some(close)) => close.code.into(),
        other => panic!("not closed: {other:?}"),
    }
}

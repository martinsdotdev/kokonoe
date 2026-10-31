//! Guest Accounts and their sessions (spec §9), and the socket they open, against a real Postgres
//! in Docker.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{ORIGIN, postgres};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error, Message};
use tower::ServiceExt;

/// What `POST /api/session` answered.
struct Started {
    handle: String,
    /// The `Set-Cookie` header, when the session was started or rolled.
    set: Option<String>,
}

impl Started {
    /// The `id.secret` the cookie keeps.
    fn token(&self) -> String {
        let set = self.set.as_deref().expect("a cookie was sent");
        let value = set.strip_prefix("__Host-session=").unwrap();
        value.split(';').next().unwrap().to_owned()
    }
}

async fn start(app: &Router, token: Option<&str>) -> Started {
    let mut request = Request::post("/api/session").header(header::ORIGIN, ORIGIN);
    if let Some(token) = token {
        request = request.header(
            header::COOKIE,
            format!("theme=dark; __Host-session={token}"),
        );
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let set = response
        .headers()
        .get(header::SET_COOKIE)
        .map(|value| value.to_str().unwrap().to_owned());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let me: serde_json::Value = serde_json::from_slice(&body).unwrap();
    Started {
        handle: me["handle"].as_str().unwrap().to_owned(),
        set,
    }
}

async fn accounts(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM accounts")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Whether the session expires a full 30 days from now, give or take the test's own time.
async fn expires_in_full(pool: &PgPool) -> bool {
    sqlx::query_scalar("SELECT expires_at > now() + interval '29 days 23 hours' FROM sessions")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_first_visit_starts_a_guest_whose_secret_is_never_stored() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let guest = start(&app, None).await;

    let number: u16 = guest
        .handle
        .strip_prefix("guest-")
        .unwrap()
        .parse()
        .unwrap();
    assert!((1000..=9999).contains(&number), "{}", guest.handle);
    let set = guest.set.as_deref().unwrap();
    for part in [
        "Max-Age=2592000",
        "Path=/",
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
    ] {
        assert!(set.split("; ").any(|attribute| attribute == part), "{set}");
    }

    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((accounts(&pool).await, sessions), (1, 1));
    let (id, secret_hash, handle): (String, Vec<u8>, String) = sqlx::query_as(
        "SELECT s.id, s.secret_hash, a.handle::text FROM sessions s JOIN accounts a ON a.id = s.account_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let token = guest.token();
    let (sent_id, secret) = token.split_once('.').unwrap();
    assert_eq!(
        (id.as_str(), handle.as_str()),
        (sent_id, guest.handle.as_str())
    );
    assert_eq!(secret_hash, Sha256::digest(secret).to_vec());
    assert_eq!(
        (sent_id.len(), secret.len()),
        (32, 64),
        "16 and 32 random bytes, in hex"
    );
    assert!(expires_in_full(&pool).await);
}

#[tokio::test]
async fn a_reload_resumes_the_guest_without_writing() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let guest = start(&app, None).await;
    let again = start(&app, Some(&guest.token())).await;
    assert_eq!(again.handle, guest.handle);
    assert_eq!(again.set, None, "a fresh session isn't sent again");
    assert_eq!(accounts(&pool).await, 1);
}

#[tokio::test]
async fn a_session_rolls_forward_at_most_once_a_day() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let guest = start(&app, None).await;
    let token = guest.token();
    sqlx::query("UPDATE sessions SET expires_at = now() + interval '20 days'")
        .execute(&pool)
        .await
        .unwrap();

    let rolled = start(&app, Some(&token)).await;
    assert_eq!(rolled.handle, guest.handle);
    assert_eq!(rolled.token(), token, "the same session, kept for longer");
    assert!(rolled.set.unwrap().contains("Max-Age=2592000"));
    assert!(expires_in_full(&pool).await);
    assert_eq!(
        start(&app, Some(&token)).await.set,
        None,
        "not again the same day"
    );
}

#[tokio::test]
async fn a_lost_or_stale_cookie_starts_a_new_guest() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let guest = start(&app, None).await;
    let token = guest.token();
    let (id, _) = token.split_once('.').unwrap();
    let wrong_secret = format!("{id}.{}", "0".repeat(64));
    let unknown = format!("{}.{}", "0".repeat(32), "0".repeat(64));

    // No cookie is what clearing site data leaves.
    let mut guests = 1;
    for cookie in [
        None,
        Some(wrong_secret.as_str()),
        Some(unknown.as_str()),
        Some("crumbs"),
    ] {
        let next = start(&app, cookie).await;
        assert_ne!(next.token(), token, "{cookie:?}");
        guests += 1;
        assert_eq!(accounts(&pool).await, guests, "{cookie:?}");
    }

    sqlx::query("UPDATE sessions SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_ne!(start(&app, Some(&token)).await.token(), token, "expired");
    assert_eq!(accounts(&pool).await, guests + 1);
}

/// The app on a real port, since a socket needs a real connection to upgrade.
async fn serve(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    address
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn open(
    address: SocketAddr,
    origin: Option<&str>,
    token: Option<&str>,
) -> Result<Socket, Error> {
    let mut request = format!("ws://{address}/api/socket")
        .into_client_request()
        .unwrap();
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
    }
    if let Some(token) = token {
        let cookie = format!("__Host-session={token}").parse().unwrap();
        request.headers_mut().insert(header::COOKIE, cookie);
    }
    let (socket, _) = tokio_tungstenite::connect_async(request).await?;
    Ok(socket)
}

async fn next(socket: &mut Socket) -> Message {
    tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .expect("the server answers")
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn the_socket_takes_only_a_session_from_this_site() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool);
    let token = start(&app, None).await.token();
    let address = serve(app).await;

    for origin in [Some("https://evil.test"), None] {
        match open(address, origin, Some(&token)).await {
            Err(Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin:?}")
            }
            other => panic!("{origin:?} upgraded: {:?}", other.map(|_| ())),
        }
    }

    for stale in [None, Some("crumbs")] {
        let mut socket = open(address, Some(ORIGIN), stale).await.unwrap();
        let Message::Close(Some(close)) = next(&mut socket).await else {
            panic!("{stale:?} wasn't closed");
        };
        assert_eq!(u16::from(close.code), 4003, "{stale:?}");
    }

    let mut socket = open(address, Some(ORIGIN), Some(&token)).await.unwrap();
    socket
        .send(Message::Ping("still there?".into()))
        .await
        .unwrap();
    assert_eq!(
        next(&mut socket).await,
        Message::Pong("still there?".into())
    );
}

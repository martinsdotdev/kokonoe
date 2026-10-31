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

/// `POST /api/session` from the site's own page, with these `Cookie` headers.
async fn post(app: &Router, cookies: &[&str]) -> Started {
    let mut request = Request::post("/api/session").header(header::ORIGIN, ORIGIN);
    for cookies in cookies {
        request = request.header(header::COOKIE, *cookies);
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

/// `POST /api/session` holding `token`, if any. Another cookie comes first, as a browser's
/// would, so the session's is found among others.
async fn start(app: &Router, token: Option<&str>) -> Started {
    match token {
        Some(token) => post(app, &[&format!("theme=dark; __Host-session={token}")]).await,
        None => post(app, &[]).await,
    }
}

async fn accounts(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM accounts")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// When the session `token` names expires, to compare before and after.
async fn expiry(pool: &PgPool, token: &str) -> String {
    let (id, _) = token.split_once('.').unwrap();
    sqlx::query_scalar("SELECT expires_at::text FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Sets the session `token` names to expire `when`, a Postgres interval from now.
async fn expire(pool: &PgPool, token: &str, when: &str) {
    let (id, _) = token.split_once('.').unwrap();
    sqlx::query("UPDATE sessions SET expires_at = now() + $2::interval WHERE id = $1")
        .bind(id)
        .bind(when)
        .execute(pool)
        .await
        .unwrap();
}

/// Whether the session `token` names expires a full 30 days from now, give or take the test's
/// own time.
async fn expires_in_full(pool: &PgPool, token: &str) -> bool {
    let (id, _) = token.split_once('.').unwrap();
    sqlx::query_scalar(
        "SELECT expires_at > now() + interval '29 days 23 hours' FROM sessions WHERE id = $1",
    )
    .bind(id)
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
    assert!(expires_in_full(&pool, &token).await);
}

#[tokio::test]
async fn a_reload_resumes_the_guest_without_writing() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let guest = start(&app, None).await;
    let token = guest.token();
    let before = expiry(&pool, &token).await;
    let again = start(&app, Some(&token)).await;
    assert_eq!(again.handle, guest.handle);
    assert_eq!(again.set, None, "a fresh session isn't sent again");
    assert_eq!(expiry(&pool, &token).await, before, "nor moved");
    assert_eq!(accounts(&pool).await, 1);
}

#[tokio::test]
async fn the_session_cookie_is_found_by_its_exact_name_in_any_cookie_header() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let guest = start(&app, None).await;
    let token = guest.token();

    // HTTP/2 can send each cookie in a header of its own.
    let split = post(&app, &["theme=dark", &format!("__Host-session={token}")]).await;
    assert_eq!((split.handle, split.set), (guest.handle, None));
    assert_eq!(accounts(&pool).await, 1);

    let lookalike = post(&app, &[&format!("__Host-sessions={token}")]).await;
    assert_ne!(
        lookalike.token(),
        token,
        "another cookie's name isn't the session's"
    );
    assert_eq!(accounts(&pool).await, 2);
}

#[tokio::test]
async fn a_session_rolls_forward_at_most_once_a_day() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let guest = start(&app, None).await;
    let token = guest.token();
    expire(&pool, &token, "20 days").await;

    let rolled = start(&app, Some(&token)).await;
    assert_eq!(rolled.handle, guest.handle);
    assert_eq!(rolled.token(), token, "the same session, kept for longer");
    assert!(rolled.set.unwrap().contains("Max-Age=2592000"));
    assert!(expires_in_full(&pool, &token).await);
    assert_eq!(
        start(&app, Some(&token)).await.set,
        None,
        "not again the same day"
    );
}

#[tokio::test]
async fn a_lost_or_unknown_cookie_starts_a_new_guest() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let token = start(&app, None).await.token();
    let (id, _) = token.split_once('.').unwrap();
    let wrong_secret = format!("{id}.{}", "0".repeat(64));
    let unknown_id = format!("{}.{}", "0".repeat(32), "0".repeat(64));

    // No cookie is what clearing site data leaves.
    let cookies = [
        None,
        Some(&*wrong_secret),
        Some(&*unknown_id),
        Some("crumbs"),
    ];
    for (made, cookie) in (2..).zip(cookies) {
        let next = start(&app, cookie).await;
        assert_ne!(next.token(), token, "{cookie:?}");
        assert_eq!(accounts(&pool).await, made, "{cookie:?}");
    }
}

#[tokio::test]
async fn an_expired_session_starts_a_new_guest() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let token = start(&app, None).await.token();
    expire(&pool, &token, "-1 second").await;
    assert_ne!(start(&app, Some(&token)).await.token(), token);
    assert_eq!(accounts(&pool).await, 2);
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
async fn the_socket_takes_only_a_live_session_from_this_site() {
    let (_container, pool) = postgres().await;
    let (app, _root) = common::app(pool.clone());
    let token = start(&app, None).await.token();
    let expired = start(&app, None).await.token();
    expire(&pool, &expired, "-1 second").await;
    let (id, _) = token.split_once('.').unwrap();
    let wrong_secret = format!("{id}.{}", "0".repeat(64));
    let unknown_id = format!("{}.{}", "0".repeat(32), "0".repeat(64));
    let address = serve(app).await;

    for origin in [Some("https://evil.test"), None] {
        match open(address, origin, Some(&token)).await {
            Err(Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin:?}")
            }
            other => panic!("{origin:?} upgraded: {:?}", other.map(|_| ())),
        }
    }

    let stale = [
        None,
        Some("crumbs"),
        Some(&*wrong_secret),
        Some(&*unknown_id),
        Some(&*expired),
    ];
    for cookie in stale {
        let mut socket = open(address, Some(ORIGIN), cookie).await.unwrap();
        let Message::Close(Some(close)) = next(&mut socket).await else {
            panic!("{cookie:?} wasn't closed");
        };
        assert_eq!(u16::from(close.code), 4003, "{cookie:?}");
    }

    // Due to roll, so a socket that moved it would show.
    expire(&pool, &token, "20 days").await;
    let before = expiry(&pool, &token).await;
    let mut socket = open(address, Some(ORIGIN), Some(&token)).await.unwrap();
    socket
        .send(Message::Ping("still there?".into()))
        .await
        .unwrap();
    assert_eq!(
        next(&mut socket).await,
        Message::Pong("still there?".into())
    );
    assert_eq!(
        expiry(&pool, &token).await,
        before,
        "the socket doesn't roll it"
    );
}

//! Sessions, as Pilcrow's auth book has them (<https://auth.pilcrowonpaper.com/>). The cookie
//! holds a session's id and a secret; the database keeps only the secret's SHA-256, so a leaked
//! table opens no session. A session lasts 30 days from its last use, moved forward at most once
//! a day so a page load doesn't write every time.

use std::fmt::Write;

use axum::http::{HeaderMap, HeaderValue, header};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use subtle::ConstantTimeEq;

/// The cookie's name. With `__Host-` the browser keeps it only if it's Secure, for the whole
/// site, and set by this host itself, so a subdomain can't plant a session.
const COOKIE: &str = "__Host-session";

/// How long a session lasts without use, in seconds: 30 days.
const LIFETIME: i32 = 30 * 24 * 60 * 60;

/// How long a session goes before its expiry moves forward again, in seconds: a day.
const ROLL_AFTER: i32 = 24 * 60 * 60;

/// What the cookie holds: the session's id, to find it, and the secret that proves it's yours.
pub struct Token {
    id: String,
    secret: String,
}

impl Token {
    fn new() -> Token {
        Token {
            id: hex(&random::<16>()),
            secret: hex(&random::<32>()),
        }
    }

    /// The `Set-Cookie` value that keeps this token for the session's lifetime.
    pub fn cookie(&self) -> HeaderValue {
        let cookie = format!(
            "{COOKIE}={}.{}; Max-Age={LIFETIME}; Path=/; HttpOnly; Secure; SameSite=Lax",
            self.id, self.secret
        );
        HeaderValue::try_from(cookie).expect("hex is a valid header value")
    }
}

/// The token in a request's cookies, if it has one.
pub fn token(headers: &HeaderMap) -> Option<Token> {
    let (id, secret) = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|cookies| cookies.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .find_map(|cookie| cookie.trim().strip_prefix(COOKIE)?.strip_prefix('='))?
        .split_once('.')?;
    Some(Token {
        id: id.to_owned(),
        secret: secret.to_owned(),
    })
}

/// Who a session belongs to.
pub struct Account {
    pub handle: String,
}

/// A live session found by its token.
pub struct Session {
    pub account: Account,
    /// Whether it's a day or more since its expiry last moved, so it's due to [`roll`].
    pub due: bool,
}

/// Creates a Guest Account with a generated Handle, which needn't be unique, and a session for it.
pub async fn start(pool: &PgPool) -> Result<(Token, Account), sqlx::Error> {
    let token = Token::new();
    let [high, low] = random::<2>();
    let handle = format!("guest-{}", 1000 + u16::from_be_bytes([high, low]) % 9000);
    sqlx::query(
        "WITH guest AS (INSERT INTO accounts (handle) VALUES ($1) RETURNING id)
         INSERT INTO sessions (id, secret_hash, account_id, expires_at)
         SELECT $2, $3, id, now() + $4 * interval '1 second' FROM guest",
    )
    .bind(&handle)
    .bind(&token.id)
    .bind(hash(&token.secret))
    .bind(LIFETIME)
    .execute(pool)
    .await?;
    Ok((token, Account { handle }))
}

/// The live session `token` names, if its secret matches. An expired session is no session.
pub async fn find(pool: &PgPool, token: &Token) -> Result<Option<Session>, sqlx::Error> {
    let found: Option<(Vec<u8>, String, bool)> = sqlx::query_as(
        "SELECT s.secret_hash, a.handle::text,
                s.expires_at < now() + ($2 - $3) * interval '1 second'
         FROM sessions s JOIN accounts a ON a.id = s.account_id
         WHERE s.id = $1 AND s.expires_at > now()",
    )
    .bind(&token.id)
    .bind(LIFETIME)
    .bind(ROLL_AFTER)
    .fetch_optional(pool)
    .await?;
    Ok(found.and_then(|(secret_hash, handle, due)| {
        let matches: bool = hash(&token.secret).ct_eq(&secret_hash).into();
        matches.then_some(Session {
            account: Account { handle },
            due,
        })
    }))
}

/// Moves the session's expiry to a full lifetime from now; send [`Token::cookie`] with it.
pub async fn roll(pool: &PgPool, token: &Token) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE sessions SET expires_at = now() + $2 * interval '1 second' WHERE id = $1")
        .bind(&token.id)
        .bind(LIFETIME)
        .execute(pool)
        .await?;
    Ok(())
}

fn hash(secret: &str) -> Vec<u8> {
    Sha256::digest(secret).to_vec()
}

/// `N` bytes from the operating system's generator, which fails only on a broken system.
fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the operating system has no random numbers");
    bytes
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

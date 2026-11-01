//! Sessions, as Pilcrow's auth book has them (<https://auth.pilcrowonpaper.com/>). The cookie
//! holds a session's id and a secret; the database keeps only the secret's SHA-256, so a leaked
//! table opens no session. A session lasts 30 days from its last use, moved forward at most once
//! a day so a page load doesn't write every time.

use axum::http::{HeaderMap, HeaderValue, header};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use subtle::ConstantTimeEq;

use crate::random::{bytes, hex};

/// The cookie's name. With `__Host-` the browser keeps it only if it's Secure, for the whole
/// site, and set by this host itself, so a subdomain can't plant a session.
const COOKIE: &str = "__Host-session";

/// How long a session lasts without use, in seconds: 30 days.
const LIFETIME: i32 = 30 * 24 * 60 * 60;

/// How long a session goes before its expiry moves forward again, in seconds: a day.
const ROLL_AFTER: i32 = 24 * 60 * 60;

/// What the cookie holds: the session's id, to find it, and the secret that proves it's yours.
struct Token {
    id: String,
    secret: String,
}

impl Token {
    fn new() -> Token {
        Token {
            id: hex(&bytes::<16>()),
            secret: hex(&bytes::<32>()),
        }
    }

    /// The token in a request's cookies, if it has one.
    fn from_headers(headers: &HeaderMap) -> Option<Token> {
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
}

/// An Account, by its id in `accounts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct AccountId(i64);

/// A live session, and the Account it belongs to.
pub struct Session {
    token: Token,
    pub account: AccountId,
    pub handle: String,
    /// Whether it's a day or more since its expiry last moved, so it's due to [`roll`].
    pub due: bool,
}

impl Session {
    /// The `Set-Cookie` value that keeps the session for its lifetime.
    pub fn cookie(&self) -> HeaderValue {
        let Token { id, secret } = &self.token;
        let cookie = format!(
            "{COOKIE}={id}.{secret}; Max-Age={LIFETIME}; Path=/; HttpOnly; Secure; SameSite=Lax"
        );
        HeaderValue::try_from(cookie).expect("hex is a valid header value")
    }
}

/// Creates a Guest Account with a generated Handle, which needn't be unique, and a session for it.
pub async fn start(pool: &PgPool) -> Result<Session, sqlx::Error> {
    let token = Token::new();
    let handle = format!("guest-{}", 1000 + u16::from_be_bytes(bytes()) % 9000);
    let account = sqlx::query_scalar(
        "WITH guest AS (INSERT INTO accounts (handle) VALUES ($1) RETURNING id)
         INSERT INTO sessions (id, secret_hash, account_id, expires_at)
         SELECT $2, $3, id, now() + $4 * interval '1 second' FROM guest
         RETURNING account_id",
    )
    .bind(&handle)
    .bind(&token.id)
    .bind(hash(&token.secret))
    .bind(LIFETIME)
    .fetch_one(pool)
    .await?;
    Ok(Session {
        token,
        account,
        handle,
        due: false,
    })
}

/// The live session a request's cookie names, if its secret matches. An expired session is no
/// session.
pub async fn find(pool: &PgPool, headers: &HeaderMap) -> Result<Option<Session>, sqlx::Error> {
    let Some(token) = Token::from_headers(headers) else {
        return Ok(None);
    };
    let found: Option<(Vec<u8>, AccountId, String, bool)> = sqlx::query_as(
        "SELECT s.secret_hash, a.id, a.handle::text,
                s.expires_at < now() + $2 * interval '1 second'
         FROM sessions s JOIN accounts a ON a.id = s.account_id
         WHERE s.id = $1 AND s.expires_at > now()",
    )
    .bind(&token.id)
    .bind(LIFETIME - ROLL_AFTER)
    .fetch_optional(pool)
    .await?;
    Ok(found.and_then(|(secret_hash, account, handle, due)| {
        let matches: bool = hash(&token.secret).ct_eq(&secret_hash).into();
        matches.then_some(Session {
            token,
            account,
            handle,
            due,
        })
    }))
}

/// Moves the session's expiry to a full lifetime from now; send [`Session::cookie`] with it.
pub async fn roll(pool: &PgPool, session: &Session) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE sessions SET expires_at = now() + $2 * interval '1 second'
         WHERE id = $1 AND expires_at > now()",
    )
    .bind(&session.token.id)
    .bind(LIFETIME)
    .execute(pool)
    .await?;
    Ok(())
}

fn hash(secret: &str) -> Vec<u8> {
    Sha256::digest(secret).to_vec()
}

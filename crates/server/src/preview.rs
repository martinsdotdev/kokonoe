//! `/r/{room}`: the SPA, which opens the Room, with Open Graph tags in its head for wherever the
//! link is shared. They name the host and the Variant when the Room can be found in time, and
//! give a plain invitation otherwise, so the page itself never fails.

use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::Html;

use crate::room::RoomId;
use crate::{AppState, games};

/// How long the page waits to name the game before it gives the plain invitation.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

/// What a link to a Room no one can find shows.
const PLAIN: &str = r#"<meta property="og:type" content="website" />
<meta property="og:site_name" content="Kokonoe" />
<meta property="og:title" content="Join a game of Kokonoe" />
<meta property="og:description" content="You’re invited to a game of Ultimate Tic-Tac-Toe." />
<meta name="twitter:card" content="summary" />
"#;

/// The SPA's fallback page, split where the tags go: just before `</head>`.
#[derive(Clone)]
pub struct RoomPage {
    head: String,
    tail: String,
}

impl RoomPage {
    /// The page `index` with room for the tags, if it has a head to put them in.
    pub fn new(index: &str) -> Option<RoomPage> {
        let at = index.find("</head>")?;
        let (head, tail) = index.split_at(at);
        Some(RoomPage {
            head: head.to_owned(),
            tail: tail.to_owned(),
        })
    }

    fn with(&self, tags: &str) -> Html<String> {
        Html(format!("{}{tags}{}", self.head, self.tail))
    }
}

/// The page for the Room `room`'s link.
pub async fn room(State(state): State<AppState>, Path(room): Path<String>) -> Html<String> {
    let Some(room) = RoomId::parse(&room) else {
        return state.room_page.with(PLAIN);
    };
    match tokio::time::timeout(LOOKUP_TIMEOUT, games::summary(&state.pool, &room)).await {
        Ok(Ok(Some(summary))) => state
            .room_page
            .with(&named(&summary.host, summary.variant())),
        Ok(Ok(None)) => state.room_page.with(PLAIN),
        Ok(Err(error)) => {
            tracing::warn!(%error, "a Room's preview couldn't name its game");
            state.room_page.with(PLAIN)
        }
        Err(_) => {
            tracing::warn!("a Room's preview took too long to name its game");
            state.room_page.with(PLAIN)
        }
    }
}

/// The tags that name the game. A Handle is the player's to choose, so it's escaped.
fn named(host: &str, variant: &str) -> String {
    let title = escape(&format!("{host} invites you to a game of {variant}"));
    format!(
        r#"<meta property="og:type" content="website" />
<meta property="og:site_name" content="Kokonoe" />
<meta property="og:title" content="{title}" />
<meta property="og:description" content="Casual, untimed Ultimate Tic-Tac-Toe on Kokonoe." />
<meta name="twitter:card" content="summary" />
"#
    )
}

/// `text` as an HTML attribute's value.
fn escape(text: &str) -> String {
    text.chars().fold(String::new(), |mut escaped, c| {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            c => escaped.push(c),
        }
        escaped
    })
}

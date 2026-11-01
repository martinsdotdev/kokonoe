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
}

/// The page for the Room `room`'s link.
pub async fn room(State(state): State<AppState>, Path(room): Path<String>) -> Html<String> {
    let named = match RoomId::parse(&room) {
        Some(room) => named(&state, &room).await,
        None => None,
    };
    let (title, description) = match &named {
        Some(title) => (
            title.as_str(),
            "Casual, untimed Ultimate Tic-Tac-Toe on Kokonoe.",
        ),
        None => (
            "Join a game of Kokonoe",
            "You’re invited to a game of Ultimate Tic-Tac-Toe.",
        ),
    };
    let RoomPage { head, tail } = &state.room_page;
    Html(format!("{head}{}{tail}", tags(title, description)))
}

/// "guest-4821 invites you to a game of Standard", if the Room is found in time.
async fn named(state: &AppState, room: &RoomId) -> Option<String> {
    match tokio::time::timeout(LOOKUP_TIMEOUT, games::load(&state.pool, room)).await {
        Ok(Ok(stored)) => {
            let stored = stored?;
            let variant = stored.variant();
            Some(format!(
                "{} invites you to a game of {variant}",
                stored.host
            ))
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, "a Room's preview couldn't name its game");
            None
        }
        Err(_) => {
            tracing::warn!("a Room's preview took too long to name its game");
            None
        }
    }
}

/// The Open Graph tags. A Handle is the player's to choose, so the text is escaped.
fn tags(title: &str, description: &str) -> String {
    let (title, description) = (escape(title), escape(description));
    format!(
        r#"<meta property="og:type" content="website" />
<meta property="og:site_name" content="Kokonoe" />
<meta property="og:title" content="{title}" />
<meta property="og:description" content="{description}" />
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_in_text_stays_text() {
        assert_eq!(
            escape(r#"<b>&"x'</b>"#),
            "&lt;b&gt;&amp;&quot;x&#39;&lt;/b&gt;"
        );
        assert_eq!(escape("guest-4821 · Standard"), "guest-4821 · Standard");
    }
}

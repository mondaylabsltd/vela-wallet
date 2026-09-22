//! The relay's two URLs, and what a host answers before any upgrade.
//!
//! ```text
//! GET /v1/rooms/{room}?role=requester|signer      (WebSocket upgrade)
//! GET /healthz                                     → 200 "ok"
//! ```

/// The health probe's path.
pub const HEALTHZ_PATH: &str = "/healthz";

/// Every room URL starts with this.
pub const ROOMS_PREFIX: &str = "/v1/rooms/";

/// A room id is 22 characters of base64url: 128 random bits, unpadded.
pub const ROOM_ID_LEN: usize = 22;

/// Which end of the room a connection is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The wallet: it chose the room and shows the pairing link.
    Requester,
    /// The Clear Signer page: it opened the pairing link.
    Signer,
}

impl Role {
    /// Parses the `role` query value. Exact and case-sensitive.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "requester" => Some(Role::Requester),
            "signer" => Some(Role::Signer),
            _ => None,
        }
    }

    /// The `role` query value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Role::Requester => "requester",
            Role::Signer => "signer",
        }
    }

    /// The other end.
    pub const fn peer(self) -> Self {
        match self {
            Role::Requester => Role::Signer,
            Role::Signer => Role::Requester,
        }
    }
}

/// Whether `room` is a valid room id: exactly [`ROOM_ID_LEN`] characters of the
/// base64url alphabet (`A–Z a–z 0–9 - _`), no padding.
///
/// The spare low bits of the last character are not checked: every base64url
/// encoder emits them as zero, and a room id is only a meeting point — the
/// session inside the room is what is authenticated.
pub fn is_valid_room_id(room: &str) -> bool {
    room.len() == ROOM_ID_LEN
        && room
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// What a request is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target<'a> {
    /// `GET /healthz` → 200 `ok`.
    Healthz,
    /// A room's WebSocket.
    Room {
        /// The room id, already validated.
        room: &'a str,
        /// The role asked for.
        role: Role,
    },
    /// A room URL with a bad room id or role. Before the upgrade a host answers
    /// HTTP 400 with this text; after it, close [`crate::Close::BadRequest`].
    BadRequest(&'static str),
    /// Anything else → 404.
    NotFound,
}

/// Classifies a request by its path and query string (without the `?`).
///
/// The query is not percent-decoded: the only valid `role` values are plain
/// ASCII, so an encoded one is as wrong as a misspelt one. Parameters other than
/// `role` are ignored; a `role` given twice is refused.
pub fn target<'a>(path: &'a str, query: Option<&'a str>) -> Target<'a> {
    if path == HEALTHZ_PATH {
        return Target::Healthz;
    }
    let Some(room) = path.strip_prefix(ROOMS_PREFIX) else {
        return Target::NotFound;
    };
    if !is_valid_room_id(room) {
        return Target::BadRequest("bad room id");
    }
    let mut roles = query
        .unwrap_or("")
        .split('&')
        .filter_map(|pair| pair.strip_prefix("role="));
    match (roles.next().and_then(Role::parse), roles.next()) {
        (Some(role), None) => Target::Room { room, role },
        _ => Target::BadRequest("bad role"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: &str = "q5x0VbXk3mFqL1a2zY-_Aw";

    #[test]
    fn roles_parse_exactly() {
        assert_eq!(Role::parse("requester"), Some(Role::Requester));
        assert_eq!(Role::parse("signer"), Some(Role::Signer));
        for bad in [
            "",
            "Signer",
            "SIGNER",
            "signer ",
            "admin",
            "requester,signer",
        ] {
            assert_eq!(Role::parse(bad), None, "{bad:?}");
        }
        for role in [Role::Requester, Role::Signer] {
            assert_eq!(Role::parse(role.as_str()), Some(role));
            assert_eq!(role.peer().peer(), role);
            assert_ne!(role.peer(), role);
        }
    }

    #[test]
    fn room_ids_are_22_base64url_characters() {
        assert_eq!(ROOM.len(), ROOM_ID_LEN);
        assert!(is_valid_room_id(ROOM));
        assert!(is_valid_room_id("AAAAAAAAAAAAAAAAAAAAAA"));
        assert!(is_valid_room_id("-_-_-_-_-_-_-_-_-_-_-_"));
        assert!(is_valid_room_id("abcdefghijklmnopqrstuv"));
        assert!(is_valid_room_id("0123456789ABCDEFGHIJKw"));
    }

    #[test]
    fn room_ids_refused() {
        let bad = [
            "",
            "q5x0VbXk3mFqL1a2zY-_A",   // 21
            "q5x0VbXk3mFqL1a2zY-_Aww", // 23
            "q5x0VbXk3mFqL1a2zY+/Aw",  // standard base64 alphabet
            "q5x0VbXk3mFqL1a2zY-_A=",  // padding
            "q5x0VbXk3mFqL1a2zY-_A.",  // punctuation
            "q5x0VbXk3mFqL1a2zY-_A ",  // space
            "q5x0VbXk3mFqL1a2zY-_Aé",  // non-ASCII (23 bytes, 22 chars)
            "q5x0VbXk3mFqL1a2zY-_%41", // percent-encoded
            "q5x0VbXk3mFqL1a2zY-_A/",  // path separator
        ];
        for room in bad {
            assert!(!is_valid_room_id(room), "{room:?}");
        }
    }

    #[test]
    fn healthz() {
        assert_eq!(target("/healthz", None), Target::Healthz);
        assert_eq!(target("/healthz", Some("x=1")), Target::Healthz);
        assert_eq!(target("/healthz/", None), Target::NotFound);
    }

    #[test]
    fn room_urls() {
        let path = format!("/v1/rooms/{ROOM}");
        assert_eq!(
            target(&path, Some("role=signer")),
            Target::Room {
                room: ROOM,
                role: Role::Signer
            }
        );
        assert_eq!(
            target(&path, Some("role=requester")),
            Target::Room {
                room: ROOM,
                role: Role::Requester
            }
        );
        // Other parameters (a cache buster, say) are ignored.
        assert_eq!(
            target(&path, Some("t=1&role=signer&x")),
            Target::Room {
                room: ROOM,
                role: Role::Signer
            }
        );
    }

    #[test]
    fn bad_roles_are_bad_requests() {
        let path = format!("/v1/rooms/{ROOM}");
        for query in [
            None,
            Some(""),
            Some("role="),
            Some("role=admin"),
            Some("role=Signer"),
            Some("role=%73igner"),
            Some("role=signer&role=requester"),
            Some("role=signer&role=signer"),
            Some("rolex=signer"),
        ] {
            assert_eq!(
                target(&path, query),
                Target::BadRequest("bad role"),
                "{query:?}"
            );
        }
    }

    #[test]
    fn bad_room_ids_are_bad_requests_before_the_role_is_looked_at() {
        for path in [
            "/v1/rooms/",
            "/v1/rooms/short",
            "/v1/rooms/q5x0VbXk3mFqL1a2zY-_Aww",
            "/v1/rooms/q5x0VbXk3mFqL1a2zY+/Aw",
            "/v1/rooms/q5x0VbXk3mFqL1a2zY-_Aw/",
            "/v1/rooms/q5x0VbXk3mFqL1a2zY-_Aw/extra",
        ] {
            assert_eq!(
                target(path, Some("role=signer")),
                Target::BadRequest("bad room id"),
                "{path}"
            );
            assert_eq!(
                target(path, None),
                Target::BadRequest("bad room id"),
                "{path}"
            );
        }
    }

    #[test]
    fn other_paths_are_not_found() {
        for path in [
            "/",
            "",
            "/v1/rooms",
            "/v1/room/x",
            "/v2/rooms/q5x0VbXk3mFqL1a2zY-_Aw",
            "/HEALTHZ",
        ] {
            assert_eq!(
                target(path, Some("role=signer")),
                Target::NotFound,
                "{path}"
            );
        }
    }
}

//! Anonymous session identity (#12 "Anonymous checkout continuity", ADR-20260720-015500).
//!
//! The client — not the server — mints the session id, because the session EXISTS BEFORE any server
//! round-trip: an anonymous visitor builds a cart, places an order and polls `operationStatus`
//! without ever authenticating, and the server scopes all of that (the cart, the command-journal
//! ownership checks behind `operationStatus`/`paymentStatus`) to whatever `X-SESSION-ID` the client
//! presents. Server-minted ids would need a set-cookie handshake before the first mutation and
//! would break the offline-first mint-on-first-use flow.
//!
//! The id must SURVIVE APP RESTARTS: an anonymous cart and the `operationStatus` rows of every
//! command this session journaled are keyed on it server-side. Lose the id and the user loses their
//! cart mid-checkout and the right to read their own in-flight operations — the exact V0 mobile
//! failure (tab killed while Stripe redirects, flaky radio forcing a reload) #12 exists to prevent.
//! Hence localStorage on the browser path, not in-memory state.
//!
//! Two construction paths mirror the two build targets:
//!   * `hydrate` (wasm32, browser) — [`SessionId::load_or_mint`]: read localStorage, mint a UUIDv7
//!     on first use and persist it.
//!   * `ssr` (native) — the id arrives ON THE REQUEST (cookie/`X-SESSION-ID` header, parsed at the
//!     server boundary — `crates/server/src/graphql/session.rs` is the validating twin of this
//!     module): [`SessionId::from_request`]. The server render path never mints or stores one — a
//!     server-side store would fork the identity the browser already owns.

use uuid::Uuid;

/// The transport header carrying the session id — MUST stay in sync with the server's
/// `graphql_session::SESSION_HEADER` (`crates/server/src/graphql/session.rs`; `web` cannot depend
/// on `server`, so the contract is mirrored here and nothing else string-literals it).
pub const SESSION_HEADER: &str = "x-session-id";

/// The single localStorage key the browser session id lives under (`hydrate` path only). One key,
/// origin-scoped: `{slug}.captain.food` and `live.captain.food` are distinct origins, so each
/// storefront keeps its own anonymous identity — matching the per-host tenant model (ADR-0006).
pub const SESSION_STORAGE_KEY: &str = "captain.session-id";

/// The client-owned anonymous session identity, sent as [`SESSION_HEADER`] on every GraphQL call.
///
/// A newtype over `Uuid` so the rest of the crate can never confuse it with the other client-minted
/// uuid (the per-command `messageId`, `actions.rs`) — both travel on the same requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Mint a fresh session id (UUIDv7 — time-ordered like every id the platform mints, so journal
    /// rows keyed on it cluster by time). Public for the first-use path and for tests; production
    /// code should reach it through [`SessionId::load_or_mint`] (hydrate) so the mint is persisted.
    pub fn mint() -> Self {
        Self(Uuid::now_v7())
    }

    /// The `ssr` construction path: the id was extracted from the incoming request by the server
    /// boundary (which already validated the header shape — malformed ids 400 there, see
    /// `crates/server/src/graphql/session.rs`). This constructor deliberately takes a parsed
    /// `Uuid`, not a raw string: parsing/rejection is the boundary's job, not ours.
    pub fn from_request(id: Uuid) -> Self {
        Self(id)
    }

    /// The `hydrate` construction path: restore the persisted id, or mint-and-persist on first use.
    /// Storage failures (private-mode quota, disabled storage) degrade to a volatile id — the
    /// session then lasts one page lifetime, which is still correct, just less continuous. A
    /// corrupt stored value is re-minted and overwritten rather than sent (the server would 400 a
    /// malformed header — fail-visible there, self-heal here).
    #[cfg(all(target_arch = "wasm32", feature = "hydrate"))]
    pub fn load_or_mint() -> Self {
        let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
        if let Some(storage) = &storage {
            if let Ok(Some(raw)) = storage.get_item(SESSION_STORAGE_KEY) {
                if let Ok(id) = Uuid::parse_str(raw.trim()) {
                    return Self(id);
                }
            }
        }
        let minted = Self::mint();
        if let Some(storage) = &storage {
            let _ = storage.set_item(SESSION_STORAGE_KEY, &minted.to_string());
        }
        minted
    }

    /// The raw uuid (e.g. to compare against a `MutationAcceptance.sessionId` echo).
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// Canonical wire form — exactly what goes into the [`SESSION_HEADER`] value (hyphenated lowercase,
/// the shape the server's validator parses back).
impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_ids_are_v7_and_unique() {
        let a = SessionId::mint();
        let b = SessionId::mint();
        // v7 is part of the contract (time-ordered journal keys), not an implementation detail.
        assert_eq!(a.as_uuid().get_version_num(), 7);
        assert_ne!(a, b);
    }

    #[test]
    fn from_request_preserves_the_boundary_parsed_id() {
        let id = Uuid::now_v7();
        assert_eq!(SessionId::from_request(id).as_uuid(), id);
    }

    #[test]
    fn wire_form_round_trips_through_the_server_parser_shape() {
        // The server parses the header with `Uuid::parse_str` — our Display must round-trip it.
        let session = SessionId::mint();
        assert_eq!(Uuid::parse_str(&session.to_string()).unwrap(), session.as_uuid());
    }
}

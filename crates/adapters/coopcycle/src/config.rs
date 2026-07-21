//! CoopCycle **federation** config — the one structural thing that distinguishes this partner from
//! Avelo37 (specs/integrations/coopcycle.md §5, ADR-20260721-122910). CoopCycle is many self-hosted
//! co-op instances (one per city/co-op), NOT a single central API, so the adapter config is an
//! *instance registry*, not a single endpoint + key:
//!
//! - **outbound** reads an instance's `base_url` + OAuth2 client credentials (per-instance client);
//! - **inbound** reads an instance's `webhook_secret` to verify that instance's webhooks;
//! - **resolution** maps an outbound job to an instance by **coverage area** (dropoff postal-code
//!   prefix), matching "a co-op serves a city".
//!
//! Configured out-of-repo via the `COOPCYCLE_INSTANCES` env var (a JSON array), exactly as Avelo37's
//! `AVELO37_API_KEY` gates that adapter: unset/empty ⇒ the composition root keeps the no-op
//! `NoopDeliveryService` stand-in (fail-closed; jobs stay open to independent riders).

use serde::Deserialize;

/// Env var holding the instance registry (a JSON array of [`CoopCycleInstance`]). Unset/empty ⇒ the
/// adapter is not configured and the caller keeps the no-op stand-in.
pub const COOPCYCLE_INSTANCES_ENV: &str = "COOPCYCLE_INSTANCES";

/// OAuth2 client-credentials config for one co-op instance (per-instance client — unlike Avelo37's
/// single static bearer key). Token fetch/refresh lives in `outbound.rs`.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    /// The instance's OAuth2 token endpoint (client-credentials grant).
    pub token_url: String,
}

/// One self-hosted CoopCycle co-op instance in the registry.
#[derive(Debug, Clone, Deserialize)]
pub struct CoopCycleInstance {
    /// Stable instance key (e.g. `"tours"`) — the `{instance}` webhook path segment and the
    /// `instance_id` recorded on every `external_coopcycle_events` row.
    pub id: String,
    /// The instance's API base URL (create-delivery is POSTed here).
    pub base_url: String,
    pub oauth: OAuthConfig,
    /// Per-instance webhook signing secret (agreed with the co-op) — verifies THIS instance's webhooks.
    pub webhook_secret: String,
    /// Coverage area as dropoff postal-code prefixes (e.g. `["37"]` for Tours / Indre-et-Loire). A job
    /// resolves to this instance when its dropoff postal code starts with one of these prefixes.
    #[serde(default)]
    pub coverage: Vec<String>,
}

/// The parsed instance registry — shared by the outbound gateway (base URL + OAuth) and the inbound
/// webhook route (per-instance secret).
#[derive(Debug, Clone, Default)]
pub struct CoopCycleRegistry {
    instances: Vec<CoopCycleInstance>,
}

impl CoopCycleRegistry {
    /// Parse a registry from the `COOPCYCLE_INSTANCES` JSON array. `Ok(None)` when the var is
    /// unset/empty (not configured); `Err` when it is set but unparsable (a misconfiguration the
    /// operator must see, not a silent no-op).
    pub fn from_env() -> Result<Option<Self>, String> {
        match std::env::var(COOPCYCLE_INSTANCES_ENV) {
            Ok(raw) if !raw.trim().is_empty() => Self::from_json(&raw).map(Some),
            _ => Ok(None),
        }
    }

    /// Parse a registry from a JSON array string.
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let instances: Vec<CoopCycleInstance> = serde_json::from_str(raw)
            .map_err(|e| format!("COOPCYCLE_INSTANCES is not a valid instance array: {e}"))?;
        if instances.is_empty() {
            return Err("COOPCYCLE_INSTANCES is an empty array (no co-op instances configured)".into());
        }
        Ok(Self { instances })
    }

    /// Build directly from instances (tests / programmatic construction).
    pub fn new(instances: Vec<CoopCycleInstance>) -> Self {
        Self { instances }
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Look an instance up by its id (used by the inbound webhook route to pick the verification
    /// secret from the `{instance}` path segment).
    pub fn instance(&self, id: &str) -> Option<&CoopCycleInstance> {
        self.instances.iter().find(|i| i.id == id)
    }

    /// The per-instance webhook secret for signature verification, or `None` for an unknown instance.
    pub fn webhook_secret(&self, instance_id: &str) -> Option<&str> {
        self.instance(instance_id).map(|i| i.webhook_secret.as_str())
    }

    /// Resolve an outbound job to the co-op instance covering its dropoff postal code — the
    /// **longest** matching `coverage` prefix wins (most specific co-op), so a national + a city
    /// instance can coexist. `None` ⇒ no co-op covers this area (the outbound call fails closed).
    pub fn resolve_by_postal(&self, postal_code: &str) -> Option<&CoopCycleInstance> {
        self.instances
            .iter()
            .filter_map(|i| {
                i.coverage
                    .iter()
                    .filter(|p| postal_code.starts_with(p.as_str()))
                    .map(|p| p.len())
                    .max()
                    .map(|len| (len, i))
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, i)| i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_json() -> &'static str {
        r#"[
          {"id":"tours","base_url":"https://coopcycle.tours","oauth":{"client_id":"c","client_secret":"s","token_url":"https://coopcycle.tours/oauth/token"},"webhook_secret":"whs_tours","coverage":["37"]},
          {"id":"national","base_url":"https://coopcycle.fr","oauth":{"client_id":"c2","client_secret":"s2","token_url":"https://coopcycle.fr/oauth/token"},"webhook_secret":"whs_nat","coverage":["3"]}
        ]"#
    }

    #[test]
    fn parses_and_looks_up_by_id() {
        let reg = CoopCycleRegistry::from_json(registry_json()).unwrap();
        assert_eq!(reg.instance("tours").unwrap().base_url, "https://coopcycle.tours");
        assert_eq!(reg.webhook_secret("tours"), Some("whs_tours"));
        assert_eq!(reg.webhook_secret("unknown"), None);
    }

    #[test]
    fn resolves_by_longest_covering_prefix() {
        let reg = CoopCycleRegistry::from_json(registry_json()).unwrap();
        // "37000" is covered by both "37" (tours) and "3" (national) — the longer prefix wins.
        assert_eq!(reg.resolve_by_postal("37000").unwrap().id, "tours");
        // "35000" only matches "3" (national).
        assert_eq!(reg.resolve_by_postal("35000").unwrap().id, "national");
        // "75000" matches neither — fail closed.
        assert!(reg.resolve_by_postal("75000").is_none());
    }

    #[test]
    fn empty_array_is_an_error_not_a_silent_noop() {
        assert!(CoopCycleRegistry::from_json("[]").is_err());
        assert!(CoopCycleRegistry::from_json("not json").is_err());
    }
}

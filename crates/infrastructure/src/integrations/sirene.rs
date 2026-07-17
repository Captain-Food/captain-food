//! SIRENE Anti-Corruption Layer (ADR-0019/0020/0027) — pulls food-service établissements from the
//! French INSEE Sirene registry and maps each one onto the EXISTING `RegisterRestaurant` command.
//! External facts in → ordinary domain commands out; nothing here touches the read side. A registered
//! prospect then flows through the normal write path: `RestaurantRegistered` in `domain_events` →
//! `ProjectionWorker` folds it into the `restaurant` table → the `restaurants` GraphQL query serves it.
//!
//! # INSEE Sirene API targeted (researched 2026-07; NOT live-verified — needs the user's API key)
//!
//! INSEE decommissioned the legacy `api.insee.fr` OAuth2 (client-credentials) portal in 2024 and moved
//! to the new API portal (`portail-api.insee.fr`). The Sirene REST API we target:
//!
//! - **Base URL**: `https://api.insee.fr/api-sirene/3.11` (version-pinned path; see [`DEFAULT_BASE_URL`],
//!   overridable via the `INSEE_API_BASE_URL` env var in case INSEE bumps the version segment).
//! - **Auth**: a static API key created on the portal ("integration key"), sent as the HTTP header
//!   `X-INSEE-Api-Key-Integration: <key>` ([`API_KEY_HEADER`]). The key is read from the
//!   `INSEE_API_TOKEN` env var. (No OAuth2 dance on the new portal.)
//! - **Endpoint**: `GET /siret` — multi-criteria search over établissements. Parameters used:
//!   - `q` — Lucene-ish query. Ours ([`restauration_query`]) filters NAF/APE food-service codes
//!     (56.10A restauration traditionnelle, 56.10B cafétérias, 56.10C restauration rapide,
//!     56.30Z débits de boissons) on the CURRENT period, currently-active establishments
//!     (`etatAdministratifEtablissement:A`), and the geography — commune of Tours
//!     (`codeCommuneEtablissement:37261`) or department 37 (`codeCommuneEtablissement:37*`,
//!     commune codes are prefixed by the department). Historized fields are wrapped in
//!     `periode(...)` so they match on the same (current) period.
//!   - `nombre` — page size, max 1000.
//!   - `curseur` — deep cursor pagination: send `*` first, then follow `header.curseurSuivant`
//!     until it equals the cursor you sent (the documented termination condition).
//! - **Responses**: `200` with `{ header: {...}, etablissements: [...] }`; `404` means ZERO matches
//!   (not an error — mapped to an empty page); `429` when the ~30 requests/minute quota is exceeded
//!   (retried politely, honouring `Retry-After`).
//!
//! # Mapping decisions (external → `RegisterRestaurant`)
//!
//! - `restaurantId` = **UUIDv5 of the SIRET** under a fixed project namespace ([`restaurant_id_for_siret`]):
//!   stable across syncs, so the client-generated-id idempotency of `register_restaurant` absorbs re-runs.
//! - `ref` = the SIRET (the idempotent external key); `externalIdentifiers` also carry the well-known
//!   `siret` and `naf` keys (see `scalars.yaml#/ExternalIdentifierKey`).
//! - `slug` = slugify(display name) + `-<NIC>` (last 5 SIRET digits) so two establishments with the
//!   same name never collide; matches `^[a-z0-9]+(?:-[a-z0-9]+)*$`.
//! - `displayName` = enseigne → denomination usuelle (période) → denomination usuelle (unité légale) →
//!   denomination (unité légale) → "Prénom Nom" for personnes physiques. INSEE capitalisation is kept as-is.
//! - `listingStatus` = `NON_PARTNER` (a prospect, ADR-0027); `accountId` = None; `openingHours` = []
//!   (SIRENE has none); `location` = None (SIRENE exposes Lambert-93 coordinates, not WGS84 —
//!   conversion is Google-enrichment territory, ADR-0020); `timezone` = Europe/Paris (scope is dept 37).
//! - `cuisineCategory` best-effort from NAF: 56.10A → TRADITIONAL, 56.10C → FAST_FOOD, otherwise None.
//! - Closed establishments (état `F`/`C` on the current period), SIRETs that are not 14 digits, and
//!   records with no usable name or postal code/city are rejected with a descriptive error — the
//!   runner logs and skips them without aborting the run.

use domain::generated::commands::RegisterRestaurant;
use domain::generated::entities::{Address, ExternalIdentifier};
use domain::generated::scalars::{
    AddressLine, CityName, CountryCode, CuisineCategory, ExternalIdentifierKey, ExternalReference,
    PostalCode, RestaurantDisplayName, RestaurantId, RestaurantListingStatus, Slug, TimeZone,
};
use domain::shared::errors::DomainError;
use serde::Deserialize;

// ---------------------------------------------------------------------------------------------
// Constants & id derivation
// ---------------------------------------------------------------------------------------------

/// Version-pinned base URL of the Sirene API on the 2024+ INSEE portal.
pub const DEFAULT_BASE_URL: &str = "https://api.insee.fr/api-sirene/3.11";
/// Env var holding the portal API key (repo secret in the GitHub Actions workflow).
pub const INSEE_API_TOKEN_ENV: &str = "INSEE_API_TOKEN";
/// Optional env override of [`DEFAULT_BASE_URL`] (e.g. when INSEE bumps the `/3.11` version segment).
pub const INSEE_API_BASE_URL_ENV: &str = "INSEE_API_BASE_URL";
/// Header carrying the API key on the new INSEE portal (no OAuth2).
pub const API_KEY_HEADER: &str = "X-INSEE-Api-Key-Integration";
/// Code commune INSEE of Tours (the V0 target market).
pub const TOURS_CODE_COMMUNE: &str = "37261";
/// NAF/APE codes for the food-service scope (ADR-0019/0020).
pub const RESTAURATION_NAF_CODES: [&str; 4] = ["56.10A", "56.10B", "56.10C", "56.30Z"];
/// Sirene's documented maximum `nombre` (page size).
pub const MAX_PAGE_SIZE: u32 = 1000;

/// Fixed UUIDv5 namespace for every id this ACL derives. NEVER change it: the derived
/// `restaurantId`s are the idempotency keys of the whole sync.
fn sirene_namespace() -> uuid::Uuid {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, b"https://captain.food/integrations/sirene")
}

/// Deterministic `restaurantId` for a SIRET — the same SIRET always maps to the same aggregate id,
/// so replaying the sync hits `register_restaurant`'s creation-idempotency and is a no-op.
pub fn restaurant_id_for_siret(siret: &str) -> RestaurantId {
    RestaurantId(uuid::Uuid::new_v5(&sirene_namespace(), siret.as_bytes()))
}

/// Fixed system user id stamping the event envelope (`domain_events.user_id`, ADR-0041) for events
/// this synchronizer causes. Deterministic so every run is attributable to the same principal.
pub fn sirene_system_user_id() -> uuid::Uuid {
    uuid::Uuid::new_v5(&sirene_namespace(), b"system:sirene-sync")
}

// ---------------------------------------------------------------------------------------------
// Wire types (the subset of the Sirene /siret response the ACL reads)
// ---------------------------------------------------------------------------------------------

/// One Sirene établissement (deserialization subset). Field names mirror the INSEE JSON exactly
/// (French, camelCase); the ACL is the ONLY place they exist — they never leak into `domain`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Etablissement {
    pub siret: String,
    #[serde(default)]
    pub unite_legale: Option<UniteLegale>,
    #[serde(default)]
    pub adresse_etablissement: Option<AdresseEtablissement>,
    /// Historized periods, most recent first; the current one has `dateFin: null`.
    #[serde(default)]
    pub periodes_etablissement: Vec<PeriodeEtablissement>,
}

impl Etablissement {
    /// The current (open-ended) period, falling back to the first (most recent) one.
    pub fn current_period(&self) -> Option<&PeriodeEtablissement> {
        self.periodes_etablissement
            .iter()
            .find(|p| p.date_fin.is_none())
            .or_else(|| self.periodes_etablissement.first())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniteLegale {
    #[serde(default)]
    pub denomination_unite_legale: Option<String>,
    #[serde(default)]
    pub denomination_usuelle_1_unite_legale: Option<String>,
    #[serde(default)]
    pub nom_unite_legale: Option<String>,
    #[serde(default)]
    pub nom_usage_unite_legale: Option<String>,
    #[serde(default)]
    pub prenom_1_unite_legale: Option<String>,
    #[serde(default)]
    pub activite_principale_unite_legale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdresseEtablissement {
    #[serde(default)]
    pub numero_voie_etablissement: Option<String>,
    #[serde(default)]
    pub indice_repetition_etablissement: Option<String>,
    #[serde(default)]
    pub type_voie_etablissement: Option<String>,
    #[serde(default)]
    pub libelle_voie_etablissement: Option<String>,
    #[serde(default)]
    pub complement_adresse_etablissement: Option<String>,
    #[serde(default)]
    pub code_postal_etablissement: Option<String>,
    #[serde(default)]
    pub libelle_commune_etablissement: Option<String>,
    #[serde(default)]
    pub code_commune_etablissement: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodeEtablissement {
    #[serde(default)]
    pub date_fin: Option<String>,
    #[serde(default)]
    pub etat_administratif_etablissement: Option<String>,
    #[serde(default)]
    pub enseigne_1_etablissement: Option<String>,
    #[serde(default)]
    pub denomination_usuelle_etablissement: Option<String>,
    #[serde(default)]
    pub activite_principale_etablissement: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SireneResponse {
    header: SireneHeader,
    #[serde(default)]
    etablissements: Vec<Etablissement>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SireneHeader {
    #[serde(default)]
    total: u64,
    #[serde(default)]
    curseur: Option<String>,
    #[serde(default)]
    curseur_suivant: Option<String>,
}

/// One fetched page, cursor already resolved: `next_cursor = None` ⇔ this was the last page.
#[derive(Debug)]
pub struct SirenePage {
    pub etablissements: Vec<Etablissement>,
    pub total: u64,
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Query building
// ---------------------------------------------------------------------------------------------

/// Geographic scope of a sync run. Commune codes embed the department, so department filtering is a
/// prefix wildcard on `codeCommuneEtablissement`.
#[derive(Debug, Clone)]
pub enum SireneScope {
    /// A single code commune INSEE (e.g. [`TOURS_CODE_COMMUNE`]).
    Commune(String),
    /// A whole department (e.g. `"37"` for Indre-et-Loire).
    Department(String),
}

/// Build the `q=` parameter: currently-active food-service establishments in `scope`.
/// `activitePrincipaleEtablissement` and `etatAdministratifEtablissement` are historized, hence the
/// single `periode(...)` wrapper (both conditions must hold on the SAME — current — period).
pub fn restauration_query(scope: &SireneScope) -> String {
    let naf = RESTAURATION_NAF_CODES
        .iter()
        .map(|code| format!("activitePrincipaleEtablissement:{code}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let geo = match scope {
        SireneScope::Commune(code) => format!("codeCommuneEtablissement:{code}"),
        SireneScope::Department(dept) => format!("codeCommuneEtablissement:{dept}*"),
    };
    format!("{geo} AND periode(etatAdministratifEtablissement:A AND ({naf}))")
}

// ---------------------------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------------------------

/// Minimal Sirene API client: base URL + API key, one page per call. All failures come back as
/// `DomainError::Repository` (the infrastructure-boundary error, like every other adapter here).
pub struct SireneClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl SireneClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
        }
    }

    /// Build from env: `INSEE_API_TOKEN` (required) + `INSEE_API_BASE_URL` (optional override).
    pub fn from_env() -> Result<Self, DomainError> {
        let token = std::env::var(INSEE_API_TOKEN_ENV).map_err(|_| {
            DomainError::Repository(format!("{INSEE_API_TOKEN_ENV} must be set"))
        })?;
        let base_url =
            std::env::var(INSEE_API_BASE_URL_ENV).unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Ok(Self::new(base_url, token))
    }

    /// Fetch one `/siret` page. `cursor` is `"*"` for the first page, then the previous page's
    /// `next_cursor`. Retries politely on 429 (honouring `Retry-After`, capped) and transient 5xx;
    /// a 404 is Sirene's "zero results" and maps to an empty final page.
    pub async fn fetch_page(
        &self,
        query: &str,
        cursor: &str,
        page_size: u32,
    ) -> Result<SirenePage, DomainError> {
        let url = format!("{}/siret", self.base_url);
        let page_size = page_size.min(MAX_PAGE_SIZE).to_string();
        let mut attempts = 0u32;
        loop {
            attempts += 1;
            let response = self
                .http
                .get(&url)
                .header(API_KEY_HEADER, &self.token)
                .header(reqwest::header::ACCEPT, "application/json")
                .query(&[("q", query), ("nombre", &page_size), ("curseur", cursor)])
                .send()
                .await
                .map_err(|e| DomainError::Repository(format!("sirene: request failed: {e}")))?;

            let status = response.status();
            if status == reqwest::StatusCode::NOT_FOUND {
                // Sirene answers 404 for an empty result set — a legitimate "nothing to sync".
                return Ok(SirenePage { etablissements: vec![], total: 0, next_cursor: None });
            }
            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                && attempts < 3
            {
                let wait = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(if status.is_server_error() { 5 } else { 30 })
                    .min(60);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                continue;
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(DomainError::Repository(format!(
                    "sirene: GET /siret returned {status}: {}",
                    body.chars().take(300).collect::<String>()
                )));
            }
            let body = response
                .text()
                .await
                .map_err(|e| DomainError::Repository(format!("sirene: reading body: {e}")))?;
            return parse_page(&body, cursor);
        }
    }
}

/// Parse a `/siret` response body into a [`SirenePage`], resolving cursor termination: the run is
/// over when `curseurSuivant` is absent or equal to the cursor we just sent (INSEE's documented
/// end-of-pagination signal), or when the page came back empty.
fn parse_page(body: &str, requested_cursor: &str) -> Result<SirenePage, DomainError> {
    let response: SireneResponse = serde_json::from_str(body)
        .map_err(|e| DomainError::Repository(format!("sirene: unexpected response shape: {e}")))?;
    let sent = response.header.curseur.as_deref().unwrap_or(requested_cursor);
    let next_cursor = match response.header.curseur_suivant {
        Some(next) if next != sent && !response.etablissements.is_empty() => Some(next),
        _ => None,
    };
    Ok(SirenePage {
        etablissements: response.etablissements,
        total: response.header.total,
        next_cursor,
    })
}

// ---------------------------------------------------------------------------------------------
// Mapping — the actual Anti-Corruption boundary
// ---------------------------------------------------------------------------------------------

/// Trimmed, non-empty, non-"[ND]" (INSEE's redaction marker for non-diffusible data) view of an
/// optional INSEE string field.
fn clean(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty() && *v != "[ND]")
}

/// Lowercase-dash slug matching `^[a-z0-9]+(?:-[a-z0-9]+)*$`, with French accents folded to ASCII.
/// Non-alphanumeric runs collapse to a single dash; leading/trailing dashes are trimmed.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        let folded: &str = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'À' | 'Â' | 'Ä' | 'Á' | 'Ã' => "a",
            'ç' | 'Ç' => "c",
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => "e",
            'î' | 'ï' | 'í' | 'Î' | 'Ï' | 'Í' => "i",
            'ô' | 'ö' | 'ó' | 'õ' | 'Ô' | 'Ö' | 'Ó' | 'Õ' => "o",
            'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => "u",
            'ÿ' | 'Ÿ' | 'ý' => "y",
            'ñ' | 'Ñ' => "n",
            'œ' | 'Œ' => "oe",
            'æ' | 'Æ' => "ae",
            _ => {
                if c.is_ascii_alphanumeric() {
                    out.push(c.to_ascii_lowercase());
                } else if !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
                continue;
            }
        };
        out.push_str(folded);
    }
    out.trim_matches('-').to_string()
}

fn mapping_error(siret: &str, reason: &str) -> DomainError {
    DomainError::Invariant(format!("sirene: établissement {siret}: {reason}"))
}

/// Best-effort display name, in INSEE priority order (shop sign → usual name → legal name → person).
fn display_name(e: &Etablissement) -> Option<String> {
    let period = e.current_period();
    if let Some(p) = period {
        if let Some(name) = clean(&p.enseigne_1_etablissement) {
            return Some(name.to_string());
        }
        if let Some(name) = clean(&p.denomination_usuelle_etablissement) {
            return Some(name.to_string());
        }
    }
    let ul = e.unite_legale.as_ref()?;
    if let Some(name) = clean(&ul.denomination_usuelle_1_unite_legale) {
        return Some(name.to_string());
    }
    if let Some(name) = clean(&ul.denomination_unite_legale) {
        return Some(name.to_string());
    }
    // Personne physique (sole trader): "Prénom Nom(d'usage)".
    let last = clean(&ul.nom_usage_unite_legale).or_else(|| clean(&ul.nom_unite_legale))?;
    match clean(&ul.prenom_1_unite_legale) {
        Some(first) => Some(format!("{first} {last}")),
        None => Some(last.to_string()),
    }
}

/// NAF/APE code of the current period, falling back to the unité légale's.
fn naf_code(e: &Etablissement) -> Option<String> {
    e.current_period()
        .and_then(|p| clean(&p.activite_principale_etablissement).map(str::to_string))
        .or_else(|| {
            e.unite_legale
                .as_ref()
                .and_then(|ul| clean(&ul.activite_principale_unite_legale).map(str::to_string))
        })
}

/// Best-effort cuisine from NAF: only the two unambiguous codes are mapped; everything else is left
/// for the admin / Google enrichment (ADR-0020) — never guessed.
fn cuisine_from_naf(naf: Option<&str>) -> Option<CuisineCategory> {
    match naf {
        Some("56.10A") => Some(CuisineCategory::TRADITIONAL), // restauration traditionnelle
        Some("56.10C") => Some(CuisineCategory::FAST_FOOD),   // restauration de type rapide
        _ => None,
    }
}

fn address(e: &Etablissement) -> Result<Address, DomainError> {
    let a = e
        .adresse_etablissement
        .as_ref()
        .ok_or_else(|| mapping_error(&e.siret, "no adresseEtablissement"))?;
    let street: String = [
        clean(&a.numero_voie_etablissement),
        clean(&a.indice_repetition_etablissement), // bis/ter…
        clean(&a.type_voie_etablissement),         // RUE/AV/BD… (INSEE abbreviation, kept as-is)
        clean(&a.libelle_voie_etablissement),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    let complement = clean(&a.complement_adresse_etablissement);
    let city = clean(&a.libelle_commune_etablissement)
        .ok_or_else(|| mapping_error(&e.siret, "no commune label"))?;
    // Fall back to the complement, then the commune label, so sparsely-addressed prospects (e.g.
    // food trucks registered at a domicile) are still listed rather than dropped.
    let (line1, line2) = if !street.is_empty() {
        (street, complement.map(str::to_string))
    } else if let Some(c) = complement {
        (c.to_string(), None)
    } else {
        (city.to_string(), None)
    };
    Ok(Address {
        line1: AddressLine(line1),
        line2: line2.map(AddressLine),
        postal_code: PostalCode(
            clean(&a.code_postal_etablissement)
                .ok_or_else(|| mapping_error(&e.siret, "no postal code"))?
                .to_string(),
        ),
        city: CityName(city.to_string()),
        country: CountryCode("FR".to_string()), // SIRENE is the French registry
    })
}

/// Map one Sirene établissement to the existing `RegisterRestaurant` command (pure — no I/O).
/// Rejects closed/unusable records with a descriptive error; the runner logs and skips those.
pub fn etablissement_to_command(e: &Etablissement) -> Result<RegisterRestaurant, DomainError> {
    let siret = e.siret.trim();
    if siret.len() != 14 || !siret.bytes().all(|b| b.is_ascii_digit()) {
        return Err(mapping_error(&e.siret, "SIRET is not 14 digits"));
    }
    if let Some(period) = e.current_period() {
        match period.etat_administratif_etablissement.as_deref() {
            None | Some("A") => {}
            Some(state) => {
                return Err(mapping_error(siret, &format!("not administratively active ({state})")))
            }
        }
    }

    let name =
        display_name(e).ok_or_else(|| mapping_error(siret, "no usable name (enseigne/denomination)"))?;
    let address = address(e)?;
    let naf = naf_code(e);

    let nic = &siret[9..]; // last 5 digits — unique per establishment within the legal unit
    let base = slugify(&name);
    let slug = if base.is_empty() { format!("restaurant-{nic}") } else { format!("{base}-{nic}") };

    let mut external_identifiers = vec![ExternalIdentifier {
        key: ExternalIdentifierKey("siret".to_string()),
        value: siret.to_string(),
    }];
    if let Some(naf) = &naf {
        external_identifiers.push(ExternalIdentifier {
            key: ExternalIdentifierKey("naf".to_string()),
            value: naf.clone(),
        });
    }

    Ok(RegisterRestaurant {
        mode: None,
        restaurant_id: restaurant_id_for_siret(siret),
        account_id: None, // a prospect has no owning RestaurantAccount yet (ADR-0027)
        listing_status: Some(RestaurantListingStatus::NON_PARTNER),
        slug: Slug(slug),
        display_name: RestaurantDisplayName(name),
        contact: None,  // SIRENE exposes no email/phone
        website: None,  // Google-enrichment territory (ADR-0020)
        tags: vec![],
        margin_rate: None,
        cuisine_category: cuisine_from_naf(naf.as_deref()),
        uber_prices_opt_in: None,
        address,
        location: None, // SIRENE coordinates are Lambert-93, not WGS84 — enrichment fills this later
        timezone: Some(TimeZone("Europe/Paris".to_string())), // scope is metropolitan dept 37
        preparation_time_minutes: None,
        opening_hours: vec![], // unknown from SIRENE
        external_identifiers,
        r#ref: Some(ExternalReference(siret.to_string())),
    })
}

// ---------------------------------------------------------------------------------------------
// Tests (pure — no network, no DB)
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A realistic Sirene 3.11 `/siret` établissement (subset of fields, real shape/casing).
    fn sample_etablissement_json() -> &'static str {
        r#"{
            "siren": "852421099",
            "nic": "00021",
            "siret": "85242109900021",
            "uniteLegale": {
                "denominationUniteLegale": "SARL CHEZ MARCO",
                "activitePrincipaleUniteLegale": "56.10A",
                "etatAdministratifUniteLegale": "A"
            },
            "adresseEtablissement": {
                "numeroVoieEtablissement": "12",
                "indiceRepetitionEtablissement": null,
                "typeVoieEtablissement": "RUE",
                "libelleVoieEtablissement": "NATIONALE",
                "complementAdresseEtablissement": null,
                "codePostalEtablissement": "37000",
                "libelleCommuneEtablissement": "TOURS",
                "codeCommuneEtablissement": "37261"
            },
            "periodesEtablissement": [
                {
                    "dateFin": null,
                    "dateDebut": "2019-07-01",
                    "etatAdministratifEtablissement": "A",
                    "enseigne1Etablissement": "CHEZ MARCO",
                    "denominationUsuelleEtablissement": null,
                    "activitePrincipaleEtablissement": "56.10A"
                }
            ]
        }"#
    }

    fn sample() -> Etablissement {
        serde_json::from_str(sample_etablissement_json()).expect("parse sample établissement")
    }

    #[test]
    fn maps_a_real_shaped_etablissement_to_register_restaurant() {
        let cmd = etablissement_to_command(&sample()).expect("mapping succeeds");

        assert_eq!(cmd.r#ref, Some(ExternalReference("85242109900021".into()))); // ref = SIRET
        assert_eq!(cmd.display_name.0, "CHEZ MARCO"); // enseigne wins over denomination
        assert_eq!(cmd.slug.0, "chez-marco-00021"); // slugified name + NIC suffix
        assert_eq!(cmd.listing_status, Some(RestaurantListingStatus::NON_PARTNER)); // a prospect
        assert_eq!(cmd.account_id, None);
        assert_eq!(cmd.cuisine_category, Some(CuisineCategory::TRADITIONAL)); // NAF 56.10A
        assert!(cmd.opening_hours.is_empty());
        assert_eq!(cmd.address.line1.0, "12 RUE NATIONALE");
        assert_eq!(cmd.address.postal_code.0, "37000");
        assert_eq!(cmd.address.city.0, "TOURS");
        assert_eq!(cmd.address.country.0, "FR");
        assert_eq!(cmd.timezone, Some(TimeZone("Europe/Paris".into())));
        let ids: Vec<(&str, &str)> = cmd
            .external_identifiers
            .iter()
            .map(|i| (i.key.0.as_str(), i.value.as_str()))
            .collect();
        assert_eq!(ids, vec![("siret", "85242109900021"), ("naf", "56.10A")]);
    }

    #[test]
    fn restaurant_id_is_deterministic_from_the_siret() {
        // Stable across calls (and therefore across sync runs → idempotent registration)…
        let a = etablissement_to_command(&sample()).unwrap().restaurant_id;
        let b = etablissement_to_command(&sample()).unwrap().restaurant_id;
        assert_eq!(a, b);
        assert_eq!(a, restaurant_id_for_siret("85242109900021"));
        // …and different for a different SIRET.
        assert_ne!(a, restaurant_id_for_siret("85242109900039"));
        // v5 UUID, version nibble = 5.
        assert_eq!(a.0.get_version_num(), 5);
    }

    #[test]
    fn slug_matches_the_domain_pattern_even_for_accented_messy_names() {
        let mut e = sample();
        e.periodes_etablissement[0].enseigne_1_etablissement =
            Some("  CRÊPERIE L'ÉTOILE — Chez Œdipe & Co !!".into());
        let slug = etablissement_to_command(&e).unwrap().slug.0;
        assert_eq!(slug, "creperie-l-etoile-chez-oedipe-co-00021");
        let re_ok = slug
            .split('-')
            .all(|seg| !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()));
        assert!(re_ok, "slug {slug} must match ^[a-z0-9]+(?:-[a-z0-9]+)*$");
    }

    #[test]
    fn falls_back_to_denomination_and_no_cuisine_for_a_bar() {
        let mut e = sample();
        e.periodes_etablissement[0].enseigne_1_etablissement = None;
        e.periodes_etablissement[0].activite_principale_etablissement = Some("56.30Z".into());
        let cmd = etablissement_to_command(&e).unwrap();
        assert_eq!(cmd.display_name.0, "SARL CHEZ MARCO");
        assert_eq!(cmd.cuisine_category, None); // bars are not guessed
        assert_eq!(
            cmd.external_identifiers.last().map(|i| i.value.clone()),
            Some("56.30Z".into())
        );
    }

    #[test]
    fn rejects_closed_establishments_and_bad_sirets() {
        let mut closed = sample();
        closed.periodes_etablissement[0].etat_administratif_etablissement = Some("F".into());
        assert!(etablissement_to_command(&closed).is_err());

        let mut bad = sample();
        bad.siret = "1234".into();
        assert!(etablissement_to_command(&bad).is_err());

        let mut nameless = sample();
        nameless.periodes_etablissement[0].enseigne_1_etablissement = None;
        nameless.unite_legale = None;
        assert!(etablissement_to_command(&nameless).is_err());
    }

    #[test]
    fn parses_a_page_and_resolves_cursor_termination() {
        let body = format!(
            r#"{{
                "header": {{ "statut": 200, "message": "OK", "total": 1201, "debut": 0, "nombre": 1,
                             "curseur": "*", "curseurSuivant": "AoEpOTYxODAwNDI1" }},
                "etablissements": [ {} ]
            }}"#,
            sample_etablissement_json()
        );
        let page = parse_page(&body, "*").expect("parse page");
        assert_eq!(page.total, 1201);
        assert_eq!(page.etablissements.len(), 1);
        assert_eq!(page.next_cursor.as_deref(), Some("AoEpOTYxODAwNDI1")); // more pages

        // Last page: INSEE echoes the same cursor back as curseurSuivant.
        let last = body.replace("AoEpOTYxODAwNDI1", "*");
        assert!(parse_page(&last, "*").unwrap().next_cursor.is_none());
    }

    #[test]
    fn builds_the_documented_query() {
        let q = restauration_query(&SireneScope::Commune(TOURS_CODE_COMMUNE.into()));
        assert_eq!(
            q,
            "codeCommuneEtablissement:37261 AND periode(etatAdministratifEtablissement:A AND \
             (activitePrincipaleEtablissement:56.10A OR activitePrincipaleEtablissement:56.10B OR \
             activitePrincipaleEtablissement:56.10C OR activitePrincipaleEtablissement:56.30Z))"
        );
        let dept = restauration_query(&SireneScope::Department("37".into()));
        assert!(dept.starts_with("codeCommuneEtablissement:37* AND "));
    }
}

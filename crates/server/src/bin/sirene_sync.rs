//! SIRENE → Captain.Food prospect synchronizer (ADR-0019/0020/0027).
//!
//! Pulls currently-active food-service établissements (NAF 56.10A/B/C + 56.30Z) for the target
//! geography (Tours, code commune 37261, or a whole department) from the INSEE Sirene API, maps each
//! through the SIRENE ACL (`infrastructure::integrations::sirene`) and registers it as a NON_PARTNER
//! prospect via the NORMAL write path: `application::commands::register_restaurant` →
//! `RestaurantRegistered` in `domain_events`. The existing `ProjectionWorker` (in-process on the web
//! service, or the `projector` bin) then folds it into the `restaurant` table — this runner owns no
//! read/projection/GraphQL code.
//!
//! Idempotent by construction: the restaurantId is a UUIDv5 of the SIRET, so a re-run replays the
//! same client-generated ids and `register_restaurant` absorbs them as no-ops.
//!
//! Usage: `sirene_sync --once` (designed for a scheduled GitHub Actions run).
//! Env:
//! - `DATABASE_URL`     (required) — Postgres; the event store to append to.
//! - `INSEE_API_TOKEN`  (required) — API key from the INSEE portal (portail-api.insee.fr), sent as
//!   the `X-INSEE-Api-Key-Integration` header.
//! - `INSEE_API_BASE_URL` (optional) — overrides `https://api.insee.fr/api-sirene/3.11`.
//! - `SIRENE_DEPARTMENT` (optional) — sync a whole department (e.g. `37`) instead of one commune.
//! - `SIRENE_CODE_COMMUNE` (optional) — commune to sync; default `37261` (Tours).

use application::commands::register_restaurant;
use application::ports::Actor;
use infrastructure::integrations::sirene::{
    etablissement_to_command, restauration_query, sirene_system_user_id, SireneClient, SireneScope,
    TOURS_CODE_COMMUNE,
};
use infrastructure::PgEventStore;

/// The new INSEE portal's public quota is ~30 requests/minute — pace page fetches accordingly.
const PAGE_PAUSE: std::time::Duration = std::time::Duration::from_millis(2100);
const PAGE_SIZE: u32 = 1000; // Sirene's maximum `nombre` — Tours-scale scopes fit in one page

#[tokio::main]
async fn main() {
    if !std::env::args().any(|a| a == "--once") {
        eprintln!("usage: sirene_sync --once   (one full sync pass, then exit)");
        std::process::exit(2);
    }

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect(&url)
        .await
        .expect("connect to DATABASE_URL");
    let store = PgEventStore::new(pool);

    let client = SireneClient::from_env().unwrap_or_else(|e| {
        eprintln!("sirene_sync: {e}");
        std::process::exit(2);
    });

    // Fixed system principal for the event envelope (ADR-0041); fresh correlation id per run so all
    // events of one sync pass are traceable together.
    let actor = Actor {
        user_id: sirene_system_user_id(),
        user_type: 6, // UserType::EXTERNAL ordinal (enums stored as declaration-order ints, ADR-0037)
        correlation_id: uuid::Uuid::new_v4(),
        cause_id: None,
    };

    let scope = match std::env::var("SIRENE_DEPARTMENT") {
        Ok(dept) if !dept.trim().is_empty() => SireneScope::Department(dept.trim().to_string()),
        _ => SireneScope::Commune(
            std::env::var("SIRENE_CODE_COMMUNE")
                .ok()
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| TOURS_CODE_COMMUNE.to_string()),
        ),
    };
    let query = restauration_query(&scope);
    println!("sirene_sync: scope {scope:?} — q={query}");

    let (mut fetched, mut registered, mut skipped, mut failed) = (0usize, 0usize, 0usize, 0usize);
    let mut cursor = "*".to_string();
    loop {
        let page = match client.fetch_page(&query, &cursor, PAGE_SIZE).await {
            Ok(page) => page,
            Err(e) => {
                // A page-level failure (after the client's own retries) aborts the RUN, not silently
                // half-syncs: report what was done and exit non-zero so the workflow surfaces it.
                eprintln!("sirene_sync: page fetch failed at cursor {cursor}: {e}");
                eprintln!(
                    "sirene_sync: partial summary — fetched {fetched}, registered {registered}, \
                     skipped {skipped}, failed {failed}"
                );
                std::process::exit(1);
            }
        };
        fetched += page.etablissements.len();

        for etablissement in &page.etablissements {
            match etablissement_to_command(etablissement) {
                Ok(cmd) => match register_restaurant(&store, cmd, &actor).await {
                    // Ok covers BOTH a new registration and an idempotent replay of a known SIRET.
                    Ok(()) => registered += 1,
                    Err(e) => {
                        failed += 1;
                        eprintln!(
                            "sirene_sync: register failed for siret {}: {e}",
                            etablissement.siret
                        );
                    }
                },
                Err(e) => {
                    // Unusable record (closed, redacted, nameless…) — log + continue, never crash.
                    skipped += 1;
                    eprintln!("sirene_sync: skipped: {e}");
                }
            }
        }

        match page.next_cursor {
            Some(next) => {
                cursor = next;
                tokio::time::sleep(PAGE_PAUSE).await; // stay under the INSEE rate limit
            }
            None => break,
        }
    }

    println!(
        "sirene_sync: done — fetched {fetched}, registered (new or already known) {registered}, \
         skipped (unmappable) {skipped}, failed (write errors) {failed}"
    );
    // Record-level write failures are visible above but don't fail the run wholesale unless
    // NOTHING succeeded while records were present — that smells like a systemic DB problem.
    if failed > 0 && registered == 0 && fetched > 0 {
        std::process::exit(1);
    }
}

//! Captain.Food server (Axum BFF) — the composition root (ADR-0035).
//!
//! This is where dependency injection happens: concrete `infrastructure` adapters are constructed and
//! injected behind the `application` ports, then the HTTP/GraphQL/SDUI surface is built over them. Exposed
//! as a library so `desktop` (Tauri) can embed the same server in-process. Referencing `application`,
//! `infrastructure` and `shared_types` proves the server → those-three edges.

use application::ports::RestaurantRepository;
use infrastructure::PgRestaurantRepository;
use shared_types::HealthDto;

/// Build the application wiring (skeleton). The real version returns the Axum `Router` after injecting all
/// adapters; for now it just constructs a port impl and reports health.
pub fn wire() -> HealthDto {
    let _restaurants: Box<dyn RestaurantRepository> = Box::new(PgRestaurantRepository);
    HealthDto::ok()
}

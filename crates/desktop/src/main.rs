//! Captain.Food desktop shell (Tauri 2.0) — skeleton (ADR-0035).
//! Embeds the Axum `server` and the Leptos `web` frontend in one process; for now it just wires both to
//! prove the desktop → server, web edges compile.

fn main() {
    let server_health = server::wire();
    let web_health = web::boot();
    println!(
        "captain-food desktop skeleton — server: {}, web: {}",
        server_health.status, web_health.status
    );
}

//! Captain.Food server binary — thin entry point over the `server` library wiring (ADR-0035).
//! Becomes `axum::serve(listener, server::router())` once the HTTP surface lands.

fn main() {
    let health = server::wire();
    println!("captain-food server skeleton — status: {}", health.status);
}

//! Standalone projection worker (ADR-0040/0043) — the graduation path off the in-process worker.
//!
//! `--once` drains pending events and exits (free GitHub Actions cron / CI); `--loop` (default) polls
//! forever (a paid Render Background Worker). The web service runs the worker in-process today
//! (RUN_PROJECTOR); set RUN_PROJECTOR=false there once this runs separately.

use infrastructure::ProjectionWorker;

#[tokio::main]
async fn main() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect(&url)
        .await
        .expect("connect to DATABASE_URL");

    let worker = ProjectionWorker::new(pool);
    if std::env::args().any(|a| a == "--once") {
        match worker.run_once().await {
            Ok(()) => println!("projector: drained pending events (--once)"),
            Err(e) => {
                eprintln!("projector --once failed: {e}");
                std::process::exit(1);
            }
        }
    } else {
        println!("projector: polling loop (--loop) — Ctrl-C to stop");
        worker.run_loop().await;
    }
}

//! The GraphQL master schema (ADR-0006). Stage 1 exposes a scaffold `QueryRoot` so the schema builds and
//! introspects; the generated output/input types and the real read resolvers land next. Read-model
//! repositories will be injected via `.data(...)` in `build_schema`.

use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema};

pub type CaptainSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Scaffold field so the schema is non-empty and introspectable. Superseded by the generated query
    /// fields + read-model resolvers.
    async fn api_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

/// Build the master schema served under every role path. Read-model repositories are injected here via
/// `.data(...)` once the read resolvers land.
pub fn build_schema() -> CaptainSchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish()
}

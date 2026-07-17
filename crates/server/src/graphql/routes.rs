//! Role-as-path GraphQL endpoints (ADR-0006). The master schema is mounted under `/{role}/graphql`; the
//! role is parsed from the path and injected into the request context (the ACL seam — the runtime guard
//! that filters `@auth` fields per role is deferred). `GET` renders GraphiQL, `POST` executes.

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};

use super::acl::RequestRole;
use super::schema::CaptainSchema;

/// Mount `/{role}/graphql` for the seven roles (unknown role segments 404). Returns a `Router<()>` (the
/// schema is applied as state) so it can be merged into the main router.
pub fn graphql_routes(schema: CaptainSchema) -> Router {
    Router::new()
        .route("/{role}/graphql", get(graphiql).post(graphql_handler))
        .with_state(schema)
}

async fn graphql_handler(
    State(schema): State<CaptainSchema>,
    Path(role_seg): Path<String>,
    req: GraphQLRequest,
) -> Response {
    match RequestRole::from_segment(&role_seg) {
        // Inject the role so a future ACL guard can filter @auth fields per path (ADR-0006).
        Some(role) => {
            let resp: GraphQLResponse = schema.execute(req.into_inner().data(role)).await.into();
            resp.into_response()
        }
        None => (StatusCode::NOT_FOUND, "unknown role path").into_response(),
    }
}

async fn graphiql(Path(role_seg): Path<String>) -> Response {
    match RequestRole::from_segment(&role_seg) {
        Some(role) => Html(
            GraphiQLSource::build()
                .endpoint(&format!("/{}/graphql", role.segment()))
                .finish(),
        )
        .into_response(),
        None => (StatusCode::NOT_FOUND, "unknown role path").into_response(),
    }
}

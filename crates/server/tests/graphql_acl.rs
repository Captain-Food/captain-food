//! Per-role GraphQL ACL enforcement (ADR-0006 "role = path"), spec-derived from api.yaml `roles`.
//! Executes against the schema directly with a `RequestRole` in the request context (what
//! `/{role}/graphql` injects from the URL path) — no DB needed (`build_schema(None, None)`):
//! - EXECUTION: a role calling an operation outside its api.yaml `roles` gets a FORBIDDEN error
//!   (extension `code`) and the resolver never runs; an authorized role reaches the resolver.
//! - INTROSPECTION: a role only sees its authorized fields, and (via async-graphql's
//!   `find_visible_types`) only the types reachable through them — this is what per-role Voyager renders.
//! - PUBLIC operations (api.yaml `roles` include PUBLIC) are open to every role, including the
//!   unauthenticated PUBLIC path; a request context without a role fails closed to PUBLIC.

use async_graphql::Request;
use serde_json::Value;
use server::graphql_acl::RequestRole;
use server::graphql_schema::{build_schema, CaptainSchema};

fn schema() -> CaptainSchema {
    // No read/write deps: ACL runs before resolvers, and introspection needs none.
    build_schema(None, None)
}

/// Execute `query` under `role` (mirrors routes.rs' `request.data(role)`).
async fn execute_as(schema: &CaptainSchema, role: RequestRole, query: &str) -> async_graphql::Response {
    schema.execute(Request::new(query).data(role)).await
}

/// True when the error is the RoleGuard rejection (extension `code: FORBIDDEN`).
fn is_forbidden(err: &async_graphql::ServerError) -> bool {
    serde_json::to_value(err)
        .ok()
        .and_then(|v| v.get("extensions").and_then(|e| e.get("code")).cloned())
        == Some(serde_json::json!("FORBIDDEN"))
}

/// The Query/Mutation field names this role's introspection exposes.
async fn introspected_fields(schema: &CaptainSchema, role: RequestRole) -> (Vec<String>, Vec<String>) {
    let resp = execute_as(
        schema,
        role,
        "{ __schema { queryType { fields { name } } mutationType { fields { name } } } }",
    )
    .await;
    assert!(resp.errors.is_empty(), "introspection errored: {:?}", resp.errors);
    let data = resp.data.into_json().expect("introspection json");
    let names = |v: &Value| -> Vec<String> {
        v["fields"]
            .as_array()
            .expect("fields array")
            .iter()
            .map(|f| f["name"].as_str().expect("field name").to_string())
            .collect()
    };
    (names(&data["__schema"]["queryType"]), names(&data["__schema"]["mutationType"]))
}

/// Whether this role's introspection resolves `__type(name:)` (types reachable only through hidden
/// fields are hidden too — async-graphql's `find_visible_types`).
async fn type_visible(schema: &CaptainSchema, role: RequestRole, ty: &str) -> bool {
    let resp =
        execute_as(schema, role, &format!("{{ __type(name: \"{ty}\") {{ name }} }}")).await;
    assert!(resp.errors.is_empty(), "__type errored: {:?}", resp.errors);
    !resp.data.into_json().expect("__type json")["__type"].is_null()
}

/// Introspection is role-filtered: PUBLIC does not see @auth-only operations (`prospectionPipeline` is
/// [ADMIN], `registerRestaurant` is [ADMIN, RESTAURANT_ACCOUNT]) nor the types reachable only through
/// them; ADMIN sees them; RESTAURANT sees neither (not in either roles list). Public operations show
/// for everyone.
#[tokio::test]
async fn introspection_is_filtered_per_role() {
    let schema = schema();

    let (public_q, public_m) = introspected_fields(&schema, RequestRole::Public).await;
    assert!(public_q.contains(&"restaurants".into()), "public query missing: {public_q:?}");
    assert!(!public_q.contains(&"prospectionPipeline".into()), "admin-only query leaked to PUBLIC");
    assert!(!public_q.contains(&"pricingPolicy".into()), "admin-only query leaked to PUBLIC");
    assert!(public_m.contains(&"verifyPhone".into()), "public mutation missing: {public_m:?}");
    assert!(!public_m.contains(&"registerRestaurant".into()), "@auth mutation leaked to PUBLIC");

    let (admin_q, admin_m) = introspected_fields(&schema, RequestRole::Admin).await;
    assert!(admin_q.contains(&"prospectionPipeline".into()), "ADMIN query missing: {admin_q:?}");
    assert!(admin_q.contains(&"restaurants".into()), "public query missing under ADMIN");
    assert!(admin_m.contains(&"registerRestaurant".into()), "ADMIN mutation missing: {admin_m:?}");

    let (rest_q, rest_m) = introspected_fields(&schema, RequestRole::Restaurant).await;
    assert!(!rest_q.contains(&"prospectionPipeline".into()), "admin-only query leaked to RESTAURANT");
    assert!(!rest_m.contains(&"registerRestaurant".into()), "mutation leaked to RESTAURANT");
    assert!(rest_q.contains(&"orders".into()), "RESTAURANT query missing: {rest_q:?}");

    // Type visibility follows field visibility: PricingPolicy is reachable only via admin-only
    // queries, RegisterRestaurantInput/Payload only via registerRestaurant.
    for ty in ["PricingPolicy", "RegisterRestaurantInput", "RegisterRestaurantPayload"] {
        assert!(!type_visible(&schema, RequestRole::Public, ty).await, "{ty} leaked to PUBLIC");
        assert!(!type_visible(&schema, RequestRole::Restaurant, ty).await, "{ty} leaked to RESTAURANT");
        assert!(type_visible(&schema, RequestRole::Admin, ty).await, "{ty} missing under ADMIN");
    }
    assert!(type_visible(&schema, RequestRole::Public, "Restaurant").await, "public type hidden");
}

/// Executing an operation outside the role's api.yaml `roles` is rejected by the guard (FORBIDDEN)
/// before the resolver runs; an authorized role passes the guard and reaches the resolver.
#[tokio::test]
async fn unauthorized_execution_is_forbidden() {
    let schema = schema();
    let admin_query = "{ prospectionPipeline { score } }"; // [ADMIN]

    // PUBLIC → the guard rejects; the (wired) resolver never runs, so the only error is FORBIDDEN.
    let resp = execute_as(&schema, RequestRole::Public, admin_query).await;
    assert_eq!(resp.errors.len(), 1, "expected one error: {:?}", resp.errors);
    assert!(is_forbidden(&resp.errors[0]), "expected FORBIDDEN: {:?}", resp.errors[0]);
    // No role in the context at all (direct execution) fails closed to PUBLIC too.
    let resp = schema.execute(admin_query).await;
    assert!(is_forbidden(&resp.errors[0]), "missing role must fail closed: {:?}", resp.errors);

    // ADMIN → the guard passes; with no deps injected the resolver itself errors (missing repo),
    // which proves execution reached it — and it is NOT the FORBIDDEN rejection.
    let resp = execute_as(&schema, RequestRole::Admin, admin_query).await;
    assert_eq!(resp.errors.len(), 1, "expected the resolver error: {:?}", resp.errors);
    assert!(!is_forbidden(&resp.errors[0]), "guard must pass for ADMIN: {:?}", resp.errors[0]);

    // Same for a mutation: registerRestaurant is [ADMIN, RESTAURANT_ACCOUNT].
    let mutation = r#"mutation {
        registerRestaurant(input: {
            restaurantId: "00000000-0000-0000-0000-000000000001",
            slug: "chez-marco",
            displayName: "Chez Marco",
            address: { line1: "1 Rue Nationale", postalCode: "37000", city: "Tours", country: "FR" }
        }) { correlationId }
    }"#;
    for role in [RequestRole::Public, RequestRole::Restaurant, RequestRole::Rider] {
        let resp = execute_as(&schema, role, mutation).await;
        assert_eq!(resp.errors.len(), 1, "expected one error for {role:?}: {:?}", resp.errors);
        assert!(is_forbidden(&resp.errors[0]), "expected FORBIDDEN for {role:?}: {:?}", resp.errors[0]);
    }
    let resp = execute_as(&schema, RequestRole::RestaurantAccount, mutation).await;
    assert!(!is_forbidden(&resp.errors[0]), "guard must pass for RESTAURANT_ACCOUNT: {:?}", resp.errors);
}

/// PUBLIC operations run under the unauthenticated PUBLIC role — and under every other role.
#[tokio::test]
async fn public_operations_are_open_to_all_roles() {
    let schema = schema();
    // phoneCountries is [PUBLIC] and unwired: reaching its `not implemented` stub proves the ACL let
    // the resolver run (no DB in this test, so wired resolvers can't fully succeed).
    for role in [RequestRole::Public, RequestRole::Customer, RequestRole::Admin] {
        let resp = execute_as(&schema, role, "{ phoneCountries { dialingCode } }").await;
        assert_eq!(resp.errors.len(), 1, "expected the stub error for {role:?}: {:?}", resp.errors);
        assert!(!is_forbidden(&resp.errors[0]), "public op forbidden for {role:?}");
        assert_eq!(resp.errors[0].message, "not implemented", "resolver did not run for {role:?}");
    }
    // restaurants ([PUBLIC], wired) passes the ACL under PUBLIC: the only failure without deps is the
    // missing repository, never FORBIDDEN.
    let resp = execute_as(&schema, RequestRole::Public, "{ restaurants { slug } }").await;
    assert!(!resp.errors.is_empty() && !is_forbidden(&resp.errors[0]), "restaurants blocked: {:?}", resp.errors);
}

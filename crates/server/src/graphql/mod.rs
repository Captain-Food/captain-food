//! GraphQL BFF (ADR-0006 "role = path"). The SDL is generated from `api.yaml`; here we host it with
//! async-graphql. Stage 1 = runtime + schema serving (this scaffold); the generated output/input type
//! layer and the real read resolvers land next.

pub mod acl;
pub mod routes;
pub mod schema;

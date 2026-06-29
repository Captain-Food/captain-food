# ADR-0006 — GraphQL "role = path" ACL with generated SDL

## Status
Accepted

## Context
Multiple personas (PUBLIC, CUSTOMER, RESTAURANT_ACCOUNT, RESTAURANT, RIDER, ADMIN, EXTERNAL) share one
domain but must see different slices of the API, with authorization that is simple to reason about and
hard to get wrong.

## Decision
`specs/api.yaml` is the single source for the GraphQL surface (output-type registry, queries, mutations,
and an `@auth`/`@public` ACL keyed by role). One master schema is served per role under
`/{role}/graphql`, filtered by the ACL — **role = path**. The SDL is **generated** to
`tools/codegen/out/schema.generated.graphql`; the hand-written `schema.graphql` was removed. The codegen
validates roles against `scalars.yaml#/UserType`, that each mutation maps to exactly one command, and
that query return types resolve.

## Alternatives considered
- Per-role hand-written schemas — drift and duplication.
- Field-level directives only, one endpoint — harder to reason about per-persona exposure.

## Consequences
### Positive
- Authorization is a path concern; the exposed slice per role is explicit and generated.
### Negative
- The gateway must enforce the ACL filter at runtime (deferred).
### Follow-up actions
- Story-map steps are validated against the ACL (persona may call the op).

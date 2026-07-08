//! Hand-written projector logic — the `…Compute` trait impls for the materialized read-model tables
//! (ADR-0040). The generator (crates/application/src/generated/projectors.rs) maps the mechanical columns
//! and calls these hooks for the computed / cross-stream / accumulate ones; this is where that business
//! logic lives, tested and out of the DB. One module per table as they are implemented.
//!
//! `Customer` is the worked example (identity fold + jsonb accumulations + the email-verified flag). The
//! remaining tables' impls land with the query/runtime layer.

pub mod customer;

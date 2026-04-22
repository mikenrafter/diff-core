//! Per-language extraction modules.
//!
//! Split out from the monolithic `query_engine.rs`. Each language owns its
//! bespoke `extract_imports` implementation, its `extract_definitions`
//! match-arm body, and its test cases.

pub(crate) mod common;

pub(crate) mod typescript;
pub(crate) mod python;
pub(crate) mod go;
pub(crate) mod rust;
pub(crate) mod java;
pub(crate) mod csharp;
pub(crate) mod php;
pub(crate) mod ruby;
pub(crate) mod kotlin;
pub(crate) mod swift;
pub(crate) mod c;
pub(crate) mod cpp;
pub(crate) mod scala;

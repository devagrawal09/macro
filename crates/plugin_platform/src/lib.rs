#![deny(missing_docs)]
//! Core domain for immutable Macro Plugin releases and project installations.
//!
//! The crate is a hexagonal bounded context. [`domain`] owns plugin policy and
//! use cases. [`outbound`] contains replaceable storage, authority, time, ID,
//! and signing adapters. HTTP composition and server execution are deliberately
//! outside this core checkpoint.

pub mod domain;
pub mod outbound;

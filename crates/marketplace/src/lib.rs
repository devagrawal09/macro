#![deny(missing_docs)]
//! The marketplace: HTML artifacts a person publishes for their team to
//! install, following the hexagonal architecture pattern.
//!
//! A listing's content is its versions: immutable snapshots of the page's
//! source. An install follows the newest minor version of the major it is on,
//! so a minor version reaches every install at once and a major version waits
//! for each person to take it.
//!
//! # Architecture
//!
//! - **domain**: models, ports, and the service that holds every policy.
//! - **inbound**: the Axum router.
//! - **outbound**: the Postgres repository and the page builder adapter.

pub mod domain;
pub mod inbound;
pub mod outbound;

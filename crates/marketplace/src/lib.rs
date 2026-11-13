#![deny(missing_docs)]
//! The marketplace: HTML artifacts and routines a person publishes for their
//! team to install, following the hexagonal architecture pattern.
//!
//! A listing's content is its versions: immutable snapshots of a page's
//! source or of a routine. An install follows the newest minor version of the
//! major it is on, so a minor version reaches every install at once and a
//! major version waits for each person to take it. A routine install is a
//! routine of the installer's own, which a minor version updates until they
//! edit it.
//!
//! # Architecture
//!
//! - **domain**: models, ports, and the service that holds every policy.
//! - **inbound**: the Axum router.
//! - **outbound**: the Postgres repository, the page builder adapter and the
//!   routines adapter over the scheduler's service.

pub mod domain;
pub mod inbound;
pub mod outbound;

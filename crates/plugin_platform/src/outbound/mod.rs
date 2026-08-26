//! Replaceable outbound adapters for the Plugin Platform core.

pub mod in_memory;

#[cfg(feature = "postgres")]
pub mod postgres;

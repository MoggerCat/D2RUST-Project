// Spec: specs/sim/intents-events.md
//! Client<->server message types, shared by `d2-server` and `d2-client`.
//!
//! The message ids, names, size rules and field layouts of both directions
//! are generated from the spec's machine tables
//! (`specs/sim/client-messages.tsv`, `server-messages.tsv`, §5) into
//! [`generated`] by `data-tool gen-proto`; never edit that file.
//!
//! - [`schema`]: descriptor types and the size-rule evaluation (§2.1 rule
//!   5, §3.1 rule 1).
//! - [`transport`]: size lookup per direction, the C→S classifier (§2.1
//!   rule 4) and the S→C buffer split (§3.3).
//! - [`generated::client`] / [`generated::server`]: typed decode/encode of
//!   the fixed layouts ([`wire::FixedMessage`]).
//! - [`tsv`]: strict TSV parser and the TSV-vs-code check; [`codegen`]: the
//!   generator.

pub mod codegen;
// Generated layout (one row per line); `cargo fmt` leaves it alone.
#[rustfmt::skip]
pub mod generated;
pub mod schema;
pub mod transport;
pub mod tsv;
pub mod wire;

#[cfg(test)]
mod mutant_tests;

pub use generated::{client, server, CLIENT_MESSAGES, SERVER_MESSAGES};
pub use wire::{DecodeError, FixedMessage};

/// Protocol version (METHODS M20). Covers the message tables generated
/// from the spec's TSVs and the typed layouts. Bump on any incompatible
/// message change; client and server refuse to talk across versions.
pub const PROTOCOL_VERSION: u32 = 1;

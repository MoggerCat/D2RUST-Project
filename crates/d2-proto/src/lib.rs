//! Client<->server message types, shared by `d2-server` and `d2-client`.
//! Phase 5 fills this in.

/// Protocol version. Bump on any incompatible message change; client and
/// server refuse to talk across versions.
pub const PROTOCOL_VERSION: u32 = 1;

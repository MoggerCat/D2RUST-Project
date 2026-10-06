// Spec: specs/render/lighting.md
//! The world's light (`render/lighting.md`): the 48 × 48 light map around
//! the local player, its ambient fill and blocks-light flags, the light
//! records and their contributions, the light quality, the day / night
//! environment, the scripted ambient overrides, the light sources and the
//! light value each draw receives. Plain Rust, client state only, no Bevy
//! types; the environment (§9, §10) uses doubles and `sin` as the original
//! does (§13: client-only, never `d2-sim`).
//!
//! - [`map`]: the light map (§1), the build order (§2), the ambient fill
//!   (§3), the blocks-light flags (§4), the frame-end digest (§12).
//! - [`records`]: light records and the list (§6).
//! - [`contribute`]: the contribution of one record (§7).
//! - [`quality`]: the light quality `q` and the measured draw rate (§5).
//! - [`environment`]: the act environment, day and night (§9).
//! - [`overrides`]: the scripted ambient overrides (§10).
//! - [`sources`]: what creates and changes light records (§8).
//! - [`draws`]: the light values handed to the draws (§11).
//! - [`view`]: [`view::LitRules`], the world view's `shade` / `blend`
//!   hooks from the frame's light map (§11 r1, `blend-modes.md` §3).

pub mod contribute;
pub mod draws;
pub mod environment;
pub mod map;
pub mod overrides;
pub mod quality;
pub mod records;
pub mod sources;

#[cfg(test)]
mod draws_tests;
pub mod view;

#[cfg(test)]
mod view_tests;

pub use map::{LightCell, LightMap};
#[cfg(test)]
mod core_tests;

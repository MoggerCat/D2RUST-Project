//! Bevy client. Renders state and sends intents; never decides outcomes.
//! All contact with the game goes through [`bridge`].
//!
//! [`map`] is plain Rust (no Bevy types): asset assembly and the CPU
//! reference renderer used to check the GPU output.

pub mod app;
pub mod assets;
pub mod bridge;
pub mod map;
pub mod render;

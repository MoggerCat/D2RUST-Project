//! Bevy client. Renders state and sends intents; never decides outcomes.
//! All contact with the game goes through [`bridge`].
//!
//! [`map`] is plain Rust (no Bevy types): asset assembly and the CPU
//! reference renderer used to check the GPU output. [`scene`] is plain
//! Rust too: the draw list and the CPU reference compositor. [`verify`]
//! runs the render cases (CPU reference vs GPU).
//! Rust too: the draw list and the CPU reference compositor, as is
//! [`composite`], which turns COF frames into scene draw items.

pub mod app;
pub mod assets;
pub mod audio;
pub mod bridge;
pub mod composite;
pub mod controls;
pub mod frames;
pub mod gpu_compositor;
pub mod map;
pub mod render;
pub mod scene;
pub mod ui;
pub mod verify;

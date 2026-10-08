// Spec: specs/ui/frontend-credits.md (C1 start-up chain, C5 video hook, C6 progress byte)
//! The start-up chain (`0x004359D0` → `0x00435230`) and the video stub.
//!
//! Video playback is a stub: [`VideoHook::play`] logs one line and returns
//! at once, as if skipped on its first frame; the caller keeps every side
//! effect (progress byte write, input flush, screen rebuild) in order.

/// Logo videos (C1 r2.3, mode 0).
pub const LOGO_VIDEOS: [&str; 2] = [
    r"Data\Local\Video\New_BLIZ640x480.bik",
    r"Data\Local\Video\BlizNorth640x480.bik",
];
/// The classic intro (first run; C5 r1 with `ENG` and `640x292`).
pub const CLASSIC_INTRO: &str = r"DATA\LOCAL\video\ENG\d2intro640x292.bik";
/// The expansion intro (C1 r2.5).
pub const EXPANSION_INTRO: &str = r"DATA\LOCAL\video\ENG\D2x_Intro_640x292.bik";

/// Progress byte default (C6 r1: `216.148.246.34`).
pub const PROGRESS_DEFAULT: u8 = 0x22;

/// One video request. Implemented by the host; the stub logs.
pub trait VideoHook {
    fn play(&mut self, path: &str);
}

/// The d2rs stub (C5 r6): `video stub: <path>`, then return.
#[derive(Debug, Default)]
pub struct StubVideo;

impl VideoHook for StubVideo {
    fn play(&mut self, path: &str) {
        bevy::log::info!("video stub: {path}");
    }
}

/// Records requests (tests).
#[derive(Debug, Default)]
pub struct RecordVideo(pub Vec<String>);

impl VideoHook for RecordVideo {
    fn play(&mut self, path: &str) {
        self.0.push(path.to_owned());
    }
}

/// The cinematics progress byte N (C6): original registry value
/// `Aux Battle.net`; d2rs keeps it in client settings.
pub trait ProgressStore {
    fn get(&self) -> Option<u8>;
    fn set(&mut self, n: u8);
}

/// In-memory store (the host swaps in its settings file).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemProgress(pub Option<u8>);

impl ProgressStore for MemProgress {
    fn get(&self) -> Option<u8> {
        self.0
    }
    fn set(&mut self, n: u8) {
        self.0 = Some(n);
    }
}

/// `0x0042FAB0`: N has bit 0x10 but not bit 0x80.
pub fn wants_expansion_intro(n: u8) -> bool {
    n & 0x10 != 0 && n & 0x80 == 0
}

/// C1 r2 steps 1–5, everything before the trademark build. `backend`:
/// the video backend reports video (`0x004F9050`); the d2rs stub does.
/// Returns nothing: the effects are the hook calls, the store writes and
/// the caller's flush/teardown, which the driver performs around it.
pub fn run_startup(
    expansion: bool,
    backend: bool,
    store: &mut dyn ProgressStore,
    video: &mut dyn VideoHook,
) {
    if !backend {
        return;
    }
    for p in LOGO_VIDEOS {
        video.play(p);
    }
    match store.get() {
        None => {
            // `0x0042FA40` writes the default; the first run plays the intro
            // and skips step 5.
            store.set(PROGRESS_DEFAULT);
            video.play(CLASSIC_INTRO);
        }
        Some(n) if expansion && wants_expansion_intro(n) => {
            video.play(EXPANSION_INTRO);
            store.set(n | 0x80);
        }
        Some(_) => {}
    }
}

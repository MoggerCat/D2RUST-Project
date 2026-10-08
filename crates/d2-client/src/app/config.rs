// Spec: specs/ui/frontend-options.md (§O7, §O8), specs/client/ui.md (§A6)
//! The d2rs config files, kept next to the saves: `settings.toml`
//! (resolution, window mode) and `controls.toml` (key bindings, §A6). The
//! Esc menu's Options entry reads and writes them.
//!
//! A missing `controls.toml` is written from the `original` preset
//! (`ui/controls.md` §3, §B4); the `dev` preset applies only when a file
//! names it. The window mode row and the
//! `[video] window_mode` key are d2rs-own (the 1.14d menu has none).
// d2rs-own, unverified

use std::path::{Path, PathBuf};

use crate::controls::{self, Bindings, ControlsFile, Preset};

/// `settings.toml` format version (every persisted format has one).
pub const SETTINGS_VERSION: i64 = 1;

/// How the window is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowMode {
    #[default]
    Windowed,
    Borderless,
    Fullscreen,
}

impl WindowMode {
    pub const ALL: [WindowMode; 3] = [
        WindowMode::Windowed,
        WindowMode::Borderless,
        WindowMode::Fullscreen,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WindowMode::Windowed => "windowed",
            WindowMode::Borderless => "borderless",
            WindowMode::Fullscreen => "fullscreen",
        }
    }

    pub fn next(self) -> WindowMode {
        match self {
            WindowMode::Windowed => WindowMode::Borderless,
            WindowMode::Borderless => WindowMode::Fullscreen,
            WindowMode::Fullscreen => WindowMode::Windowed,
        }
    }
}

/// Every setting of `frontend-options.md` §O7 (1.14d integers, so a
/// registry import is a copy). `resolution` is 0 = 640x480, 1 = 800x600;
/// the frame stays 800 x 600 (§O8), the window is sized to it. The rows
/// of §O8 are stored and shown, with no effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub master_volume: u8,
    pub music_volume: u8,
    pub mixer: u8,
    pub positional_bias: u8,
    pub npc_speech: u8,
    pub resolution: u8,
    pub window_mode: WindowMode,
    pub light_quality: u8,
    pub blended_shadows: u8,
    pub perspective: u8,
    pub gamma: u16,
    pub contrast: u8,
    pub automap_mode: u8,
    pub automap_fade: u8,
    pub automap_centers: u8,
    pub automap_party: u8,
    pub automap_party_names: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            master_volume: 100,
            music_volume: 50,
            mixer: 0,
            positional_bias: 50,
            npc_speech: 2,
            resolution: 1,
            window_mode: WindowMode::Windowed,
            light_quality: 2,
            blended_shadows: 1,
            perspective: 1,
            gamma: 155,
            contrast: 100,
            automap_mode: 0,
            automap_fade: 0,
            automap_centers: 1,
            automap_party: 1,
            automap_party_names: 1,
        }
    }
}

/// The integer keys: `(section, key, min, max)`, §O7 r2 order.
pub const INT_KEYS: [(&str, &str, i64, i64); 16] = [
    ("audio", "master_volume", 0, 100),
    ("audio", "music_volume", 0, 100),
    ("audio", "mixer", 0, 2),
    ("audio", "positional_bias", 0, 100),
    ("audio", "npc_speech", 0, 2),
    ("video", "resolution", 0, 1),
    ("video", "light_quality", 0, 2),
    ("video", "blended_shadows", 0, 1),
    ("video", "perspective", 0, 1),
    ("video", "gamma", 55, 255),
    ("video", "contrast", 0, 100),
    ("automap", "mode", 0, 1),
    ("automap", "fade", 0, 3),
    ("automap", "centers", 0, 1),
    ("automap", "party", 0, 1),
    ("automap", "party_names", 0, 1),
];

impl Settings {
    /// The window size the resolution row asks for.
    pub fn window_size(&self) -> (u32, u32) {
        if self.resolution == 0 {
            (640, 480)
        } else {
            (800, 600)
        }
    }

    /// The value of integer key `i` of [`INT_KEYS`].
    pub fn get(&self, i: usize) -> i64 {
        match i {
            0 => self.master_volume.into(),
            1 => self.music_volume.into(),
            2 => self.mixer.into(),
            3 => self.positional_bias.into(),
            4 => self.npc_speech.into(),
            5 => self.resolution.into(),
            6 => self.light_quality.into(),
            7 => self.blended_shadows.into(),
            8 => self.perspective.into(),
            9 => self.gamma.into(),
            10 => self.contrast.into(),
            11 => self.automap_mode.into(),
            12 => self.automap_fade.into(),
            13 => self.automap_centers.into(),
            14 => self.automap_party.into(),
            _ => self.automap_party_names.into(),
        }
    }

    /// Set integer key `i` (the caller keeps `v` in range).
    pub fn set(&mut self, i: usize, v: i64) {
        match i {
            0 => self.master_volume = v as u8,
            1 => self.music_volume = v as u8,
            2 => self.mixer = v as u8,
            3 => self.positional_bias = v as u8,
            4 => self.npc_speech = v as u8,
            5 => self.resolution = v as u8,
            6 => self.light_quality = v as u8,
            7 => self.blended_shadows = v as u8,
            8 => self.perspective = v as u8,
            9 => self.gamma = v as u16,
            10 => self.contrast = v as u8,
            11 => self.automap_mode = v as u8,
            12 => self.automap_fade = v as u8,
            13 => self.automap_centers = v as u8,
            14 => self.automap_party = v as u8,
            _ => self.automap_party_names = v as u8,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{path}: {message}")]
    Io { path: String, message: String },
    #[error("settings.toml: {0}")]
    Settings(String),
}

/// The config folder: the parent of the save folder (`.../d2rs` for the
/// default `.../d2rs/saves`), else the save folder itself.
pub fn config_dir(save_dir: &Path) -> PathBuf {
    save_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| save_dir.to_path_buf(), Path::to_path_buf)
}

pub fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.toml")
}

pub fn controls_path(dir: &Path) -> PathBuf {
    dir.join("controls.toml")
}

/// The folder of `controls.toml`: the folder of `spec_path`, the
/// `<config_dir>/d2rs/controls.toml` of `client/ui.md` §A6 that the
/// Configure Controls screen saves to
/// ([`crate::ui::front_end::screens::controls::config_path`]), so a
/// rebind is what the next start loads; `fallback` when the platform has
/// no config dir.
pub fn controls_dir(spec_path: Option<&Path>, fallback: &Path) -> PathBuf {
    spec_path
        .and_then(Path::parent)
        .map_or_else(|| fallback.to_path_buf(), Path::to_path_buf)
}

/// Parse `settings.toml`. Strict: an unknown key, a wrong type or an
/// out-of-range value is an error naming the key.
pub fn parse_settings(text: &str) -> Result<Settings, ConfigError> {
    use toml_edit::Document;
    let bad = |m: String| ConfigError::Settings(m);
    let doc = Document::parse(text).map_err(|e| bad(e.message().to_string()))?;
    let mut s = Settings::default();
    let mut version = None;
    for (k, item) in doc.as_table().iter() {
        match k {
            "version" => version = item.as_integer(),
            "audio" | "video" | "automap" => {
                let t = item
                    .as_table_like()
                    .ok_or_else(|| bad(format!("[{k}] must be a table")))?;
                for (vk, v) in t.iter() {
                    if k == "video" && vk == "window_mode" {
                        let name = v.as_str().unwrap_or("");
                        s.window_mode = WindowMode::ALL
                            .into_iter()
                            .find(|m| m.name() == name)
                            .ok_or_else(|| {
                                bad(
                                    "video.window_mode must be windowed, borderless or fullscreen"
                                        .into(),
                                )
                            })?;
                        continue;
                    }
                    let Some(i) = INT_KEYS.iter().position(|e| e.0 == k && e.1 == vk) else {
                        return Err(bad(format!("unknown key {k}.{vk}")));
                    };
                    let (_, _, lo, hi) = INT_KEYS[i];
                    match v.as_integer() {
                        Some(n) if (lo..=hi).contains(&n) => s.set(i, n),
                        _ => return Err(bad(format!("{k}.{vk} must be {lo} to {hi}"))),
                    }
                }
            }
            other => return Err(bad(format!("unknown key {other}"))),
        }
    }
    match version {
        Some(SETTINGS_VERSION) => Ok(s),
        Some(v) => Err(bad(format!("version {v} is not supported"))),
        None => Err(bad("missing version".into())),
    }
}

pub fn write_settings(s: &Settings) -> String {
    let mut out = format!("version = {SETTINGS_VERSION}\n");
    let mut section = "";
    for (i, (sec, key, _, _)) in INT_KEYS.iter().enumerate() {
        if *sec != section {
            section = sec;
            out.push_str(&format!("\n[{sec}]\n"));
        }
        out.push_str(&format!("{key} = {}\n", s.get(i)));
        if *sec == "video" && *key == "resolution" {
            out.push_str(&format!("window_mode = \"{}\"\n", s.window_mode.name()));
        }
    }
    out
}

fn io_err(path: &Path, e: impl std::fmt::Display) -> ConfigError {
    ConfigError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

/// Write through a temp file and a rename, so a crash keeps the old file.
fn write_atomic(path: &Path, text: &str) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| io_err(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| io_err(path, e))
}

/// Load `settings.toml` from `dir`; a missing file is the defaults (and
/// is not created until a setting changes). A bad file is an error: no
/// silent default.
pub fn load_settings(dir: &Path) -> Result<Settings, ConfigError> {
    let path = settings_path(dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_settings(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(io_err(&path, e)),
    }
}

pub fn save_settings(dir: &Path, s: &Settings) -> Result<(), ConfigError> {
    write_atomic(&settings_path(dir), &write_settings(s))
}

/// Load `controls.toml` from `dir`; a missing file is created from the
/// `original` preset (`client/ui.md` §A6, `ui/controls.md` §B4) and
/// returned. The `dev` preset is used only when a file names it.
pub fn load_controls(dir: &Path) -> anyhow::Result<Bindings> {
    let path = controls_path(dir);
    if !path.exists() {
        let preset = Preset::Original;
        let bindings = preset.bindings().expect("original preset");
        let file = ControlsFile::from_effective(preset, &bindings).expect("original preset");
        write_atomic(&path, &controls::write(&file))?;
        return Ok(bindings);
    }
    Ok(controls::load(&path)?.1)
}

/// The play app's config: the folder and the settings in force.
#[derive(bevy::prelude::Resource, Clone, Debug)]
pub struct ConfigRes {
    pub dir: PathBuf,
    pub settings: Settings,
    pub bindings: Bindings,
}

impl WindowMode {
    pub fn bevy(self) -> bevy::window::WindowMode {
        use bevy::window::{MonitorSelection, VideoModeSelection};
        match self {
            WindowMode::Windowed => bevy::window::WindowMode::Windowed,
            WindowMode::Borderless => {
                bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            }
            WindowMode::Fullscreen => bevy::window::WindowMode::Fullscreen(
                MonitorSelection::Current,
                VideoModeSelection::Current,
            ),
        }
    }
}

/// The primary window the settings ask for (before the app starts).
pub fn window_for(s: &Settings) -> bevy::window::Window {
    let (w, h) = s.window_size();
    bevy::window::Window {
        title: "d2rs".into(),
        resolution: bevy::window::WindowResolution::new(w, h),
        mode: s.window_mode.bevy(),
        ..Default::default()
    }
}

/// Hands the settings to the Esc menu once, then writes `settings.toml`
/// and applies the window on each change the Options page makes.
pub fn apply_settings(
    ui: Option<bevy::prelude::NonSendMut<crate::world_view::WorldViewUi>>,
    cfg: Option<bevy::prelude::ResMut<ConfigRes>>,
    mut windows: bevy::prelude::Query<
        &mut bevy::window::Window,
        bevy::prelude::With<bevy::window::PrimaryWindow>,
    >,
    mut init: bevy::prelude::Local<bool>,
) {
    let (Some(mut ui), Some(mut cfg)) = (ui, cfg) else {
        return;
    };
    if ui.original.is_none() {
        return;
    }
    if !*init {
        ui.bindings = Some(cfg.bindings.clone());
        if let Some(o) = ui.original.as_mut() {
            o.set_settings(cfg.settings);
        }
        *init = true;
    }
    let Some(original) = ui.original.as_mut() else {
        return;
    };
    let Some(new) = original.take_settings_change() else {
        return;
    };
    cfg.settings = new;
    if let Err(e) = save_settings(&cfg.dir, &new) {
        bevy::log::error!("settings not saved: {e}");
    }
    for mut w in &mut windows {
        w.mode = new.window_mode.bevy();
        let (width, height) = new.window_size();
        w.resolution.set(width as f32, height as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("d2rs-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn missing_settings_are_the_defaults_and_roundtrip() {
        let d = tmp("rt");
        assert_eq!(load_settings(&d).unwrap(), Settings::default());
        let s = Settings {
            resolution: 0,
            window_mode: WindowMode::Fullscreen,
            music_volume: 45,
            gamma: 165,
            automap_fade: 3,
            ..Settings::default()
        };
        save_settings(&d, &s).unwrap();
        assert_eq!(load_settings(&d).unwrap(), s);
        assert_eq!(s.window_size(), (640, 480));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn settings_are_strict() {
        for bad in [
            "[video]\nresolution = 1\n",
            "version = 2\n",
            "version = 1\nfoo = 1\n",
            "version = 1\n[video]\nresolution = 2\n",
            "version = 1\n[video]\nwindow_mode = \"big\"\n",
            "version = 1\n[video]\nbogus = 1\n",
            "version = 1\n[audio]\nmaster_volume = 101\n",
            "version = 1\n[video]\ngamma = 54\n",
            "version = 1\n[automap]\nfade = 4\n",
        ] {
            assert!(parse_settings(bad).is_err(), "{bad}");
        }
    }

    // Play loads `controls.toml` from the folder the Configure Controls
    // screen saves to (`<config_dir>/d2rs/`); the save-folder parent only
    // when the platform has no config dir.
    // Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 text
    #[test]
    fn controls_dir_is_the_configure_controls_folder() {
        let spec = Path::new("/cfg/d2rs/controls.toml");
        let fallback = Path::new("/docs/d2rs");
        assert_eq!(controls_dir(Some(spec), fallback), Path::new("/cfg/d2rs"));
        assert_eq!(controls_path(&controls_dir(Some(spec), fallback)), spec);
        assert_eq!(controls_dir(None, fallback), fallback);
    }

    #[test]
    fn controls_file_is_created_once_then_read_back() {
        let d = tmp("ctl");
        let first = load_controls(&d).unwrap();
        assert!(controls_path(&d).exists());
        // An edit to the file is what the next run reads.
        let text = std::fs::read_to_string(controls_path(&d)).unwrap();
        std::fs::write(
            controls_path(&d),
            text.replace("[unbind]\nlist = []", "[unbind]\nlist = [\"swap_weapons\"]"),
        )
        .unwrap();
        let second = load_controls(&d).unwrap();
        assert_ne!(first, second);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn config_dir_is_beside_the_saves() {
        assert_eq!(
            config_dir(Path::new("/h/Documents/d2rs/saves")),
            PathBuf::from("/h/Documents/d2rs")
        );
        assert_eq!(config_dir(Path::new("saves")), PathBuf::from("saves"));
    }
}

// Spec: specs/ui/frontend-options.md (§O7, §O8), specs/client/ui.md (§A6)
//! The d2rs config files, kept next to the saves: `settings.toml`
//! (resolution, window mode) and `controls.toml` (key bindings, §A6). The
//! Esc menu's Options entry reads and writes them.
//!
//! PROVISIONAL (REC-172): the original's default key table has no
//! `Preset::Original` data in the tree yet, so a missing `controls.toml`
//! is written from the `dev` preset. The window mode row and the
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

/// The video settings. `resolution` keeps the 1.14d value (0 = 640x480,
/// 1 = 800x600, `frontend-options.md` §O7); the frame stays 800 x 600
/// (§O8), the window is sized to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub resolution: u8,
    pub window_mode: WindowMode,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            resolution: 1,
            window_mode: WindowMode::Windowed,
        }
    }
}

impl Settings {
    /// The window size the resolution row asks for.
    pub fn window_size(&self) -> (u32, u32) {
        if self.resolution == 0 {
            (640, 480)
        } else {
            (800, 600)
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
            "video" => {
                let t = item
                    .as_table_like()
                    .ok_or_else(|| bad("[video] must be a table".into()))?;
                for (vk, v) in t.iter() {
                    match vk {
                        "resolution" => {
                            s.resolution = match v.as_integer() {
                                Some(n @ 0..=1) => n as u8,
                                _ => return Err(bad("video.resolution must be 0 or 1".into())),
                            }
                        }
                        "window_mode" => {
                            let name = v.as_str().unwrap_or("");
                            s.window_mode = WindowMode::ALL
                                .into_iter()
                                .find(|m| m.name() == name)
                                .ok_or_else(|| {
                                    bad("video.window_mode must be windowed, borderless or fullscreen".into())
                                })?;
                        }
                        other => return Err(bad(format!("unknown key video.{other}"))),
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
    format!(
        "version = {SETTINGS_VERSION}\n\n[video]\nresolution = {}\nwindow_mode = \"{}\"\n",
        s.resolution,
        s.window_mode.name()
    )
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
/// `dev` preset (PROVISIONAL, REC-172) and returned.
pub fn load_controls(dir: &Path) -> anyhow::Result<Bindings> {
    let path = controls_path(dir);
    if !path.exists() {
        let preset = Preset::Dev;
        let bindings = preset.bindings().expect("dev preset");
        let file = ControlsFile::from_effective(preset, &bindings).expect("dev preset");
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
        ] {
            assert!(parse_settings(bad).is_err(), "{bad}");
        }
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

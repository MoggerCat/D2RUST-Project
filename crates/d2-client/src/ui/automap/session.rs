// Spec: specs/ui/automap.md (§5 r1, §7, §8 r2, §14)
//! The automap of a running game against the client model: the act
//! load opens the character's `.map` / `.ma<k>` files (§7 r1; the save
//! sub-directory first, the plain save directory when the sub-directory
//! file cannot be opened), each frame runs the reveal (§5 r1) on the
//! world view's near rooms, the toggle keys flip UI state 0x0A (§8 r2)
//! and the teardown saves the current layer and writes the data file
//! (§14 r3). Plain Rust: the world view calls it (no game logic in Bevy
//! systems).

use std::path::{Path, PathBuf};

use super::persist::{open_files, MaFile};
use super::place::{AutomapUnit, UnitCels};
use super::view::FrameFacts;
use super::{Automap, AutomapError, AutomapLevels, AutomapTables, CelPicker, RevealRooms};
use crate::bridge::world::{ClientWorld, UnitKey};
use crate::rules::camera::{moving_to_client, ClientPos};
use crate::rules::draw_order::NearRooms;

/// Where the character's automap files live (§7): the save directory,
/// the save sub-directory `[0x007A0500]` (if any) and the character name
/// `[0x007A05C4]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFiles {
    pub dir: PathBuf,
    pub sub: Option<String>,
    pub name: String,
}

/// §7 on disk with the sub-directory rule: `<dir>\<sub>\<name>.map` when
/// the sub-directory can be opened, else `<dir>\<name>.map`. Returns the
/// data file, its path and its slot.
///
/// Reading (`0x00457F40`): "cannot be opened" is the sub-directory not
/// existing as a directory; an I/O error inside an existing
/// sub-directory is an error, not a fallback.
pub fn open_files_in(
    files: &SaveFiles,
    key: u32,
) -> Result<(MaFile, PathBuf, usize), AutomapError> {
    if let Some(sub) = files.sub.as_deref().filter(|s| !s.is_empty()) {
        let d = files.dir.join(sub);
        if d.is_dir() {
            return open_files(&d, &files.name, key);
        }
    }
    open_files(&files.dir, &files.name, key)
}

/// The tables the session hands to every automap call (§2–§4).
pub struct AutomapSource {
    pub picker: CelPicker,
    pub levels: Box<dyn AutomapLevels + Send + Sync>,
    pub cels: Box<dyn UnitCels + Send + Sync>,
}

/// One game's automap and its files.
pub struct AutomapSession {
    pub map: Automap,
    pub source: AutomapSource,
    /// `None`: no save files (the cells live in memory only).
    pub files: Option<SaveFiles>,
    /// The open data file's path (written at teardown).
    data_path: Option<PathBuf>,
    /// The act record the files were opened for (S→C 0x03 u32@2, u32@8).
    act: Option<(u32, u32)>,
    /// UI state 0x0A (automap shown).
    pub open: bool,
}

impl AutomapSession {
    /// §14 r1: the automap's UI init with its tables and files.
    pub fn new(map: Automap, source: AutomapSource, files: Option<SaveFiles>) -> Self {
        AutomapSession {
            map,
            source,
            files,
            data_path: None,
            act: None,
            open: false,
        }
    }

    /// The act load (S→C 0x03, `client/model.md` §7 r4): a new act record
    /// tears the old one down (its layer saved and written, §14 r3) and
    /// opens the index for key `init_seed` (§7 r1), the data file then
    /// attached with the act record's second u32 `f8`.
    pub fn act_load(&mut self, init_seed: u32, f8: u32) -> Result<(), AutomapError> {
        if self.act == Some((init_seed, f8)) {
            return Ok(());
        }
        if self.act.is_some() {
            self.teardown()?;
        }
        self.act = Some((init_seed, f8));
        let file = match &self.files {
            Some(files) => {
                let (file, path, _) = open_files_in(files, init_seed)?;
                self.data_path = Some(path);
                file
            }
            None => MaFile::default(),
        };
        self.map.attach_file(file, f8);
        Ok(())
    }

    /// One frame (§5 r1, from the frame `0x0044C7EB`): the act load when
    /// the model's act record changed, then the reveal on the near rooms
    /// of the local player's room. No act: nothing.
    pub fn frame(
        &mut self,
        world: &ClientWorld,
        near: Option<&mut NearRooms>,
    ) -> Result<(), AutomapError> {
        let Some(act) = world.act else {
            return Ok(());
        };
        self.act_load(act.init_seed, act.f8)?;
        let player = world.local().map(|p| unit_pos(p.cell()));
        let mut units: Vec<Vec<AutomapUnit>>;
        let rooms = match near {
            Some(near) => {
                units = near
                    .rooms
                    .iter()
                    .map(|r| {
                        r.units
                            .iter()
                            .map(|u| automap_unit(world, u.key, u.facts.flags))
                            .collect()
                    })
                    .collect();
                Some(RevealRooms {
                    near,
                    units: &mut units,
                })
            }
            None => None,
        };
        let AutomapSession { map, source, .. } = self;
        let t = AutomapTables {
            picker: &source.picker,
            levels: source.levels.as_ref(),
            cels: source.cels.as_ref(),
        };
        map.reveal_frame(player, rooms, &t)
    }

    /// §8 r2: Tab / the middle button / the mini-panel button toggle UI
    /// state 0x0A; closing re-centres with force 0.
    pub fn toggle(&mut self, f: &FrameFacts) {
        self.open = !self.open;
        self.map.toggled(self.open, f);
    }

    /// §14 r3 (`0x0045A5C0`): the current layer saved, cells and layers
    /// freed, the data file written to its path.
    pub fn teardown(&mut self) -> Result<(), AutomapError> {
        let file = self.map.teardown()?;
        if let (Some(file), Some(path)) = (file, self.data_path.take()) {
            write(&file, &path)?;
        }
        self.act = None;
        Ok(())
    }

    /// The whole-preset-level DRLG callback (+0x454, §5 r3) for a room
    /// the caller built.
    pub fn preset_room(
        &mut self,
        room: &mut crate::rules::draw_order::Room,
        units: &mut [AutomapUnit],
    ) -> Result<(), AutomapError> {
        let AutomapSession { map, source, .. } = self;
        let t = AutomapTables {
            picker: &source.picker,
            levels: source.levels.as_ref(),
            cels: source.cels.as_ref(),
        };
        map.preset_room(room, units, &t)
    }

    /// The town-art DRLG callback (+0x488, §6).
    pub fn town_art(&mut self, level: u32, f: u32, centre: (i32, i32)) -> Result<(), AutomapError> {
        let AutomapSession { map, source, .. } = self;
        let t = AutomapTables {
            picker: &source.picker,
            levels: source.levels.as_ref(),
            cels: source.cels.as_ref(),
        };
        map.town_art(level, f, centre, &t)
    }

    /// The data file's path once an act opened it.
    pub fn data_path(&self) -> Option<&Path> {
        self.data_path.as_deref()
    }
}

fn write(file: &MaFile, path: &Path) -> Result<(), AutomapError> {
    file.write_to(path)
        .map_err(|e| AutomapError::File(format!("{}: {e}", path.display())))
}

/// A unit's client position from its model cell (the cell centre,
/// `client/model.md` §3 r3; `render/camera.md` §2).
fn unit_pos((x, y): (u16, u16)) -> ClientPos {
    moving_to_client((u32::from(x) << 16) | 0x8000, (u32::from(y) << 16) | 0x8000)
}

/// What §4 reads of a room unit: type, class and mode from the model,
/// flags from the frame's unit facts.
fn automap_unit(world: &ClientWorld, key: UnitKey, flags: u32) -> AutomapUnit {
    let u = world.units.get(&key);
    AutomapUnit {
        unit_type: key.unit_type,
        class: u.map_or(0, |u| u.class),
        mode: u.map_or(0, |u| u.mode),
        flags,
        pos: u.map_or(ClientPos::default(), |u| unit_pos(u.cell())),
    }
}

// Spec: specs/render/unit-composite.md (§8, §9), specs/render/camera.md (§2, §4), specs/missiles/missiles.md (R4.1), specs/client/msg-units.md (§4), specs/sim/intents-events.md (§7.6 r1)
//! Client missiles and cast overlays in the play preview's world view: the
//! skill messages S→C 0x4C / 0x4D (mode request codes 0x16 / 0x15, kept
//! as the unit's `last_mode_request`) start a client missile, which flies
//! and explodes by the tick; the skill's cast overlay and the units'
//! state overlays play on their units. Everything is drawn only: the
//! server's missile decides every outcome (CLAUDE.md rule 7).
//!
//! What follows the specs:
//! - the real client creates its own missiles from the skill messages
//!   (`intents-events.md` §7.6 r1: the server sends none, 0x73 aside);
//! - the art: `missiles` CelFile, one cel of one file by direction and
//!   frame (`unit-composite.md` §9); offsets `xoffset`, `yoffset` +
//!   `zoffset` (§8); camera position of a moving unit (`camera.md` §2, §4).
//!
//! d2rs-own, unverified (decision D1, the preview's fills; REC-110):
//! - which missile and when: the skill's `cltmissile` at the unit's cast
//!   request, with no delay to the action frame (the client skill start
//!   `0x004C6F40` is not specified, `model.md` OQ 1);
//! - the flight: one straight line toward the target at `Vel` · 4096
//!   (16.16 subtiles per tick, `missiles.md` R4.1 step 3 read with a
//!   12-bit direction vector), `Range` ticks, no `VelLev`, `Accel` or
//!   skill level (the request does not carry the level);
//! - the impact: the first living monster other than the caster within one
//!   subtile of the missile, else the end of the range; the missile's
//!   `explosionmissile` (when `Explosion`) then plays once where it ended.
//!   The server's hits are not sent to the client (`q-skills-cast` §3.3);
//! - the art: `data\global\missiles\<CelFile>.dcc`, else `.dc6`; overlays
//!   `data\global\overlays\<Filename>.dcc`, else `.dc6` (file names only
//!   from the `Missiles.txt` / `Overlay.txt` columns); frame
//!   `age · AnimRate >> 8` (looping when `LoopAnim`, else the last frame);
//! - opaque blend (the `Trans` columns are not applied), no light, draw key
//!   pass 6 after every other unit;
//! - state overlays: each of the unit's states with an `overlay1` plays
//!   looping on the unit; `ModelFeed` states the unit's position.
//!
//! A file that is missing or fails to parse is logged once and not drawn.

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::tables::{Missiles as MissileTable, Overlay, Skills, States};

use crate::assets::path::{CanonicalPath, FileSource};
use crate::bridge::predict::{cell_centre, facing};
use crate::bridge::world::{ClientUnit, ClientWorld, ModeRequest, UnitKey, MONSTER};
use crate::frames::{FramePart, FrameSet, FrameSetKey};
use crate::rules::camera::{moving_to_client, Camera, FrameSize};
use crate::rules::placement::place;
use crate::rules::unit_composite::{file_direction, unit_offset, TableOffset};
use crate::scene::order::pass;
use crate::scene::{BlendOp, DrawItem, DrawKey, ItemTag, Rect, ShadeChain};

use super::feed::ViewFeed;
use super::{ViewAssets, WorldFrame};

/// The open mode whose frames draw no world (`composition.md` §3 step 3).
const NO_WORLD_MODE: u8 = 3;
/// The mode request codes of the client skill start (`modes.rs`): to a
/// point (S→C 0x4D) and to a unit (S→C 0x4C).
const CAST_POINT: u8 = 0x15;
const CAST_UNIT: u8 = 0x16;
/// Ticks one frame step may cover before the layer restarts its clock.
const MAX_CATCH_UP: u64 = 64;
/// The tag base of an effect's draw (the unit tag space is the GUIDs).
const TAG_BASE: u32 = 0xE000_0000;

/// One `missiles` row as the client reads it. d2rs-own, unverified.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissileRow {
    pub cel_file: String,
    pub vel: u32,
    pub range: u32,
    pub anim_rate: u32,
    pub anim_len: u32,
    pub loop_anim: bool,
    /// `xoffset`, `yoffset`, `zoffset`.
    pub offset: (i16, i16, i16),
    /// `explosionmissile` when `Explosion`, else 0 (none).
    pub explosion: u16,
}

/// One `overlay` row as the client reads it. d2rs-own, unverified.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OverlayRow {
    pub file: String,
    pub frames: u32,
    pub anim_rate: u32,
    pub offset: (i32, i32),
}

/// The rows the effect layer reads, by id (0 = none).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectRows {
    /// `cltmissile` by skill id.
    pub skill_missile: Vec<u16>,
    /// `castoverlay` by skill id.
    pub skill_overlay: Vec<u16>,
    pub missiles: Vec<MissileRow>,
    pub overlays: Vec<OverlayRow>,
    /// `overlay1` by state id.
    pub state_overlay: BTreeMap<u8, u16>,
}

fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_owned()
}

impl EffectRows {
    /// The rows of the decoded tables.
    pub fn from_tables(
        skills: &[Skills],
        missiles: &[MissileTable],
        overlays: &[Overlay],
        states: &[States],
    ) -> Self {
        EffectRows {
            skill_missile: skills.iter().map(|s| s.cltmissile).collect(),
            skill_overlay: skills.iter().map(|s| s.castoverlay).collect(),
            missiles: missiles
                .iter()
                .map(|m| MissileRow {
                    cel_file: text(&m.celfile),
                    vel: u32::from(m.vel),
                    range: u32::from(m.range),
                    anim_rate: u32::from(m.animrate),
                    anim_len: u32::from(m.animlen),
                    loop_anim: m.loopanim != 0,
                    offset: (m.xoffset as i16, m.yoffset as i16, m.zoffset as i16),
                    explosion: if m.explosion { m.explosionmissile } else { 0 },
                })
                .collect(),
            overlays: overlays
                .iter()
                .map(|o| OverlayRow {
                    file: text(&o.filename),
                    frames: o.frames,
                    anim_rate: o.animrate,
                    offset: (o.xoffset as i32, o.yoffset as i32),
                })
                .collect(),
            state_overlay: states
                .iter()
                .filter(|s| s.overlay1 != 0)
                .filter_map(|s| Some((u8::try_from(s.state).ok()?, s.overlay1)))
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.missiles.is_empty() && self.overlays.is_empty()
    }
}

/// Where an effect's art lives and how it plays.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Art {
    /// Archive paths to try, in order.
    files: [String; 2],
    /// 8.8 frames per tick (256 when the row states none).
    rate: u32,
    /// Ticks to live when it is not a flight (`life`).
    loops: bool,
    offset: (i32, i32),
}

/// One live effect.
#[derive(Clone, Debug)]
struct Fx {
    art: Art,
    /// 16.16 subtiles.
    at: (u32, u32),
    /// Follows this unit's cell instead of `at`.
    follow: Option<UnitKey>,
    /// 16.16 subtiles per tick.
    step: (i64, i64),
    dir64: u8,
    born: u64,
    /// Ticks to live.
    life: u64,
    /// The explosion missile id played where a flight ends.
    explosion: u16,
    owner: UnitKey,
    id: u32,
}

/// A file made resident: its archive path, directions and frames.
#[derive(Clone, Debug)]
struct Resident {
    path: CanonicalPath,
    directions: u8,
    frames: usize,
}

/// What [`Missiles::draw_one`] places.
struct Placing<'a> {
    art: &'a Art,
    at: (u32, u32),
    dir64: u8,
    age: u64,
    /// `u64::MAX`: loops for as long as it is drawn.
    life: u64,
    tag: u32,
}

/// One drawn effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissileDraw {
    pub id: u32,
    pub item: DrawItem,
}

/// The effect layer of the world view: the rows (handed in by the app),
/// the file source, the live effects and the files loaded so far. The
/// default draws nothing.
#[derive(Default)]
pub struct Missiles {
    rows: EffectRows,
    source: Option<Arc<dyn FileSource>>,
    files: BTreeMap<String, Option<Resident>>,
    live: Vec<Fx>,
    /// The cast request last seen per unit.
    seen: BTreeMap<UnitKey, ModeRequest>,
    started: bool,
    now: u64,
    next_id: u32,
    last: Vec<MissileDraw>,
}

impl std::fmt::Debug for Missiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Missiles")
            .field("live", &self.live.len())
            .field("files", &self.files.len())
            .field("last", &self.last)
            .finish()
    }
}

/// The integer square root.
fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

impl Missiles {
    /// The layer with the effect rows and the archives to read art from.
    pub fn new(source: Arc<dyn FileSource>, rows: EffectRows) -> Self {
        Missiles {
            rows,
            source: Some(source),
            ..Missiles::default()
        }
    }

    /// The last drawn frame's effects, in draw order.
    pub fn last(&self) -> &[MissileDraw] {
        &self.last
    }

    /// The number of live effects (flights, explosions, cast overlays).
    pub fn live(&self) -> usize {
        self.live.len()
    }

    fn missile_art(&self, id: u16) -> Option<(Art, &MissileRow)> {
        let row = self.rows.missiles.get(usize::from(id))?;
        if id == 0 || row.cel_file.is_empty() {
            return None;
        }
        let base = format!("data\\global\\missiles\\{}", row.cel_file);
        let art = Art {
            files: [format!("{base}.dcc"), format!("{base}.dc6")],
            rate: if row.anim_rate == 0 {
                256
            } else {
                row.anim_rate
            },
            loops: row.loop_anim,
            offset: unit_offset(None, TableOffset::Missile(Some(row.offset))).unwrap_or((0, 0)),
        };
        Some((art, row))
    }

    fn overlay_art(&self, id: u16, looped: bool) -> Option<(Art, &OverlayRow)> {
        let row = self.rows.overlays.get(usize::from(id))?;
        if id == 0 || row.file.is_empty() {
            return None;
        }
        let base = format!("data\\global\\overlays\\{}", row.file);
        let art = Art {
            files: [format!("{base}.dcc"), format!("{base}.dc6")],
            rate: if row.anim_rate == 0 {
                256
            } else {
                row.anim_rate
            },
            loops: looped,
            offset: row.offset,
        };
        Some((art, row))
    }

    fn fresh_id(&mut self) -> u32 {
        self.next_id = self.next_id.wrapping_add(1);
        TAG_BASE | (self.next_id & 0x00FF_FFFF)
    }

    /// Ticks of one pass through an animation of `frames` at 8.8 `rate`.
    fn ticks_of(frames: u32, rate: u32) -> u64 {
        (u64::from(frames.max(1)) * 256).div_ceil(u64::from(rate.max(1)))
    }

    /// Starts the effects of every new cast request in the model. The
    /// first call only learns the requests already there.
    fn observe(&mut self, world: &ClientWorld) {
        let learn = !self.started;
        self.started = true;
        self.seen.retain(|k, _| world.units.contains_key(k));
        let mut casts = Vec::new();
        for unit in world.units.values() {
            let Some(req) = unit.last_mode_request else {
                continue;
            };
            if !matches!(req.code, CAST_POINT | CAST_UNIT) {
                continue;
            }
            if self.seen.insert(unit.key, req) != Some(req) && !learn {
                casts.push((unit.key, req));
            }
        }
        for (key, req) in casts {
            self.start_cast(world, key, req);
        }
    }

    fn start_cast(&mut self, world: &ClientWorld, key: UnitKey, req: ModeRequest) {
        let Some(unit) = world.units.get(&key) else {
            return;
        };
        let Some(cell) = unit.position else { return };
        let Ok(skill) = usize::try_from(req.record[0]) else {
            return;
        };
        let origin = cell_centre(cell);
        if let Some(&id) = self.rows.skill_overlay.get(skill) {
            if let Some((art, row)) = self.overlay_art(id, false) {
                let life = Self::ticks_of(row.frames, row.anim_rate.max(1));
                let id = self.fresh_id();
                self.live.push(Fx {
                    art,
                    at: origin,
                    follow: Some(key),
                    step: (0, 0),
                    dir64: 0,
                    born: world.server_ticks,
                    life,
                    explosion: 0,
                    owner: key,
                    id,
                });
            }
        }
        let Some(&missile) = self.rows.skill_missile.get(skill) else {
            return;
        };
        let target = if req.code == CAST_POINT {
            let (Ok(x), Ok(y)) = (u16::try_from(req.record[2]), u16::try_from(req.record[3]))
            else {
                return;
            };
            (x, y)
        } else {
            let (Ok(t), Ok(g)) = (u8::try_from(req.record[2]), u32::try_from(req.record[3])) else {
                return;
            };
            match world
                .units
                .get(&UnitKey::new(t, g))
                .and_then(|u| u.position)
            {
                Some(p) => p,
                None => return,
            }
        };
        let to = cell_centre(target);
        let Some((art, row)) = self.missile_art(missile) else {
            return;
        };
        let (dx, dy) = (
            i64::from(to.0) - i64::from(origin.0),
            i64::from(to.1) - i64::from(origin.1),
        );
        let len = isqrt((dx * dx + dy * dy) as u64) as i64;
        if len == 0 || row.vel == 0 {
            return;
        }
        // d2rs-own, unverified (module doc): `Vel` · 4096 per tick.
        let speed = i64::from(row.vel) * 4096;
        let (life, explosion) = (u64::from(row.range.max(1)), row.explosion);
        let id = self.fresh_id();
        self.live.push(Fx {
            art,
            at: origin,
            follow: None,
            step: (dx * speed / len, dy * speed / len),
            dir64: facing(origin, to).unwrap_or(0),
            born: world.server_ticks,
            life,
            explosion,
            owner: key,
            id,
        });
    }

    /// Runs the effects up to the model's server tick.
    fn advance(&mut self, world: &ClientWorld) {
        let to = world.server_ticks;
        if to < self.now || to - self.now > MAX_CATCH_UP {
            self.now = to;
            return;
        }
        while self.now < to {
            self.now += 1;
            self.step(world);
        }
    }

    fn step(&mut self, world: &ClientWorld) {
        let now = self.now;
        let mut ended = Vec::new();
        for fx in &mut self.live {
            if fx.step != (0, 0) {
                let x = i64::from(fx.at.0) + fx.step.0;
                let y = i64::from(fx.at.1) + fx.step.1;
                let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
                    fx.life = 0;
                    continue;
                };
                fx.at = (x, y);
            }
        }
        for (i, fx) in self.live.iter().enumerate() {
            let expired = now.saturating_sub(fx.born) >= fx.life;
            let hit = fx.step != (0, 0) && hits_monster(world, fx);
            if expired || hit {
                ended.push(i);
            }
        }
        let mut booms = Vec::new();
        for &i in ended.iter().rev() {
            let fx = self.live.remove(i);
            if fx.explosion != 0 && fx.step != (0, 0) {
                booms.push((fx.explosion, fx.at, fx.owner));
            }
        }
        for (id, at, owner) in booms {
            if let Some((art, row)) = self.missile_art(id) {
                let life = Self::ticks_of(row.anim_len, art.rate);
                let fid = self.fresh_id();
                self.live.push(Fx {
                    art,
                    at,
                    follow: None,
                    step: (0, 0),
                    dir64: 0,
                    born: now,
                    life,
                    explosion: 0,
                    owner,
                    id: fid,
                });
            }
        }
    }

    /// Makes the art of the live effects and of the units' state overlays
    /// resident, once per file. One log line per new failure.
    fn ensure(&mut self, world: &ClientWorld, assets: &mut ViewAssets) -> Vec<String> {
        let mut log = Vec::new();
        let Some(source) = self.source.clone() else {
            return log;
        };
        let mut wanted: Vec<[String; 2]> = self.live.iter().map(|f| f.art.files.clone()).collect();
        for unit in world.units.values() {
            for state in &unit.states {
                if let Some(&id) = self.rows.state_overlay.get(state) {
                    if let Some((art, _)) = self.overlay_art(id, true) {
                        wanted.push(art.files);
                    }
                }
            }
        }
        for files in wanted {
            if self.files.contains_key(&files[0]) {
                continue;
            }
            let loaded = load(source.as_ref(), &files, assets);
            if let Err(e) = &loaded {
                log.push(format!("effect art: {}: {e}", files[0]));
            }
            self.files.insert(files[0].clone(), loaded.ok());
        }
        log
    }

    fn draw_one(&self, camera: &Camera, assets: &ViewAssets, p: Placing<'_>) -> Option<DrawItem> {
        let Placing {
            art,
            at,
            dir64,
            age,
            life,
            tag,
        } = p;
        let offset = art.offset;
        let Some(Some(res)) = self.files.get(&art.files[0]) else {
            return None;
        };
        let frame = (age.saturating_mul(u64::from(art.rate)) >> 8) as usize;
        let frame = if art.loops || life == u64::MAX {
            frame % res.frames.max(1)
        } else {
            frame.min(res.frames.saturating_sub(1))
        };
        let dir = if res.directions > 1 {
            file_direction(res.directions, dir64).unwrap_or(0)
        } else {
            0
        };
        let set = FrameSetKey::new(res.path.as_str(), FramePart::Dir(dir)).ok()?;
        let (id, image) = (
            assets.id(&set, frame).ok()?,
            assets.frame(&set, frame).ok()?,
        );
        let (x, y) = camera.unit_draw(moving_to_client(at.0, at.1), offset);
        let placed = place(image, x, y, Rect::FRAME);
        let clip = placed.clip?;
        let mut d = DrawItem::new(id, placed.x, placed.y);
        d.clip = clip;
        d.shade = ShadeChain::EMPTY;
        d.blend = BlendOp::Opaque;
        d.tag = ItemTag::Unit(tag);
        Some(d)
    }

    /// The draws of the live effects and the state overlays whose art is
    /// resident, under `camera`, in creation order.
    pub fn draws(
        &self,
        world: &ClientWorld,
        camera: &Camera,
        assets: &ViewAssets,
    ) -> Vec<MissileDraw> {
        let mut found: Vec<(u32, DrawItem)> = Vec::new();
        for fx in &self.live {
            let at = fx
                .follow
                .and_then(|k| world.units.get(&k))
                .and_then(|u| u.position)
                .map_or(fx.at, cell_centre);
            let age = world.server_ticks.saturating_sub(fx.born);
            let p = Placing {
                art: &fx.art,
                at,
                dir64: fx.dir64,
                age,
                life: fx.life,
                tag: fx.id,
            };
            if let Some(d) = self.draw_one(camera, assets, p) {
                found.push((fx.id, d));
            }
        }
        for unit in world.units.values() {
            for state in &unit.states {
                let Some(&oid) = self.rows.state_overlay.get(state) else {
                    continue;
                };
                let Some((art, _)) = self.overlay_art(oid, true) else {
                    continue;
                };
                let Some(cell) = unit.position else { continue };
                let tag = TAG_BASE
                    | 0x0080_0000
                    | (unit.key.guid & 0xFFFF) << 4
                    | u32::from(*state & 0xF);
                let p = Placing {
                    art: &art,
                    at: cell_centre(cell),
                    dir64: 0,
                    age: world.server_ticks,
                    life: u64::MAX,
                    tag,
                };
                if let Some(d) = self.draw_one(camera, assets, p) {
                    found.push((tag, d));
                }
            }
        }
        found
            .into_iter()
            .enumerate()
            .filter_map(|(minor, (id, mut item))| {
                let minor = u32::try_from(minor).ok()?;
                item.key = DrawKey::new(pass::WALLS_UNITS, DrawKey::MAJOR_MAX, minor, 0).ok()?;
                Some(MissileDraw { id, item })
            })
            .collect()
    }

    /// Runs the layer for this frame and adds its draws to a built
    /// `frame` (re-sorted by key), under the camera of `feed`
    /// (`camera.md` §3, no shake). No local player, or open mode 3 (no
    /// world): nothing. Never fails the frame; returns log lines.
    pub fn add_to_frame<F: ViewFeed + ?Sized>(
        &mut self,
        world: &ClientWorld,
        feed: &F,
        assets: &mut ViewAssets,
        frame: &mut WorldFrame,
    ) -> Vec<String> {
        self.last.clear();
        if self.rows.is_empty() {
            return Vec::new();
        }
        self.observe(world);
        self.advance(world);
        let mut log = self.ensure(world, assets);
        let camera = match (feed.player(world), feed.open_mode(world)) {
            (Ok(Some(p)), Ok(mode)) if mode.get() != NO_WORLD_MODE => {
                Camera::new(FrameSize::D2RS, mode, p.client(), (0, 0))
            }
            (Err(e), _) | (_, Err(e)) => {
                log.push(format!("effects: no camera: {e}"));
                return log;
            }
            _ => return log,
        };
        self.last = self.draws(world, &camera, assets);
        if !self.last.is_empty() {
            frame.items.extend(self.last.iter().map(|d| d.item));
            crate::scene::order(&mut frame.items);
        }
        log
    }
}

/// Whether the flight at `fx.at` is within one subtile of a living
/// monster other than its owner.
fn hits_monster(world: &ClientWorld, fx: &Fx) -> bool {
    let (px, py) = ((fx.at.0 >> 16) as i32, (fx.at.1 >> 16) as i32);
    world.units.values().any(|u: &ClientUnit| {
        u.key.unit_type == MONSTER
            && u.key != fx.owner
            && !u.is_dead()
            && u.position.is_some_and(|(x, y)| {
                (i32::from(x) - px).abs() <= 1 && (i32::from(y) - py).abs() <= 1
            })
    })
}

/// Reads the first of `files` found into the frame store, every
/// direction.
fn load(
    source: &dyn FileSource,
    files: &[String; 2],
    assets: &mut ViewAssets,
) -> Result<Resident, String> {
    let (dcc, dc6) = (&files[0], &files[1]);
    let path = CanonicalPath::new(dcc).map_err(|e| e.to_string())?;
    let mut sets = Vec::new();
    let frames;
    if let Some(r) = crate::assets::path::read_dcc(source, dcc) {
        let d = r?;
        let n = u8::try_from(d.directions.len()).map_err(|_| "too many directions")?;
        for dir in 0..n {
            sets.push(FrameSet::from_dcc(&d, dir).map_err(|e| e.to_string())?);
        }
        frames = d.frames_per_direction as usize;
    } else if let Some(r) = crate::assets::path::read_dc6(source, dc6) {
        let d = r?;
        let n = u8::try_from(d.header.directions).map_err(|_| "too many directions")?;
        for dir in 0..n {
            sets.push(FrameSet::from_dc6(&d, dir).map_err(|e| e.to_string())?);
        }
        frames = d.header.frames_per_direction as usize;
    } else {
        return Err("in no archive".into());
    }
    // Both formats are filed under the `.dcc` name: one key per file.
    let directions = sets.len() as u8;
    for (dir, set) in sets.into_iter().enumerate() {
        let key = FrameSetKey::new(path.as_str(), FramePart::Dir(dir as u8))
            .map_err(|e| e.to_string())?;
        if !assets.frames.contains(&key) {
            assets.frames.insert(key, set).map_err(|e| e.to_string())?;
        }
    }
    Ok(Resident {
        path,
        directions,
        frames,
    })
}

#[cfg(test)]
#[path = "missiles_tests.rs"]
mod tests;

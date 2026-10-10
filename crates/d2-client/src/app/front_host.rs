// Spec: specs/ui/frontend-menus.md (§F1.1, §F1.3, §F1.6), specs/ui/frontend-credits.md (C0)
//! The Bevy host of the front end ([`crate::ui::front_end`]): a window that
//! runs [`FrontEnd::tick`] every 40 ms (C0), feeds it the window's mouse and
//! keys, and draws [`FrontEnd::draw`] with the DC6 art and the sky palette
//! of the user's archives into an 800×600 sprite. The flow's end
//! ([`Outcome`]) closes the window; `main` then starts the game
//! ([`Outcome::GameLoad`]) or exits, and comes back here (main menu) after
//! the game window closes. No game logic lives here (CLAUDE.md rule 5).
//!
//! `// d2rs-own, unverified` and PROVISIONAL (REC-178): text items are
//! drawn as glyphs (REC-189); art is frame `frame` of the DC6 placed with its
//! bottom-left at the control position; the sky palette is used for every
//! screen; the game starts with the default character (the select / create
//! screens do not yet report a choice).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use d2_formats::cof::Cof;
use d2_formats::dc6::{Dc6, Dc6Frame};
use d2_formats::dcc::Dcc;
use d2_formats::font::FontTable;
use d2_formats::palette::{Palette, Pl2};

use crate::assets::path::FileSource;
use crate::ui::front_end::glyphs::text_quads;
use crate::ui::front_end::screens::create::{NewCharacter, NewCharacterSink};
use crate::ui::front_end::screens::credits;
use crate::ui::front_end::startup::{MemProgress, StubVideo};
use crate::ui::front_end::{
    DrawItem, FrontEnd, FrontInput, Outcome, Registry, SaveFolder, SKY_PALETTE, TICK_MS,
};
use crate::ui::geom::Point;
use crate::ui::text::font_info;

pub const WIDTH: u32 = 800;
pub const HEIGHT: u32 = 600;

/// `<dir>/*.d2s` exists (`0x00430BC0`).
pub struct DirSaves(pub std::path::PathBuf);

impl SaveFolder for DirSaves {
    fn has_saves(&self) -> bool {
        std::fs::read_dir(&self.0).is_ok_and(|mut d| {
            d.any(|e| {
                e.is_ok_and(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("d2s"))
                })
            })
        })
    }
}

/// DC6 art from the archives and the screen's palette.
pub struct FrontArt {
    source: Arc<dyn FileSource>,
    /// The palette files in use and their tables (`pal.dat`, `pal.pl2`).
    palette_files: [&'static str; 2],
    palette: Option<Palette>,
    pl2: Option<Pl2>,
    cache: HashMap<&'static str, Option<Dc6>>,
    fonts: HashMap<u16, Option<(FontTable, Dc6)>>,
    /// UTF-16 text of a string id (button labels); `None`: labels blank.
    strings: Option<Box<dyn Fn(u32) -> Vec<u16>>>,
    /// The paper dolls' tables (`DrawItem::Doll`); `None`: no dolls.
    dolls: Option<crate::ui::front_end::doll::DollTables>,
    cofs: HashMap<String, Option<Cof>>,
    dccs: HashMap<String, Option<Dcc>>,
    /// Item colour maps by file name (`0x00505470`, 21 maps of 256).
    colour_files: HashMap<&'static str, Option<Vec<u8>>>,
    /// Per doll key: draws so far and the 8.8 frame phase (`0x00503BA0`).
    doll_phase: HashMap<u32, (u32, u32)>,
    doll_epoch: u64,
}

fn read_palette(source: &dyn FileSource, files: [&str; 2]) -> (Option<Palette>, Option<Pl2>) {
    let read = |f: &str| source.read_file(f).and_then(Result::ok);
    (
        read(files[0]).and_then(|b| Palette::parse(&b).ok()),
        read(files[1]).and_then(|b| Pl2::parse(&b).ok()),
    )
}

impl FrontArt {
    pub fn new(source: Arc<dyn FileSource>) -> Self {
        let (palette, pl2) = read_palette(&*source, SKY_PALETTE);
        Self {
            source,
            palette_files: SKY_PALETTE,
            palette,
            pl2,
            cache: HashMap::new(),
            fonts: HashMap::new(),
            strings: None,
            dolls: None,
            cofs: HashMap::new(),
            dccs: HashMap::new(),
            colour_files: HashMap::new(),
            doll_phase: HashMap::new(),
            doll_epoch: u64::MAX,
        }
    }

    /// The tables the character-select paper dolls are built from.
    pub fn with_dolls(mut self, t: crate::ui::front_end::doll::DollTables) -> Self {
        self.dolls = Some(t);
        self
    }

    fn cof(&mut self, path: &str) -> Option<&Cof> {
        let source = &self.source;
        self.cofs
            .entry(path.to_owned())
            .or_insert_with(|| Cof::parse(&source.read_file(path)?.ok()?).ok())
            .as_ref()
    }

    fn dcc(&mut self, path: &str) -> Option<&Dcc> {
        let source = &self.source;
        self.dccs
            .entry(path.to_owned())
            .or_insert_with(|| Dcc::parse(&source.read_file(path)?.ok()?).ok())
            .as_ref()
    }

    /// The 256-byte map `i` of item colour file `name`.
    fn colour_map(&mut self, name: &'static str, i: usize) -> Option<[u8; 256]> {
        let source = &self.source;
        let bytes = self
            .colour_files
            .entry(name)
            .or_insert_with(|| {
                source
                    .read_file(&format!(r"data\global\items\palette\{name}.dat"))?
                    .ok()
            })
            .as_ref()?;
        bytes.get(i * 256..i * 256 + 256)?.try_into().ok()
    }

    /// Switches to the palette `files` the current screen loaded
    /// (`FrontEnd::palette`, §F1.6 r1); `None` keeps the current one.
    pub fn use_palette(&mut self, files: Option<[&'static str; 2]>) {
        let Some(files) = files else { return };
        if files != self.palette_files {
            let (palette, pl2) = read_palette(&*self.source, files);
            self.palette_files = files;
            self.palette = palette;
            self.pl2 = pl2;
        }
    }

    /// The string-table lookup the button labels use.
    pub fn with_strings(mut self, f: impl Fn(u32) -> Vec<u16> + 'static) -> Self {
        self.strings = Some(Box::new(f));
        self
    }

    fn font(&mut self, id: u16) -> Option<&(FontTable, Dc6)> {
        let source = &self.source;
        self.fonts
            .entry(id)
            .or_insert_with(|| {
                let info = font_info(id)?;
                let table = crate::assets::path::read_font_table(&**source, info.tbl_path)?.ok()?;
                let dc6 = Dc6::parse(&source.read_file(info.dc6_path)?.ok()?).ok()?;
                Some((table, dc6))
            })
            .as_ref()
    }

    /// Width of `text` in font `id`, from the font table's advances (0 when
    /// the font is missing).
    pub fn text_width(&mut self, id: u16, text: &[u16]) -> i32 {
        self.font(id).map_or(0, |(t, _)| {
            crate::ui::front_end::glyphs::text_width(t, text)
        })
    }

    fn dc6(&mut self, file: &'static str) -> Option<&Dc6> {
        let source = &self.source;
        self.cache
            .entry(file)
            .or_insert_with(|| {
                let name = format!(r"data\global\ui\{file}.dc6");
                let bytes = source.read_file(&name)?.ok()?;
                Dc6::parse(&bytes).ok()
            })
            .as_ref()
    }
}

/// The 8-bit frame the front end draws into (`render/blend-modes.md` §1:
/// every write is a palette index; blends read the PL2 tables).
struct IndexFrame {
    px: Vec<u8>,
}

impl IndexFrame {
    /// Writes source index `s` at (x, y) with draw mode `mode` (§1): 5 (and
    /// every mode without a table, or no PL2) writes `s`; 0–2 alpha, 3
    /// additive, 4 multiplicative, 6 max-component read `T[256·d + s]`
    /// (row = destination, §2).
    fn put(&mut self, x: i32, y: i32, s: u8, mode: u8, pl2: Option<&Pl2>) {
        if !(0..WIDTH as i32).contains(&x) || !(0..HEIGHT as i32).contains(&y) {
            return;
        }
        let i = (y as u32 * WIDTH + x as u32) as usize;
        let d = usize::from(self.px[i]);
        let s = usize::from(s);
        self.px[i] = match (mode, pl2) {
            // Mode 0 is alpha level 2, mode 2 level 0 (§1 table).
            (0..=2, Some(t)) => t.alpha_blend[usize::from(2 - mode)][d][s],
            (3, Some(t)) => t.additive_blend[d][s],
            (4, Some(t)) => t.multiplicative_blend[d][s],
            (6, Some(t)) => t.max_component_blend[d][s],
            _ => s as u8,
        };
    }

    /// Draws DC6 frame `f` at (x, bottom y) (`sprite-placement.md` §2:
    /// columns `x + xoff …`, rows `y + yoff − h + 1 … y + yoff`; no offsets
    /// when `offsets` is false), each source index through `map` first
    /// (DC6 index 0 is transparent: the encoding's skips).
    fn cel(
        &mut self,
        f: &Dc6Frame,
        at: Point,
        offsets: bool,
        mode: u8,
        map: Option<&[u8; 256]>,
        pl2: Option<&Pl2>,
    ) {
        let (ox, oy) = if offsets {
            (f.offset_x, f.offset_y)
        } else {
            (0, 0)
        };
        let top = at.y + oy - f.height as i32 + 1;
        for row in 0..f.height {
            for col in 0..f.width {
                let idx = f.pixels[(row * f.width + col) as usize];
                if idx != 0 {
                    let s = map.map_or(idx, |m| m[usize::from(idx)]);
                    self.put(at.x + ox + col as i32, top + row as i32, s, mode, pl2);
                }
            }
        }
    }
}

impl IndexFrame {
    /// Draws a DCC frame's indices (0 transparent) with its top-left at
    /// (`x`, `y`), each through `map` first.
    #[allow(clippy::too_many_arguments)]
    fn indices(
        &mut self,
        px: &[u8],
        w: u32,
        x: i32,
        y: i32,
        mode: u8,
        map: Option<&[u8; 256]>,
        pl2: Option<&Pl2>,
    ) {
        if w == 0 {
            return;
        }
        for (i, &idx) in px.iter().enumerate() {
            if idx != 0 {
                let s = map.map_or(idx, |m| m[usize::from(idx)]);
                let (c, r) = ((i as u32 % w) as i32, (i as u32 / w) as i32);
                self.put(x + c, y + r, s, mode, pl2);
            }
        }
    }
}

/// The palette index nearest to `rgb` (the d2rs-own boxes).
fn nearest(pal: &Palette, rgb: [u8; 3]) -> u8 {
    let dist = |i: u8| {
        let c = pal.colors[usize::from(i)];
        let e = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        e(c.r, rgb[0]) + e(c.g, rgb[1]) + e(c.b, rgb[2])
    };
    (0..=255u8).min_by_key(|&i| dist(i)).unwrap_or(0)
}

/// Composes the draw items into 800×600 RGBA (opaque black background):
/// every item writes palette indices into one 8-bit frame (draw modes and
/// text colours through the PL2 tables, `render/blend-modes.md` §1,
/// `ui/text.md` §4), shown with the screen's palette.
pub fn compose(items: &[DrawItem], art: Option<&mut FrontArt>) -> Vec<u8> {
    let mut frame = IndexFrame {
        px: vec![0u8; (WIDTH * HEIGHT) as usize],
    };
    let mut art = art;
    if let Some(a) = art.as_deref_mut() {
        let pl2 = a.pl2.take();
        for it in items {
            draw_item(&mut frame, a, pl2.as_ref(), it);
        }
        a.pl2 = pl2;
    }
    // The palette shown: the PL2's (`render/composition.md` §4), else
    // `pal.dat`.
    let shown = art.as_deref().and_then(|a| {
        a.pl2
            .as_ref()
            .map(|t| t.base_palette.clone())
            .or_else(|| a.palette.clone())
    });
    let mut px = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    for (o, &i) in px.as_chunks_mut::<4>().0.iter_mut().zip(&frame.px) {
        let c = shown
            .as_ref()
            .map_or(Default::default(), |p| p.colors[usize::from(i)]);
        *o = [c.r, c.g, c.b, 255];
    }
    px
}

fn draw_item(frame: &mut IndexFrame, a: &mut FrontArt, pl2: Option<&Pl2>, it: &DrawItem) {
    let Some(pal) = a.palette.clone() else {
        return;
    };
    match it {
        DrawItem::Art { file, frame: n, at } => {
            if let Some(f) = a.dc6(file).and_then(|d| d.frames.get(*n as usize)) {
                frame.cel(f, *at, true, 5, None, pl2);
            }
        }
        // d2rs-own, unverified (REC-231): a dark box and a 1 px outline.
        DrawItem::Doll {
            class,
            mode,
            components,
            colours,
            at,
            draw_mode,
            key,
            epoch,
            ticks,
        } => draw_doll(
            frame,
            a,
            pl2,
            (*class, *mode, components, colours),
            (*at, *draw_mode),
            (*key, *epoch, *ticks),
        ),
        DrawItem::Rect { at, w, h } => {
            let c = nearest(&pal, [8, 8, 12]);
            for y in at.y..at.y + h {
                for x in at.x..at.x + w {
                    frame.put(x, y, c, 5, None);
                }
            }
        }
        DrawItem::Border { at, w, h } => {
            let c = nearest(&pal, [120, 100, 60]);
            for x in at.x..at.x + w {
                frame.put(x, at.y, c, 5, None);
                frame.put(x, at.y + h - 1, c, 5, None);
            }
            for y in at.y..at.y + h {
                frame.put(at.x, y, c, 5, None);
                frame.put(at.x + w - 1, y, c, 5, None);
            }
        }
        // Draw mode `mode` through the PL2 table (3 = additive, §F1.5 r2);
        // the frame's offsets add to the position unless `boxed`.
        DrawItem::Blend {
            file,
            frame: n,
            at,
            mode,
            boxed,
        } => {
            if let Some(f) = a.dc6(file).and_then(|d| d.frames.get(*n as usize)) {
                frame.cel(f, *at, !*boxed, *mode, None, pl2);
            }
        }
        DrawItem::Text { font, label, .. } => {
            let none = |_| Vec::new();
            let strings = a.strings.take();
            let resolve: &dyn Fn(u32) -> Vec<u16> = match &strings {
                Some(f) => f,
                None => &none,
            };
            let table = a.font(*font).map(|(t, _)| t.clone());
            let quads = text_quads(it, resolve, &|_| table.clone());
            a.strings = strings;
            // A button label draws every glyph with mode 4 (§F1.1 r5),
            // other text with mode 5 (`ui/text.md` §4.1, §7).
            let mode = if label.is_some() { 4 } else { 5 };
            for q in quads {
                // Colour k ≠ 0: the PL2 text-colour map k (`ui/text.md`
                // §4.3–4.4); 0 draws the glyph's own indices.
                let map = match (q.color, pl2) {
                    (1..=12, Some(t)) => t.text_color_shifts.get(q.color as usize),
                    _ => None,
                };
                if let Some(f) = a
                    .font(q.font)
                    .and_then(|(_, d)| d.frames.get(usize::from(q.frame)))
                {
                    frame.cel(f, q.at, true, mode, map, pl2);
                }
            }
        }
    }
}

/// PROVISIONAL (REC-2183): the 1.14d capture draws every doll one row above
/// `at.y + y_min` (all three visible slots, best pixel match at dy = -1 of
/// -4..4); the cause (cel placement of `D2GFX_DrawCelContext`) is not read.
const DOLL_DY: i32 = -1;

/// One paper doll (`0x00503BA0`): the COF's slot order for direction 0 at
/// the animation frame, each component's DCC cel at the anchor, through
/// the colour map of its stored colour.
fn draw_doll(
    frame: &mut IndexFrame,
    a: &mut FrontArt,
    pl2: Option<&Pl2>,
    (class, mode, comp, col): (u8, u8, &[u8; 16], &[u8; 16]),
    (at, draw_mode): (Point, u8),
    (key, epoch, ticks): (u32, u64, u32),
) {
    use crate::ui::front_end::doll::{colour_map, Doll};
    let Some(tables) = a.dolls.as_ref() else {
        return;
    };
    // `0x005066C0`, then the retry of `0x00504040`: no class → class 7,
    // mode 5, `hth`, then no figure.
    let built = tables.build(class, mode, comp, col);
    let mut doll = built.unwrap_or_else(Doll::fallback);
    if a.cof(&doll.cof_path()).is_none() {
        doll = Doll::fallback();
    }
    let Some(cof) = a.cof(&doll.cof_path()).cloned() else {
        return;
    };
    if a.doll_epoch != epoch {
        a.doll_epoch = epoch;
        a.doll_phase.clear();
    }
    let (frames, step) = (u32::from(cof.frames), cof.animation_rate);
    let st = a.doll_phase.entry(key).or_insert((0, 0));
    if ticks < st.0 {
        *st = (0, 0);
    }
    while st.0 < ticks {
        st.1 = crate::ui::front_end::doll::advance(st.1, frames, step);
        st.0 += 1;
    }
    let f = (st.1 >> 8) as usize;
    let Ok(dir) = crate::rules::unit_composite::file_direction(cof.directions, 0) else {
        return;
    };
    for slot in 0..usize::from(cof.layers_count) {
        let Some(c) = cof.component_at(0, f, slot) else {
            continue;
        };
        let c = usize::from(c);
        let wc = cof
            .layers
            .iter()
            .find(|l| usize::from(l.component) == c)
            .map(|l| {
                String::from_utf8_lossy(&l.weapon_class)
                    .trim_end_matches(['\0', ' '])
                    .to_owned()
            })
            .unwrap_or_else(|| doll.weapon_class_name().to_owned());
        let path = doll.dcc_path(c, &wc);
        let map = colour_map(doll.colours[c]).and_then(|(n, i)| a.colour_map(n, i));
        let Some(dcc) = a.dcc(&path) else {
            continue;
        };
        let Some(fr) = dcc
            .directions
            .get(usize::from(dir))
            .and_then(|d| d.frames.get(f))
        else {
            continue;
        };
        frame.indices(
            &fr.pixels,
            fr.width,
            at.x + fr.x_min,
            at.y + fr.y_min + DOLL_DY,
            draw_mode,
            map.as_ref(),
            pl2,
        );
    }
}

/// The front end in the app (a non-send resource: screens are not `Send`).
pub struct FrontHost {
    pub front: FrontEnd,
    pub art: Option<FrontArt>,
    acc_ms: u64,
    /// The last frame's draw items (tests read them).
    pub drawn: Vec<DrawItem>,
    /// Set when the flow ended.
    pub outcome: Option<Outcome>,
    pub frames: u32,
    /// Where the create screen leaves its choice, and the Save folder the
    /// stub is written to ([`FrontHost::with_stub_writer`]).
    stub: Option<(NewCharacterSink, PathBuf)>,
    /// The stub `.d2s` written for the last created character, or why not.
    pub created: Option<Result<PathBuf, String>>,
    /// A copy of [`Self::outcome`] the caller keeps: the app's world (and
    /// this resource with it) may be gone once the window has closed.
    outcome_out: std::sync::Arc<std::sync::Mutex<Option<Outcome>>>,
}

/// Registers the credits screen with the text read from the archives when no
/// loose file is under `D2_GAME_DIR`.
pub fn register_credits(reg: &mut Registry, art: &FrontArt) {
    let src = art.source.clone();
    credits::register_with(
        reg,
        Box::new(move |expansion| {
            let name = if expansion {
                "ExpansionCredits.txt"
            } else {
                "Credits.txt"
            };
            // d2rs-own, unverified: loose file first, then the archives.
            credits::loose_file(expansion)
                .or_else(|| src.read_file(&format!(r"data\local\ui\eng\{name}"))?.ok())
        }),
    );
}

/// The 335-byte stub `.d2s` of a new character (`formats/d2s.md` §2.6),
/// written to `<dir>/<name>.d2s`.
pub fn write_stub(dir: &Path, c: &NewCharacter, time: u32) -> Result<PathBuf, String> {
    use d2_formats::d2s::{self, D2s, SaveTables, StatSave};
    // The stub has no body: no table is consulted.
    struct NoTables;
    impl SaveTables for NoTables {
        fn stat_save(&self, _: u16) -> Option<StatSave> {
            None
        }
        fn item_entry_len(&self, _: &[u8]) -> Result<usize, String> {
            Err("stub has no items".into())
        }
    }
    let mut flags = 0u16;
    if c.hardcore {
        flags |= d2s::status::HARDCORE;
    }
    if c.expansion {
        flags |= d2s::status::EXPANSION;
    }
    let stub = D2s::new_stub(c.name.as_bytes(), c.class.id(), flags, time)
        .ok_or_else(|| format!("name {:?} does not fit", c.name))?;
    let bytes = d2s::write(&stub, &NoTables).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.d2s", c.name));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

impl FrontHost {
    /// `first_entry`: the start-up chain and the trademark (C1); later: the
    /// main menu.
    pub fn new(
        expansion: bool,
        saves: Box<dyn SaveFolder>,
        art: Option<FrontArt>,
        first_entry: bool,
    ) -> Self {
        Self::with_front(FrontEnd::with_screens(expansion, saves), art, first_entry)
    }

    /// [`FrontHost::new`] over a front end the caller built (a registry
    /// with the select / create screens wired, `app::front_start`).
    pub fn with_front(mut front: FrontEnd, art: Option<FrontArt>, first_entry: bool) -> Self {
        front.start(first_entry, &mut MemProgress::default(), &mut StubVideo);
        Self {
            stub: None,
            created: None,
            front,
            art,
            acc_ms: 0,
            drawn: Vec::new(),
            outcome: None,
            frames: 0,
            outcome_out: Default::default(),
        }
    }

    /// Writes the stub `.d2s` of a character the create screen finishes
    /// into `save_dir`. The choice stays in the sink for the game start.
    pub fn with_stub_writer(mut self, sink: NewCharacterSink, save_dir: PathBuf) -> Self {
        self.stub = Some((sink, save_dir));
        self
    }

    /// The flow ended in a game load from character create: write the stub.
    fn write_created(&mut self) {
        let Some((sink, dir)) = &self.stub else {
            return;
        };
        let Some(c) = sink.borrow().clone() else {
            return;
        };
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);
        self.created = Some(write_stub(dir, &c, time));
    }
}

#[derive(Resource)]
struct FrontImage(Handle<Image>);

pub fn add_front_end(app: &mut App, host: FrontHost) {
    app.insert_non_send(host)
        .add_systems(Startup, setup)
        .add_systems(Update, (feed_input, drive, draw, finish).chain());
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new(
        Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (WIDTH * HEIGHT * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    let handle = images.add(image);
    commands.spawn(Camera2d);
    commands.spawn(Sprite::from_image(handle.clone()));
    commands.insert_resource(FrontImage(handle));
}

/// Windows virtual-key codes of the keys the front end reads.
pub fn vk(key: KeyCode) -> Option<u16> {
    Some(match key {
        KeyCode::Escape => 27,
        KeyCode::Enter | KeyCode::NumpadEnter => 13,
        KeyCode::Space => 32,
        KeyCode::Backspace => 8,
        KeyCode::Tab => 9,
        KeyCode::ArrowLeft => 37,
        KeyCode::ArrowUp => 38,
        KeyCode::ArrowRight => 39,
        KeyCode::ArrowDown => 40,
        KeyCode::Delete => 46,
        k => {
            let s = format!("{k:?}");
            let b = s.strip_prefix("Key").or_else(|| s.strip_prefix("Digit"))?;
            let c = b.bytes().next().filter(|_| b.len() == 1)?;
            u16::from(c)
        }
    })
}

/// Window cursor (logical px) to 800×600 frame coordinates.
pub fn frame_point(cursor: Vec2, window: Vec2) -> Point {
    Point::new(
        (cursor.x * WIDTH as f32 / window.x.max(1.0)) as i32,
        (cursor.y * HEIGHT as f32 / window.y.max(1.0)) as i32,
    )
}

fn feed_input(
    mut host: NonSendMut<FrontHost>,
    windows: Query<&Window>,
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    mut keys: MessageReader<KeyboardInput>,
    mut wheel: MessageReader<MouseWheel>,
) {
    let at = windows.single().ok().and_then(|w| {
        Some(frame_point(
            w.cursor_position()?,
            Vec2::new(w.width(), w.height()),
        ))
    });
    if let Some(p) = at {
        host.front.input(FrontInput::Move(p));
        if let Some(b) = buttons {
            if b.just_pressed(MouseButton::Left) {
                host.front.input(FrontInput::Down(p));
            }
            if b.just_released(MouseButton::Left) {
                host.front.input(FrontInput::Up(p));
            }
            if b.just_pressed(MouseButton::Middle) {
                host.front.input(FrontInput::Middle);
            }
        }
    }
    for w in wheel.read() {
        host.front.input(FrontInput::Wheel((w.y * 120.0) as i32));
    }
    for k in keys.read() {
        if k.state == ButtonState::Released {
            if let Some(code) = vk(k.key_code) {
                host.front.input(FrontInput::KeyUp(code));
            }
        }
        if k.state == ButtonState::Pressed {
            if let Some(code) = vk(k.key_code) {
                host.front.input(FrontInput::Key(code));
            }
            if let Some(t) = &k.text {
                for u in t.encode_utf16() {
                    host.front.input(FrontInput::Char(u));
                }
            }
        }
    }
}

/// The 40 ms driver: whole ticks out of the frame time.
fn drive(mut host: NonSendMut<FrontHost>, time: Res<Time>) {
    host.acc_ms += time.delta().as_millis() as u64;
    while host.acc_ms >= TICK_MS && host.outcome.is_none() {
        host.acc_ms -= TICK_MS;
        host.front.tick();
        host.outcome = host.front.outcome();
        if let Ok(mut out) = host.outcome_out.lock() {
            *out = host.outcome;
        }
        if matches!(host.outcome, Some(Outcome::GameLoad(g)) if g.new_character) {
            host.write_created();
        }
    }
}

fn draw(
    mut host: NonSendMut<FrontHost>,
    img: Option<Res<FrontImage>>,
    mut images: ResMut<Assets<Image>>,
) {
    let host = &mut *host;
    host.frames += 1;
    host.drawn = host.front.draw();
    if let Some(art) = host.art.as_mut() {
        art.use_palette(host.front.palette());
    }
    let over = match host.art.as_mut() {
        Some(art) => {
            let art = std::cell::RefCell::new(art);
            host.front
                .overlay(&|font, text| art.borrow_mut().text_width(font, text))
        }
        None => host.front.overlay(&|_, _| 0),
    };
    host.drawn.extend(over);
    if let Some(img) = img {
        if let Some(mut i) = images.get_mut(&img.0) {
            i.data = Some(compose(&host.drawn, host.art.as_mut()));
        }
    }
}

fn finish(host: NonSend<FrontHost>, mut exit: MessageWriter<AppExit>) {
    if host.outcome.is_some() {
        exit.write(AppExit::Success);
    }
}

/// Opens the front-end window; returns how the flow ended.
pub fn run_front_end(host: FrontHost) -> Outcome {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "d2rs".into(),
            resolution: (WIDTH, HEIGHT).into(),
            ..default()
        }),
        ..default()
    }));
    let outcome = host.outcome_out.clone();
    add_front_end(&mut app, host);
    app.run();
    // A closed window ends the program. The resource may be gone after
    // the run, so the outcome is read from the caller's copy.
    let out = outcome.lock().ok().and_then(|o| *o);
    out.unwrap_or(Outcome::Exit)
}

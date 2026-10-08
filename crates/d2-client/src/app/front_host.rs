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
use d2_formats::dc6::Dc6;
use d2_formats::font::FontTable;
use d2_formats::palette::Palette;

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

/// DC6 art from the archives and the sky palette.
pub struct FrontArt {
    source: Arc<dyn FileSource>,
    palette: Option<Palette>,
    cache: HashMap<&'static str, Option<Dc6>>,
    fonts: HashMap<u16, Option<(FontTable, Dc6)>>,
    /// UTF-16 text of a string id (button labels); `None`: labels blank.
    strings: Option<Box<dyn Fn(u32) -> Vec<u16>>>,
}

impl FrontArt {
    pub fn new(source: Arc<dyn FileSource>) -> Self {
        let palette = source
            .read_file(SKY_PALETTE[0])
            .and_then(Result::ok)
            .and_then(|b| Palette::parse(&b).ok());
        Self {
            source,
            palette,
            cache: HashMap::new(),
            fonts: HashMap::new(),
            strings: None,
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

/// Composes the draw items into 800×600 RGBA (opaque black background).
pub fn compose(items: &[DrawItem], art: Option<&mut FrontArt>) -> Vec<u8> {
    let mut px = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    for p in px.as_chunks_mut::<4>().0 {
        p[3] = 255;
    }
    let mut plot = |x: i32, y: i32, rgb: [u8; 3], additive: bool| {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            let i = ((y as u32 * WIDTH + x as u32) * 4) as usize;
            for (d, s) in px[i..i + 3].iter_mut().zip(rgb) {
                *d = if additive { d.saturating_add(s) } else { s };
            }
        }
    };
    let mut art = art;
    for it in items {
        match it {
            DrawItem::Art { file, frame, at } => {
                let Some(a) = art.as_deref_mut() else {
                    continue;
                };
                let Some(pal) = a.palette.clone() else {
                    continue;
                };
                let Some(dc6) = a.dc6(file) else { continue };
                let Some(f) = dc6.frames.get(*frame as usize) else {
                    continue;
                };
                // The frame's offsets add to the position (`sprite-placement.md`
                // §2), as for the blended cels; y is the bottom edge.
                let top = at.y + f.offset_y - f.height as i32 + 1;
                for row in 0..f.height {
                    for col in 0..f.width {
                        let idx = f.pixels[(row * f.width + col) as usize];
                        if idx != 0 {
                            let c = pal.colors[usize::from(idx)];
                            let x = at.x + f.offset_x + col as i32;
                            plot(x, top + row as i32, [c.r, c.g, c.b], false);
                        }
                    }
                }
            }
            // d2rs-own, unverified (REC-231): a dark box and a 1 px outline.
            DrawItem::Rect { at, w, h } => {
                for y in at.y..at.y + h {
                    for x in at.x..at.x + w {
                        plot(x, y, [8, 8, 12], false);
                    }
                }
            }
            DrawItem::Border { at, w, h } => {
                for x in at.x..at.x + w {
                    plot(x, at.y, [120, 100, 60], false);
                    plot(x, at.y + h - 1, [120, 100, 60], false);
                }
                for y in at.y..at.y + h {
                    plot(at.x, y, [120, 100, 60], false);
                    plot(at.x + w - 1, y, [120, 100, 60], false);
                }
            }
            // PROVISIONAL (REC-189): draw mode 3 is the additive blend of
            // §F1.5 r2 (per-channel `min(255, d + s)`, the spec's fit of the
            // PL2 table); the frame's offsets add to the position
            // (`sprite-placement.md` §2). Other modes draw opaque.
            DrawItem::Blend {
                file,
                frame,
                at,
                mode,
            } => {
                let Some(a) = art.as_deref_mut() else {
                    continue;
                };
                let Some(pal) = a.palette.clone() else {
                    continue;
                };
                let Some(dc6) = a.dc6(file) else { continue };
                let Some(f) = dc6.frames.get(*frame as usize) else {
                    continue;
                };
                let top = at.y + f.offset_y - f.height as i32 + 1;
                for row in 0..f.height {
                    for col in 0..f.width {
                        let idx = f.pixels[(row * f.width + col) as usize];
                        if idx != 0 {
                            let c = pal.colors[usize::from(idx)];
                            let (x, y) = (at.x + f.offset_x + col as i32, top + row as i32);
                            plot(x, y, [c.r, c.g, c.b], *mode == 3);
                        }
                    }
                }
            }
            DrawItem::Text { .. } => {
                let Some(a) = art.as_deref_mut() else {
                    continue;
                };
                let Some(pal) = a.palette.clone() else {
                    continue;
                };
                let none = |_| Vec::new();
                let strings = a.strings.take();
                let resolve: &dyn Fn(u32) -> Vec<u16> = match &strings {
                    Some(f) => f,
                    None => &none,
                };
                let mut tables = HashMap::new();
                if let DrawItem::Text { font, .. } = it {
                    if let Some((t, _)) = a.font(*font) {
                        tables.insert(*font, t.clone());
                    }
                }
                let quads = text_quads(it, resolve, &|id| tables.get(&id).cloned());
                a.strings = strings;
                for q in quads {
                    let Some((_, dc6)) = a.font(q.font) else {
                        continue;
                    };
                    let Some(f) = dc6.frames.get(usize::from(q.frame)) else {
                        continue;
                    };
                    let top = q.at.y - f.height as i32 + 1;
                    for row in 0..f.height {
                        for col in 0..f.width {
                            let idx = f.pixels[(row * f.width + col) as usize];
                            if idx != 0 {
                                let c = pal.colors[usize::from(idx)];
                                plot(
                                    q.at.x + col as i32,
                                    top + row as i32,
                                    [c.r, c.g, c.b],
                                    false,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    px
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

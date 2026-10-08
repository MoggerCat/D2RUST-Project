// Spec: specs/ui/frontend-menus.md (§F1.1, §F1.3, §F1.6), specs/ui/frontend-credits.md (C0)
//! The Bevy host of the front end ([`crate::ui::front_end`]): a window that
//! runs [`FrontEnd::tick`] every 40 ms (C0), feeds it the window's mouse and
//! keys, and draws [`FrontEnd::draw`] with the DC6 art and the sky palette
//! of the user's archives into an 800×600 sprite. The flow's end
//! ([`Outcome`]) closes the window; `main` then starts the game
//! ([`Outcome::GameLoad`]) or exits, and comes back here (main menu) after
//! the game window closes. No game logic lives here (CLAUDE.md rule 5).
//!
//! `// d2rs-own, unverified` and PROVISIONAL (REC-231): text items are not
//! drawn (a bar marks each); art is frame `frame` of the DC6 placed with its
//! bottom-left at the control position; the sky palette is used for every
//! screen; the game starts with the default character (the select / create
//! screens do not yet report a choice).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use d2_formats::dc6::Dc6;
use d2_formats::palette::Palette;

use crate::assets::path::FileSource;
use crate::ui::front_end::startup::{MemProgress, StubVideo};
use crate::ui::front_end::{
    DrawItem, FrontEnd, FrontInput, Outcome, SaveFolder, SKY_PALETTE, TICK_MS,
};
use crate::ui::geom::Point;

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
        }
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
    let mut put = |x: i32, y: i32, rgb: [u8; 3]| {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            let i = ((y as u32 * WIDTH + x as u32) * 4) as usize;
            px[i..i + 3].copy_from_slice(&rgb);
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
                let top = at.y - f.height as i32 + 1;
                for row in 0..f.height {
                    for col in 0..f.width {
                        let idx = f.pixels[(row * f.width + col) as usize];
                        if idx != 0 {
                            let c = pal.colors[usize::from(idx)];
                            put(at.x + col as i32, top + row as i32, [c.r, c.g, c.b]);
                        }
                    }
                }
            }
            // PROVISIONAL (REC-231): no glyphs yet; a bar marks the text.
            DrawItem::Text { at, .. } => {
                for x in 0..24 {
                    put(at.x + x, at.y, [200, 180, 120]);
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
        let mut front = FrontEnd::with_screens(expansion, saves);
        front.start(first_entry, &mut MemProgress::default(), &mut StubVideo);
        Self {
            front,
            art,
            acc_ms: 0,
            drawn: Vec::new(),
            outcome: None,
            frames: 0,
        }
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
        }
    }
    for k in keys.read() {
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
    add_front_end(&mut app, host);
    app.run();
    // A closed window ends the program.
    app.world()
        .non_send::<FrontHost>()
        .outcome
        .unwrap_or(Outcome::Exit)
}

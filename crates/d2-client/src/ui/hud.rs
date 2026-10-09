// Spec: specs/ui/control-panel.md (§3, §4, §6, §7, §9, §10)
//! The control panel overlays in play ([`HudUi`]): the rules of
//! `ui::panels::control` bound to the client model each frame. Drawn
//! after the border and base art (§1 step 7): life and mana globes (§3),
//! experience and stamina bars (§4), run / walk and menu buttons (§6),
//! the skill buttons with the selected skills' icons (§7), the mini panel
//! with state 0x15 open (§9); the mouse input of §10 (run toggle, menu
//! button, skill buttons → state 3, mini panel functions).
//!
//! The new-stats / new-skills buttons (§8, 800 × 600) draw `Panel\Level`:
//! the glowing button while stat (skill) points are unspent, frame 2
//! otherwise; a click opens the character panel (state 2) / skill tree
//! (state 4). d2rs-own, unverified: states 6 and 7 are not driven by the
//! level-up; the unspent points (stats 4 and 5 of the model) stand for
//! them.
//!
//! The belt is `hud_belt`. Not drawn: the 640 × 480 variant of §8, the
//! tool tips, globe numbers and the stamina tip are `hud_tips`.
//!
//! Preview fills (decision D1), each `// d2rs-own, unverified`:
//! - the bars' lines and rectangle are cels of a synthetic one-colour file
//!   ([`FILL_FILE`]) clipped to the bar (no line / rectangle draw in the
//!   UI draw list); the stamina colour is the palette's nearest index;
//! - the smoothing counter `C` is the UI frame tick;
//! - the skill select panel (state 3, `0x004AA7E0`) has no layout spec:
//!   the local player's skills are icons in rows of 10 above the button,
//!   a click sends C→S 0x3C and closes state 3.

use std::collections::BTreeMap;

use d2_proto::client::SelectSkill;

use super::{class_u8, left, SharedRef, EMPTY};
use crate::bridge::world::{ClientWorld, UnitKey, PLAYER};
use crate::ui::draw::{ImageRef, ImageRequest, UiDraw, UiDrawSink};
use crate::ui::geom::{Point, Rect};
use crate::ui::layout::Screen;
use crate::ui::panel::{ClientIntent, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::control::belt::BeltColor;
use crate::ui::panels::control::buttons::{
    draw_800, menu_button, run_button, skill_icon_file, skill_icon_pos, skill_icon_state,
    BtnEffect, BtnEnv, ButtonCel, NewBtn, NewButtons, SkillSide,
};
use crate::ui::panels::control::globes::{
    exp_bar, life_globe, mana_globe, stamina_bar, ExpIn, GlobeDraw, GlobeFile, GlobeSmoothing,
    LifeIn, ManaIn, NumbersIn, StaminaColor, StaminaIn, TextToggle,
};
use crate::ui::panels::control::input::{CtrlEffect, CtrlInput, InputEnv, UpFacts};
use crate::ui::panels::control::minipanel::{self, MiniAction, MiniPanel, PlayerFacts, UI_MINI};
use crate::ui::panels::{PanelOutput, UiFiles};

/// The HUD adapter's id: not a UI state, open for good (like the border).
pub const HUD_PANEL: PanelId = PanelId(0x101);

/// The skill select state (§7 r3).
pub const UI_SKILL_SELECT: u8 = 3;

/// The synthetic one-colour file of the bars (module doc). Frames: 0 red,
/// 1 gold, 2 blue (stamina, §4 r2), 3 index 0xFF (experience, §4 r1), 4 the
/// darkest colour (the Esc menu's box), 5–8 the rectangle primitive's belt
/// colours (red, green, blue, yellow: [`BELT_FILL_BASE`]).
/// The first of the four rectangle colours (`BeltColor` order).
pub const BELT_FILL_BASE: u32 = 5;
/// The fill frame of the unidentified tint (`inventory.md` §2 r1 index 4).
pub const UNIDENTIFIED_FILL: u32 = 9;
pub const FILL_FILE: &str = "d2rs\\hudfill";
/// Each fill frame is this wide and high (covers both bars).
pub const FILL_W: u32 = 128;
pub const FILL_H: u32 = 18;

/// The files the HUD names, beside the layout's (lowercase, `UiFiles`).
pub fn hud_files() -> Vec<String> {
    let mut v: Vec<String> = [
        "panel\\hlthmana",
        "panel\\overlap",
        "panel\\runbutton",
        "panel\\menubutton",
        "panel\\level",
        "panel\\minipanel",
        "panel\\minipanel_s",
        "panel\\minipanelbtn",
        "panel\\ctrlpnl_popbelt",
        FILL_FILE,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for c in 0..8u8 {
        v.push(skill_icon_file(c).to_ascii_lowercase());
    }
    v
}

/// Whether a missing archive file of `name` may draw nothing instead of
/// failing the frame (D1: HUD art only).
pub fn optional_file(name: &str) -> bool {
    hud_files().iter().any(|f| f == name) || super::esc_art::is_esc_file(name)
}

/// The fill frames for `palette` (module doc). d2rs-own, unverified.
pub fn fill_frames(palette: &d2_formats::palette::Palette) -> Vec<crate::frames::IndexFrame> {
    let nearest = |(r, g, b): (u8, u8, u8)| -> u8 {
        let mut best = (u32::MAX, 0u8);
        // Index 0 is transparent: never chosen.
        for (i, c) in palette.colors.iter().enumerate().skip(1) {
            let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2) as u32;
            let dist = d(c.r, r) + d(c.g, g) + d(c.b, b);
            if dist < best.0 {
                best = (dist, i as u8);
            }
        }
        best.1
    };
    let colors = [
        nearest(StaminaColor::Red.rgb()),
        nearest(StaminaColor::Gold.rgb()),
        nearest(StaminaColor::Blue.rgb()),
        0xFF,
        nearest((0, 0, 0)),
        nearest(BeltColor::Red.rgb()),
        nearest(BeltColor::Green.rgb()),
        nearest(BeltColor::Blue.rgb()),
        nearest(BeltColor::Yellow.rgb()),
        nearest((0x80, 0x40, 0x40)),
    ];
    colors
        .iter()
        .map(|&c| {
            crate::frames::IndexFrame::new(
                FILL_W,
                FILL_H,
                0,
                0,
                vec![c; (FILL_W * FILL_H) as usize],
            )
            .expect("fill size")
        })
        .collect()
}

/// The tables the HUD reads (from the user's `skills`, `skilldesc` and
/// `experience`; empty without game files).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HudTables {
    /// Skill id → (`charclass`, skilldesc `IconCel`) (§7 r2).
    pub icons: BTreeMap<u16, (u8, u8)>,
    /// `experience` rows: row 0 `MaxLvl`, row L + 1 level L, 7 classes.
    pub experience: Vec<[u32; 7]>,
    /// The states with flag bit 24 (`stambarblue`, §4 r2).
    pub stambarblue: Vec<u8>,
    /// Skill id → the `skills` flags the button state reads (§7 r2).
    pub flags: BTreeMap<u16, SkillButtonFlags>,
}

/// The `skills` row flags of the skill button state (§7 r2, `use.md` §2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillButtonFlags {
    pub in_game: bool,
    pub aura: bool,
    pub passive: bool,
    /// `InTown` (flags bit 8).
    pub in_town: bool,
}

/// The state `k` of a skill button icon (`control-panel.md` §7 r2,
/// `0x004A8D30`): the use state u (`0x004D9FC0`) 0 -> 0, aura -> 4, any
/// other -> 1; then 1 when the record lacks `InTown` and P stands in town.
/// PROVISIONAL (REC-724): u runs the level, `InGame`, aura and passive
/// tests of `skills/use.md` §2 only (no mana, item or cooldown provider on
/// this seam), as `app/skill_rest.rs` `use_state`. Before the flag table
/// is filled only the level test runs.
pub fn skill_button_state(
    tables: &HudTables,
    skill: u16,
    level: i32,
    in_town: bool,
    mouse: (i32, i32),
    at: (i32, i32),
) -> u8 {
    let f = tables.flags.get(&skill).copied();
    let base = if !tables.flags.is_empty() && !f.is_some_and(|f| f.in_game) {
        1
    } else if level <= 0 {
        1
    } else if f.is_some_and(|f| f.aura) {
        4
    } else if f.is_some_and(|f| f.passive) {
        1
    } else {
        0
    };
    let lacks_in_town = f.is_some_and(|f| !f.in_town);
    skill_icon_state(base, lacks_in_town, in_town, mouse, at)
}

impl HudTables {
    /// `threshold(class, L)` = row L + 1 (`0x00611800`).
    fn threshold(&self, class: usize, level: u32) -> Option<u32> {
        let row = self.experience.get(level as usize + 1)?;
        row.get(class).copied()
    }

    fn max_level(&self, class: usize) -> Option<u32> {
        self.experience.first()?.get(class).copied()
    }
}

/// The HUD's own state (the control panel globals of §3, §6, §10).
#[derive(Clone, Debug)]
pub struct HudState {
    pub tables: HudTables,
    pub smoothing: GlobeSmoothing,
    pub input: CtrlInput,
    pub mini: MiniPanel,
    /// The run toggle as the walk holds it (set by the host each frame).
    pub running: bool,
    /// Run toggles asked by the run button since the host last read them.
    pub run_toggles: u32,
    /// The skill button that opened state 3 (`0x004A8CE0(left)`).
    pub select_left: bool,
    /// The belt (`hud_belt`).
    pub belt: super::hud_belt::HudBelt,
    /// The new-stats / new-skills pressed flags (§8).
    pub new_btns: NewButtons,
    /// The play bindings, for the tips' key names ([`key_names`]).
    pub bindings: Option<crate::controls::Bindings>,
    /// The stamina bar's red, gold and blue: the act palette's nearest
    /// indices (`control-panel.md` §4 r2, `0x004FB180`); `None` until the
    /// host sets the palette (the bar then falls back to the fill cel).
    pub stamina_colors: Option<[u8; 3]>,
}

impl Default for HudState {
    fn default() -> Self {
        HudState {
            tables: HudTables::default(),
            smoothing: GlobeSmoothing::default(),
            input: CtrlInput::default(),
            // Single player (game type 0, §9 r1).
            mini: MiniPanel::new(0),
            running: false,
            run_toggles: 0,
            select_left: true,
            belt: Default::default(),
            new_btns: NewButtons::default(),
            bindings: None,
            stamina_colors: None,
        }
    }
}

/// The adapter (installed after the border, §1 step 7).
pub struct HudUi {
    pub(super) sh: SharedRef,
}

/// The primary and secondary key names of command `cmd` (`ui/controls.md`
/// §3 numbering) for the run and mini-panel tips (§6 r1, §9 r6).
/// d2rs-own, unverified: the play bindings' first two inputs and their
/// names, not the short / long key name strings of §5 r13 (as the belt
/// labels, REC-264); no bindings → none.
pub fn key_names(b: Option<&crate::controls::Bindings>, cmd: i32) -> [Option<Vec<u16>>; 2] {
    let inputs = b
        .zip(crate::controls::keymap::action_of_cmd(cmd))
        .map_or(&[][..], |(b, a)| b.inputs(a));
    let name = |i: usize| inputs.get(i).map(|k| k.name().encode_utf16().collect());
    [name(0), name(1)]
}

/// The belt facts the mini panel reads (§9 r2, r7): the belt has extra
/// rows (`[0x007BEFA0]`) and its row count (§5 r7).
fn belt_rows(hud: &HudState) -> (bool, u8) {
    let st = &hud.belt.state;
    (
        st.extra_boxes,
        crate::ui::panels::control::belt::row_count(st.belt_type),
    )
}

/// The local player's key, when it is a player.
fn local(world: &ClientWorld) -> Option<UnitKey> {
    world
        .local()
        .filter(|u| u.key.unit_type == PLAYER)
        .map(|u| u.key)
}

fn image(files: &UiFiles, name: &str, frame: u32, x: i32, y: i32, clip: Rect) -> Option<UiDraw> {
    let file = files.id(name)?;
    Some(UiDraw::Image(ImageRequest {
        image: ImageRef { file, frame },
        at: Point::new(x, y),
        clip,
        look: crate::ui::CelLook::PLAIN,
        call: crate::ui::draw::CelCall::Draw,
    }))
}

/// `d` with the cel wrapper `call` (`tools/facts-render.md` §5 r18).
fn with_call(d: Option<UiDraw>, call: crate::ui::draw::CelCall) -> Option<UiDraw> {
    d.map(|d| match d {
        UiDraw::Image(mut r) => {
            r.call = call;
            UiDraw::Image(r)
        }
        other => other,
    })
}

fn globe_file(f: GlobeFile) -> &'static str {
    match f {
        GlobeFile::Hlthmana => "panel\\hlthmana",
        GlobeFile::Overlap => "panel\\overlap",
    }
}

/// A globe request as an image draw: a window is the cel clipped to its
/// rows (counted from the cel's bottom row y).
/// The play screen as the clip of a full-screen draw.
fn screen_clip() -> Rect {
    Screen::play().rect()
}

fn globe_draw(files: &UiFiles, d: &GlobeDraw) -> Option<UiDraw> {
    match *d {
        GlobeDraw::Window {
            file,
            frame,
            x,
            y,
            skip,
            lines,
            ..
        } => {
            let top = y - skip - lines + 1;
            let clip = Rect::new(0, top, screen_clip().w, u16::try_from(lines).ok()?);
            // The row window is the `Ex` cel draw (`a4-town-pandemonium-
            // fortress` row 249: `hlthmana` `CelDrawEx`).
            with_call(
                image(files, globe_file(file), frame, x, y, clip),
                crate::ui::draw::CelCall::Ex,
            )
        }
        GlobeDraw::Cel { file, frame, x, y } => {
            image(files, globe_file(file), frame, x, y, screen_clip())
        }
    }
}

/// A fill cel clipped to x…x + w − 1, y…y + h − 1 (the frame's top-left is
/// its cel position minus its height − 1: synthetic frames anchor top).
fn fill(files: &UiFiles, frame: u32, x: i32, y: i32, w: i32, h: i32) -> Option<UiDraw> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let clip = Rect::new(x, y, u16::try_from(w).ok()?, u16::try_from(h).ok()?);
    image(files, FILL_FILE, frame, x, y, clip)
}

/// The skills of the select panel and their icon positions (module doc).
fn select_icons(world: &ClientWorld, left_side: bool, w: i32, h: i32) -> Vec<(u16, (i32, i32))> {
    let Some(list) = world.local().and_then(|u| u.skills.as_ref()) else {
        return Vec::new();
    };
    let mut skills: Vec<u16> = Vec::new();
    for e in &list.entries {
        if e.base + e.level_bonus > 0 && !skills.contains(&e.skill) {
            skills.push(e.skill);
        }
    }
    skills
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let (col, row) = ((i % 10) as i32, (i / 10) as i32);
            let x = if left_side {
                117 + 48 * col
            } else {
                w - 165 - 48 * col
            };
            (s, (x, h - 48 - 48 * row))
        })
        .collect()
}

fn icon_hit(at: (i32, i32), p: Point) -> bool {
    (at.0..at.0 + 48).contains(&p.x) && (at.1 - 47..=at.1).contains(&p.y)
}

impl HudUi {
    fn icon_draw(
        files: &UiFiles,
        tables: &HudTables,
        skill: u16,
        at: (i32, i32),
        k: u8,
    ) -> Option<UiDraw> {
        // `control-panel.md` §7 r2: the colored cel draw with the state of
        // `0x004A8D30` as `k` (REC-720).
        let (class, cel) = *tables.icons.get(&skill)?;
        let name = skill_icon_file(if class > 6 { 7 } else { class }).to_ascii_lowercase();
        // The skill buttons' icons are the colour (palette) cel draw
        // (`a4-town-pandemonium-fortress` rows 69–70: `CelDrawColor`).
        let mut d = with_call(
            image(files, &name, u32::from(cel), at.0, at.1, screen_clip()),
            crate::ui::draw::CelCall::Color,
        );
        if let Some(UiDraw::Image(r)) = &mut d {
            r.look.remap = crate::ui::Remap::Palette(i32::from(k));
        }
        d
    }
}

impl Panel for HudUi {
    fn id(&self) -> PanelId {
        HUD_PANEL
    }

    /// The control panel strip (y > H − 48); with the mini panel open
    /// also its row; with state 3 open the screen (module doc).
    fn rect(&self) -> Rect {
        let sh = self.sh.borrow();
        let s = sh.config.screen;
        if sh.states.is_open(UI_SKILL_SELECT) {
            return Rect::new(0, 0, s.w as u16, s.h as u16);
        }
        let mut top = if sh.states.is_open(UI_MINI) {
            s.h - 76
        } else {
            s.h - 47
        };
        if let Some(t) = sh.hud.belt.popped_top(s.res2()) {
            top = top.min(t);
        }
        if s.w <= 0 {
            return EMPTY;
        }
        Rect::new(0, top, s.w as u16, (s.h - top) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let mut sh = self.sh.borrow_mut();
        let sh = &mut *sh;
        let (w, h) = (sh.config.screen.w, sh.config.screen.h);
        let files = &sh.tables.files;
        let world = ctx.world;
        let Some(key) = local(world) else {
            return;
        };
        let unit = world.units.get(&key);
        let living = unit.is_some_and(|u| !u.is_dead());
        let has = |s: u8| unit.is_some_and(|u| u.states.contains(&s));
        let stat = |id: u16| world.total(key, id, 0);
        // d2rs-own, unverified: C is the UI frame tick (module doc).
        let c = ctx.tick as u32;
        let hud = &mut sh.hud;
        // §3 r2 life globe.
        let (life, life_max) = (stat(6), stat(7));
        let shown = hud.smoothing.records[0].shown(life, life_max, c, true);
        let life_shown = shown;
        let life_in = LifeIn {
            shown,
            max: life_max,
            living_player: living,
            health_potion: has(100),
            stat74: stat(74),
            poisoned: has(2),
        };
        for d in life_globe(&life_in, h, false) {
            out.extend_one(globe_draw(files, &d));
        }
        // §3 r3 mana globe.
        let (mana, mana_max) = (stat(8), stat(9));
        let shown = hud.smoothing.records[1].shown(mana, mana_max, c, false);
        let mana_shown = shown.min(mana_max);
        let mana_in = ManaIn {
            shown,
            max: mana_max,
            mana_potion: has(106),
            stat26: stat(26),
        };
        for d in mana_globe(&mana_in, w, h, false) {
            out.extend_one(globe_draw(files, &d));
        }
        // §4 r1 experience bar: two lines of index 0xFF.
        let class = class_u8(unit.map(|u| u.class)).map_or(0, usize::from);
        let level = stat(12).max(0) as u32;
        let exp_in = ExpIn {
            level,
            exp: stat(13) as u32,
            next: hud.tables.threshold(class, level).unwrap_or(0),
            prev: hud
                .tables
                .threshold(class, level.saturating_sub(1))
                .unwrap_or(0),
            max_level: hud.tables.max_level(class).unwrap_or(0),
        };
        for l in exp_bar(&exp_in, w, h) {
            out.extend_one(fill(files, 3, l.x0, l.y, l.x1 - l.x0, 1));
        }
        // §6 r1 run button.
        let mouse = (sh.mouse.x, sh.mouse.y);
        let run = run_button(w, h, hud.running, hud.input.run_pressed, mouse);
        out.extend_one(cel(files, "panel\\runbutton", run));
        // §4 r2 stamina bar: blue with a `stambarblue` state.
        let (stamina, stamina_max) = (stat(10), stat(11));
        let shown = hud.smoothing.records[2].shown(stamina, stamina_max, c, false);
        let stamina_shown = shown;
        let blue = hud.tables.stambarblue.iter().any(|&s| has(s));
        let bar = stamina_bar(shown, stamina_max, blue, w, h);
        let frame = match bar.color {
            StaminaColor::Red => 0,
            StaminaColor::Gold => 1,
            StaminaColor::Blue => 2,
        };
        // §4 r2: the rectangle `0x0046EFD0(x, y, w, 18, colour, mode 2)`
        // (`a4-town-pandemonium-fortress` row 254: `DrawBox` colour 109).
        match hud.stamina_colors {
            Some(c) if bar.w > 0 => out.push(UiDraw::Rect(crate::ui::draw::RectRequest::sized(
                bar.x,
                bar.y,
                bar.w,
                bar.h,
                c[frame as usize],
                bar.mode,
            ))),
            Some(_) => {}
            None => out.extend_one(fill(files, frame, bar.x, bar.y, bar.w, bar.h)),
        }
        // §6 r2 menu button.
        let mini_open = sh.states.is_open(UI_MINI);
        let menu = menu_button(w, h, mini_open, hud.input.menu_pressed, mouse);
        out.extend_one(cel(files, "panel\\menubutton", menu));
        // §5 the belt (before the skill buttons, §1 r3).
        let (res2, items_ui) = (sh.config.screen.res2(), &sh.items);
        hud.belt
            .draw(world, items_ui, files, (w, h), res2, mouse, living, out);
        // §7 r2 skill buttons.
        let list = unit.and_then(|u| u.skills.as_ref());
        for (side, entry) in [
            (SkillSide::Left, list.and_then(|l| l.left_entry())),
            (SkillSide::Right, list.and_then(|l| l.right_entry())),
        ] {
            if let Some(e) = entry {
                let at = skill_icon_pos(side, w, h);
                let town = unit.is_some_and(|u| crate::bridge::modes::in_town(world, u.key));
                let k = skill_button_state(
                    &hud.tables,
                    e.skill,
                    e.base + e.level_bonus,
                    town,
                    mouse,
                    at,
                );
                out.extend_one(HudUi::icon_draw(files, &hud.tables, e.skill, at, k));
            }
        }
        // §8 r1 new-stats / new-skills buttons: step 8 of the UI pass, after
        // the control panel's skill buttons (§1 r3; `a4-town-pandemonium-
        // fortress` rows 256–269).
        let benv = BtnEnv {
            w,
            h,
            res2: sh.config.screen.res2(),
            open_mode: 0,
        };
        for (which, points, pressed) in [
            (NewBtn::Stats, stat(4), hud.new_btns.stats_pressed),
            (NewBtn::Skills, stat(5), hud.new_btns.skills_pressed),
        ] {
            let c = draw_800(&benv, which, points > 0, pressed, mouse);
            out.extend_one(cel(files, "panel\\level", c));
        }
        // §9 the mini panel with state 0x15 open.
        let mut mini_layout = None;
        if mini_open {
            let open = |ui: u8| sh.states.is_open(ui);
            let (extra, rows) = belt_rows(hud);
            let sides = minipanel::sides(&open, extra, rows);
            hud.mini.set_sides(&sides);
            mini_layout = minipanel::layout(sides.left_blocked, sides.right_blocked);
            if let Some(((ax, ay), buttons)) =
                hud.mini.draw(sides.left_blocked, sides.right_blocked, w, h)
            {
                out.extend_one(image(
                    files,
                    &hud.mini.art().to_ascii_lowercase(),
                    0,
                    ax,
                    ay,
                    screen_clip(),
                ));
                for b in buttons {
                    out.extend_one(image(
                        files,
                        "panel\\minipanelbtn",
                        b.frame,
                        b.x,
                        b.y,
                        screen_clip(),
                    ));
                }
            }
        }
        // Tool tips (§4 r1, §6 r1/r4, §8 r1, §9 r6), last so they draw on
        // top.
        let bindings = hud.bindings.as_ref();
        super::hud_tips::draw_tips(
            &super::hud_tips::TipIn {
                w,
                h,
                mouse,
                res2,
                mini_open,
                state9_open: sh.states.is_open(9),
                exp: exp_in,
                strings: ctx.strings,
                fonts: sh.fonts.as_ref(),
                keys: &|cmd| key_names(bindings, cmd),
                mini: mini_layout.map(|l| (l, &hud.mini)),
            },
            out,
        );
        // §3 r6 life / mana numbers and the §4 r2 stamina tip.
        let fonts = sh.fonts.as_ref();
        super::hud_tips::draw_globe_text(
            &super::hud_tips::GlobeTextIn {
                w,
                h,
                numbers: NumbersIn {
                    show_hp: hud.input.show_hp,
                    show_mp: hud.input.show_mp,
                    mouse,
                    life_shown,
                    life_max,
                    mana_shown,
                    mana_max,
                    living_player: living,
                },
                stamina: StaminaIn {
                    shown: stamina_shown,
                    max: stamina_max,
                    shrine: has(136),
                },
                strings: ctx.strings,
                width_a: &|t| fonts.and_then(|f| f.width_a(1, t)).unwrap_or(0),
                fonts,
            },
            out,
        );
        // The skill select panel (state 3, d2rs-own, unverified).
        if sh.states.is_open(UI_SKILL_SELECT) {
            for (skill, at) in select_icons(world, hud.select_left, w, h) {
                // The select panel is d2rs-own: k 0.
                out.extend_one(HudUi::icon_draw(files, &hud.tables, skill, at, 0));
            }
        }
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        let Some((down, at)) = left(e) else {
            return UiResponse::Ignored;
        };
        let mut sh = self.sh.borrow_mut();
        let sh = &mut *sh;
        let (w, h) = (sh.config.screen.w, sh.config.screen.h);
        let world = ctx.world;
        let alive = local(world)
            .and_then(|k| world.units.get(&k))
            .is_some_and(|u| !u.is_dead());
        let select_open = sh.states.is_open(UI_SKILL_SELECT);
        let mini_open = sh.states.is_open(UI_MINI);
        // The skill select panel takes the click first (d2rs-own).
        if select_open && at.y <= h - 48 {
            if !down {
                for (skill, icon) in select_icons(world, sh.hud.select_left, w, h) {
                    if icon_hit(icon, at) {
                        sh.outputs
                            .push(PanelOutput::Intent(ClientIntent::from_message(
                                &SelectSkill {
                                    skill: u32::from(skill),
                                    left: sh.hud.select_left,
                                    item: u32::MAX,
                                },
                            )));
                        break;
                    }
                }
                sh.outputs.push(PanelOutput::SetUi {
                    ui: UI_SKILL_SELECT,
                    mode: 1,
                    jump: false,
                });
            }
            return UiResponse::Consumed;
        }
        let player = PlayerFacts {
            living: alive,
            blocked: false,
            dead: !alive,
        };
        // §9 r7, r8: the mini panel row.
        if mini_open && at.y < h - 47 {
            let open = |ui: u8| sh.states.is_open(ui);
            let mouse = (at.x, at.y);
            let cursor_item = crate::bridge::items::cursor_item(world).is_some();
            let (extra, rows) = belt_rows(&sh.hud);
            let state9 = open(9);
            // d2rs-own, unverified: the cursor mode is read as 7 (no
            // cursor mode in the model; with no cursor item the reset
            // has nothing to do).
            let (acts, consumed) = if down {
                sh.hud
                    .mini
                    .press(w, h, mouse, cursor_item, extra, rows, state9, &player)
            } else {
                sh.hud
                    .mini
                    .release(w, h, mouse, cursor_item, 7, extra, rows, &player, &open)
            };
            for a in acts {
                match a {
                    MiniAction::Ui(o) => sh.outputs.push(o),
                    MiniAction::Sound(id) => sh.outputs.push(PanelOutput::Sound(id as i32)),
                    // `frontend-options.md` §O1 r2: the game menu opens through
                    // `0x0047E090(1, 0)` (`OriginalUi::open_game_menu`).
                    MiniAction::GameMenu => sh.outputs.push(PanelOutput::SetUi {
                        ui: 9,
                        mode: 0,
                        jump: false,
                    }),
                    // `0x0044DA40` after a release that ran a function
                    // (`control-panel.md` §9).
                    MiniAction::InputReset => sh.input_reset = true,
                    // `0x004A3FE0(0)`: the quest log toggles like the Q
                    // key, asking for the quest data when it opens
                    // (`quest_log_ui`; d2rs-own, unverified).
                    MiniAction::QuestLog => {
                        let ui = super::quest_log_ui::UI_QUEST_SCREEN;
                        let opening = !sh.states.is_open(ui);
                        sh.outputs.push(PanelOutput::SetUi {
                            ui,
                            mode: 2,
                            jump: false,
                        });
                        if opening {
                            sh.outputs.push(PanelOutput::Intent(
                                super::quest_log_ui::request_quest_data(),
                            ));
                        }
                    }
                    MiniAction::CursorReset => {}
                }
            }
            return if consumed {
                UiResponse::Consumed
            } else {
                UiResponse::Ignored
            };
        }
        // §5 the belt click (the box hit on release; a press over the belt
        // is consumed like `over_belt`, §10 r1).
        let res2 = sh.config.screen.res2();
        let at_px = (at.x, at.y);
        if alive && sh.hud.belt.over(world, (w, h), res2, at_px) {
            if down {
                // §10 r1: a press over the belt records `[0x007BEFA4]`.
                sh.hud.input.press_recorded = true;
            } else {
                // §10 r2: no press recorded → no belt click; always
                // cleared after the release.
                if sh.hud.input.press_recorded {
                    for i in sh.hud.belt.click(world, res2, at_px) {
                        sh.outputs.push(PanelOutput::Intent(i));
                    }
                }
                sh.hud.input.press_recorded = false;
                sh.hud.input.menu_pressed = false;
                sh.hud.input.run_pressed = false;
            }
            return UiResponse::Consumed;
        }
        // §8 r4, r5 the new-stats / new-skills buttons, while points are unspent.
        let benv = BtnEnv {
            w,
            h,
            res2: sh.config.screen.res2(),
            open_mode: 0,
        };
        let unspent = local(world).map(|k| (world.total(k, 4, 0), world.total(k, 5, 0)));
        for (which, points) in [
            (NewBtn::Stats, unspent.map_or(0, |u| u.0)),
            (NewBtn::Skills, unspent.map_or(0, |u| u.1)),
        ] {
            let mouse = (at.x, at.y);
            let (effects, consumed) = if down && points > 0 {
                sh.hud
                    .new_btns
                    .press(&benv, which, at.x, at.y, mouse, false, false)
            } else if !down {
                sh.hud
                    .new_btns
                    .release(&benv, which, at.x, at.y, mouse, false)
            } else {
                continue;
            };
            for eff in effects {
                if let BtnEffect::Out(o) = eff {
                    sh.outputs.push(o);
                }
            }
            if consumed {
                return UiResponse::Consumed;
            }
        }
        // §10 the control panel strip.
        let env = InputEnv {
            w,
            h,
            blocked: false,
            alive,
            over_belt: false,
        };
        let (effects, consumed) = if down {
            sh.hud.input.mouse_down(&env, at.x, at.y)
        } else {
            let facts = UpFacts {
                belt_popped: false,
                cursor_mode: 0,
                state3_open: select_open,
                state15_open: mini_open,
            };
            sh.hud.input.mouse_up(&env, &facts, at.x, at.y)
        };
        for eff in effects {
            match eff {
                CtrlEffect::Ui(o) => sh.outputs.push(o),
                CtrlEffect::Sound(id) => sh.outputs.push(PanelOutput::Sound(id as i32)),
                CtrlEffect::Send(i) => sh.outputs.push(PanelOutput::Intent(i)),
                CtrlEffect::SkillSelect(l) => sh.hud.select_left = l,
                CtrlEffect::ToggleRun => sh.hud.run_toggles += 1,
                // §3 r5: stored at once (`settings.toml`, written by
                // the host as an Options change).
                CtrlEffect::StoreRegistry { which, on } => {
                    let s = &mut sh.esc.menu.settings;
                    match which {
                        TextToggle::Hp => s.show_hp_text = u8::from(on),
                        TextToggle::Mp => s.show_mp_text = u8::from(on),
                    }
                    sh.esc.menu.changed = true;
                }
                CtrlEffect::CursorMode6 | CtrlEffect::BeltClick => {}
            }
        }
        if consumed {
            UiResponse::Consumed
        } else {
            UiResponse::Ignored
        }
    }
}

fn cel(files: &UiFiles, name: &str, b: ButtonCel) -> Option<UiDraw> {
    image(files, name, b.frame, b.x, b.y, screen_clip())
}

/// Pushes an optional draw.
trait ExtendOne {
    fn extend_one(&mut self, d: Option<UiDraw>);
}

impl ExtendOne for dyn UiDrawSink + '_ {
    fn extend_one(&mut self, d: Option<UiDraw>) {
        if let Some(d) = d {
            self.push(d);
        }
    }
}

/// The frame set the panel art loader makes itself for `name` (D1,
/// d2rs-own, unverified): the fill cels ([`fill_frames`]); for a HUD file
/// no archive holds (`missing`) 256 transparent 1 × 1 frames, logged.
/// `None`: read the file from the archives.
pub fn preview_set(
    name: &str,
    missing: impl FnOnce() -> bool,
    palette: &d2_formats::palette::Palette,
) -> Option<crate::frames::FrameSet> {
    if name == FILL_FILE {
        return Some(crate::frames::FrameSet {
            frames: fill_frames(palette),
        });
    }
    if optional_file(name) && missing() {
        bevy::log::warn!("hud (d2rs-own, unverified): {name}.dc6 in no archive, drawn empty");
        let blank = crate::frames::IndexFrame::new(1, 1, 0, 0, vec![0]).expect("1 × 1");
        return Some(crate::frames::FrameSet {
            frames: vec![blank; 256],
        });
    }
    None
}

#[cfg(test)]
mod skill_button_tests {
    use super::*;

    fn tables() -> HudTables {
        let mut t = HudTables::default();
        let row = |aura, passive, in_town| SkillButtonFlags {
            in_game: true,
            aura,
            passive,
            in_town,
        };
        t.flags.insert(1, row(false, false, true)); // usable, InTown
        t.flags.insert(2, row(true, false, true)); // aura
        t.flags.insert(3, row(false, true, true)); // passive
        t.flags.insert(4, row(false, false, false)); // usable, no InTown
        t.flags.insert(5, SkillButtonFlags::default()); // InGame clear
        t
    }

    fn k(t: &HudTables, skill: u16, level: i32, town: bool) -> u8 {
        skill_button_state(t, skill, level, town, (0, 0), (117, 600))
    }

    // Covers: specs/ui/control-panel.md §7 r2
    #[test]
    fn the_button_state_is_0_usable_4_aura_1_otherwise() {
        let t = tables();
        assert_eq!(k(&t, 1, 3, false), 0, "usable");
        assert_eq!(k(&t, 2, 3, false), 4, "aura");
        assert_eq!(k(&t, 1, 0, false), 1, "no level");
        assert_eq!(k(&t, 3, 3, false), 1, "passive");
        assert_eq!(k(&t, 5, 3, false), 1, "InGame clear");
    }

    // Covers: specs/ui/control-panel.md §7 r2
    #[test]
    fn a_skill_without_in_town_is_1_in_town() {
        let t = tables();
        assert_eq!(k(&t, 4, 3, true), 1);
        assert_eq!(k(&t, 4, 3, false), 0);
        assert_eq!(k(&t, 1, 3, true), 0, "with InTown");
        assert_eq!(k(&t, 2, 3, true), 4, "an aura with InTown");
    }
}

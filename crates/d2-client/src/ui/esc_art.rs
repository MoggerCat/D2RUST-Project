// Spec: specs/ui/frontend-options.md (§O2 r3 art, §O4 draw)
//! The DC6 art of the Esc menu: which image each row's label and value
//! is, and the draw list of a menu (labels, value images, slider bar and
//! skull, `pentspin` pentagrams). The files go through the same
//! [`UiFiles`] / `archive_name` path as every other panel image (no
//! second loader); label and value images live under
//! `data\local\ui\eng\` ([`LOCAL_PREFIX`]), the widgets under
//! `data\global\ui\`.
//!
//! The widths are the spec's (§O2 r3): the original reads them from the
//! DC6 headers; multi-frame images tile at 256 px.
//!
//! d2rs-own, unverified (REC-257): the disabled look (draw mode 1) and
//! the dark slider rectangles of §O4 r2 have no field in the image
//! request and are not drawn; the `pentspin` frame follows the client
//! tick (one step per two ticks, the original's > 50 ms); Window Mode has
//! no art and stays text; `textslid` is the key-config screen's, not
//! this menu's.

use super::options_menu::{Kind, MenuId, OptionsMenu, Row, HALF};
use crate::ui::draw::{ImageRef, ImageRequest, UiDraw, UiDrawSink};
use crate::ui::geom::Point;
use crate::ui::panels::UiFiles;
use crate::ui::FRAME;

/// Prefix of a `UiFiles` name under `data\local\ui\eng\`.
pub const LOCAL_PREFIX: &str = "*local\\";
const BAR: &str = "widgets\\optbar";
const BAR_C: &str = "widgets\\optbarc";
const SKULL: &str = "widgets\\optskull";
const PENTSPIN: &str = "cursor\\pentspin";

/// Widths of the `pentspin` frames (§O4 r3).
pub const PENT_WIDTHS: [i32; 8] = [51, 43, 27, 9, 23, 40, 50, 52];

/// An image and its total width.
pub type Art = (&'static str, i32);

/// The label image of a row (§O2 r3); `None`: no art (Window Mode).
pub fn label(menu: MenuId, row: Row) -> Option<Art> {
    Some(match row {
        Row::Options => ("options", 160),
        Row::ExitGame => ("exit", 434),
        Row::ReturnToGame => ("returntogame", 356),
        Row::SoundOptions => ("soundoptions", 309),
        Row::VideoOptions => ("videooptions", 295),
        Row::AutomapOptions => ("automapoptions", 378),
        Row::ConfigureControls => ("cfgoptions", 423),
        Row::Previous if menu == MenuId::Options => ("previous", 313),
        Row::Previous => ("sprevious", 231),
        Row::Title => match menu {
            MenuId::Sound => ("soundoptions", 309),
            MenuId::Video => ("videooptions", 295),
            _ => ("automapoptions", 378),
        },
        Row::Sound => ("sound", 95),
        Row::Music => ("music", 84),
        Row::Sound3d => ("3dsound", 144),
        Row::Eax => ("eax", 362),
        Row::Bias3d => ("3dbias", 108),
        Row::NpcSpeech => ("npcspeech", 157),
        Row::Resolution => ("resolution", 173),
        Row::LightQuality => ("lightquality", 265),
        Row::BlendShadow => ("blendshadow", 266),
        Row::Perspective => ("perspective", 173),
        Row::Gamma => ("gamma", 105),
        Row::Contrast => ("contrast", 151),
        Row::MapMode => ("automapmode", 213),
        Row::MapFade => ("automapfade", 70),
        Row::MapCenter => ("automapcenter", 336),
        Row::MapParty => ("automapparty", 194),
        Row::MapNames => ("automappartynames", 190),
        Row::WindowMode => return None,
    })
}

/// The value image of choice value `v` of a row (§O2 r3).
pub fn value(row: Row, v: usize) -> Option<Art> {
    const ON_OFF: [Art; 2] = [("smalloff", 51), ("smallon", 40)];
    const NO_YES: [Art; 2] = [("smallno", 40), ("smallyes", 51)];
    let list: &[Art] = match row {
        Row::Sound3d | Row::Eax | Row::BlendShadow | Row::Perspective => &ON_OFF,
        Row::NpcSpeech => &[("audioonly", 184), ("textonly", 169), ("audiotext", 260)],
        Row::Resolution => &[("640x480", 123), ("800x600", 135)],
        Row::LightQuality => &[("low", 66), ("medium", 108), ("high", 57)],
        Row::MapMode => &[("full", 180), ("mini", 131)],
        Row::MapFade => &[
            ("smallno", 40),
            ("center", 104),
            ("everything", 173),
            ("auto", 86),
        ],
        Row::MapCenter | Row::MapParty | Row::MapNames => &NO_YES,
        _ => return None,
    };
    list.get(v).copied()
}

/// Every file the menu names (lowercase `UiFiles` names).
pub fn esc_files() -> Vec<String> {
    let mut v: Vec<String> = [BAR, BAR_C, SKULL, PENTSPIN]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for menu in [
        MenuId::Game,
        MenuId::Options,
        MenuId::Sound,
        MenuId::Video,
        MenuId::Automap,
    ] {
        for d in menu.rows() {
            let vals = (0..4).filter_map(|i| value(d.row, i));
            for (name, _) in label(menu, d.row).into_iter().chain(vals) {
                let n = format!("{LOCAL_PREFIX}{name}");
                if !v.contains(&n) {
                    v.push(n);
                }
            }
        }
    }
    v
}

/// Whether `name` is one of the menu's files (a missing one draws empty).
pub fn is_esc_file(name: &str) -> bool {
    esc_files().iter().any(|f| f == name)
}

/// Frames of an image `w` px wide (tiled at 256 px).
fn frames(w: i32) -> i32 {
    (w + 255) / 256
}

/// Every frame of `name` from cel position (x, y).
fn art(files: &UiFiles, out: &mut dyn UiDrawSink, name: &str, w: i32, x: i32, y: i32) {
    let Some(file) = files.id(name) else { return };
    for k in 0..frames(w) {
        out.push(UiDraw::Image(ImageRequest {
            image: ImageRef {
                file,
                frame: k as u32,
            },
            at: Point::new(x + 256 * k, y),
            clip: FRAME,
        }));
    }
}

fn local(name: &str) -> String {
    format!("{LOCAL_PREFIX}{name}")
}

/// The `pentspin` counter for a client tick: one step per 50 ms
/// (two 25 Hz ticks), d2rs-own.
pub fn pent_frame(tick: u64) -> u32 {
    ((tick / 2) % 8) as u32
}

/// Draws the art of row `i` (label, value or slider); `false`: the row
/// has no art and the caller draws its text.
pub fn draw_row(files: &UiFiles, m: &OptionsMenu, i: usize, out: &mut dyn UiDrawSink) -> bool {
    let def = &m.rows()[i];
    let Some((name, w)) = label(m.menu, def.row) else {
        return false;
    };
    let name = local(name);
    if files.id(&name).is_none() {
        return false;
    }
    let yb = m.baseline(i);
    match def.kind {
        Kind::Title | Kind::Action => art(files, out, &name, w, HALF - 1 - (w >> 1), yb),
        Kind::Choice(_) => {
            let Some((vn, vw)) = value(def.row, m.value(i) as usize) else {
                return false;
            };
            art(files, out, &name, w, HALF - 230, yb);
            art(files, out, &local(vn), vw, HALF + 230 - vw, yb);
        }
        Kind::Slider { style, .. } => {
            art(files, out, &name, w, HALF - 230, yb);
            let y = m.slider_y(i);
            let bar = if style == 1 { BAR_C } else { BAR };
            art(files, out, bar, 290, HALF - 60, y);
            // Skull at X0 + t, y − 1 (− 1 more for style 0).
            let sy = y - 1 - i32::from(style == 0);
            art(files, out, SKULL, 28, HALF - 60 + m.slider_t(i), sy);
        }
    }
    true
}

/// The pentagrams at the selected row (§O4 r3): the left one spins the
/// other way, drawn right-aligned in its 52 px cell.
pub fn draw_pents(files: &UiFiles, m: &OptionsMenu, tick: u64, out: &mut dyn UiDrawSink) {
    let Some(file) = files.id(PENTSPIN) else {
        return;
    };
    let f = pent_frame(tick);
    let left = if f == 0 { 0 } else { 8 - f };
    let y = m.pentagram_y();
    for (frame, x) in [(left, HALF - 52 - 249), (f, HALF + 249)] {
        out.push(UiDraw::Image(ImageRequest {
            image: ImageRef { file, frame },
            at: Point::new(x, y),
            clip: FRAME,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::super::options_menu::MenuId;
    use super::*;

    fn files() -> UiFiles {
        let mut f = UiFiles::new(&[]);
        f.extend(esc_files());
        f
    }

    /// (name, frame, x, y) of every image drawn.
    fn draws(f: &UiFiles, m: &OptionsMenu, tick: u64) -> Vec<(String, u32, i32, i32)> {
        let mut out: Vec<UiDraw> = Vec::new();
        for i in 0..m.rows().len() {
            assert!(draw_row(f, m, i, &mut out) || m.rows()[i].row == Row::WindowMode);
        }
        draw_pents(f, m, tick, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Image(r) => Some((
                    f.name(r.image.file).unwrap().to_string(),
                    r.image.frame,
                    r.at.x,
                    r.at.y,
                )),
                _ => None,
            })
            .collect()
    }

    // d2rs-own, unverified: draw list against the spec's §O4 positions
    #[test]
    fn game_menu_draws_centred_labels_and_pentspin_at_the_selected_row() {
        let f = files();
        let mut m = OptionsMenu::default();
        m.open();
        let d = draws(&f, &m, 0);
        // 160 / 434 / 356 px labels centred at 400 − 1 − w/2, baselines 224 / 274 / 324.
        assert_eq!(d[0], ("*local\\options".into(), 0, 319, 224));
        assert_eq!(d[1], ("*local\\exit".into(), 0, 182, 274));
        assert_eq!(d[2], ("*local\\exit".into(), 1, 438, 274));
        assert_eq!(d[3], ("*local\\returntogame".into(), 0, 221, 324));
        assert_eq!(d[4], ("*local\\returntogame".into(), 1, 477, 324));
        // Return to Game selected: pentagrams at (99, 336), (649, 336).
        assert_eq!(d[5], ("cursor\\pentspin".into(), 0, 99, 336));
        assert_eq!(d[6], ("cursor\\pentspin".into(), 0, 649, 336));
        // Later frame: the left one spins the other way.
        let d = draws(&f, &m, 6);
        assert_eq!((d[5].1, d[6].1), (5, 3));
    }

    // d2rs-own, unverified: slider art against §O4 r2
    #[test]
    fn slider_draws_the_bar_and_the_knob_at_the_position() {
        let f = files();
        let mut m = OptionsMenu::default();
        m.open();
        m.menu = MenuId::Sound;
        m.selected = 1;
        // Master volume 100 → position 20: knob at X0 + 265.
        let d = draws(&f, &m, 0);
        let y_top = m.y_top(1);
        let label = d.iter().position(|e| e.0 == "*local\\sound").unwrap();
        assert_eq!(d[label], ("*local\\sound".into(), 0, 170, y_top + 34));
        let bar = (&d[label + 1], &d[label + 2]);
        assert_eq!(bar.0 .0, "widgets\\optbar");
        assert_eq!((bar.0 .2, bar.1 .2), (340, 596));
        let knob = d.iter().find(|e| e.0 == "widgets\\optskull").unwrap();
        let t = m.slider_t(1);
        assert_eq!(knob.2, 340 + t);
        assert_eq!(knob.3, y_top + 36 - 2);
        // Pentagrams at the selected row.
        assert_eq!(d.last().unwrap().3, y_top + 49);
    }

    #[test]
    fn choice_values_are_right_aligned_images() {
        let f = files();
        let mut m = OptionsMenu::default();
        m.open();
        m.menu = MenuId::Video;
        m.selected = 3;
        let d = draws(&f, &m, 0);
        let v = d.iter().find(|e| e.0.starts_with("*local\\h")).unwrap();
        assert_eq!(v.2, 630 - 57);
    }
}

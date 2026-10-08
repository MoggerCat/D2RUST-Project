// Spec: specs/ui/messages.md (§5 r1–r6), specs/client/msg-ui.md (§4 r4)
//! Overhead text bubbles in play: the per-unit overhead records of S→C
//! 0x26 type 5 and 0x27 (kind 3), drawn as framed bubbles above their
//! units and freed when the counter passes their end. The rules are
//! [`crate::ui::messages::overhead`] (point, text, box, placement, draw);
//! this adapter owns the records in the UI's shared state, the unit
//! points and the draw requests.
// d2rs-own, unverified: the overhead counter `[0x007BF20E]` steps once
// per client frame (the bridge frame count, 25 Hz) instead of once per
// text pass; the unit's pixel point is the unit's feet under the frame's
// one camera (`seams/world-screen.md` §2.2: the host's
// [`FrameAnchor`], the local player at its drawn position); the
// text of a player bubble is read as Latin-1; the backing is the HUD's
// dark fill tiles instead of `DrawRectangle`.

use std::collections::BTreeMap;

use super::game_messages::{backing, wide, Measure};
use super::SharedRef;
use crate::bridge::hover::unit_feet;
use crate::bridge::world::{ClientWorld, UnitKey, PLAYER};
use crate::rules::camera::{moving_to_client, Camera, FrameAnchor, FrameSize, OpenMode};
use crate::ui::draw::{TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::geom::{Point, Rect};
use crate::ui::messages::overhead::{
    bubble_point, bubble_text, BubbleBox, BubbleResult, OverheadPass, OverheadRecord, UnitKind,
};
use crate::ui::messages::FONT_CHAT;
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::text::TextOpts;
use crate::ui::FRAME;

/// The adapter's id: not a UI state, open for good.
pub const OVERHEAD_PANEL: PanelId = PanelId(0x112);

/// The overhead records by unit and the counter they are timed by
/// (`msg-ui.md` §4 r4: end = counter at creation + d).
#[derive(Clone, Debug, Default)]
pub struct Bubbles {
    records: BTreeMap<UnitKey, OverheadRecord>,
    counter: u32,
    last_tick: Option<u64>,
    /// The frame's local-player position and shake, set by the host
    /// before the UI frame (`seams/world-screen.md` §2.2, §2.4).
    pub(super) anchor: Option<FrameAnchor>,
}

impl Bubbles {
    /// `0x0049F410(unit, text, lang)`: an empty text frees the record,
    /// else a new one replaces it; d = 8 · min(len, 254) + 125.
    pub fn set(&mut self, unit: UnitKey, text: &[u8], lang: u8) {
        if text.is_empty() {
            self.records.remove(&unit);
            return;
        }
        let text = &text[..text.len().min(254)];
        let d = 8 * text.len() as u32 + 125;
        self.records
            .insert(unit, OverheadRecord::new(self.counter, d, text, lang));
    }

    /// 0x76 / the empty text: the unit's record is freed.
    pub fn clear(&mut self, unit: UnitKey) {
        self.records.remove(&unit);
    }

    pub fn contains(&self, unit: UnitKey) -> bool {
        self.records.contains_key(&unit)
    }

    /// The counter steps once per client frame (module doc).
    fn step(&mut self, tick: u64) {
        if self.last_tick != Some(tick) {
            self.last_tick = Some(tick);
            self.counter = self.counter.wrapping_add(1);
        }
    }
}

/// The frame's camera (`seams/world-screen.md` §2.2): from the host's
/// anchor; without one (no host), the local player's own position
/// ([`ClientWorld::local_position`]) with no shake. None without a local
/// player.
fn camera(w: &ClientWorld, anchor: Option<FrameAnchor>, open_mode: u8) -> Option<Camera> {
    let mode = OpenMode::new(open_mode).unwrap_or(OpenMode::NONE);
    if let Some(a) = anchor {
        return Some(a.camera(FrameSize::play(), mode));
    }
    let (x16, y16) = w.local_position()?;
    Some(Camera::new(
        FrameSize::play(),
        mode,
        moving_to_client(x16, y16),
        (0, 0),
    ))
}

/// A unit's feet under the frame's camera: the local player at the
/// anchor's position, the one it is drawn at (`seams/world-screen.md`
/// §2.4), every other unit at its draw anchor by type (§2.5).
fn unit_point(
    w: &ClientWorld,
    cam: &Camera,
    anchor: Option<FrameAnchor>,
    key: UnitKey,
    cell: (u16, u16),
) -> (i32, i32) {
    match anchor {
        Some(a) if w.local_player == Some(key) => cam.unit_draw(a.player.client(), (0, 0)),
        _ => unit_feet(cam, key.unit_type, cell),
    }
}

/// The panel that draws the bubbles (module doc).
pub(super) struct OverheadUi {
    pub(super) sh: SharedRef,
}

impl Panel for OverheadUi {
    fn id(&self) -> PanelId {
        OVERHEAD_PANEL
    }

    fn rect(&self) -> Rect {
        Rect::new(0, 0, 0, 0)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let mut sh = self.sh.borrow_mut();
        let sh = &mut *sh;
        sh.bubbles.step(ctx.tick);
        if sh.bubbles.records.is_empty() {
            return;
        }
        let (w, h) = (sh.config.screen.w, sh.config.screen.h);
        let open_mode = sh.states.open_mode().get();
        let anchor = sh.bubbles.anchor;
        let Some(cam) = camera(ctx.world, anchor, open_mode) else {
            return;
        };
        let fill = sh.tables.files.id(super::hud::FILL_FILE);
        let m = Measure(sh.fonts.as_ref());
        let counter = sh.bubbles.counter;
        let mut pass = OverheadPass::default();
        // §5 r1: players, then monsters and objects (the unit map is
        // ordered by type); no dialog panel rectangle in the preview.
        pass.begin((0, 0), None);
        let mut freed = Vec::new();
        for (key, unit) in &ctx.world.units {
            let Some(rec) = sh.bubbles.records.get(key) else {
                continue;
            };
            let Some(cell) = unit.position else {
                continue;
            };
            let (ux, uy) = unit_point(ctx.world, &cam, anchor, *key, cell);
            let kind = if key.unit_type == PLAYER {
                UnitKind::Player
            } else {
                UnitKind::Other
            };
            // §5 r4: past its end the record is freed, and this frame
            // still draws it.
            if rec.expired(counter) {
                freed.push(*key);
            }
            let Some((px, py)) = bubble_point(ux, uy, pass.view, kind, open_mode, w, h) else {
                continue;
            };
            let string = |id: u32| {
                u16::try_from(id)
                    .ok()
                    .and_then(|i| ctx.strings.get_id(i))
                    .map(<[u16]>::to_vec)
            };
            let convert = |t: &[u8], _lang: u8| Some(wide(t));
            let Some(text) = bubble_text(kind, rec, &string, &convert).filter(|t| !t.is_empty())
            else {
                continue;
            };
            let bx = BubbleBox::new(&text, &m).at(px, py);
            if let BubbleResult::Drawn {
                draw: Some(draw), ..
            } = pass.bubble(bx, w, h, open_mode)
            {
                if let Some(file) = fill {
                    backing(file, &draw.backing, out);
                }
                for l in draw.lines {
                    out.push(UiDraw::Text(TextRequest {
                        text: l.text,
                        at: Point::new(l.x, l.y),
                        style: TextStyle {
                            font: FONT_CHAT,
                            color: u16::try_from(l.color).unwrap_or(0),
                        },
                        opts: TextOpts::default(),
                        clip: FRAME,
                    }));
                }
            }
        }
        for k in freed {
            sh.bubbles.records.remove(&k);
        }
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, _e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        UiResponse::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::output::Output;
    use crate::bridge::world::{ClientUnit, MONSTER, OBJECT};
    use crate::ui::layout::Screen;
    use crate::ui::original::{OriginalUi, UiConfig};
    use crate::ui::{NoPanelRules, StringLookup, UiRoot};

    struct Strs;
    impl StringLookup for Strs {
        fn get(&self, _: &str) -> Option<&[u16]> {
            None
        }
        fn get_id(&self, id: u16) -> Option<&[u16]> {
            const T: [u16; 5] = [
                b'H' as u16,
                b'e' as u16,
                b'l' as u16,
                b'l' as u16,
                b'o' as u16,
            ];
            (id == 2500).then_some(&T[..])
        }
    }

    const ME: UnitKey = UnitKey {
        unit_type: PLAYER,
        guid: 1,
    };

    fn setup() -> (OriginalUi, UiRoot, ClientWorld) {
        let config = UiConfig {
            screen: Screen::R800,
            expansion_installed: true,
        };
        let ui = OriginalUi::new(config, None).unwrap();
        let mut root = UiRoot::new(Box::new(NoPanelRules));
        ui.install(&mut root).unwrap();
        let mut w = ClientWorld::default();
        for (key, cell) in [(ME, (1000, 1000)), (UnitKey::new(MONSTER, 5), (1002, 1000))] {
            let mut u = ClientUnit::new(key);
            u.position = Some(cell);
            w.units.insert(key, u);
        }
        w.local_player = Some(ME);
        (ui, root, w)
    }

    fn chat5(unit: UnitKey, text: &str) -> Output {
        Output::ChatLine {
            kind: 5,
            lang: 0,
            unit,
            b8: 0,
            b9: 0,
            name: Vec::new(),
            text: text.as_bytes().to_vec(),
            present: true,
            player_name: None,
        }
    }

    fn texts(root: &UiRoot, w: &ClientWorld, tick: u64) -> Vec<(String, i32, i32)> {
        let ctx = UiCtx {
            tick,
            world: w,
            strings: &Strs,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        root.draw(&ctx, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) if t.style.font == FONT_CHAT => {
                    Some((String::from_utf16_lossy(&t.text), t.at.x, t.at.y))
                }
                _ => None,
            })
            .collect()
    }

    // Covers: specs/ui/messages.md §5 r3, §5 r4, §5 r6; specs/client/msg-ui.md §4 r4
    #[test]
    fn a_player_overhead_message_makes_a_bubble_that_expires_on_its_end() {
        let (mut ui, root, w) = setup();
        assert!(texts(&root, &w, 1).is_empty());
        // The record of "hi": end = counter at creation + 8 · 2 + 125.
        let c0 = ui.shared.borrow().bubbles.counter;
        ui.apply_output(&chat5(ME, "hi"), &w).unwrap();
        let end = c0 + 141;
        let v = texts(&root, &w, 2);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].0, "hi");
        // Drawn on every frame whose counter has not passed the end, and
        // once more on the frame that frees it.
        let counter = ui.shared.borrow().bubbles.counter;
        for tick in 3..=u64::from(end - counter + 2) {
            assert_eq!(texts(&root, &w, tick).len(), 1, "tick {tick}");
        }
        let past = u64::from(end - counter + 3);
        assert!(ui.shared.borrow().bubbles.contains(ME));
        assert_eq!(texts(&root, &w, past + 1).len(), 1, "the freeing frame");
        assert!(!ui.shared.borrow().bubbles.contains(ME));
        assert!(texts(&root, &w, past + 2).is_empty());
    }

    // Covers: specs/ui/messages.md §5 r3
    #[test]
    fn a_monster_overhead_record_shows_its_string() {
        let (mut ui, root, w) = setup();
        let m = UnitKey::new(MONSTER, 5);
        ui.apply_output(&chat5(m, "2500"), &w).unwrap();
        // A record without a valid string id draws nothing.
        ui.apply_output(&chat5(ME, "hi"), &w).unwrap();
        let v = texts(&root, &w, 1);
        assert!(v.iter().any(|t| t.0 == "Hello"), "{v:?}");
        // The unit is two cells right of the player: right of centre.
        let hello = v.iter().find(|t| t.0 == "Hello").unwrap();
        let mine = v.iter().find(|t| t.0 == "hi").unwrap();
        assert!(hello.1 > mine.1);
    }

    // Covers: specs/world/objects.md §9.1 r3; specs/world/objects.md §14 r2; specs/ui/messages.md §5 r3; specs/ui/messages.md §5 r4
    #[test]
    fn a_shrine_overhead_text_draws_above_the_object_for_its_frames() {
        let (mut ui, root, mut w) = setup();
        let shrine = UnitKey::new(OBJECT, 9);
        let mut u = ClientUnit::new(shrine);
        u.position = Some((1000, 1003));
        w.units.insert(shrine, u);
        // The server's hover text is the decimal string id; its record
        // lives 8 · 4 + 125 frames from the message.
        assert!(texts(&root, &w, 1).is_empty());
        let c0 = ui.shared.borrow().bubbles.counter;
        ui.apply_output(&chat5(shrine, "2500"), &w).unwrap();
        let end = c0 + 157;
        let v = texts(&root, &w, 2);
        assert!(v.iter().any(|t| t.0 == "Hello"), "{v:?}");
        let counter = ui.shared.borrow().bubbles.counter;
        for tick in 3..=u64::from(end - counter + 2) {
            assert_eq!(texts(&root, &w, tick).len(), 1, "tick {tick}");
        }
        let past = u64::from(end - counter + 3);
        assert_eq!(texts(&root, &w, past + 1).len(), 1, "the freeing frame");
        assert!(!ui.shared.borrow().bubbles.contains(shrine));
        assert!(texts(&root, &w, past + 2).is_empty());
    }

    // Covers: specs/client/msg-ui.md §21
    #[test]
    fn the_overhead_clear_removes_the_bubble() {
        let (mut ui, root, w) = setup();
        ui.apply_output(&chat5(ME, "hi"), &w).unwrap();
        assert_eq!(texts(&root, &w, 1).len(), 1);
        ui.apply_output(&Output::OverheadClear { unit: ME }, &w)
            .unwrap();
        assert!(texts(&root, &w, 2).is_empty());
    }
}

#[cfg(test)]
mod anchor_tests {
    use super::*;
    use crate::bridge::world::ClientUnit;
    use crate::rules::camera::UnitPosition;

    // Covers: specs/seams/world-screen.md §2.2
    // Covers: specs/seams/world-screen.md §2.4
    #[test]
    fn the_local_players_bubble_stands_on_the_drawn_player() {
        let me = UnitKey::new(PLAYER, 1);
        let mut w = ClientWorld::default();
        let mut u = ClientUnit::new(me);
        u.position = Some((100, 100));
        w.units.insert(me, u);
        w.local_player = Some(me);
        let c = |s: u32| (s << 16) | 0x8000;
        let anchor = FrameAnchor {
            player: UnitPosition::Moving {
                x16: c(103),
                y16: c(100),
            },
            shake: (0, 0),
        };
        for mode in 0..=3 {
            let cam = camera(&w, Some(anchor), mode).unwrap();
            // The frame's camera, from the predicted position.
            let at = moving_to_client(c(103), c(100));
            assert_eq!(
                cam,
                Camera::new(FrameSize::D2RS, OpenMode::new(mode).unwrap(), at, (0, 0))
            );
            // The player's feet: where the player is drawn (camera.md §4).
            assert_eq!(
                unit_point(&w, &cam, Some(anchor), me, (100, 100)),
                (400 + cam.view.shift_x, 292)
            );
        }
        // Without a host anchor: the model cell (the strict path).
        let cam = camera(&w, None, 0).unwrap();
        assert_eq!(unit_point(&w, &cam, None, me, (100, 100)), (400, 292));
    }
}

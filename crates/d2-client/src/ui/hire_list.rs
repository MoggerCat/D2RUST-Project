// Spec: specs/ui/menus.md §3 (hire list); specs/world/npc.md §7.3; specs/client/bridge.md §10 (0x4E / 0x4F)
//! The hire list of Kashya (and the other sellers): the offers the server
//! sends (S→C 0x4F resets the list, each 0x4E adds a slot), drawn as the
//! `menus.md` §3 box, and the row choice that sends C→S 0x36.
//!
//! Seam to the NPC menu box (stitch-npc): its "Hire" option calls
//! [`OriginalUi::open_hire_list`](super::original::OriginalUi) with the
//! NPC's GUID; everything else (offers, gold line, rows, back, choose)
//! lives here.
//!
//! Preview fills (`// d2rs-own, unverified`): the row's Life and Def
//! fields are 0 (no `hireling` stat source in the client), the box uses
//! plain text rows without the list widget's scroll, and the choose goes
//! straight to the 0x36 send (the confirm dialog of §3.4 is not built).

use std::cell::RefCell;
use std::rc::Rc;

use super::draw::{TextRequest, TextStyle, UiDraw, UiDrawSink};
use super::geom::{Point, Rect, FRAME};
use super::messages::msg_u32s;
use super::original::OriginalUi;
use super::panel::{ClientIntent, Panel, UiEvent};
use super::panel::{PanelId, UiCtx, UiResponse, WidgetId};
use super::panels::npc_menu::{
    hire_choose, hire_geometry, hire_row_text, ChooseFacts, HireAction, HireStats, HIRE_BOX,
    STR_BACK, STR_YOUR_GOLD,
};
use super::text::TextOpts;
use super::PointerButton;
use crate::bridge::output::Output;
use crate::bridge::world::{ClientWorld, KindData};

/// The hire list panel id (one past the border panel).
pub const HIRE_PANEL: PanelId = PanelId(0x101);
/// Offers a list holds (`menus.md` §3.3: 10 records).
pub const MAX_OFFERS: usize = 10;
/// Row height of the list (`menus.md` §3.3: item heights 21 / 15).
const ROW_H: i32 = 15;
/// "There are no Mercenaries left to hire."
const STR_NO_MERCS: u16 = 3365;

/// One offer: name id and slot seed (S→C 0x4E).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub name: u16,
    pub seed: u32,
}

/// The stats of an offer for the row text: `(name, seed, player level)`.
pub type StatsFn = dyn Fn(u16, u32, u32) -> Option<HireStats>;

/// The list state, shared between [`OriginalUi`](super::original::OriginalUi)
/// (fed by the bridge outputs) and the panel.
#[derive(Default)]
pub struct HireState {
    pub offers: Vec<Offer>,
    /// The list is up for this NPC GUID.
    pub up: Option<u32>,
    stats: Option<Box<StatsFn>>,
    /// The list closed by a hire: the reset the server sends right after
    /// it does not open the list again.
    pub hired: bool,
    pub screen: (i32, i32),
}

impl HireState {
    /// S→C 0x4F: the list is reset.
    pub fn reset(&mut self) {
        self.offers.clear();
    }

    /// S→C 0x4E: one more offer (more than ten are dropped).
    pub fn offer(&mut self, name: u16, seed: u32) {
        if self.offers.len() < MAX_OFFERS {
            self.offers.push(Offer { name, seed });
        }
    }

    pub fn set_stats(&mut self, f: Box<StatsFn>) {
        self.stats = Some(f);
    }
}

pub type SharedHire = Rc<RefCell<HireState>>;

/// The panel.
pub struct HireListUi {
    pub st: SharedHire,
}

fn utf16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `%d` replaced by `n`.
fn fmt_d(fmt: &[u16], n: i32) -> Vec<u16> {
    let pat: Vec<u16> = utf16s("%d");
    let mut out = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i..].starts_with(&pat) {
            out.extend(utf16s(&n.to_string()));
            i += 2;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    out
}

fn local_gold(world: &ClientWorld) -> i32 {
    // Stats 14 (gold) + 15 (stash gold) (`menus.md` §3.2).
    world.local().map_or(0, |u| {
        world
            .base(u.key, 14, 0)
            .saturating_add(world.base(u.key, 15, 0))
    })
}

fn local_level(world: &ClientWorld) -> u32 {
    world
        .local()
        .map_or(1, |u| u32::try_from(world.base(u.key, 12, 0)).unwrap_or(1))
}

fn is_player(world: &ClientWorld) -> bool {
    world
        .local()
        .is_some_and(|u| matches!(u.kind, KindData::Player(_)))
}

impl HireListUi {
    fn box_rect(&self) -> Rect {
        let st = self.st.borrow();
        if st.up.is_none() {
            return Rect::new(0, 0, 0, 0);
        }
        let (pos, _) = hire_geometry(st.screen.0, st.screen.1);
        Rect::new(pos.0, pos.1, HIRE_BOX.0 as u16, HIRE_BOX.1 as u16)
    }

    /// The row index at `p`, or `Some(None)` for the Back row.
    fn row_at(&self, p: Point) -> Option<Option<usize>> {
        let st = self.st.borrow();
        let (pos, list) = hire_geometry(st.screen.0, st.screen.1);
        let back_y = pos.1 + 315;
        if p.y >= back_y && p.y < back_y + 21 {
            return Some(None);
        }
        let k = (p.y - list.1) / ROW_H;
        (p.y >= list.1 && (k as usize) < st.offers.len()).then_some(Some(k as usize))
    }
}

impl Panel for HireListUi {
    fn id(&self) -> PanelId {
        HIRE_PANEL
    }

    fn rect(&self) -> Rect {
        self.box_rect()
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let st = self.st.borrow();
        if st.up.is_none() || !is_player(ctx.world) {
            return;
        }
        let strings = |id: u16| {
            ctx.strings
                .get_id(id)
                .map(<[u16]>::to_vec)
                .unwrap_or_default()
        };
        let (pos, list) = hire_geometry(st.screen.0, st.screen.1);
        let text = |text: Vec<u16>, at: Point, color: u16| {
            UiDraw::Text(TextRequest {
                text,
                at,
                style: TextStyle { font: 1, color },
                opts: TextOpts::default(),
                clip: FRAME,
            })
        };
        out.push(text(
            fmt_d(&strings(STR_YOUR_GOLD), local_gold(ctx.world)),
            Point::new(pos.0 + 20, pos.1 + 21),
            4,
        ));
        let level = local_level(ctx.world);
        if st.offers.is_empty() {
            out.push(text(
                strings(STR_NO_MERCS),
                Point::new(list.0 + 10, list.1),
                0,
            ));
        }
        for (i, o) in st.offers.iter().enumerate() {
            let Some(stats) = st.stats.as_ref().and_then(|f| f(o.name, o.seed, level)) else {
                continue;
            };
            let (left, _right) = hire_row_text(o.name, &stats, None, &strings);
            out.push(text(
                left,
                Point::new(list.0 + 10, list.1 + ROW_H * i as i32),
                0,
            ));
        }
        out.push(text(
            strings(STR_BACK),
            Point::new(pos.0 + 20, pos.1 + 330),
            0,
        ));
    }

    fn hit(&self, p: Point) -> Option<WidgetId> {
        self.box_rect().contains(p).then_some(WidgetId(0))
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        let up = self.st.borrow().up;
        let Some(npc) = up else {
            return UiResponse::Ignored;
        };
        let UiEvent::Release {
            button: PointerButton::Left,
            at,
        } = e
        else {
            // The box takes every press inside it.
            return match e {
                UiEvent::Press {
                    button: PointerButton::Left,
                    ..
                } => UiResponse::Consumed,
                _ => UiResponse::Ignored,
            };
        };
        let Some(hit) = self.row_at(at) else {
            return UiResponse::Consumed;
        };
        let Some(row) = hit else {
            self.st.borrow_mut().up = None;
            return UiResponse::Consumed;
        };
        let offer = self.st.borrow().offers.get(row).copied();
        let Some(offer) = offer else {
            return UiResponse::Consumed;
        };
        let expansion = ctx.world.expansion != 0;
        // d2rs-own, unverified: the player's current hireling state is not
        // read, so the "no hireling" / "state clear" facts are true and
        // the choose sends at once (`menus.md` §3.4 first branch).
        let c = hire_choose(&ChooseFacts {
            row,
            item_value: row,
            npc_guid: npc,
            record_name: offer.name,
            no_hireling: true,
            classic_game: !expansion,
            merc_state_clear: true,
        });
        match c.action {
            HireAction::Hire { send } => {
                {
                    let mut st = self.st.borrow_mut();
                    st.up = None;
                    st.hired = true;
                }
                match send {
                    super::panels::PanelOutput::Intent(i) => UiResponse::Intent(i),
                    _ => UiResponse::Consumed,
                }
            }
            _ => UiResponse::Consumed,
        }
    }
}

/// C→S 0x36 for tests that skip the panel.
pub fn hire_intent(npc: u32, name: u16) -> ClientIntent {
    msg_u32s(0x36, &[npc, u32::from(name)])
}

impl OriginalUi {
    /// The NPC menu's "Hire" option (`menus.md` §3.1): the hire list opens
    /// for the NPC with this GUID. Seam for the NPC menu box (stitch-npc):
    /// call it from the Hire option's handler; the offers arrive as
    /// 0x4F / 0x4E.
    pub fn open_hire_list(&mut self, npc_guid: u32) {
        self.hire.borrow_mut().up = Some(npc_guid);
    }

    /// Row stats for the hire list (`(name, seed, player level)`).
    pub fn set_hire_stats(&mut self, f: Box<StatsFn>) {
        self.hire.borrow_mut().set_stats(f);
    }

    /// The hire list state (offers, whether it is up).
    pub fn hire_list(&self) -> std::cell::Ref<'_, HireState> {
        self.hire.borrow()
    }

    /// d2rs-own, unverified: until the NPC menu box calls
    /// [`Self::open_hire_list`], the server's list reset (S→C 0x4F, sent
    /// when a seller is talked to) opens the list for the seller nearest
    /// to the local player.
    pub(super) fn hire_auto_open(&mut self, o: &Output, world: &ClientWorld) {
        if !matches!(o, Output::HireListReset) {
            return;
        }
        let mut h = self.hire.borrow_mut();
        if std::mem::take(&mut h.hired) || h.up.is_some() {
            return;
        }
        let (px, py) = world.local().and_then(|u| u.position).unwrap_or((0, 0));
        let d = |u: &crate::bridge::world::ClientUnit| {
            let (x, y) = u.position.unwrap_or((u16::MAX, u16::MAX));
            (i64::from(x) - i64::from(px)).pow(2) + (i64::from(y) - i64::from(py)).pow(2)
        };
        h.up = world
            .units
            .values()
            .filter(|u| {
                u.key.unit_type == crate::bridge::world::MONSTER
                    && d2_sim::world::npc::SELLERS
                        .iter()
                        .any(|&c| u32::from(c) == u.class)
            })
            .min_by_key(|u| d(u))
            .map(|u| u.key.guid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{ClientUnit, UnitKey, MONSTER, PLAYER};
    use crate::ui::layout::Screen;
    use crate::ui::original::UiConfig;
    use crate::ui::{NoPanelRules, NoStrings, UiRoot};

    const KASHYA: u32 = 150;

    fn world() -> ClientWorld {
        let mut w = ClientWorld::default();
        let pk = UnitKey::new(PLAYER, 1);
        let mut p = ClientUnit::new(pk);
        p.position = Some((100, 100));
        w.units.insert(pk, p);
        w.local_player = Some(pk);
        for (guid, at) in [(70, (400, 400)), (77, (104, 100))] {
            let mut n = ClientUnit::new(UnitKey::new(MONSTER, guid));
            n.class = KASHYA;
            n.position = Some(at);
            w.units.insert(n.key, n);
        }
        w
    }

    fn setup() -> (OriginalUi, UiRoot) {
        let config = UiConfig {
            screen: Screen::R800,
            expansion_installed: true,
        };
        let ui = OriginalUi::new(config, None).unwrap();
        let mut root = UiRoot::new(Box::new(NoPanelRules));
        ui.install(&mut root).unwrap();
        (ui, root)
    }

    fn release(root: &mut UiRoot, w: &ClientWorld, x: i32, y: i32) {
        let ctx = UiCtx {
            tick: 0,
            world: w,
            strings: &NoStrings,
        };
        let e = UiEvent::Release {
            button: PointerButton::Left,
            at: Point::new(x, y),
        };
        root.dispatch(e, &ctx);
    }

    // Covers: specs/ui/menus.md §3 r4
    #[test]
    fn the_server_list_opens_for_the_nearest_seller_and_a_row_sends_0x36() {
        let w = world();
        let (mut ui, mut root) = setup();
        ui.apply_output(&Output::HireListReset, &w).unwrap();
        ui.apply_output(
            &Output::HireOffer {
                name: 3000,
                seed: 5,
            },
            &w,
        )
        .unwrap();
        ui.apply_output(
            &Output::HireOffer {
                name: 3001,
                seed: 6,
            },
            &w,
        )
        .unwrap();
        assert_eq!(ui.hire_list().up, Some(77), "the seller beside the player");
        assert_eq!(ui.hire_list().offers.len(), 2);
        // Row 1 of the list at 800 × 600: list y 120, rows 15 high.
        release(&mut root, &w, 200, 120 + 15 + 3);
        assert_eq!(root.take_intents(), vec![hire_intent(77, 3001)]);
        assert_eq!(ui.hire_list().up, None, "the list closes on a hire");
        // The reset the server sends after the hire does not reopen it.
        ui.apply_output(&Output::HireListReset, &w).unwrap();
        assert_eq!(ui.hire_list().up, None);
    }

    #[test]
    fn back_closes_without_a_message() {
        let w = world();
        let (mut ui, mut root) = setup();
        ui.apply_output(&Output::HireListReset, &w).unwrap();
        ui.apply_output(
            &Output::HireOffer {
                name: 3000,
                seed: 5,
            },
            &w,
        )
        .unwrap();
        release(&mut root, &w, 200, 85 + 315 + 5);
        assert!(root.take_intents().is_empty());
        assert_eq!(ui.hire_list().up, None);
    }

    #[test]
    fn the_npc_menu_hook_opens_the_list() {
        let (mut ui, _root) = setup();
        ui.open_hire_list(9);
        assert_eq!(ui.hire_list().up, Some(9));
    }
}

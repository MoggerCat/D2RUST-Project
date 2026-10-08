// Spec: specs/ui/panels.md (§13), specs/ui/menus.md (§1), specs/ui/panels-3.md (§26), specs/client/msg-ui.md (§2); preview fills: docs/handoff/stitch-npc2.md
//! The waypoint menu (ui 0x14) installed in the original UI: the
//! [`WaypointPanel`] press / release rules (`ui/menus.md` §1.2, §1.6)
//! over the rows the row rebuild (`panels-3.md` §26 r2) makes from the
//! record S→C 0x63 stored (`msg_ui`, `client/msg-ui.md` §2) and the
//! levels' waypoint indexes ([`OriginalUi::set_waypoint_map`]). A row's
//! release leaves as C→S 0x49 through the root (the client decides
//! nothing: the server validates and warps, `world/waypoints.md` §6–§7).
//!
//! Preview fills (d2rs-own, unverified): the client quest flags are not
//! in the model, so every act tab opens (the quest gate is skipped); the row and tab text needs the string table by
//! id (`ctx.strings`, the string tables in play).

use d2_sim::world::waypoints::{WaypointMap, WaypointRecord};

use super::{OriginalUi, SharedRef};
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::waypoint::{
    WaypointPanel, WaypointView, WpEffect, WpEnv, WpRow, WpRows, MAX_ROWS, UI_WAYPOINT,
};
use crate::ui::panels::waypoint_rows::{self, TabCache, WaypointData};
use crate::ui::panels::{PanelOutput, TextMeasure};
use crate::ui::root::UiRoot;
use crate::ui::PointerButton;

/// What the open menu reads: the stored 0x63 (GUID, record) and the
/// level the local player stood in when it opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaypointOpen {
    pub guid: u32,
    pub record: WaypointRecord,
    /// The level of the player's room at the open (§26 r2 `current`).
    pub current: u32,
    /// Bumped by every 0x63, so the panel resets its latches.
    pub seq: u32,
}

/// The record's known bits over the level map (`0x00660E00`,
/// `0x00660E50`, `0x00660D90`).
struct Known<'a> {
    map: &'a WaypointMap,
    record: &'a WaypointRecord,
}

impl WaypointData for Known<'_> {
    fn index_of_level(&self, level: u32) -> Option<u8> {
        self.map.index_of_level(level)
    }
    fn known(&self, index: u8) -> bool {
        self.record.test(u32::from(index)).unwrap_or(false)
    }
    fn level_of_index(&self, index: u8) -> Option<u32> {
        self.map.level_of_index(u32::from(index))
    }
}

/// The rows of `tab` (§26 r2) for the open menu; none without the map.
pub fn rows_of(map: Option<&WaypointMap>, open: &WaypointOpen, tab: u8) -> Vec<WpRow> {
    let Some(map) = map else {
        return Vec::new();
    };
    let data = Known {
        map,
        record: &open.record,
    };
    let mut cache = [TabCache::default(); 5];
    waypoint_rows::rebuild(usize::from(tab), &mut cache, open.current, &data)
        .rows
        .into_iter()
        .map(|r| WpRow {
            level: r.level as u16,
            known: r.used || r.level == open.current,
            current: r.level == open.current,
            name: Vec::new(),
        })
        .collect()
}

/// The row table the hit tests read (`[0x007BF03C]`, `[0x007BF040]`).
pub fn hit_rows(rows: &[WpRow]) -> WpRows {
    let mut out = WpRows::default();
    for (i, r) in rows.iter().take(MAX_ROWS).enumerate() {
        out.used[i] = r.known && !r.current;
        out.level[i] = u32::from(r.level);
    }
    out
}

struct View {
    rows: Vec<WpRow>,
}

impl WaypointView for View {
    fn rows(&self, _tab: u8) -> &[WpRow] {
        &self.rows
    }
    /// d2rs-own, unverified (q-act-travel): every act tab is shown; the
    /// original's gate reads the client quest flags (`msg-ui.md` OQ 4).
    /// The rows of an act with no known waypoint have nothing to click.
    fn tab_reachable(&self, _tab: u8) -> bool {
        true
    }
    fn any_other_known(&self) -> bool {
        self.rows.iter().any(|r| r.known && !r.current)
    }
}

struct NoMeasure;

impl TextMeasure for NoMeasure {
    fn width(&self, _: u16, _: &[u16]) -> Option<i32> {
        None
    }
}

/// The waypoint menu adapter (ui 0x14, left slot).
pub(super) struct WaypointUi {
    pub(super) sh: SharedRef,
    pub(super) panel: WaypointPanel,
    pub(super) seq: u32,
}

impl WaypointUi {
    /// The open record and its rows, the panel reset on a new 0x63.
    fn sync(&mut self) -> Option<Vec<WpRow>> {
        let sh = self.sh.borrow();
        let open = sh.waypoint_open?;
        if open.seq != self.seq {
            self.seq = open.seq;
            self.panel = WaypointPanel::new();
            self.panel.guid = open.guid;
        }
        Some(rows_of(sh.waypoint_map.as_ref(), &open, self.panel.tab))
    }
}

impl Panel for WaypointUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_WAYPOINT))
    }

    /// The left half above the control panel: `mouse_down` takes every
    /// press inside it (§1.2).
    fn rect(&self) -> Rect {
        let s = self.sh.borrow().config.screen;
        Rect::new(0, 0, (s.w / 2 + 1) as u16, (s.h - 48) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let Some(open) = sh.waypoint_open else {
            return;
        };
        let view = View {
            rows: rows_of(sh.waypoint_map.as_ref(), &open, self.panel.tab),
        };
        self.panel
            .draw(&sh.tables, &sh.env(), &view, ctx.strings, &NoMeasure, out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        let (press, at) = match e {
            UiEvent::Press {
                button: PointerButton::Left,
                at,
            } => (true, at),
            UiEvent::Release {
                button: PointerButton::Left,
                at,
            } => (false, at),
            _ => return UiResponse::Ignored,
        };
        let Some(rows) = self.sync() else {
            return UiResponse::Ignored;
        };
        let hit = hit_rows(&rows);
        let mut sh = self.sh.borrow_mut();
        let env = WpEnv::new(&sh.config.screen, false);
        // d2rs-own, unverified: no client quest flags, every act tab
        // opens (the server checks the destination's bit on C→S 0x49).
        let (effects, consumed) = if press {
            self.panel
                .mouse_down(&env, at.x, at.y, &hit, false, &|_| true)
        } else {
            self.panel.mouse_up(&env, at.x, at.y, &hit, false)
        };
        for e in effects {
            match e {
                WpEffect::Out(o) => sh.outputs.push(o),
                WpEffect::Sound(_) => sh.outputs.push(PanelOutput::ClickSound),
                WpEffect::RebuildRows => {}
            }
        }
        if consumed {
            UiResponse::Consumed
        } else {
            UiResponse::Ignored
        }
    }
}

impl OriginalUi {
    /// Mirrors the UI flags on `root` ([`UiRoot::sync_states`]): the
    /// flags a delivered output set outside an event.
    pub fn sync_root(&self, root: &mut UiRoot) {
        root.sync_states(&self.shared.borrow().states);
    }

    /// The levels' waypoint indexes the menu's rows read (`levels`
    /// `Waypoint`, `world/waypoints.md` §1).
    pub fn set_waypoint_map(&mut self, map: WaypointMap) {
        self.shared.borrow_mut().waypoint_map = Some(map);
    }

    /// The rows of the open waypoint menu's current tab; empty when
    /// closed.
    pub fn waypoint_rows(&self) -> Vec<WpRow> {
        let sh = self.shared.borrow();
        match sh.waypoint_open {
            Some(open) => rows_of(sh.waypoint_map.as_ref(), &open, 0),
            None => Vec::new(),
        }
    }
}

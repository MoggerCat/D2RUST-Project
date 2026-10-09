// Spec: specs/client/ui.md
//! The panel model (spec §A2): ids, events, responses, context and the
//! `Panel` trait.

use d2_proto::FixedMessage;

use super::draw::UiDrawSink;
use super::geom::{Point, Rect};
use crate::bridge::intent;
use crate::bridge::world::ClientWorld;

/// A panel's id, unique within one [`super::UiRoot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelId(pub u16);

/// A widget's id, unique within its panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WidgetId(pub u16);

/// An action of the controls' closed `Action` enum, by index (spec §A4,
/// §A6). The enum is owned by `d2-client::controls` (task C9); the UI
/// only routes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(pub u16);

/// Pointer buttons the UI routes (`ui/controls.md` §7 r5): Left and
/// Right are the fixed world buttons of §6 (never rebindable); Middle
/// (and the X buttons) run the command bound to keys 0x100–0x102 (§4.2;
/// default middle = command 7, automap). Shift / Ctrl / Alt meanings
/// come only from the bindings of commands 36 / 34 / 37 (§3, §4.3),
/// except right up, which reads the event's MK_SHIFT / MK_CONTROL (§6
/// r1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// One UI event. Positions are in the 800×600 frame: the input layer
/// drops pointer input that maps to [`super::FramePos::Outside`] and sends
/// `CursorLeft` instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiEvent {
    CursorMoved(Point),
    CursorLeft,
    Press {
        button: PointerButton,
        at: Point,
    },
    Release {
        button: PointerButton,
        at: Point,
    },
    /// Wheel steps, positive away from the user.
    Wheel {
        steps: i32,
        at: Point,
    },
    /// One UTF-16 code unit of typed text (§A3 encoding).
    Char(u16),
    Action(ActionId),
}

impl UiEvent {
    /// The frame point of a pointer event; `None` for the others.
    pub fn at(&self) -> Option<Point> {
        match *self {
            UiEvent::CursorMoved(p)
            | UiEvent::Press { at: p, .. }
            | UiEvent::Release { at: p, .. }
            | UiEvent::Wheel { at: p, .. } => Some(p),
            UiEvent::CursorLeft | UiEvent::Char(_) | UiEvent::Action(_) => None,
        }
    }
}

/// The input layer's side of the UI: drains this frame's input as UI
/// events, in order. Implemented by the controls/input module (task C9:
/// key and button bindings → actions; [`super::edge`] for the cursor).
pub trait UiInput {
    fn drain(&mut self, out: &mut Vec<UiEvent>);
}

/// An encoded 1.14d C→S message (what `bridge::intent` sends). Only the
/// root forwards it (§A2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientIntent(pub Vec<u8>);

impl ClientIntent {
    pub fn from_message<M: FixedMessage>(msg: &M) -> Self {
        Self(intent::encode(msg))
    }
}

/// A panel's answer to an event (§A2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiResponse {
    Consumed,
    Ignored,
    Intent(ClientIntent),
}

/// String lookup by table key, UTF-16 as stored (`formats/tbl.md`).
pub trait StringLookup {
    fn get(&self, key: &str) -> Option<&[u16]>;

    /// Lookup by string-table id (`ui/text.md` §2; the panels of
    /// `ui/panels.md` name strings by id). None until a table by id is
    /// wired (`d2-data::strings`).
    fn get_id(&self, _id: u16) -> Option<&[u16]> {
        None
    }
}

/// A lookup with no strings (tests, panels that need none).
pub struct NoStrings;

impl StringLookup for NoStrings {
    fn get(&self, _key: &str) -> Option<&[u16]> {
        None
    }
}

/// Read-only data for drawing and events (§A2). Fonts are not here: text
/// layout runs on the draw-sink side (§A3), which owns them.
pub struct UiCtx<'a> {
    /// Client frame tick.
    pub tick: u64,
    pub world: &'a ClientWorld,
    pub strings: &'a dyn StringLookup,
}

/// A panel (§A2). `rect` and `hit` are integer, in the 800×600 frame.
pub trait Panel {
    fn id(&self) -> PanelId;
    fn rect(&self) -> Rect;
    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink);
    /// A panel drawn right before another open panel rather than at its
    /// own place in the root (the routing order is unchanged): the
    /// inventory family's mode panels before the inventory
    /// (`ui/panels.md` §9 r1 revision).
    fn draw_before(&self) -> Option<PanelId> {
        None
    }
    fn hit(&self, p: Point) -> Option<WidgetId>;
    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse;
}

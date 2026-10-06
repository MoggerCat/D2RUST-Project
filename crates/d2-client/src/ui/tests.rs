// Spec: specs/client/ui.md
//! Test vectors of spec §A2 and §A4, plus the module's own rules.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::math::{DVec2, Vec2};
use bevy::window::{Window, WindowResolution};
use d2_proto::client::Walk;
use d2_proto::PROTOCOL_VERSION;

use super::edge;
use super::widget::{Button, Cell, CellGrid, FrameImage, Label, ScrollList, TextInput, Widget};
use super::*;
use crate::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use crate::bridge::world::ClientWorld;
use crate::bridge::{Bridge, BridgeError};

type Log = Rc<RefCell<Vec<(PanelId, UiEvent)>>>;

/// A panel with one button that answers every event with `answer` and
/// logs what it saw.
struct TestPanel {
    id: PanelId,
    rect: Rect,
    button: Button,
    answer: UiResponse,
    log: Log,
}

impl TestPanel {
    fn boxed(id: u16, rect: Rect, answer: UiResponse, log: &Log) -> Box<dyn Panel> {
        Box::new(Self {
            id: PanelId(id),
            rect,
            button: Button {
                id: WidgetId(id * 10),
                rect: Rect::new(rect.x, rect.y, 10, 10),
                image: Some(ImageRef {
                    file: u32::from(id),
                    frame: 0,
                }),
            },
            answer,
            log: log.clone(),
        })
    }
}

impl Panel for TestPanel {
    fn id(&self) -> PanelId {
        self.id
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn draw(&self, _ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        self.button.draw(out);
    }
    fn hit(&self, p: Point) -> Option<WidgetId> {
        self.button.hit(p)
    }
    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        self.log.borrow_mut().push((self.id, e));
        self.answer.clone()
    }
}

fn with_ctx<R>(f: impl FnOnce(&UiCtx) -> R) -> R {
    let world = ClientWorld::default();
    let ctx = UiCtx {
        tick: 0,
        world: &world,
        strings: &NoStrings,
    };
    f(&ctx)
}

fn press(x: i32, y: i32) -> UiEvent {
    UiEvent::Press {
        button: PointerButton::Left,
        at: Point::new(x, y),
    }
}

/// Panel 1 at (0,0) 200×200 below panel 2 at (100,100) 200×200, both open.
fn overlapping(a1: UiResponse, a2: UiResponse) -> (UiRoot, Log) {
    let log = Log::default();
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    root.add(TestPanel::boxed(1, Rect::new(0, 0, 200, 200), a1, &log))
        .unwrap();
    root.add(TestPanel::boxed(2, Rect::new(100, 100, 200, 200), a2, &log))
        .unwrap();
    root.open(PanelId(1)).unwrap();
    root.open(PanelId(2)).unwrap();
    (root, log)
}

// ---- §A4 frame mapping ----

// Covers: specs/client/ui.md §a4-input-actions
#[test]
fn scale_two_corner_maps_to_last_pixel() {
    let p = Presentation::new(1600, 1200).unwrap();
    assert_eq!((p.scale, p.left, p.top), (2, 0, 0));
    assert_eq!(
        p.to_frame(1599, 1199),
        FramePos::Inside(Point::new(799, 599))
    );
    assert_eq!(p.to_frame(0, 0), FramePos::Inside(Point::new(0, 0)));
    assert_eq!(p.to_frame(1, 1), FramePos::Inside(Point::new(0, 0)));
    assert_eq!(p.to_frame(2, 3), FramePos::Inside(Point::new(1, 1)));
}

// Covers: specs/client/ui.md §a4-input-actions
#[test]
fn black_bars_are_outside() {
    let p = Presentation::new(1700, 1200).unwrap();
    assert_eq!((p.scale, p.left, p.top), (2, 50, 0));
    assert_eq!(p.to_frame(25, 0), FramePos::Outside);
    assert_eq!(p.to_frame(49, 0), FramePos::Outside);
    assert_eq!(p.to_frame(50, 0), FramePos::Inside(Point::new(0, 0)));
    assert_eq!(
        p.to_frame(1649, 1199),
        FramePos::Inside(Point::new(799, 599))
    );
    assert_eq!(p.to_frame(1650, 0), FramePos::Outside);
    // Outside the window entirely.
    assert_eq!(p.to_frame(-1, 5), FramePos::Outside);
    assert_eq!(p.to_frame(100, 1200), FramePos::Outside);
}

// Covers: specs/client/ui.md §a4-input-actions
#[test]
fn presentation_scale_and_bars() {
    let p = Presentation::new(800, 600).unwrap();
    assert_eq!((p.scale, p.left, p.top), (1, 0, 0));
    assert_eq!(p.to_frame(799, 599), FramePos::Inside(Point::new(799, 599)));
    assert_eq!(p.to_frame(800, 0), FramePos::Outside);
    // Scale limited by the height; odd remainder rounds the left bar down.
    let p = Presentation::new(2401, 1300).unwrap();
    assert_eq!((p.scale, p.left, p.top), (2, 400, 50));
    assert_eq!(p.to_frame(400, 50), FramePos::Inside(Point::new(0, 0)));
    assert_eq!(p.to_frame(399, 50), FramePos::Outside);
    assert_eq!(
        Presentation::new(799, 600),
        Err(FrameError::TooSmall { w: 799, h: 600 })
    );
    assert_eq!(
        Presentation::new(1600, 599),
        Err(FrameError::TooSmall { w: 1600, h: 599 })
    );
}

// Covers: specs/client/ui.md §a4-input-actions
#[test]
fn bevy_edge_reads_physical_cursor() {
    let mut w = Window {
        resolution: WindowResolution::new(1600, 1200).with_scale_factor_override(1.0),
        ..Default::default()
    };
    assert_eq!(edge::cursor_frame_pos(&w), Ok(FramePos::Outside));
    w.set_physical_cursor_position(Some(DVec2::new(1599.75, 1199.5)));
    assert_eq!(
        edge::cursor_frame_pos(&w),
        Ok(FramePos::Inside(Point::new(799, 599)))
    );
    assert_eq!(edge::window_pixel(Vec2::new(-0.5, 2.9)), (-1, 2));
    let small = Window {
        resolution: WindowResolution::new(640, 480).with_scale_factor_override(1.0),
        ..Default::default()
    };
    assert!(edge::cursor_frame_pos(&small).is_err());
}

// ---- §A2 routing ----

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn overlapping_panels_top_most_gets_the_event() {
    let (mut root, log) = overlapping(UiResponse::Consumed, UiResponse::Consumed);
    with_ctx(|ctx| {
        assert_eq!(
            root.dispatch(press(150, 150), ctx),
            Routed::Panel(PanelId(2))
        );
        assert_eq!(
            root.hit(Point::new(150, 150)),
            Some(UiHit {
                panel: PanelId(2),
                widget: None
            })
        );
        // Only the lower panel covers (50,50).
        assert_eq!(root.dispatch(press(50, 50), ctx), Routed::Panel(PanelId(1)));
        assert_eq!(root.dispatch(press(500, 500), ctx), Routed::Unhandled);
    });
    assert_eq!(
        *log.borrow(),
        vec![(PanelId(2), press(150, 150)), (PanelId(1), press(50, 50))]
    );
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn ignored_event_walks_down_then_is_unhandled() {
    let (mut root, log) = overlapping(UiResponse::Consumed, UiResponse::Ignored);
    with_ctx(|ctx| {
        assert_eq!(
            root.dispatch(press(150, 150), ctx),
            Routed::Panel(PanelId(1))
        );
        // Panel 2 alone covers (250,250) and ignores it.
        assert_eq!(root.dispatch(press(250, 250), ctx), Routed::Unhandled);
    });
    assert_eq!(
        *log.borrow(),
        vec![
            (PanelId(2), press(150, 150)),
            (PanelId(1), press(150, 150)),
            (PanelId(2), press(250, 250)),
        ]
    );
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn closed_panels_get_nothing_and_draw_nothing() {
    let (mut root, log) = overlapping(UiResponse::Consumed, UiResponse::Consumed);
    root.close(PanelId(2)).unwrap();
    with_ctx(|ctx| {
        assert_eq!(
            root.dispatch(press(150, 150), ctx),
            Routed::Panel(PanelId(1))
        );
        assert_eq!(root.dispatch(press(250, 250), ctx), Routed::Unhandled);
        let mut out = Vec::new();
        root.draw(ctx, &mut out);
        assert_eq!(out.len(), 1);
    });
    assert_eq!(log.borrow().len(), 1);
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn non_pointer_events_go_top_most_first_to_all_open_panels() {
    let (mut root, log) = overlapping(UiResponse::Consumed, UiResponse::Ignored);
    with_ctx(|ctx| {
        let a = UiEvent::Action(ActionId(3));
        assert_eq!(root.dispatch(a, ctx), Routed::Panel(PanelId(1)));
        assert_eq!(*log.borrow(), vec![(PanelId(2), a), (PanelId(1), a)]);
    });
    let (mut root, _) = overlapping(UiResponse::Ignored, UiResponse::Ignored);
    with_ctx(|ctx| assert_eq!(root.dispatch(UiEvent::Char(0x41), ctx), Routed::Unhandled));
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn draw_is_bottom_most_first() {
    let (root, _) = overlapping(UiResponse::Consumed, UiResponse::Consumed);
    let mut out = Vec::new();
    with_ctx(|ctx| root.draw(ctx, &mut out));
    let files: Vec<u32> = out
        .iter()
        .map(|d| match d {
            UiDraw::Image(i) => i.image.file,
            UiDraw::Text(_) => panic!("text"),
        })
        .collect();
    assert_eq!(files, vec![1, 2]);
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn hover_follows_cursor_and_widgets() {
    let (mut root, _) = overlapping(UiResponse::Ignored, UiResponse::Ignored);
    with_ctx(|ctx| {
        assert_eq!(root.hovered(), None);
        root.dispatch(UiEvent::CursorMoved(Point::new(105, 105)), ctx);
        assert_eq!(
            root.hovered(),
            Some(UiHit {
                panel: PanelId(2),
                widget: Some(WidgetId(20))
            })
        );
        root.close(PanelId(2)).unwrap();
        assert_eq!(
            root.hovered(),
            Some(UiHit {
                panel: PanelId(1),
                widget: None
            })
        );
        root.dispatch(UiEvent::CursorLeft, ctx);
        assert_eq!(root.hovered(), None);
    });
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn panel_ids_are_strict() {
    let log = Log::default();
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    root.add(TestPanel::boxed(1, FRAME, UiResponse::Ignored, &log))
        .unwrap();
    assert_eq!(
        root.add(TestPanel::boxed(1, FRAME, UiResponse::Ignored, &log)),
        Err(UiError::DuplicatePanel(PanelId(1)))
    );
    assert_eq!(
        root.open(PanelId(9)),
        Err(UiError::UnknownPanel(PanelId(9)))
    );
    assert_eq!(
        root.close(PanelId(9)),
        Err(UiError::UnknownPanel(PanelId(9)))
    );
    assert_eq!(
        root.is_open(PanelId(9)),
        Err(UiError::UnknownPanel(PanelId(9)))
    );
    assert_eq!(root.is_open(PanelId(1)), Ok(false));
    root.toggle(PanelId(1)).unwrap();
    assert_eq!(root.open_panels(), vec![PanelId(1)]);
    root.toggle(PanelId(1)).unwrap();
    assert!(root.open_panels().is_empty());
}

/// Closes every listed panel when `opening` opens.
struct Excludes(Vec<(PanelId, Vec<PanelId>)>, Rc<RefCell<Vec<Vec<PanelId>>>>);

impl PanelRules for Excludes {
    fn on_open(&mut self, opening: PanelId, open: &[PanelId]) -> Vec<PanelId> {
        self.1.borrow_mut().push(open.to_vec());
        self.0
            .iter()
            .find(|(p, _)| *p == opening)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn panel_rules_hook_closes_and_is_strict() {
    let log = Log::default();
    let seen = Rc::default();
    let rules = Excludes(
        vec![
            (PanelId(3), vec![PanelId(1)]),
            (PanelId(4), vec![PanelId(1), PanelId(4)]),
            (PanelId(5), vec![PanelId(1), PanelId(77)]),
        ],
        Rc::clone(&seen),
    );
    let mut root = UiRoot::new(Box::new(rules));
    for id in 1..=5 {
        root.add(TestPanel::boxed(id, FRAME, UiResponse::Ignored, &log))
            .unwrap();
    }
    root.open(PanelId(1)).unwrap();
    root.open(PanelId(2)).unwrap();
    root.open(PanelId(3)).unwrap();
    assert_eq!(root.open_panels(), vec![PanelId(2), PanelId(3)]);
    // Already open: rules not consulted.
    root.open(PanelId(3)).unwrap();
    assert_eq!(
        *seen.borrow(),
        vec![vec![], vec![PanelId(1)], vec![PanelId(1), PanelId(2)]]
    );
    root.open(PanelId(1)).unwrap();
    // A rule that closes the opening panel, or names an unknown one, is an
    // error and changes nothing.
    assert_eq!(
        root.open(PanelId(4)),
        Err(UiError::RuleClosedOpening { closed: PanelId(4) })
    );
    assert_eq!(
        root.open(PanelId(5)),
        Err(UiError::UnknownPanel(PanelId(77)))
    );
    assert_eq!(root.open_panels(), vec![PanelId(1), PanelId(2), PanelId(3)]);
}

// ---- intents ----

struct RecordLink(Rc<RefCell<Vec<Vec<u8>>>>);

impl ServerLink for RecordLink {
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
    fn send(&mut self, _queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.0.borrow_mut().push(msg.to_vec());
        Ok(Sent::Queued)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        Ok(Pumped { ticked: false })
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        Vec::new()
    }
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn only_the_root_forwards_intents_in_order() {
    let walk = ClientIntent::from_message(&Walk { x: 7, y: 9 });
    let (mut root, _) = overlapping(UiResponse::Consumed, UiResponse::Intent(walk.clone()));
    with_ctx(|ctx| {
        assert_eq!(
            root.dispatch(press(150, 150), ctx),
            Routed::Panel(PanelId(2))
        );
        assert_eq!(
            root.dispatch(press(250, 250), ctx),
            Routed::Panel(PanelId(2))
        );
    });
    assert_eq!(root.intents(), &[walk.clone(), walk.clone()]);

    let sent = Rc::default();
    let mut bridge = Bridge::new(RecordLink(Rc::clone(&sent))).unwrap();
    assert_eq!(root.forward(&mut bridge).unwrap(), 2);
    assert!(root.intents().is_empty());
    assert_eq!(*sent.borrow(), vec![walk.0.clone(), walk.0.clone()]);
}

/// A frame-sized panel answering events with a fixed list of intents.
struct ListPanel {
    id: PanelId,
    answers: std::collections::VecDeque<ClientIntent>,
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn refused_intent_stays_queued() {
    let walk = ClientIntent::from_message(&Walk { x: 1, y: 2 });
    let bad = ClientIntent(vec![0x80; 5]);
    let mut r = UiRoot::new(Box::new(NoPanelRules));
    r.add(Box::new(ListPanel {
        id: PanelId(1),
        answers: [walk.clone(), bad.clone(), walk.clone()].into(),
    }))
    .unwrap();
    r.open(PanelId(1)).unwrap();
    with_ctx(|ctx| {
        for _ in 0..3 {
            assert_eq!(r.dispatch(press(1, 1), ctx), Routed::Panel(PanelId(1)));
        }
    });
    let sent = Rc::default();
    let mut bridge = Bridge::new(RecordLink(Rc::clone(&sent))).unwrap();
    assert!(matches!(
        r.forward(&mut bridge),
        Err(BridgeError::Intent(_))
    ));
    assert_eq!(*sent.borrow(), vec![walk.0.clone()]);
    assert_eq!(r.intents(), &[bad, walk]);
}

impl Panel for ListPanel {
    fn id(&self) -> PanelId {
        self.id
    }
    fn rect(&self) -> Rect {
        FRAME
    }
    fn draw(&self, _ctx: &UiCtx, _out: &mut dyn UiDrawSink) {}
    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }
    fn event(&mut self, _e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        self.answers
            .pop_front()
            .map_or(UiResponse::Ignored, UiResponse::Intent)
    }
}

struct Script(Vec<UiEvent>);

impl UiInput for Script {
    fn drain(&mut self, out: &mut Vec<UiEvent>) {
        out.append(&mut self.0);
    }
}

// Covers: specs/client/ui.md §a2-panel-model, §a4-input-actions
#[test]
fn pump_returns_unhandled_events_in_order() {
    let (mut root, _) = overlapping(UiResponse::Consumed, UiResponse::Consumed);
    let events = vec![
        UiEvent::CursorMoved(Point::new(700, 10)),
        press(150, 150),
        press(700, 10),
        UiEvent::Wheel {
            steps: -1,
            at: Point::new(10, 10),
        },
    ];
    let mut input = Script(events.clone());
    let left = with_ctx(|ctx| root.pump(&mut input, ctx));
    assert_eq!(left, vec![events[0], events[2]]);
    assert!(input.0.is_empty());
}

// ---- widgets ----

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn rect_hit_is_half_open() {
    let r = Rect::new(10, 20, 30, 40);
    assert!(r.contains(Point::new(10, 20)));
    assert!(r.contains(Point::new(39, 59)));
    assert!(!r.contains(Point::new(40, 20)));
    assert!(!r.contains(Point::new(10, 60)));
    assert!(!r.contains(Point::new(9, 20)));
    assert!(!Rect::new(0, 0, 0, 5).contains(Point::new(0, 0)));
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn widgets_emit_requests_and_hit_by_rect() {
    let img = ImageRef { file: 4, frame: 2 };
    let style = TextStyle { font: 1, color: 3 };
    let b = Button {
        id: WidgetId(1),
        rect: Rect::new(5, 6, 7, 8),
        image: None,
    };
    let f = FrameImage {
        id: WidgetId(2),
        rect: Rect::new(1, 1, 2, 2),
        image: img,
    };
    let l = Label {
        id: WidgetId(3),
        rect: Rect::new(0, 50, 100, 10),
        text: vec![0x48, 0x69],
        style,
    };
    let mut out = Vec::new();
    b.draw(&mut out);
    f.draw(&mut out);
    l.draw(&mut out);
    assert_eq!(
        out,
        vec![
            UiDraw::Image(ImageRequest {
                image: img,
                at: Point::new(1, 1),
                clip: FRAME
            }),
            UiDraw::Text(TextRequest {
                text: vec![0x48, 0x69],
                at: Point::new(0, 50),
                style,
                clip: FRAME
            }),
        ]
    );
    assert_eq!(b.hit(Point::new(11, 13)), Some(WidgetId(1)));
    assert_eq!(b.hit(Point::new(12, 13)), None);
    assert_eq!(l.hit(Point::new(99, 59)), Some(WidgetId(3)));
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn cell_grid_integer_hit() {
    let g = CellGrid::new(WidgetId(1), Point::new(100, 200), 10, 4, 29, 29).unwrap();
    assert_eq!(g.rect(), Rect::new(100, 200, 290, 116));
    assert_eq!(
        g.cell_at(Point::new(100, 200)),
        Some(Cell { col: 0, row: 0 })
    );
    assert_eq!(
        g.cell_at(Point::new(128, 228)),
        Some(Cell { col: 0, row: 0 })
    );
    assert_eq!(
        g.cell_at(Point::new(129, 229)),
        Some(Cell { col: 1, row: 1 })
    );
    assert_eq!(
        g.cell_at(Point::new(389, 315)),
        Some(Cell { col: 9, row: 3 })
    );
    assert_eq!(g.cell_at(Point::new(390, 315)), None);
    assert_eq!(g.cell_at(Point::new(99, 200)), None);
    assert_eq!(
        g.cell_rect(Cell { col: 2, row: 1 }),
        Some(Rect::new(158, 229, 29, 29))
    );
    assert_eq!(g.cell_rect(Cell { col: 10, row: 0 }), None);
    assert!(CellGrid::new(WidgetId(2), Point::new(0, 0), 0, 4, 29, 29).is_err());
    assert!(CellGrid::new(WidgetId(2), Point::new(0, 0), 4, 4, 29, 0).is_err());
    assert!(CellGrid::new(WidgetId(2), Point::new(0, 0), 1000, 1, 100, 1).is_err());
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn scroll_list_rows_and_clamp() {
    let mut s = ScrollList::new(WidgetId(1), Rect::new(0, 0, 100, 50), 10).unwrap();
    assert_eq!(s.rows_visible(), 5);
    assert_eq!(s.row_at(Point::new(5, 5)), None);
    s.set_len(8);
    assert_eq!(s.row_at(Point::new(5, 49)), Some(4));
    s.scroll(10);
    assert_eq!(s.first(), 3);
    assert_eq!(s.row_at(Point::new(5, 0)), Some(3));
    assert_eq!(s.row_at(Point::new(5, 49)), Some(7));
    s.scroll(-1);
    assert_eq!(s.first(), 2);
    s.scroll(-100);
    assert_eq!(s.first(), 0);
    s.scroll(3);
    s.set_len(6);
    assert_eq!(s.first(), 1);
    s.set_len(2);
    assert_eq!(s.first(), 0);
    assert_eq!(s.row_at(Point::new(5, 25)), None);
    // A 55-high list shows 5 whole rows; the last 5 pixels are no row.
    let s = ScrollList::new(WidgetId(1), Rect::new(0, 0, 10, 55), 10).unwrap();
    assert_eq!(s.rows_visible(), 5);
    assert!(ScrollList::new(WidgetId(1), Rect::new(0, 0, 10, 5), 10).is_err());
    assert!(ScrollList::new(WidgetId(1), Rect::new(0, 0, 10, 5), 0).is_err());
}

// Covers: specs/client/ui.md §a2-panel-model
#[test]
fn text_input_keeps_code_units() {
    let mut t = TextInput::new(
        WidgetId(1),
        Rect::new(0, 0, 50, 10),
        TextStyle::default(),
        2,
    );
    assert!(!t.backspace());
    assert!(t.insert(0x00FF));
    assert!(t.insert(0x0063));
    assert!(!t.insert(0x0031));
    assert_eq!(t.text(), &[0x00FF, 0x0063]);
    assert!(t.backspace());
    assert_eq!(t.take(), vec![0x00FF]);
    assert!(t.text().is_empty());
}

// Covers: specs/client/ui.md §a5-logical-resolution
#[test]
fn one_logical_frame_of_800_by_600() {
    assert_eq!((FRAME_W, FRAME_H), (800, 600));
    assert_eq!(FRAME, Rect::new(0, 0, 800, 600));
    // Every window shows the same 800×600 frame, only scaled: its last
    // image pixel is frame pixel (799, 599) whatever the window size.
    for (w, h) in [
        (800, 600),
        (1024, 768),
        (1280, 720),
        (1920, 1080),
        (2560, 1440),
    ] {
        let p = Presentation::new(w, h).unwrap();
        let (x1, y1) = (p.left + 800 * p.scale - 1, p.top + 600 * p.scale - 1);
        assert_eq!(
            p.to_frame(i64::from(x1), i64::from(y1)),
            FramePos::Inside(Point::new(799, 599)),
            "{w}×{h}"
        );
    }
    // No 640×480 layout: a window that cannot hold 800×600 is refused.
    assert_eq!(
        Presentation::new(640, 480),
        Err(FrameError::TooSmall { w: 640, h: 480 })
    );
}

// --- Text (§A3) --------------------------------------------------------------

mod text {
    use std::cell::RefCell;

    use d2_formats::font::{FontTable, Glyph};

    use super::super::text::*;
    use super::*;

    fn glyph(code: u16, width: u8, frame: u8) -> Glyph {
        Glyph {
            code,
            unknown1: 0,
            width,
            height: 10,
            unknown2: 1,
            unknown3: 0,
            frame,
            unknown4: 0,
            unknown5: 0,
        }
    }

    /// Records are deliberately not in code order: lookup goes by the
    /// `code` field, not by index.
    fn font() -> FontTable {
        FontTable {
            version: 1,
            unknown: [0; 4],
            height: 10,
            width: 8,
            glyphs: vec![glyph(0x42, 7, 9), glyph(0x41, 5, 3), glyph(0x00ff, 4, 200)],
        }
    }

    /// Test-only rules (not the original's): one glyph per code unit,
    /// advancing by the record's `width`; records the text it was given.
    #[derive(Default)]
    struct Advance {
        seen: RefCell<Vec<Vec<u16>>>,
    }

    impl TextRules for Advance {
        fn place(
            &self,
            glyphs: &GlyphLookup<'_>,
            text: &[u16],
            origin: Point,
            style: TextStyle,
            _: &TextOpts,
        ) -> Result<Vec<GlyphPlacement>, TextError> {
            self.seen.borrow_mut().push(text.to_vec());
            let mut x = origin.x;
            text.iter()
                .map(|&code| {
                    let at = Point::new(x, origin.y);
                    x += i32::from(glyphs.font().glyphs[glyphs.record(code)?].width);
                    Ok(GlyphPlacement {
                        code,
                        at,
                        color: style.color,
                    })
                })
                .collect()
        }
    }

    // Covers: specs/client/ui.md §a3-text
    #[test]
    fn glyphs_resolve_code_to_record_to_frame() {
        let style = TextStyle { font: 1, color: 4 };
        let out = layout_text(
            &font(),
            &[0x41, 0x42, 0x41],
            Point::new(10, 20),
            style,
            &TextOpts::default(),
            &Advance::default(),
        )
        .unwrap();
        assert_eq!(
            out,
            [
                GlyphDraw {
                    code: 0x41,
                    record: 1,
                    frame: 3,
                    at: Point::new(10, 20),
                    color: 4
                },
                GlyphDraw {
                    code: 0x42,
                    record: 0,
                    frame: 9,
                    at: Point::new(15, 20),
                    color: 4
                },
                GlyphDraw {
                    code: 0x41,
                    record: 1,
                    frame: 3,
                    at: Point::new(22, 20),
                    color: 4
                },
            ]
        );
    }

    /// Records the options `layout_text` hands the rules (decision CG2).
    #[derive(Default)]
    struct SeeOpts {
        seen: RefCell<Vec<TextOpts>>,
    }

    impl TextRules for SeeOpts {
        fn place(
            &self,
            _: &GlyphLookup<'_>,
            _: &[u16],
            _: Point,
            _: TextStyle,
            opts: &TextOpts,
        ) -> Result<Vec<GlyphPlacement>, TextError> {
            self.seen.borrow_mut().push(*opts);
            Ok(Vec::new())
        }
    }

    #[test]
    fn layout_hands_the_clip_to_the_rules_unchanged() {
        let rules = SeeOpts::default();
        let clip = Rect::new(-3, 7, 120, 40);
        for opts in [TextOpts { clip: Some(clip) }, TextOpts::default()] {
            let out = layout_text(
                &font(),
                &[0x41],
                Point::new(0, 0),
                TextStyle::default(),
                &opts,
                &rules,
            )
            .unwrap();
            assert!(out.is_empty());
        }
        assert_eq!(
            *rules.seen.borrow(),
            [TextOpts { clip: Some(clip) }, TextOpts { clip: None }]
        );
    }

    // Covers: specs/client/ui.md §a3-text
    #[test]
    fn missing_code_is_an_error_not_a_fallback_glyph() {
        let err = layout_text(
            &font(),
            &[0x41, 0x43],
            Point::new(0, 0),
            TextStyle::default(),
            &TextOpts::default(),
            &Advance::default(),
        )
        .unwrap_err();
        assert_eq!(err, TextError::MissingGlyph(0x43));

        // A placement naming a code the font lacks fails at resolution too.
        struct Stray;
        impl TextRules for Stray {
            fn place(
                &self,
                _: &GlyphLookup<'_>,
                _: &[u16],
                origin: Point,
                _: TextStyle,
                _: &TextOpts,
            ) -> Result<Vec<GlyphPlacement>, TextError> {
                Ok(vec![GlyphPlacement {
                    code: 0x7a,
                    at: origin,
                    color: 0,
                }])
            }
        }
        let err = layout_text(
            &font(),
            &[0x41],
            Point::new(0, 0),
            TextStyle::default(),
            &TextOpts::default(),
            &Stray,
        )
        .unwrap_err();
        assert_eq!(err, TextError::MissingGlyph(0x7a));

        let mut dup = font();
        dup.glyphs.push(glyph(0x41, 1, 1));
        assert_eq!(
            GlyphLookup::new(&dup).record(0x41),
            Err(TextError::AmbiguousGlyph {
                code: 0x41,
                count: 2
            })
        );
    }

    // Covers: specs/client/ui.md §a3-text
    #[test]
    fn text_reaches_the_rules_as_utf16_units_unchanged() {
        // `ÿ` (0x00ff, the color-code lead) and a lone surrogate: the
        // units are passed as given, never re-encoded.
        let text = [0x00ff, 0x41, 0xd800];
        let rules = Advance::default();
        let err = layout_text(
            &font(),
            &text,
            Point::new(0, 0),
            TextStyle::default(),
            &TextOpts::default(),
            &rules,
        )
        .unwrap_err();
        assert_eq!(*rules.seen.borrow(), [text.to_vec()]);
        assert_eq!(err, TextError::MissingGlyph(0xd800));
    }

    // Covers: specs/client/ui.md §a3-text
    #[test]
    fn layout_rules_are_unspecified_until_ui_text_md() {
        let err = layout_text(
            &font(),
            &[0x41],
            Point::new(0, 0),
            TextStyle::default(),
            &TextOpts::default(),
            &NoTextRules,
        )
        .unwrap_err();
        assert_eq!(err, TextError::Unspecified("ui/text.md §B3"));
        assert!(err.to_string().contains("TODO(spec: ui/text.md §B3)"));
    }
}

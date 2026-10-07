// Spec: specs/client/ui.md
//! Widgets (spec §A2): plain structs with integer rects that emit draw
//! requests and answer hit tests. No retained GPU state.
//!
//! Hit tests are by rect. Whether the original tests a control's rect or
//! its opaque pixels, and every size, position and frame a widget uses,
//! come from the §B owner specs through the caller.

use super::draw::{ImageRef, ImageRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
use super::geom::{Point, Rect, FRAME};
use super::panel::WidgetId;
use super::text::TextOpts;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WidgetError {
    #[error("widget {0:?}: zero size")]
    ZeroSize(WidgetId),
    #[error("grid {0:?}: rect overflows i32")]
    TooLarge(WidgetId),
}

/// What every widget answers.
pub trait Widget {
    fn id(&self) -> WidgetId;
    fn rect(&self) -> Rect;
    fn draw(&self, out: &mut dyn UiDrawSink);
    fn hit(&self, p: Point) -> Option<WidgetId> {
        self.rect().contains(p).then_some(self.id())
    }
}

/// A clickable rect with an optional image. Pressed/hover frames are
/// `TODO(spec: ui/panels.md §B1)`: the image does not change here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    pub id: WidgetId,
    pub rect: Rect,
    pub image: Option<ImageRef>,
}

impl Widget for Button {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn draw(&self, out: &mut dyn UiDrawSink) {
        if let Some(image) = self.image {
            out.push(UiDraw::Image(ImageRequest {
                image,
                at: self.rect.origin(),
                clip: FRAME,
            }));
        }
    }
}

/// A static frame image (panel art) over a rect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameImage {
    pub id: WidgetId,
    pub rect: Rect,
    pub image: ImageRef,
}

impl Widget for FrameImage {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn draw(&self, out: &mut dyn UiDrawSink) {
        out.push(UiDraw::Image(ImageRequest {
            image: self.image,
            at: self.rect.origin(),
            clip: FRAME,
        }));
    }
}

/// A text label. The request's pen is the bottom row of the first-drawn
/// line (`ui/text.md` §4.2); where it sits in the rect per panel is
/// `TODO(spec: ui/panels.md)`: the request carries the rect origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub id: WidgetId,
    pub rect: Rect,
    pub text: Vec<u16>,
    pub style: TextStyle,
}

impl Widget for Label {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn draw(&self, out: &mut dyn UiDrawSink) {
        out.push(UiDraw::Text(TextRequest {
            text: self.text.clone(),
            at: self.rect.origin(),
            style: self.style,
            opts: TextOpts::default(),
            clip: FRAME,
        }));
    }
}

/// A grid cell, column and row from the top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub col: u16,
    pub row: u16,
}

/// A grid of equal cells (inventory, stash, cube, belt). Cell sizes,
/// gaps, item placement and highlight are `TODO(spec: ui/inventory.md
/// §B5)`: here cells are adjacent, and the grid draws nothing itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellGrid {
    id: WidgetId,
    origin: Point,
    cols: u16,
    rows: u16,
    cell_w: u16,
    cell_h: u16,
}

impl CellGrid {
    pub fn new(
        id: WidgetId,
        origin: Point,
        cols: u16,
        rows: u16,
        cell_w: u16,
        cell_h: u16,
    ) -> Result<Self, WidgetError> {
        if cols == 0 || rows == 0 || cell_w == 0 || cell_h == 0 {
            return Err(WidgetError::ZeroSize(id));
        }
        let w = u32::from(cols) * u32::from(cell_w);
        let h = u32::from(rows) * u32::from(cell_h);
        // Every pixel of the grid must be an i32 point, so cell rects and
        // hit math stay in range.
        let past_end = |o: i32, size: u32| i64::from(o) + i64::from(size) > i64::from(i32::MAX) + 1;
        if w > u32::from(u16::MAX)
            || h > u32::from(u16::MAX)
            || past_end(origin.x, w)
            || past_end(origin.y, h)
        {
            return Err(WidgetError::TooLarge(id));
        }
        Ok(Self {
            id,
            origin,
            cols,
            rows,
            cell_w,
            cell_h,
        })
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// The cell under `p` (integer division).
    pub fn cell_at(&self, p: Point) -> Option<Cell> {
        if !self.rect().contains(p) {
            return None;
        }
        Some(Cell {
            col: ((p.x - self.origin.x) / i32::from(self.cell_w)) as u16,
            row: ((p.y - self.origin.y) / i32::from(self.cell_h)) as u16,
        })
    }

    /// A cell's rect; `None` outside the grid.
    pub fn cell_rect(&self, c: Cell) -> Option<Rect> {
        (c.col < self.cols && c.row < self.rows).then(|| {
            Rect::new(
                self.origin.x + i32::from(c.col) * i32::from(self.cell_w),
                self.origin.y + i32::from(c.row) * i32::from(self.cell_h),
                self.cell_w,
                self.cell_h,
            )
        })
    }
}

impl Widget for CellGrid {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn rect(&self) -> Rect {
        Rect::new(
            self.origin.x,
            self.origin.y,
            self.cols * self.cell_w,
            self.rows * self.cell_h,
        )
    }
    fn draw(&self, _out: &mut dyn UiDrawSink) {
        // TODO(spec: ui/inventory.md §B5): cell art, item graphics,
        // hover highlight.
    }
}

/// A vertical list of equal rows showing `rows_visible()` rows from
/// `first`. The owning panel draws the rows; the list answers which row
/// is under a point and keeps `first` in range. How far one wheel step
/// scrolls is `TODO(spec: ui/panels.md §B2)`: the caller passes rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollList {
    id: WidgetId,
    rect: Rect,
    row_h: u16,
    len: u32,
    first: u32,
}

impl ScrollList {
    pub fn new(id: WidgetId, rect: Rect, row_h: u16) -> Result<Self, WidgetError> {
        if row_h == 0 || rect.h < row_h || rect.w == 0 {
            return Err(WidgetError::ZeroSize(id));
        }
        Ok(Self {
            id,
            rect,
            row_h,
            len: 0,
            first: 0,
        })
    }

    pub fn rows_visible(&self) -> u32 {
        u32::from(self.rect.h / self.row_h)
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn first(&self) -> u32 {
        self.first
    }

    fn max_first(&self) -> u32 {
        self.len.saturating_sub(self.rows_visible())
    }

    /// Sets the row count; `first` is clamped to the new range.
    pub fn set_len(&mut self, len: u32) {
        self.len = len;
        self.first = self.first.min(self.max_first());
    }

    /// Moves `first` by `rows` (negative: up), clamped to the range.
    pub fn scroll(&mut self, rows: i64) {
        let f = i64::from(self.first)
            .saturating_add(rows)
            .clamp(0, i64::from(self.max_first()));
        self.first = f as u32;
    }

    /// The list row under `p`; `None` past the last row.
    pub fn row_at(&self, p: Point) -> Option<u32> {
        if !self.rect.contains(p) {
            return None;
        }
        let r = ((p.y - self.rect.y) / i32::from(self.row_h)) as u32;
        (r < self.rows_visible() && self.first + r < self.len).then_some(self.first + r)
    }
}

impl Widget for ScrollList {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn draw(&self, _out: &mut dyn UiDrawSink) {}
}

/// A single-line text field holding UTF-16 code units. Which characters
/// are accepted and the maximum length come from the owner of the field
/// (chat: spec open question 3); caret drawing is
/// `TODO(spec: ui/text.md open question 3)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInput {
    pub id: WidgetId,
    pub rect: Rect,
    pub style: TextStyle,
    max_len: usize,
    text: Vec<u16>,
}

impl TextInput {
    pub fn new(id: WidgetId, rect: Rect, style: TextStyle, max_len: usize) -> Self {
        Self {
            id,
            rect,
            style,
            max_len,
            text: Vec::new(),
        }
    }

    pub fn text(&self) -> &[u16] {
        &self.text
    }

    /// Appends one code unit; `false` (unchanged) when full.
    pub fn insert(&mut self, c: u16) -> bool {
        if self.text.len() >= self.max_len {
            return false;
        }
        self.text.push(c);
        true
    }

    /// Removes the last code unit; `false` when empty.
    pub fn backspace(&mut self) -> bool {
        self.text.pop().is_some()
    }

    /// Returns the text and empties the field.
    pub fn take(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.text)
    }
}

impl Widget for TextInput {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn draw(&self, out: &mut dyn UiDrawSink) {
        out.push(UiDraw::Text(TextRequest {
            text: self.text.clone(),
            at: self.rect.origin(),
            style: self.style,
            opts: TextOpts::default(),
            clip: FRAME,
        }));
    }
}

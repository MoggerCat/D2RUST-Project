// Spec: specs/client/render-pipeline.md (A8)
//! CPU reference compositor: the only definition of the d2rs image. The GPU
//! compositor must match it byte for byte. Straight loops, integers only.

use d2_formats::palette::Palette;

use super::bins::Bins;
use super::item::{DrawItem, FrameSource, FrameView, MapTable};
use super::{Rect, SceneError};

/// Composes `items` in list order into an index framebuffer of `view`
/// (row-major, `view.width × view.height`, pixel 0 = screen `(view.x,
/// view.y)`). Every item is validated before any pixel is drawn.
///
/// TODO(spec: render/composition.md): what the original clears the frame
/// to (§B2). Until then the framebuffer starts as index 0.
pub fn compose<F: FrameSource + ?Sized>(
    items: &[DrawItem],
    frames: &F,
    maps: &MapTable,
    view: Rect,
) -> Result<Vec<u8>, SceneError> {
    let resolved = resolve_all(items, frames, maps, &view)?;
    let mut out = clear(&view);
    let stride = view.width as usize;
    for (item, (frame, area)) in items.iter().zip(&resolved) {
        let Some(area) = area else { continue };
        for sy in i64::from(area.y)..i64::from(area.y) + i64::from(area.height) {
            let row = (sy - i64::from(view.y)) as usize * stride;
            for sx in i64::from(area.x)..i64::from(area.x) + i64::from(area.width) {
                let px = &mut out[row + (sx - i64::from(view.x)) as usize];
                *px = item.pixel(frame, maps, sx, sy, *px);
            }
        }
    }
    Ok(out)
}

/// Composes through `bins` the way the GPU does (§A9): per pixel, walk the
/// pixel's bin list in order. Equal to [`compose`] for the same input.
pub fn compose_binned<F: FrameSource + ?Sized>(
    items: &[DrawItem],
    bins: &Bins,
    frames: &F,
    maps: &MapTable,
    view: Rect,
) -> Result<Vec<u8>, SceneError> {
    if bins.view() != view {
        return Err(SceneError::BinsView {
            built: bins.view(),
            view,
        });
    }
    if bins.item_count() != items.len() {
        return Err(SceneError::BinsItems {
            built: bins.item_count(),
            items: items.len(),
        });
    }
    let resolved = resolve_all(items, frames, maps, &view)?;
    let mut out = clear(&view);
    let stride = view.width as usize;
    for row in 0..bins.rows() {
        for col in 0..bins.cols() {
            let list = bins.list(col, row);
            let rect = bins.rect(col, row);
            for sy in i64::from(rect.y)..i64::from(rect.y) + i64::from(rect.height) {
                for sx in i64::from(rect.x)..i64::from(rect.x) + i64::from(rect.width) {
                    let mut value = 0u8;
                    for &index in list {
                        let index = index as usize;
                        let (frame, area) = &resolved[index];
                        if area.is_some_and(|a| a.contains(sx, sy)) {
                            value = items[index].pixel(frame, maps, sx, sy, value);
                        }
                    }
                    let y = (sy - i64::from(view.y)) as usize;
                    out[y * stride + (sx - i64::from(view.x)) as usize] = value;
                }
            }
        }
    }
    Ok(out)
}

/// [`compose`] then [`to_rgba`]: the reference image of §A8.
pub fn compose_rgba<F: FrameSource + ?Sized>(
    items: &[DrawItem],
    frames: &F,
    maps: &MapTable,
    palette: &Palette,
    view: Rect,
) -> Result<Vec<u8>, SceneError> {
    Ok(to_rgba(&compose(items, frames, maps, view)?, palette))
}

/// Maps an index framebuffer to RGBA8 through the frame palette, alpha 255.
/// Every index, 0 included, is a plain palette lookup.
/// TODO(spec: render/shading.md): palettes per screen region (§B3); one
/// palette per frame until then.
pub fn to_rgba(indexed: &[u8], palette: &Palette) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(indexed.len() * 4);
    for &i in indexed {
        let c = palette.colors[usize::from(i)];
        rgba.extend_from_slice(&[c.r, c.g, c.b, 255]);
    }
    rgba
}

type Resolved<'f> = Vec<(FrameView<'f>, Option<Rect>)>;

fn resolve_all<'f, F: FrameSource + ?Sized>(
    items: &[DrawItem],
    frames: &'f F,
    maps: &MapTable,
    view: &Rect,
) -> Result<Resolved<'f>, SceneError> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| item.resolve(frames, maps, view).map_err(|e| e.at(index)))
        .collect()
}

fn clear(view: &Rect) -> Vec<u8> {
    vec![0u8; view.width as usize * view.height as usize]
}

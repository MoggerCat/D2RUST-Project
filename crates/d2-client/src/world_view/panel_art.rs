// Spec: specs/ui/panels.md (§1.3, §1.4, §1.6, §7.1), specs/render/shading.md (§3 r1), specs/render/blend-modes.md (mode 5), specs/render/draw-order.md (§10)
//! Panel art of the original UI in the world view: the `ui_image` hook
//! for the [`ImageRef`]s of `ui::panels` ([`PanelArtRules`]) and the
//! loader that makes their DC6 files resident ([`PanelArtLoader`]).
//!
//! An [`ImageRef`] names a file of [`UiFiles`] (`data\global\ui\<name>.dc6`,
//! §7.1) and a frame of its direction 0. A panel cel draw is draw mode 5,
//! light 0xFF, no palette remap (§1.4, §1.6): no shading map
//! (`shading.md` §3 r1: `v = 0xFF` has no `L`) and the opaque copy
//! (`blend-modes.md`: mode 5 is `Opaque`). The request's point is the
//! cel draw position (§1.3), placed by `sprite-placement.md` §2
//! ([`draw_position`]). Remapped draws (skill icons, `k`) have no field
//! in [`ImageRequest`]; none is wired (`ui::original::PENDING`).

use std::sync::Arc;

use crate::assets::path::{CanonicalPath, FileSource};
use crate::bridge::world::ClientWorld;
use crate::bridge::ClientUnit;
use crate::composite::{ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use crate::frames::{FramePart, FrameSet, FrameSetKey, IndexFrame};
use crate::rules::placement::draw_position;
use crate::scene::{BlendOp, ShadeChain};
use crate::ui::panels::UiFiles;
use crate::ui::{ImageRef, ImageRequest, TextRequest, UiDraw};

use super::{TileDraw, UiRules, UiSprite, UnitPose, ViewAssets, ViewError, ViewRules};

const SPEC: &str = "ui/panels.md";

/// The archive name of a [`UiFiles`] name (§7.1: relative to
/// `DATA\GLOBAL\UI\`, `.dc6`).
pub fn archive_name(name: &str) -> String {
    format!("data\\global\\ui\\{name}.dc6")
}

/// The frame set of an image's file: direction 0 of its DC6.
pub fn image_set(files: &UiFiles, image: ImageRef) -> Result<FrameSetKey, ViewError> {
    let name = files
        .name(image.file)
        .ok_or_else(|| ViewError::Unresolved {
            what: "UI image file",
            spec: SPEC,
            message: format!("file id {} names no panel file", image.file),
        })?;
    let path = CanonicalPath::new(&archive_name(name)).map_err(|e| ViewError::Unresolved {
        what: "UI image file",
        spec: SPEC,
        message: format!("{name}: {e}"),
    })?;
    FrameSetKey::new(path.as_str(), FramePart::Dir(0)).map_err(|e| ViewError::Unresolved {
        what: "UI image file",
        spec: SPEC,
        message: format!("{name}: {e}"),
    })
}

/// The panel cel draw of `req` (module doc).
pub fn panel_sprite(
    files: &UiFiles,
    req: &ImageRequest,
    assets: &ViewAssets,
) -> Result<UiSprite, ViewError> {
    let set = image_set(files, req.image)?;
    let index = req.image.frame as usize;
    let frame: &IndexFrame = assets.frame(&set, index)?;
    let (x, y) = draw_position(frame, req.at.x, req.at.y);
    Ok(UiSprite {
        frame: ComponentFrame { set, index },
        x,
        y,
        shade: ShadeChain::EMPTY,
        blend: BlendOp::Opaque,
    })
}

/// `rules` with the original panel art as its `ui_image` answer; every
/// other hook is `rules`'.
pub struct PanelArtRules<R> {
    pub rules: R,
    pub files: UiFiles,
    /// With text colours, `ui_text` is the original text hooks with the
    /// act's PL2 text-colour maps (`ui/text.md` §4.4,
    /// [`super::ui_bind::OriginalTextHooks`]); `None`: `rules`' answer.
    pub text: Option<SharedTextColors>,
}

/// The frame's PL2 text-colour maps (`ui/text.md` §4.4), set when the
/// palette act's maps are pushed; `None` inside: not yet pushed (only
/// colour 0 draws).
pub type SharedTextColors = std::sync::Arc<std::sync::RwLock<Option<super::ui_bind::TextColors>>>;

impl<R: ViewRules> ViewRules for PanelArtRules<R> {
    fn tiles(&self, world: &ClientWorld, assets: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        self.rules.tiles(world, assets)
    }

    fn unit_pose(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
    ) -> Result<Option<UnitPose>, ViewError> {
        self.rules.unit_pose(world, unit)
    }

    fn unit_params(
        &self,
        world: &ClientWorld,
        unit: &ClientUnit,
        pose: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        self.rules.unit_params(world, unit, pose)
    }

    fn component_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        self.rules.component_frame(unit, pose, req)
    }

    fn component_slot_frame(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
    ) -> Result<Option<ComponentFrame>, CompositeError> {
        self.rules.component_slot_frame(unit, pose, req)
    }

    fn place(
        &self,
        unit: &ClientUnit,
        pose: &UnitPose,
        req: &ComponentRequest<'_>,
        image: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        self.rules.place(unit, pose, req, image)
    }

    fn shade(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        self.rules.shade(unit, req)
    }

    fn blend(
        &self,
        unit: &ClientUnit,
        req: &ComponentRequest<'_>,
    ) -> Result<BlendOp, CompositeError> {
        self.rules.blend(unit, req)
    }
}

impl<R: UiRules> UiRules for PanelArtRules<R> {
    fn ui_image(&self, req: &ImageRequest, assets: &ViewAssets) -> Result<UiSprite, ViewError> {
        panel_sprite(&self.files, req, assets)
    }

    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        let Some(text) = &self.text else {
            return self.rules.ui_text(req, assets);
        };
        let colors = *text.read().unwrap_or_else(|e| e.into_inner());
        super::ui_bind::text_sprites(&super::ui_bind::OriginalTextHooks { colors }, req, assets)
    }

    /// Pass 11: everything after the world draw (`draw-order.md` §10),
    /// as [`crate::rules::OriginalView`] answers; with no camera the
    /// frame is the UI alone and keeps the same pass.
    fn ui_pass(&self) -> Result<u32, ViewError> {
        Ok(crate::scene::order::pass::UI)
    }
}

/// Reads the panel DC6 files a frame's UI draws name, once each, from the
/// user's archives (`client/assets.md` archive order is the source's).
pub struct PanelArtLoader {
    pub source: Arc<dyn FileSource>,
    pub files: UiFiles,
}

impl PanelArtLoader {
    /// Makes every image's frame set resident. A file no archive holds, or
    /// one that does not parse, is an error (M07): never skipped.
    pub fn ensure(&self, draws: &[UiDraw], assets: &mut ViewAssets) -> Result<(), ViewError> {
        for d in draws {
            let UiDraw::Image(req) = d else { continue };
            let set = image_set(&self.files, req.image)?;
            if assets.frames.contains(&set) {
                continue;
            }
            let name = self.files.name(req.image.file).unwrap_or_default();
            let archive = archive_name(name);
            let fail = |message: String| ViewError::Unresolved {
                what: "UI image file",
                spec: SPEC,
                message: format!("{archive}: {message}"),
            };
            let bytes = self
                .source
                .read_file(&archive)
                .ok_or_else(|| fail("in no archive".into()))?
                .map_err(fail)?;
            let dc6 = d2_formats::dc6::Dc6::parse(&bytes).map_err(|e| fail(e.to_string()))?;
            let frames = FrameSet::from_dc6(&dc6, 0).map_err(|e| fail(e.to_string()))?;
            assets.frames.insert(set, frames)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::path::MemorySource;
    use crate::ui::panels::PanelTables;
    use crate::ui::{Point, FRAME};
    use d2_formats::palette::{Palette, Rgb};

    /// A DC6 of one direction with `frames` frames of `w` × `h` literal
    /// pixels (`formats/dc6.md`).
    pub(crate) fn dc6(frames: u32, w: u32, h: u32) -> Vec<u8> {
        let mut rows = Vec::new();
        for _ in 0..h {
            rows.push(w as u8);
            rows.extend((0..w).map(|i| 1 + i as u8));
            rows.push(0x80);
        }
        let mut d = Vec::new();
        for v in [6i32, 1, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&[0xEE; 4]);
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&frames.to_le_bytes());
        let mut at = d.len() + 4 * frames as usize;
        let mut body = Vec::new();
        for _ in 0..frames {
            d.extend_from_slice(&(at as u32).to_le_bytes());
            for v in [0u32, w, h, 0, 0, 0, 0, rows.len() as u32] {
                body.extend_from_slice(&v.to_le_bytes());
            }
            body.extend_from_slice(&rows);
            body.extend_from_slice(&[0xEE; 3]);
            at += 32 + rows.len() + 3;
        }
        d.extend(body);
        d
    }

    fn assets() -> ViewAssets {
        ViewAssets::new(Palette {
            colors: [Rgb::default(); 256],
        })
    }

    fn image(file: u32, frame: u32, x: i32, y: i32) -> ImageRequest {
        ImageRequest {
            image: ImageRef { file, frame },
            at: Point::new(x, y),
            clip: FRAME,
        }
    }

    // Covers: specs/ui/panels.md §1 r3, §1 r4, §7 r1
    #[test]
    fn panel_cels_load_once_and_draw_bottom_anchored() {
        let files = PanelTables::load().unwrap().files;
        let id = files.id("panel\\buysellbtn").unwrap();
        let mut src = MemorySource::default();
        src.insert("data\\global\\ui\\panel\\buysellbtn.dc6", dc6(12, 4, 3));
        let loader = PanelArtLoader {
            source: Arc::new(src),
            files: files.clone(),
        };
        let mut a = assets();
        let draws = [
            UiDraw::Image(image(id, 10, 418, 476)),
            UiDraw::Image(image(id, 11, 418, 476)),
        ];
        loader.ensure(&draws, &mut a).unwrap();
        assert_eq!(a.frames.len(), 12, "one file, read once");
        loader.ensure(&draws, &mut a).unwrap();
        assert_eq!(a.frames.len(), 12);
        // The cel draw at (418, 476) covers rows 476 − h + 1 … 476.
        let s = panel_sprite(&files, &image(id, 10, 418, 476), &a).unwrap();
        assert_eq!((s.x, s.y, s.frame.index), (418, 474, 10));
        assert_eq!(s.frame.set.path(), "data/global/ui/panel/buysellbtn.dc6");
        assert_eq!((s.shade, s.blend), (ShadeChain::EMPTY, BlendOp::Opaque));
        // Past the file's frames: an error, not a skip.
        assert!(panel_sprite(&files, &image(id, 12, 0, 0), &a).is_err());
    }

    // Covers: specs/ui/panels.md §7 r1
    #[test]
    fn missing_or_unknown_files_are_errors() {
        let files = PanelTables::load().unwrap().files;
        let id = files.id("panel\\invchar6").unwrap();
        let loader = PanelArtLoader {
            source: Arc::new(MemorySource::default()),
            files: files.clone(),
        };
        let mut a = assets();
        let missing = [UiDraw::Image(image(id, 0, 0, 0))];
        assert!(loader.ensure(&missing, &mut a).is_err());
        let unknown = [UiDraw::Image(image(u32::MAX, 0, 0, 0))];
        assert!(loader.ensure(&unknown, &mut a).is_err());
        assert!(a.frames.is_empty());
    }
}

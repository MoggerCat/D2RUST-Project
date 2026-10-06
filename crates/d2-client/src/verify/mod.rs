// Spec: specs/client/render-pipeline.md (A10)
//! The verify harness: a runner over case files
//! (`crates/d2-client/render-cases/*.toml`, [`case`]). Per case: build the
//! draw list, CPU compose (the reference), GPU compose, compare byte for
//! byte; report the mismatched pixel count and the first mismatch.
//! `--perturb N` corrupts N reference pixels so the case must fail with
//! exactly N (M08).
//!
//! Kinds: `synthetic` (inline frames, repo only; the CPU half runs in
//! `cargo test`) and `map` (today's `map-preview.md` verify, [`map`],
//! game files and a GPU).
//!
//! The GPU half of `synthetic` cases goes through the narrow
//! [`GpuCompositor`] trait; [`gpu::Wgpu`] runs the compute compositor (C5,
//! [`crate::gpu_compositor`]) on a headless adapter. Without an adapter it
//! reports [`Status::NoAdapter`]; [`NotWired`] reports
//! [`Status::GpuNotWired`]. Both are explicit statuses, never a pass.
//!
//! A synthetic case may draw COF composites (`[[unit]]`, §A7): the COF
//! bytes go through `d2_formats::cof` and [`crate::composite::build`], so
//! the case covers COF bytes → composite → scene → CPU and GPU.

pub mod case;
pub mod gpu;
pub mod map;

#[cfg(test)]
mod tests;

use std::fmt;
use std::path::{Path, PathBuf};

use d2_formats::palette::{Palette, Rgb};

use crate::composite::{
    self, ComponentFrame, ComponentRequest, ComponentResolver, CompositeError, UnitParams,
};
use crate::frames::{FramePart, FrameSet, FrameSetKey, FrameStore, IndexFrame};
use crate::scene::{
    self, Bins, BlendOp, DrawItem, DrawKey, FrameId, FrameImage, ItemTag, MapId, MapTable, Rect,
    SceneError, ShadeChain,
};
pub use case::{Case, CaseError, CaseKind};

/// Default case directory, next to this crate's `Cargo.toml`.
pub fn default_case_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("render-cases")
}

/// Everything the GPU compositor needs for one frame, already ordered and
/// binned on the CPU (§A9 stage 1). `items[i]` is the item that bin lists
/// refer to by index `i`; `maps.rows()` is the storage buffer to upload.
pub struct GpuJob<'a> {
    pub case: &'a str,
    pub items: &'a [DrawItem],
    pub bins: &'a Bins,
    pub frames: &'a [FrameImage],
    pub maps: &'a MapTable,
    pub palette: &'a Palette,
    pub view: Rect,
}

/// What the GPU half returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuOutcome {
    /// The images read back before presentation (§A9), both
    /// `view.width × view.height`, row-major: `indices` one byte per pixel
    /// (the index framebuffer), `rgba` RGBA8 with alpha 255.
    Image { indices: Vec<u8>, rgba: Vec<u8> },
    /// No GPU compositor is wired into this build.
    NotWired,
    /// A compositor is wired but the machine has no usable adapter.
    NoAdapter(String),
    /// The GPU half ran and failed (device, pipeline, readback).
    Error(String),
}

/// The seam to the GPU compute compositor (C5). Implementations run the
/// job headless and offscreen, apply the stable-capture rule of Phase 1b
/// (`map-preview.md`: judge a capture only once it repeats), and return
/// the image; the harness compares it.
pub trait GpuCompositor {
    fn compose(&mut self, job: &GpuJob<'_>) -> GpuOutcome;
}

/// The GPU half until C5 wires its compositor.
pub struct NotWired;

impl GpuCompositor for NotWired {
    fn compose(&mut self, _job: &GpuJob<'_>) -> GpuOutcome {
        GpuOutcome::NotWired
    }
}

/// The synthetic palette: `rgb(i, 255 - i, i * 37 mod 256)`. Injective in
/// red, so every index difference shows in the RGBA image.
pub fn synthetic_palette() -> Palette {
    let mut colors = [Rgb { r: 0, g: 0, b: 0 }; 256];
    for (i, c) in colors.iter_mut().enumerate() {
        let i = i as u8;
        *c = Rgb {
            r: i,
            g: 255 - i,
            b: i.wrapping_mul(37),
        };
    }
    Palette { colors }
}

/// A synthetic case turned into compositor inputs.
pub struct Built {
    pub items: Vec<DrawItem>,
    pub frames: Vec<FrameImage>,
    pub maps: MapTable,
    pub palette: Palette,
    pub view: Rect,
}

/// Errors building or composing a case. Strict: every one fails the case.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    #[error("item[{item}]: {what} {index} is not defined ({count} defined)")]
    Undefined {
        item: usize,
        what: &'static str,
        index: u32,
        count: usize,
    },
    #[error("item[{item}]: {error}")]
    Scene { item: usize, error: SceneError },
    #[error(transparent)]
    Compose(#[from] SceneError),
    #[error(
        "unit[{unit}].component[{component}]: {what} {index} is not defined ({count} defined)"
    )]
    UnitUndefined {
        unit: usize,
        component: usize,
        what: &'static str,
        index: u32,
        count: usize,
    },
    #[error("unit[{unit}].component[{component}]: {error}")]
    UnitScene {
        unit: usize,
        component: usize,
        error: SceneError,
    },
    #[error("unit[{unit}]: COF: {message}")]
    Cof { unit: usize, message: String },
    #[error("unit[{unit}]: {error}")]
    Composite { unit: usize, error: CompositeError },
}

/// The case's fixture answers for one unit, resolved to scene values: per
/// COF component, the frame drawn, the top-left, the shade chain and the
/// blend. A component the COF draws without an answer is an error
/// (`Unresolved`), never a default. The frame is frame `n` of the case's
/// one frame set ([`case_frames_key`]); its scene id comes from the frame
/// store ([`composite::build_with`]), not from the fixture.
struct UnitFixture {
    /// Indexed by component id.
    components: [Option<Answer>; 16],
}

/// One component's answer: case frame index, top-left x and y, shade
/// chain, blend.
type Answer = (u32, i32, i32, ShadeChain, BlendOp);

/// The frame set holding a synthetic case's `[[frame]]`s in file order, so
/// the frame store gives `[[frame]]` `n` the scene id `FrameId(n)`.
pub fn case_frames_key() -> FrameSetKey {
    FrameSetKey::new("synthetic/case-frames.dc6", FramePart::Dir(0)).expect("canonical path")
}

impl UnitFixture {
    fn get(
        &self,
        req: &ComponentRequest<'_>,
        what: &'static str,
    ) -> Result<&Answer, CompositeError> {
        self.components
            .get(usize::from(req.slot.component))
            .and_then(Option::as_ref)
            .ok_or_else(|| CompositeError::Unresolved {
                slot: req.slot.slot,
                component: req.slot.component,
                what,
                message: "the case gives no [[unit.component]] for it".into(),
            })
    }
}

impl ComponentResolver for UnitFixture {
    fn frame(&self, req: &ComponentRequest<'_>) -> Result<ComponentFrame, CompositeError> {
        Ok(ComponentFrame {
            set: case_frames_key(),
            index: self.get(req, "frame")?.0 as usize,
        })
    }

    fn place(
        &self,
        req: &ComponentRequest<'_>,
        _: &ComponentFrame,
    ) -> Result<(i32, i32), CompositeError> {
        let c = self.get(req, "place")?;
        Ok((c.1, c.2))
    }

    fn shade(&self, req: &ComponentRequest<'_>) -> Result<ShadeChain, CompositeError> {
        Ok(self.get(req, "shade")?.3)
    }

    fn blend(&self, req: &ComponentRequest<'_>) -> Result<BlendOp, CompositeError> {
        Ok(self.get(req, "blend")?.4)
    }
}

/// Builds the ordered draw list of a synthetic case (`scene::order`
/// applied, as in play).
pub fn build(s: &case::Synthetic) -> Result<Built, BuildError> {
    let frames: Vec<FrameImage> = s
        .frames
        .iter()
        .map(|f| FrameImage {
            width: f.width,
            height: f.height,
            pixels: f.pixels.clone(),
        })
        .collect();
    let mut maps = MapTable::new();
    let map_ids: Vec<MapId> = s.maps.iter().map(|m| maps.push(m.row)).collect();
    let table_ids: Vec<MapId> = s
        .tables
        .iter()
        .map(|rule| {
            let mut table = Box::new([[0u8; 256]; 256]);
            for (src, row) in table.iter_mut().enumerate() {
                for (dest, v) in row.iter_mut().enumerate() {
                    *v = rule.value(src as u8, dest as u8);
                }
            }
            maps.push_table(&table)
        })
        .collect();
    let mut items = Vec::with_capacity(s.items.len());
    for (i, spec) in s.items.iter().enumerate() {
        let lookup = |what, ids: &[MapId], index: u32| {
            ids.get(index as usize)
                .copied()
                .ok_or(BuildError::Undefined {
                    item: i,
                    what,
                    index,
                    count: ids.len(),
                })
        };
        // Frames are checked here too so the error names the case's item;
        // the compositor would report the same as `FrameMissing`.
        if spec.frame as usize >= frames.len() {
            return Err(BuildError::Undefined {
                item: i,
                what: "frame",
                index: spec.frame,
                count: frames.len(),
            });
        }
        let mut item = DrawItem::new(FrameId(spec.frame), spec.x, spec.y);
        if let Some((x, y, w, h)) = spec.clip {
            item.clip = Rect::new(x, y, w, h);
        }
        let chain: Vec<MapId> = spec
            .shade
            .iter()
            .map(|&m| lookup("map", &map_ids, m))
            .collect::<Result<_, _>>()?;
        item.shade =
            ShadeChain::new(&chain).map_err(|error| BuildError::Scene { item: i, error })?;
        if let Some(t) = spec.table {
            item.blend = BlendOp::IndexTable(lookup("table", &table_ids, t)?);
        }
        if let Some([pass, major, minor, sub]) = spec.key {
            let sub = u8::try_from(sub).map_err(|_| BuildError::Scene {
                item: i,
                error: SceneError::KeyField {
                    field: "sub",
                    value: sub,
                    max: u8::MAX.into(),
                },
            })?;
            item.key = DrawKey::new(pass, major, minor, sub)
                .map_err(|error| BuildError::Scene { item: i, error })?;
        }
        item.flip_x = spec.flip_x;
        items.push(item);
    }
    let mut store = FrameStore::new();
    store
        .insert(
            case_frames_key(),
            FrameSet {
                frames: frames
                    .iter()
                    .map(|f| {
                        // Same check as the compositor's frame source.
                        scene::FrameView::new(f.width, f.height, &f.pixels)?;
                        Ok(IndexFrame::new(f.width, f.height, 0, 0, f.pixels.clone())
                            .expect("pixel count checked"))
                    })
                    .collect::<Result<_, SceneError>>()?,
            },
        )
        .expect("a new store has no sets");
    for (u, spec) in s.units.iter().enumerate() {
        let mut fixture = UnitFixture {
            components: Default::default(),
        };
        for (c, cs) in spec.components.iter().enumerate() {
            let undefined = |what, index: u32, count| BuildError::UnitUndefined {
                unit: u,
                component: c,
                what,
                index,
                count,
            };
            if cs.frame as usize >= frames.len() {
                return Err(undefined("frame", cs.frame, frames.len()));
            }
            let lookup = |what, ids: &[MapId], index: u32| {
                ids.get(index as usize)
                    .copied()
                    .ok_or_else(|| undefined(what, index, ids.len()))
            };
            let chain: Vec<MapId> = cs
                .shade
                .iter()
                .map(|&m| lookup("map", &map_ids, m))
                .collect::<Result<_, _>>()?;
            let shade = ShadeChain::new(&chain).map_err(|error| BuildError::UnitScene {
                unit: u,
                component: c,
                error,
            })?;
            let blend = match cs.table {
                Some(t) => BlendOp::IndexTable(lookup("table", &table_ids, t)?),
                None => BlendOp::Opaque,
            };
            fixture.components[usize::from(cs.component)] =
                Some((cs.frame, cs.x, cs.y, shade, blend));
        }
        let cof = d2_formats::cof::Cof::parse(&spec.cof).map_err(|e| BuildError::Cof {
            unit: u,
            message: e.to_string(),
        })?;
        let [pass, major, minor] = spec.key;
        let unit = UnitParams {
            pass,
            major,
            minor,
            clip: spec
                .clip
                .map_or(Rect::FRAME, |(x, y, w, h)| Rect::new(x, y, w, h)),
            tag: ItemTag::Unit(u as u32),
        };
        let draws = composite::build_with(
            &cof,
            spec.dir as usize,
            spec.frame as usize,
            &unit,
            &fixture,
            &store,
        )
        .map_err(|error| BuildError::Composite { unit: u, error })?;
        items.extend(draws.into_iter().map(|d| d.item));
    }
    scene::order(&mut items);
    let view = s
        .view
        .map_or(Rect::FRAME, |(x, y, w, h)| Rect::new(x, y, w, h));
    Ok(Built {
        items,
        frames,
        maps,
        palette: synthetic_palette(),
        view,
    })
}

/// The first differing pixel of a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstMismatch {
    pub x: i64,
    pub y: i64,
    pub expected: [u8; 4],
    pub actual: [u8; 4],
}

/// Byte-for-byte comparison of two RGBA8 images of one view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mismatch {
    pub pixels: usize,
    pub mismatched: usize,
    /// In screen coordinates (view origin added).
    pub first: Option<FirstMismatch>,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} of {} pixels differ", self.mismatched, self.pixels)?;
        if let Some(m) = self.first {
            write!(
                f,
                ", first at ({}, {}): expected {:?}, got {:?}",
                m.x, m.y, m.expected, m.actual
            )?;
        }
        Ok(())
    }
}

/// Compares `actual` with `expected` (both RGBA8 of `view`). A size
/// difference is an error, not a count.
pub fn compare(actual: &[u8], expected: &[u8], view: Rect) -> Result<Mismatch, String> {
    let len = view.width as usize * view.height as usize * 4;
    if actual.len() != len || expected.len() != len {
        return Err(format!(
            "image sizes {} and {} bytes, view {}x{} needs {len}",
            actual.len(),
            expected.len(),
            view.width,
            view.height
        ));
    }
    let mut m = Mismatch {
        pixels: len / 4,
        mismatched: 0,
        first: None,
    };
    let a = actual.as_chunks::<4>().0;
    let e = expected.as_chunks::<4>().0;
    for (i, (a, e)) in a.iter().zip(e).enumerate() {
        if a != e {
            m.mismatched += 1;
            m.first.get_or_insert(FirstMismatch {
                x: i64::from(view.x) + (i % view.width as usize) as i64,
                y: i64::from(view.y) + (i / view.width as usize) as i64,
                expected: *e,
                actual: *a,
            });
        }
    }
    Ok(m)
}

/// Corrupts `n` pixels of an RGBA8 image, spread evenly from pixel 0 (flips
/// the red channel's top bit). The rule of today's map verify; `n` above
/// the pixel count is an error, so the corrupted count is always exactly
/// `n`.
pub fn perturb(rgba: &mut [u8], n: usize) -> Result<(), String> {
    perturb_bytes(rgba, 4, n)
}

/// [`perturb`] on an index framebuffer: the same pixels, index top bit
/// flipped. Under [`synthetic_palette`] (red = index) the RGBA image of
/// the result differs from the clean one in exactly those `n` pixels.
pub fn perturb_indices(indices: &mut [u8], n: usize) -> Result<(), String> {
    perturb_bytes(indices, 1, n)
}

fn perturb_bytes(image: &mut [u8], bpp: usize, n: usize) -> Result<(), String> {
    let pixels = image.len() / bpp;
    if n > pixels {
        return Err(format!("--perturb {n} exceeds the {pixels} pixels"));
    }
    if let Some(step) = pixels.checked_div(n) {
        for i in (0..pixels).step_by(step.max(1)).take(n) {
            image[i * bpp] ^= 0x80;
        }
    }
    Ok(())
}

/// Byte-for-byte comparison of two index framebuffers of one view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteMismatch {
    pub bytes: usize,
    pub mismatched: usize,
    /// `(x, y, expected, actual)` in screen coordinates.
    pub first: Option<(i64, i64, u8, u8)>,
}

impl fmt::Display for ByteMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} of {} bytes differ", self.mismatched, self.bytes)?;
        if let Some((x, y, e, a)) = self.first {
            write!(f, ", first at ({x}, {y}): expected {e}, got {a}")?;
        }
        Ok(())
    }
}

/// Compares `actual` with `expected` (both index framebuffers of `view`).
/// A size difference is an error, not a count.
pub fn compare_indices(actual: &[u8], expected: &[u8], view: Rect) -> Result<ByteMismatch, String> {
    let len = view.width as usize * view.height as usize;
    if actual.len() != len || expected.len() != len {
        return Err(format!(
            "index buffers {} and {} bytes, view {}x{} needs {len}",
            actual.len(),
            expected.len(),
            view.width,
            view.height
        ));
    }
    let mut m = ByteMismatch {
        bytes: len,
        mismatched: 0,
        first: None,
    };
    for (i, (&a, &e)) in actual.iter().zip(expected).enumerate() {
        if a != e {
            m.mismatched += 1;
            m.first.get_or_insert((
                i64::from(view.x) + (i % view.width as usize) as i64,
                i64::from(view.y) + (i / view.width as usize) as i64,
                e,
                a,
            ));
        }
    }
    Ok(m)
}

/// The verdict of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// CPU and GPU halves both passed.
    Pass,
    /// The CPU half passed; no GPU compositor is wired. Not a pass.
    GpuNotWired,
    /// The CPU half passed; the GPU half found no adapter. Not a pass.
    NoAdapter(String),
    /// A comparison or expectation failed.
    Fail(String),
    /// The case could not be built or run.
    Error(String),
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::GpuNotWired => "GPU NOT WIRED",
            Status::NoAdapter(_) => "NO ADAPTER",
            Status::Fail(_) => "FAIL",
            Status::Error(_) => "ERROR",
        }
    }
}

/// The CPU half of a synthetic case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuReport {
    pub items: usize,
    pub view: Rect,
    /// `[[expect]]` rows checked on the index framebuffer.
    pub expects: usize,
    /// The binned compose (the GPU's per-pixel model, §A9) against the
    /// (possibly perturbed) reference.
    pub binned: Mismatch,
    /// Failed `[[expect]]` rows.
    pub expect_failures: Vec<String>,
}

/// The result of running one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseReport {
    pub name: String,
    pub kind: &'static str,
    pub lines: Vec<String>,
    pub status: Status,
}

/// The CPU reference of a case after `--perturb`: what both halves are
/// compared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// Index framebuffer, `view.width × view.height` bytes.
    pub indices: Vec<u8>,
    /// `indices` through the case palette.
    pub rgba: Vec<u8>,
}

/// Runs the CPU half of a synthetic case: compose (reference), check the
/// expectations, perturb the reference by `perturb_n` ([`perturb_indices`]),
/// compose binned and compare with it. Returns the report and the
/// reference.
pub fn run_cpu(
    s: &case::Synthetic,
    perturb_n: usize,
) -> Result<(Built, Bins, CpuReport, Reference), String> {
    let built = build(s).map_err(|e| e.to_string())?;
    let indexed = scene::compose(&built.items, &built.frames, &built.maps, built.view)
        .map_err(|e| e.to_string())?;
    let mut expect_failures = Vec::new();
    for e in &s.expects {
        let v = built.view;
        if !v.contains(e.x.into(), e.y.into()) {
            expect_failures.push(format!("expect ({}, {}) is outside the view", e.x, e.y));
            continue;
        }
        let at = (i64::from(e.y) - i64::from(v.y)) as usize * v.width as usize
            + (i64::from(e.x) - i64::from(v.x)) as usize;
        if indexed[at] != e.index {
            expect_failures.push(format!(
                "expect ({}, {}) = {}, got {}",
                e.x, e.y, e.index, indexed[at]
            ));
        }
    }
    let mut indexed = indexed;
    perturb_indices(&mut indexed, perturb_n)?;
    let reference = Reference {
        rgba: scene::to_rgba(&indexed, &built.palette),
        indices: indexed,
    };
    let bins = scene::bin(&built.items, &built.frames, &built.maps, built.view)
        .map_err(|e| e.to_string())?;
    let binned = scene::compose_binned(&built.items, &bins, &built.frames, &built.maps, built.view)
        .map_err(|e| e.to_string())?;
    let binned = compare(
        &scene::to_rgba(&binned, &built.palette),
        &reference.rgba,
        built.view,
    )?;
    let report = CpuReport {
        items: built.items.len(),
        view: built.view,
        expects: s.expects.len(),
        binned,
        expect_failures,
    };
    Ok((built, bins, report, reference))
}

/// Runs a synthetic case: CPU half, then the GPU half through `gpu`. The
/// GPU half runs even when the CPU half failed, so `--perturb N` shows N
/// on both (M08); a failed half fails the case.
pub fn run_synthetic(
    name: &str,
    s: &case::Synthetic,
    perturb_n: usize,
    gpu: &mut dyn GpuCompositor,
) -> CaseReport {
    let mut report = CaseReport {
        name: name.to_owned(),
        kind: "synthetic",
        lines: Vec::new(),
        status: Status::Pass,
    };
    let (built, bins, cpu, reference) = match run_cpu(s, perturb_n) {
        Ok(r) => r,
        Err(e) => {
            report.status = Status::Error(e);
            return report;
        }
    };
    let v = cpu.view;
    report.lines.push(format!(
        "{} items, view {}x{} at {},{}, {} expectations",
        cpu.items, v.width, v.height, v.x, v.y, cpu.expects
    ));
    if perturb_n > 0 {
        report.lines.push(format!(
            "debug: corrupted {perturb_n} reference pixels; verify must FAIL"
        ));
    }
    for f in &cpu.expect_failures {
        report.lines.push(format!("CPU: {f}"));
    }
    report.lines.push(format!("CPU binned: {}", cpu.binned));
    let cpu_failed = !cpu.expect_failures.is_empty() || cpu.binned.mismatched > 0;
    let job = GpuJob {
        case: name,
        items: &built.items,
        bins: &bins,
        frames: &built.frames,
        maps: &built.maps,
        palette: &built.palette,
        view: built.view,
    };
    let gpu_status = match gpu.compose(&job) {
        GpuOutcome::NotWired => Status::GpuNotWired,
        GpuOutcome::NoAdapter(e) => {
            report.lines.push(format!("GPU: no adapter: {e}"));
            Status::NoAdapter(e)
        }
        GpuOutcome::Error(e) => Status::Error(format!("GPU: {e}")),
        GpuOutcome::Image { indices, rgba } => {
            let compared = compare_indices(&indices, &reference.indices, built.view)
                .and_then(|i| Ok((i, compare(&rgba, &reference.rgba, built.view)?)));
            match compared {
                Err(e) => Status::Error(format!("GPU: {e}")),
                Ok((i, m)) => {
                    report.lines.push(format!("GPU indices: {i}"));
                    report.lines.push(format!("GPU: {m}"));
                    if i.mismatched == 0 && m.mismatched == 0 {
                        Status::Pass
                    } else {
                        Status::Fail("GPU half".into())
                    }
                }
            }
        }
    };
    report.status = match (cpu_failed, gpu_status) {
        (_, Status::Error(e)) => Status::Error(e),
        (true, Status::Fail(_)) => Status::Fail("CPU and GPU halves".into()),
        (true, _) => Status::Fail("CPU half".into()),
        (false, s) => s,
    };
    report
}

/// Reads every `*.toml` in `dir`, sorted by file name. Any unreadable or
/// invalid file is an error (no case is silently skipped).
pub fn load_dir(dir: &Path) -> Result<Vec<Case>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|x| x == "toml") {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(format!("no *.toml case files in {}", dir.display()));
    }
    paths.iter().map(|p| load_file(p)).collect()
}

/// Reads one case file.
pub fn load_file(path: &Path) -> Result<Case, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("{}: file name is not UTF-8", path.display()))?;
    case::parse(name, &text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Totals of a run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub pass: usize,
    pub not_wired: usize,
    pub no_adapter: usize,
    pub fail: usize,
    pub error: usize,
}

impl Summary {
    pub fn add(&mut self, status: &Status) {
        match status {
            Status::Pass => self.pass += 1,
            Status::GpuNotWired => self.not_wired += 1,
            Status::NoAdapter(_) => self.no_adapter += 1,
            Status::Fail(_) => self.fail += 1,
            Status::Error(_) => self.error += 1,
        }
    }

    /// Process exit code: 0 all passed, 1 any failure or error, 2 none
    /// failed but some GPU halves are not wired or found no adapter
    /// (incomplete, not a pass).
    pub fn exit_code(&self) -> i32 {
        if self.fail + self.error > 0 {
            1
        } else if self.not_wired + self.no_adapter > 0 {
            2
        } else {
            0
        }
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} pass, {} fail, {} error, {} GPU not wired, {} no adapter",
            self.pass, self.fail, self.error, self.not_wired, self.no_adapter
        )
    }
}

/// Prints a case report the way the runner shows it.
pub fn print_report(r: &CaseReport) {
    for line in &r.lines {
        println!("  {line}");
    }
    match &r.status {
        Status::Fail(why) | Status::Error(why) | Status::NoAdapter(why) => {
            println!("{} {} ({}): {why}", r.status.label(), r.name, r.kind)
        }
        s => println!("{} {} ({})", s.label(), r.name, r.kind),
    }
}

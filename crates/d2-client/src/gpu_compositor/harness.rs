// Spec: specs/client/render-pipeline.md (A9, A10, Test vectors)
//! CPU = GPU comparison on synthetic cases (repo only, no game data): the
//! spec's test vectors, an off-center view, two atlas pages and a stress
//! list. C6's verify runner can call [`compare`] with its own cases; the
//! `gpu_compare` example and an ignored test run [`cases`].
//!
//! `--perturb N` (M08): N bytes of the CPU reference are changed before the
//! diff ([`perturb`]); the report must then show exactly N differing bytes.

use d2_formats::palette::{Palette, Rgb};

use super::pack::{pack, AtlasFrames};
use super::{Gpu, GpuError};
use crate::scene::{
    self, order, BlendOp, DrawItem, DrawKey, FrameId, FrameImage, MapId, MapTable, Rect, ShadeChain,
};

/// Atlas pages a case may use.
pub const MAX_PAGES: u32 = 4;

/// One synthetic comparison case: everything the compositor reads.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: &'static str,
    pub frames: Vec<FrameImage>,
    pub maps: MapTable,
    /// In draw order ([`scene::order`] already applied).
    pub items: Vec<DrawItem>,
    pub view: Rect,
    pub palette: Palette,
}

/// Byte differences between two equal-length buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Diff {
    /// Differing bytes (index buffer) or differing pixels (RGBA).
    pub differing: u64,
    /// First differing pixel `(x, y)` in the view, with both values.
    pub first: Option<(u32, u32, u32, u32)>,
}

/// The result of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub name: String,
    pub pixels: usize,
    pub items: usize,
    pub pages: u32,
    /// Bytes of the CPU reference changed on purpose.
    pub perturbed: usize,
    /// Index framebuffer, per byte.
    pub indices: Diff,
    /// RGBA image, per pixel.
    pub rgba: Diff,
}

impl Report {
    /// Both images agree byte for byte.
    pub fn is_match(&self) -> bool {
        self.indices.differing == 0 && self.rgba.differing == 0
    }
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "case {}: {} pixels, {} items, {} pages: {} differing bytes (indices), {} differing pixels (rgba)",
            self.name, self.pixels, self.items, self.pages, self.indices.differing, self.rgba.differing
        )?;
        if self.perturbed > 0 {
            write!(f, ", {} perturbed", self.perturbed)?;
        }
        if let Some((x, y, cpu, gpu)) = self.indices.first {
            write!(f, "; first at ({x},{y}): cpu {cpu} gpu {gpu}")?;
        }
        Ok(())
    }
}

/// Runs `case` on the CPU reference and on `gpu` and diffs both images,
/// after changing `perturb` bytes of the CPU reference.
pub fn compare(gpu: &Gpu, case: &Case, perturb_count: usize) -> Result<Report, GpuError> {
    let mut cpu = scene::compose(&case.items, &case.frames, &case.maps, case.view)?;
    let perturbed = perturb(&mut cpu, perturb_count);
    let cpu_rgba = scene::to_rgba(&cpu, &case.palette);
    let (atlas, packed) = prepare(case)?;
    let (indices, rgba) = gpu.compose_rgba(&packed, atlas.pages(), &case.palette)?;
    Ok(Report {
        name: case.name.to_string(),
        pixels: cpu.len(),
        items: case.items.len(),
        pages: atlas.page_count(),
        perturbed,
        indices: diff(&cpu, &indices, case.view.width, 1),
        rgba: diff(&cpu_rgba, &rgba, case.view.width, 4),
    })
}

/// The atlas and packed buffers of `case` (everything up to dispatch).
pub fn prepare(case: &Case) -> Result<(AtlasFrames, super::Packed), GpuError> {
    let atlas = AtlasFrames::from_images(&case.frames, MAX_PAGES)?;
    let bins = scene::bin(&case.items, &case.frames, &case.maps, case.view)?;
    let packed = pack(
        &case.items,
        &bins,
        &case.frames,
        &atlas.slots,
        &case.maps,
        atlas.page_count(),
    )?;
    Ok((atlas, packed))
}

/// Compares pixels of `bpp` bytes. A length mismatch counts every pixel of
/// the longer buffer past the shorter one as differing.
pub fn diff(cpu: &[u8], gpu: &[u8], width: u32, bpp: usize) -> Diff {
    let mut d = Diff::default();
    let pixels = cpu.len().max(gpu.len()).div_ceil(bpp);
    for p in 0..pixels {
        let a = cpu.get(p * bpp..(p + 1) * bpp);
        let b = gpu.get(p * bpp..(p + 1) * bpp);
        if a == b {
            continue;
        }
        d.differing += 1;
        if d.first.is_none() {
            let value = |s: Option<&[u8]>| {
                s.map_or(u32::MAX, |s| {
                    s.iter().fold(0u32, |v, &b| (v << 8) | u32::from(b))
                })
            };
            let width = width.max(1) as usize;
            d.first = Some(((p % width) as u32, (p / width) as u32, value(a), value(b)));
        }
    }
    d
}

/// Changes `n` bytes of `buf` (all of them if `n` is larger), evenly
/// spaced from byte 0, each XOR 0xFF so every one differs. Returns how many
/// bytes were changed.
pub fn perturb(buf: &mut [u8], n: usize) -> usize {
    let n = n.min(buf.len());
    if n == 0 {
        return 0;
    }
    let step = buf.len() / n;
    for i in 0..n {
        buf[i * step] ^= 0xFF;
    }
    n
}

/// A palette whose entries are all distinct, so an index change always
/// changes the RGBA pixel.
pub fn distinct_palette() -> Palette {
    let mut p = Palette {
        colors: [Rgb::default(); 256],
    };
    for (i, c) in p.colors.iter_mut().enumerate() {
        let i = i as u8;
        *c = Rgb {
            r: i,
            g: 255 - i,
            b: i ^ 0x5a,
        };
    }
    p
}

/// An identity map row with some entries replaced.
pub fn map_with(pairs: &[(u8, u8)]) -> [u8; 256] {
    let mut row = [0u8; 256];
    for (i, v) in row.iter_mut().enumerate() {
        *v = i as u8;
    }
    for &(from, to) in pairs {
        row[usize::from(from)] = to;
    }
    row
}

fn solid(width: u32, height: u32, index: u8) -> FrameImage {
    FrameImage {
        width,
        height,
        pixels: vec![index; (width * height) as usize],
    }
}

/// A synthetic 256×256 blend table: every entry depends on both indices.
fn blend_table(seed: u32) -> Box<[[u8; 256]; 256]> {
    let mut t = Box::new([[0u8; 256]; 256]);
    for (s, row) in t.iter_mut().enumerate() {
        for (d, v) in row.iter_mut().enumerate() {
            *v = ((s as u32 * 31 + d as u32 * 7 + seed * 13) % 256) as u8;
        }
    }
    t
}

fn case(
    name: &'static str,
    frames: Vec<FrameImage>,
    maps: MapTable,
    mut items: Vec<DrawItem>,
    view: Rect,
) -> Case {
    order(&mut items);
    Case {
        name,
        frames,
        maps,
        items,
        view,
        palette: distinct_palette(),
    }
}

fn key(major: u32) -> DrawKey {
    DrawKey::new(2, major, 0, 0).expect("small key")
}

/// Deterministic generator for the stress case (not game randomness).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// All synthetic cases, in a fixed order.
pub fn cases() -> Vec<Case> {
    const SMALL: Rect = Rect::new(0, 0, 32, 24);
    let spec_frame = || FrameImage {
        width: 2,
        height: 2,
        pixels: vec![0, 5, 7, 0],
    };
    let mut out = Vec::new();

    // §Test vectors 1–3: opaque frame, chain of one, chain order.
    out.push(case(
        "vector-opaque",
        vec![spec_frame()],
        MapTable::new(),
        vec![DrawItem::new(FrameId(0), 10, 10)],
        SMALL,
    ));
    let mut maps = MapTable::new();
    let m = maps.push(map_with(&[(5, 9)]));
    let mut item = DrawItem::new(FrameId(0), 10, 10);
    item.shade = ShadeChain::new(&[m]).expect("chain");
    out.push(case(
        "vector-chain1",
        vec![spec_frame()],
        maps,
        vec![item],
        SMALL,
    ));
    let mut maps = MapTable::new();
    let m1 = maps.push(map_with(&[(5, 9)]));
    let m2 = maps.push(map_with(&[(9, 3)]));
    let mut item = DrawItem::new(FrameId(0), 10, 10);
    item.shade = ShadeChain::new(&[m1, m2]).expect("chain");
    out.push(case(
        "vector-chain2",
        vec![spec_frame()],
        maps,
        vec![item],
        SMALL,
    ));

    // §Test vectors 4–5: key order, then equal keys in build order.
    let mut a = DrawItem::new(FrameId(0), 4, 4);
    a.key = key(2);
    let mut b = DrawItem::new(FrameId(1), 5, 5);
    b.key = key(1);
    out.push(case(
        "vector-key-order",
        vec![solid(3, 3, 1), solid(3, 3, 2)],
        MapTable::new(),
        vec![a, b],
        SMALL,
    ));
    out.push(case(
        "vector-equal-keys",
        vec![solid(3, 3, 1), solid(3, 3, 2)],
        MapTable::new(),
        vec![
            DrawItem::new(FrameId(1), 5, 5),
            DrawItem::new(FrameId(0), 4, 4),
        ],
        SMALL,
    ));

    // §Test vectors 6: IndexTable over an opaque background.
    let mut maps = MapTable::new();
    let base = maps.push_table(&blend_table(1));
    let mut top = DrawItem::new(FrameId(1), 6, 6);
    top.blend = BlendOp::IndexTable(base);
    out.push(case(
        "vector-index-table",
        vec![solid(20, 16, 40), spec_frame_grid()],
        maps,
        vec![DrawItem::new(FrameId(0), 2, 2), top],
        SMALL,
    ));

    // §Test vectors 7: clip.
    let mut item = DrawItem::new(FrameId(0), 3, 3);
    item.clip = Rect::new(5, 4, 6, 3);
    out.push(case(
        "vector-clip",
        vec![solid(12, 12, 77)],
        MapTable::new(),
        vec![item],
        SMALL,
    ));

    // §Test vectors 8: a frame spanning four bins, full frame (partial
    // last bin row: 600 = 18 × 32 + 24).
    out.push(case(
        "vector-four-bins",
        vec![spec_frame_grid()],
        MapTable::new(),
        vec![DrawItem::new(FrameId(0), 20, 20)],
        Rect::FRAME,
    ));

    // Empty list: the clear value everywhere.
    out.push(case(
        "empty",
        Vec::new(),
        MapTable::new(),
        Vec::new(),
        Rect::FRAME,
    ));

    // Two atlas pages: the second frame cannot share page 0.
    let big = |seed: u32| FrameImage {
        width: 1500,
        height: 1500,
        pixels: (0..1500u32 * 1500)
            .map(|i| ((i * 7 + seed + i / 1500) % 251) as u8)
            .collect(),
    };
    out.push(case(
        "two-pages",
        vec![big(1), big(2)],
        MapTable::new(),
        vec![
            DrawItem::new(FrameId(0), -700, -800),
            DrawItem::new(FrameId(1), 300, 250),
        ],
        Rect::FRAME,
    ));

    out.push(stress("offset-view", 7, 120, Rect::new(-37, 13, 333, 250)));
    out.push(stress("stress", 1, 400, Rect::FRAME));
    out
}

/// A 40×40 frame with transparent holes and every index 0..=255.
fn spec_frame_grid() -> FrameImage {
    FrameImage {
        width: 40,
        height: 40,
        pixels: (0..1600u32)
            .map(|i| if i % 11 == 3 { 0 } else { (i % 256) as u8 })
            .collect(),
    }
}

/// Random frames, chains, blends, clips and keys, some items off screen,
/// one empty frame.
fn stress(name: &'static str, seed: u64, count: u32, view: Rect) -> Case {
    let mut rng = Lcg(seed);
    let mut frames = vec![FrameImage {
        width: 0,
        height: 0,
        pixels: Vec::new(),
    }];
    for _ in 0..24 {
        let (w, h) = (1 + rng.below(120), 1 + rng.below(90));
        let pixels = (0..w * h)
            .map(|_| {
                if rng.below(10) < 3 {
                    0
                } else {
                    rng.below(256) as u8
                }
            })
            .collect();
        frames.push(FrameImage {
            width: w,
            height: h,
            pixels,
        });
    }
    let mut maps = MapTable::new();
    let mut shade_maps = Vec::new();
    for _ in 0..8 {
        let mut row = [0u8; 256];
        for v in row.iter_mut() {
            *v = rng.below(256) as u8;
        }
        shade_maps.push(maps.push(row));
    }
    let tables = [
        maps.push_table(&blend_table(2)),
        maps.push_table(&blend_table(3)),
    ];
    let mut items = Vec::new();
    for _ in 0..count {
        let frame = FrameId(rng.below(frames.len() as u32));
        let x = view.x - 150 + rng.below(view.width + 300) as i32;
        let y = view.y - 120 + rng.below(view.height + 240) as i32;
        let mut item = DrawItem::new(frame, x, y);
        let chain: Vec<MapId> = (0..rng.below(5))
            .map(|_| shade_maps[rng.below(8) as usize])
            .collect();
        item.shade = ShadeChain::new(&chain).expect("≤ 4 maps");
        if rng.below(4) == 0 {
            item.blend = BlendOp::IndexTable(tables[rng.below(2) as usize]);
        }
        if rng.below(3) == 0 {
            item.clip = Rect::new(
                view.x + rng.below(view.width) as i32 - 20,
                view.y + rng.below(view.height) as i32 - 20,
                rng.below(300),
                rng.below(200),
            );
        }
        item.key = DrawKey::new(rng.below(3), rng.below(40), rng.below(4), 0).expect("key");
        items.push(item);
    }
    case(name, frames, maps, items, view)
}

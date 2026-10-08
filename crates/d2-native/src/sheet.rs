// Spec: specs/formats/native-assets.md
//! Sprite sheets (§2.2): DC6 and DCC files as one indexed PNG plus a TOML
//! sidecar. Row `d` of the sheet is direction `d`; frames go left to right
//! at their own widths, packed with no gap, top edges on the row's top;
//! row height = the tallest frame; sheet width = the widest row; the rest
//! is index 0. No pixels (width or height 0) → no PNG, `sheet = false`.

use d2_formats::dc6::{Dc6, Dc6Frame, Dc6Header};
use d2_formats::dcc::{Dcc, DccDirection, DccFrame};

use crate::kind::{
    diff_fields, need, pixel_diff, Difference, FileStore, NativeError, NativeFile, NativeKind,
    ViewPalette,
};
use crate::native_toml::{array, hex2, hex8, Fields};
use crate::png;

const MAX_SHEET_PIXELS: u64 = 1 << 28;

/// One frame as the sheet sees it.
struct SheetFrame<'a> {
    w: u32,
    h: u32,
    pixels: &'a [u8],
}

/// The place of every frame in the sheet and the sheet's size.
#[derive(Debug, PartialEq, Eq)]
struct Layout {
    rects: Vec<Vec<[u32; 4]>>,
    width: u32,
    height: u32,
}

impl Layout {
    fn has_pixels(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// §2.2 r1: the layout of frames of the given `(w, h)` sizes per direction.
fn layout(sizes: &[Vec<(u32, u32)>]) -> Result<Layout, String> {
    let (mut width, mut y) = (0u64, 0u64);
    let mut rects = Vec::with_capacity(sizes.len());
    for dir in sizes {
        let (mut x, mut row_h) = (0u64, 0u64);
        let mut row = Vec::with_capacity(dir.len());
        for &(w, h) in dir {
            row.push([
                u32::try_from(x).map_err(|_| "sheet too wide")?,
                u32::try_from(y).map_err(|_| "sheet too tall")?,
                w,
                h,
            ]);
            x += u64::from(w);
            row_h = row_h.max(u64::from(h));
        }
        width = width.max(x);
        y += row_h;
        rects.push(row);
    }
    if width.saturating_mul(y) > MAX_SHEET_PIXELS {
        return Err(format!("sheet {width} x {y} too large"));
    }
    Ok(Layout {
        rects,
        width: u32::try_from(width).map_err(|_| "sheet too wide")?,
        height: u32::try_from(y).map_err(|_| "sheet too tall")?,
    })
}

/// Draws the frames into the sheet buffer.
fn render(lay: &Layout, dirs: &[Vec<SheetFrame<'_>>]) -> Vec<u8> {
    let sw = lay.width as usize;
    let mut sheet = vec![0u8; sw * lay.height as usize];
    for (rects, frames) in lay.rects.iter().zip(dirs) {
        for (r, f) in rects.iter().zip(frames) {
            let (x, y, w) = (r[0] as usize, r[1] as usize, f.w as usize);
            for row in 0..f.h as usize {
                let dst = (y + row) * sw + x;
                sheet[dst..dst + w].copy_from_slice(&f.pixels[row * w..(row + 1) * w]);
            }
        }
    }
    sheet
}

/// A frame's pixels cut out of the sheet.
fn extract(sheet: &[u8], sw: usize, r: [u32; 4]) -> Vec<u8> {
    let (x, y, w, h) = (r[0] as usize, r[1] as usize, r[2] as usize, r[3] as usize);
    let mut out = Vec::with_capacity(w * h);
    for row in 0..h {
        let at = (y + row) * sw + x;
        out.extend_from_slice(&sheet[at..at + w]);
    }
    out
}

fn check_frame_pixels(file: &str, what: &str, f: &SheetFrame<'_>) -> Result<(), NativeError> {
    if f.pixels.len() as u64 != u64::from(f.w) * u64::from(f.h) {
        return Err(NativeError::new(
            file,
            format!(
                "{what}: {} pixels for a {} x {} frame",
                f.pixels.len(),
                f.w,
                f.h
            ),
        ));
    }
    Ok(())
}

/// Lays out, checks and encodes. Returns the layout and the PNG (if any).
fn build_sheet(
    png_path: &str,
    dirs: &[Vec<SheetFrame<'_>>],
    view: &ViewPalette,
) -> Result<(Layout, Option<Vec<u8>>), NativeError> {
    for (d, dir) in dirs.iter().enumerate() {
        for (f, fr) in dir.iter().enumerate() {
            check_frame_pixels(png_path, &format!("direction {d} frame {f}"), fr)?;
        }
    }
    let sizes: Vec<Vec<(u32, u32)>> = dirs
        .iter()
        .map(|d| d.iter().map(|f| (f.w, f.h)).collect())
        .collect();
    let lay = layout(&sizes).map_err(|m| NativeError::new(png_path, m))?;
    if !lay.has_pixels() {
        return Ok((lay, None));
    }
    let bytes = png::write_indexed(png_path, lay.width, lay.height, &render(&lay, dirs), view)?;
    Ok((lay, Some(bytes)))
}

/// What a sidecar says about one frame, before its pixels are cut out.
struct FrameRect {
    rect: [u32; 4],
}

fn read_rect(f: &mut Fields) -> Result<FrameRect, NativeError> {
    let v = f.ints("rect")?;
    if v.len() != 4 {
        return Err(f.err("`rect` must have 4 entries"));
    }
    let mut rect = [0u32; 4];
    for (o, i) in rect.iter_mut().zip(&v) {
        *o = u32::try_from(*i).map_err(|_| f.err("`rect` entry out of range"))?;
    }
    Ok(FrameRect { rect })
}

/// Cuts every frame out of the sheet after checking the sidecar's rects
/// against the layout of the frame sizes and the PNG against the layout
/// (strict: a modder who resizes a frame must also fix the sidecar).
fn load_pixels(
    toml_path: &str,
    png_path: &str,
    sheet: bool,
    rects: &[Vec<FrameRect>],
    store: &dyn FileStore,
) -> Result<Vec<Vec<Vec<u8>>>, NativeError> {
    let sizes: Vec<Vec<(u32, u32)>> = rects
        .iter()
        .map(|d| d.iter().map(|f| (f.rect[2], f.rect[3])).collect())
        .collect();
    let lay = layout(&sizes).map_err(|m| NativeError::new(toml_path, m))?;
    for (d, (a, b)) in lay.rects.iter().zip(rects).enumerate() {
        for (f, (want, got)) in a.iter().zip(b).enumerate() {
            if *want != got.rect {
                return Err(NativeError::new(
                    toml_path,
                    format!(
                        "direction {d} frame {f}: rect {:?} does not match the sheet layout {:?}",
                        got.rect, want
                    ),
                ));
            }
        }
    }
    if sheet != lay.has_pixels() {
        return Err(NativeError::new(
            toml_path,
            format!(
                "sheet = {sheet} but the frames {} pixels",
                if lay.has_pixels() { "have" } else { "have no" }
            ),
        ));
    }
    let data = if sheet {
        let bytes = need(store, png_path)?;
        let (w, h, px) = png::read_indexed(png_path, &bytes)?;
        if (w, h) != (lay.width, lay.height) {
            return Err(NativeError::new(
                png_path,
                format!(
                    "sheet is {w} x {h}, the sidecar's frames need {} x {}",
                    lay.width, lay.height
                ),
            ));
        }
        px
    } else {
        Vec::new()
    };
    Ok(lay
        .rects
        .iter()
        .map(|d| {
            d.iter()
                .map(|r| extract(&data, lay.width as usize, *r))
                .collect()
        })
        .collect())
}

fn rect_text(r: [u32; 4]) -> String {
    array(r.iter().map(|v| v.to_string()))
}

fn sheet_png(p: &str) -> String {
    format!("{p}.png")
}
fn sheet_toml(p: &str) -> String {
    format!("{p}.toml")
}

fn files(p: &str, toml: String, png: Option<Vec<u8>>) -> Vec<NativeFile> {
    let mut v = vec![NativeFile {
        path: sheet_toml(p),
        bytes: toml.into_bytes(),
    }];
    if let Some(bytes) = png {
        v.push(NativeFile {
            path: sheet_png(p),
            bytes,
        });
    }
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

// ---- DC6 -----------------------------------------------------------------

impl NativeKind for Dc6 {
    const KIND: &'static str = "dc6";

    fn write(&self, p: &str, view: &ViewPalette) -> Result<Vec<NativeFile>, NativeError> {
        let (tp, pp) = (sheet_toml(p), sheet_png(p));
        let h = &self.header;
        let (d, f) = (h.directions as usize, h.frames_per_direction as usize);
        if self.frames.len() != d * f {
            return Err(NativeError::new(
                &tp,
                format!(
                    "{} frames for {d} directions x {f} frames",
                    self.frames.len()
                ),
            ));
        }
        let dirs: Vec<Vec<SheetFrame<'_>>> = (0..d)
            .map(|di| {
                self.frames[di * f..(di + 1) * f]
                    .iter()
                    .map(|fr| SheetFrame {
                        w: fr.width,
                        h: fr.height,
                        pixels: &fr.pixels,
                    })
                    .collect()
            })
            .collect();
        let (lay, png) = build_sheet(&pp, &dirs, view)?;
        let mut t = String::new();
        t += "native = \"dc6\"\nnative_version = 1\n";
        t += &format!("sheet = {}\n\n[header]\n", lay.has_pixels());
        t += &format!("version = {}\n", h.version);
        t += &format!("flags = {}\n", hex8(h.flags));
        t += &format!("encoding = {}\n", h.encoding);
        t += &format!(
            "termination = {}\n",
            hex8(u32::from_be_bytes(h.termination))
        );
        t += &format!("directions = {}\n", h.directions);
        t += &format!("frames_per_direction = {}\n", h.frames_per_direction);
        for (di, row) in lay.rects.iter().enumerate() {
            t += "\n[[direction]]\n";
            for (fi, r) in row.iter().enumerate() {
                let fr = &self.frames[di * f + fi];
                t += "\n[[direction.frame]]\n";
                t += &format!("rect = {}\n", rect_text(*r));
                t += &format!("flip = {}\n", fr.flip);
                t += &format!("width = {}\nheight = {}\n", fr.width, fr.height);
                t += &format!("offset_x = {}\noffset_y = {}\n", fr.offset_x, fr.offset_y);
                t += &format!("unknown = {}\n", fr.unknown);
                t += &format!(
                    "[direction.frame.encoding]\nnext_block = {}\n",
                    fr.next_block
                );
            }
        }
        Ok(files(p, t, png))
    }

    fn read(p: &str, store: &dyn FileStore) -> Result<Self, NativeError> {
        let (tp, pp) = (sheet_toml(p), sheet_png(p));
        let text =
            String::from_utf8(need(store, &tp)?).map_err(|_| NativeError::new(&tp, "not UTF-8"))?;
        let mut f = Fields::parse(&tp, &text)?;
        f.header("dc6", 1)?;
        let sheet = f.bool("sheet")?;
        let mut h = f.table("header")?;
        let version = h.i32("version")?;
        let flags = h.hex_u32("flags")?;
        let encoding = h.u32("encoding")?;
        let termination = h.hex_u32("termination")?.to_be_bytes();
        let directions = h.u32("directions")?;
        let frames_per_direction = h.u32("frames_per_direction")?;
        h.finish()?;
        let mut dirs = f.tables("direction")?;
        if dirs.len() != directions as usize {
            return Err(f.err(format!(
                "directions = {directions} but {} [[direction]] tables",
                dirs.len()
            )));
        }
        struct Raw {
            flip: u32,
            width: u32,
            height: u32,
            offset_x: i32,
            offset_y: i32,
            unknown: u32,
            next_block: u32,
        }
        let mut rects = Vec::new();
        let mut raws = Vec::new();
        for (di, d) in dirs.iter_mut().enumerate() {
            let mut frames = d.tables("frame")?;
            if frames.len() != frames_per_direction as usize {
                return Err(f.err(format!(
                    "direction {di}: {} frames, frames_per_direction = {frames_per_direction}",
                    frames.len()
                )));
            }
            let (mut rr, mut rw) = (Vec::new(), Vec::new());
            for (fi, fr) in frames.iter_mut().enumerate() {
                let rect = read_rect(fr)?;
                let raw = Raw {
                    flip: fr.u32("flip")?,
                    width: fr.u32("width")?,
                    height: fr.u32("height")?,
                    offset_x: fr.i32("offset_x")?,
                    offset_y: fr.i32("offset_y")?,
                    unknown: fr.u32("unknown")?,
                    next_block: match fr.opt_table("encoding")? {
                        Some(mut e) => {
                            let v = e.u32("next_block")?;
                            e.finish()?;
                            v
                        }
                        None => 0,
                    },
                };
                if (raw.width, raw.height) != (rect.rect[2], rect.rect[3]) {
                    return Err(f.err(format!(
                        "direction {di} frame {fi}: width / height differ from rect"
                    )));
                }
                fr.finish()?;
                rr.push(rect);
                rw.push(raw);
            }
            d.finish()?;
            rects.push(rr);
            raws.push(rw);
        }
        f.finish()?;
        let px = load_pixels(&tp, &pp, sheet, &rects, store)?;
        let frames = raws
            .into_iter()
            .zip(px)
            .flat_map(|(rw, pd)| rw.into_iter().zip(pd))
            .map(|(r, pixels)| Dc6Frame {
                flip: r.flip,
                width: r.width,
                height: r.height,
                offset_x: r.offset_x,
                offset_y: r.offset_y,
                unknown: r.unknown,
                next_block: r.next_block,
                pixels,
            })
            .collect();
        Ok(Dc6 {
            header: Dc6Header {
                version,
                flags,
                encoding,
                termination,
                directions,
                frames_per_direction,
            },
            frames,
        })
    }

    fn first_difference(&self, native: &Self, p: &str) -> Option<Difference> {
        let (tp, pp) = (sheet_toml(p), sheet_png(p));
        diff_fields!(
            tp,
            "header",
            self.header,
            native.header,
            [
                version,
                flags,
                encoding,
                termination,
                directions,
                frames_per_direction
            ]
        );
        if self.frames.len() != native.frames.len() {
            return Some(Difference {
                file: tp,
                detail: format!("{} frames != {}", self.frames.len(), native.frames.len()),
            });
        }
        let f = self.header.frames_per_direction.max(1) as usize;
        for (i, (a, b)) in self.frames.iter().zip(&native.frames).enumerate() {
            let at = format!("direction {} frame {}", i / f, i % f);
            diff_fields!(
                tp,
                at,
                a,
                b,
                [flip, width, height, offset_x, offset_y, unknown, next_block]
            );
            if let Some(d) = pixel_diff(&a.pixels, &b.pixels, a.width as usize) {
                return Some(Difference {
                    file: pp,
                    detail: format!("{at} {d}"),
                });
            }
        }
        None
    }
}

// ---- DCC -----------------------------------------------------------------

fn hex_bytes(b: &[u8]) -> String {
    let s: String = b.iter().map(|v| format!("{v:02x}")).collect();
    format!("\"{s}\"")
}

fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2)
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

impl NativeKind for Dcc {
    const KIND: &'static str = "dcc";

    fn write(&self, p: &str, view: &ViewPalette) -> Result<Vec<NativeFile>, NativeError> {
        let (tp, pp) = (sheet_toml(p), sheet_png(p));
        for (d, dir) in self.directions.iter().enumerate() {
            if dir.frames.len() != self.frames_per_direction as usize {
                return Err(NativeError::new(
                    &tp,
                    format!(
                        "direction {d}: {} frames, frames_per_direction = {}",
                        dir.frames.len(),
                        self.frames_per_direction
                    ),
                ));
            }
        }
        let dirs: Vec<Vec<SheetFrame<'_>>> = self
            .directions
            .iter()
            .map(|d| {
                d.frames
                    .iter()
                    .map(|fr| SheetFrame {
                        w: fr.width,
                        h: fr.height,
                        pixels: &fr.pixels,
                    })
                    .collect()
            })
            .collect();
        let (lay, png) = build_sheet(&pp, &dirs, view)?;
        let mut t = String::new();
        t += "native = \"dcc\"\nnative_version = 1\n";
        t += &format!("version = {}\n", self.version);
        t += &format!("frames_per_direction = {}\n", self.frames_per_direction);
        t += &format!("tag = {}\n", self.tag);
        t += &format!("sheet = {}\n\n", lay.has_pixels());
        t += &format!("[encoding]\nfinal_dc6_size = {}\n", self.final_dc6_size);
        for (di, d) in self.directions.iter().enumerate() {
            t += "\n[[direction]]\n";
            t += &format!("compression_flags = {}\n", hex2(d.compression_flags));
            t += &format!("x_min = {}\ny_min = {}\n", d.x_min, d.y_min);
            t += &format!("width = {}\nheight = {}\n", d.width, d.height);
            t += &format!(
                "[direction.encoding]\noutsize_coded = {}\npcd_leftover_bits = {}\n",
                d.outsize_coded, d.pcd_leftover_bits
            );
            for (fi, fr) in d.frames.iter().enumerate() {
                t += "\n[[direction.frame]]\n";
                t += &format!("rect = {}\n", rect_text(lay.rects[di][fi]));
                t += &format!("variable0 = {}\n", fr.variable0);
                t += &format!("x_offset = {}\ny_offset = {}\n", fr.x_offset, fr.y_offset);
                t += &format!("bottom_up = {}\n", fr.bottom_up);
                t += &format!("x_min = {}\ny_min = {}\n", fr.x_min, fr.y_min);
                t += &format!(
                    "[direction.frame.encoding]\ncoded_bytes = {}\noptional_data = {}\n",
                    fr.coded_bytes,
                    hex_bytes(&fr.optional_data)
                );
            }
        }
        Ok(files(p, t, png))
    }

    fn read(p: &str, store: &dyn FileStore) -> Result<Self, NativeError> {
        let (tp, pp) = (sheet_toml(p), sheet_png(p));
        let text =
            String::from_utf8(need(store, &tp)?).map_err(|_| NativeError::new(&tp, "not UTF-8"))?;
        let mut f = Fields::parse(&tp, &text)?;
        f.header("dcc", 1)?;
        let version = f.u8("version")?;
        let frames_per_direction = f.u32("frames_per_direction")?;
        let tag = f.u32("tag")?;
        let sheet = f.bool("sheet")?;
        let final_dc6_size = match f.opt_table("encoding")? {
            Some(mut e) => {
                let v = e.u32("final_dc6_size")?;
                e.finish()?;
                v
            }
            None => 0,
        };
        let mut dirs = f.tables("direction")?;
        let mut rects = Vec::new();
        let mut shells = Vec::new();
        for (di, d) in dirs.iter_mut().enumerate() {
            let compression_flags = d.hex_u8("compression_flags")?;
            let x_min = d.i32("x_min")?;
            let y_min = d.i32("y_min")?;
            let width = d.u32("width")?;
            let height = d.u32("height")?;
            let (outsize_coded, pcd_leftover_bits) = match d.opt_table("encoding")? {
                Some(mut e) => {
                    let a = e.u32("outsize_coded")?;
                    let b = e.int("pcd_leftover_bits")?;
                    let b = usize::try_from(b)
                        .map_err(|_| e.err("`pcd_leftover_bits` out of range"))?;
                    e.finish()?;
                    (a, b)
                }
                None => (0, 0),
            };
            let mut frames = d.tables("frame")?;
            if frames.len() != frames_per_direction as usize {
                return Err(f.err(format!(
                    "direction {di}: {} frames, frames_per_direction = {frames_per_direction}",
                    frames.len()
                )));
            }
            let mut rr = Vec::new();
            let mut fs = Vec::new();
            for fr in frames.iter_mut() {
                let rect = read_rect(fr)?;
                let variable0 = fr.u32("variable0")?;
                let x_offset = fr.i32("x_offset")?;
                let y_offset = fr.i32("y_offset")?;
                let bottom_up = fr.bool("bottom_up")?;
                let fx_min = fr.i32("x_min")?;
                let fy_min = fr.i32("y_min")?;
                let (coded_bytes, optional_data) = match fr.opt_table("encoding")? {
                    Some(mut e) => {
                        let a = e.u32("coded_bytes")?;
                        let s = e.str("optional_data")?;
                        let b = parse_hex_bytes(&s).ok_or_else(|| {
                            e.err("`optional_data` must be lowercase hex digit pairs")
                        })?;
                        e.finish()?;
                        (a, b)
                    }
                    None => (0, Vec::new()),
                };
                fr.finish()?;
                fs.push(DccFrame {
                    variable0,
                    width: rect.rect[2],
                    height: rect.rect[3],
                    x_offset,
                    y_offset,
                    coded_bytes,
                    bottom_up,
                    optional_data,
                    x_min: fx_min,
                    y_min: fy_min,
                    pixels: Vec::new(),
                });
                rr.push(rect);
            }
            d.finish()?;
            rects.push(rr);
            shells.push(DccDirection {
                outsize_coded,
                compression_flags,
                x_min,
                y_min,
                width,
                height,
                frames: fs,
                pcd_leftover_bits,
            });
        }
        f.finish()?;
        let px = load_pixels(&tp, &pp, sheet, &rects, store)?;
        for (d, pd) in shells.iter_mut().zip(px) {
            for (fr, pixels) in d.frames.iter_mut().zip(pd) {
                fr.pixels = pixels;
            }
        }
        Ok(Dcc {
            version,
            frames_per_direction,
            tag,
            final_dc6_size,
            directions: shells,
        })
    }

    fn first_difference(&self, native: &Self, p: &str) -> Option<Difference> {
        let (tp, pp) = (sheet_toml(p), sheet_png(p));
        diff_fields!(
            tp,
            "file",
            self,
            native,
            [version, frames_per_direction, tag, final_dc6_size]
        );
        if self.directions.len() != native.directions.len() {
            return Some(Difference {
                file: tp,
                detail: format!(
                    "{} directions != {}",
                    self.directions.len(),
                    native.directions.len()
                ),
            });
        }
        for (di, (a, b)) in self.directions.iter().zip(&native.directions).enumerate() {
            let at = format!("direction {di}");
            diff_fields!(
                tp,
                at,
                a,
                b,
                [
                    outsize_coded,
                    compression_flags,
                    x_min,
                    y_min,
                    width,
                    height,
                    pcd_leftover_bits
                ]
            );
            if a.frames.len() != b.frames.len() {
                return Some(Difference {
                    file: tp,
                    detail: format!("{at}: {} frames != {}", a.frames.len(), b.frames.len()),
                });
            }
            for (fi, (fa, fb)) in a.frames.iter().zip(&b.frames).enumerate() {
                let at = format!("direction {di} frame {fi}");
                diff_fields!(
                    tp,
                    at,
                    fa,
                    fb,
                    [
                        variable0,
                        width,
                        height,
                        x_offset,
                        y_offset,
                        coded_bytes,
                        bottom_up,
                        optional_data,
                        x_min,
                        y_min
                    ]
                );
                if let Some(d) = pixel_diff(&fa.pixels, &fb.pixels, fa.width as usize) {
                    return Some(Difference {
                        file: pp,
                        detail: format!("{at} {d}"),
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::{check_files, grey_palette, round_trip, CheckError, MemStore};

    const P: &str = "data/global/ui/test.dc6";

    fn frame(w: u32, h: u32, seed: u8) -> Dc6Frame {
        Dc6Frame {
            flip: 0,
            width: w,
            height: h,
            offset_x: -3,
            offset_y: 7,
            unknown: 1,
            next_block: 99,
            pixels: (0..w * h).map(|i| (i as u8).wrapping_add(seed)).collect(),
        }
    }

    fn dc6(sizes: &[(u32, u32)], directions: u32) -> Dc6 {
        let n = sizes.len() as u32 / directions;
        Dc6 {
            header: Dc6Header {
                version: 6,
                flags: 1,
                encoding: 0,
                termination: [0xee; 4],
                directions,
                frames_per_direction: n,
            },
            frames: sizes
                .iter()
                .enumerate()
                .map(|(i, &(w, h))| frame(w, h, i as u8 * 40 + 1))
                .collect(),
        }
    }

    fn vector() -> Dc6 {
        dc6(&[(3, 2), (0, 0), (4, 1), (2, 3)], 2)
    }

    fn toml_of(files: &[NativeFile]) -> String {
        let f = files.iter().find(|f| f.path.ends_with(".toml")).unwrap();
        String::from_utf8(f.bytes.clone()).unwrap()
    }

    // Covers: specs/formats/native-assets.md §2.2 r1
    // Covers: specs/formats/native-assets.md §2.2 r2
    // Covers: specs/formats/native-assets.md §2.2 r3
    // Covers: specs/formats/native-assets.md §7.1 r1
    #[test]
    fn dc6_test_vector_layout_and_round_trip() {
        let d = vector();
        let files = round_trip(&d, P, &grey_palette()).unwrap();
        let png = files.iter().find(|f| f.path == format!("{P}.png")).unwrap();
        let (w, h, _) = png::read_indexed("x", &png.bytes).unwrap();
        assert_eq!((w, h), (6, 5));
        let t = toml_of(&files);
        for r in [
            "[0, 0, 3, 2]",
            "[3, 0, 0, 0]",
            "[0, 2, 4, 1]",
            "[4, 2, 2, 3]",
        ] {
            assert!(t.contains(&format!("rect = {r}")), "{r} in\n{t}");
        }
        assert!(t.contains("[[direction.frame]]"));
    }

    // Covers: specs/formats/native-assets.md §2.1 r2
    #[test]
    fn png_is_indexed_8bit_with_trns_and_plte() {
        let files = vector().write(P, &grey_palette()).unwrap();
        let png = files.iter().find(|f| f.path.ends_with(".png")).unwrap();
        let dec = png::read_indexed("x", &png.bytes).unwrap();
        assert_eq!(dec.0, 6);
        // chunk order IHDR, PLTE, tRNS, IDAT..., IEND
        let b = &png.bytes;
        let mut at = 8;
        let mut names = Vec::new();
        while at < b.len() {
            let len = u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize;
            let n = String::from_utf8(b[at + 4..at + 8].to_vec()).unwrap();
            if n == "tRNS" {
                assert_eq!(len, 256);
                assert_eq!((b[at + 8], b[at + 9], b[at + 263]), (0, 255, 255));
            }
            if n == "IHDR" {
                assert_eq!((b[at + 16], b[at + 17], b[at + 20]), (8, 3, 0));
            }
            if names.last() != Some(&n) {
                names.push(n);
            }
            at += 12 + len;
        }
        assert_eq!(names, ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]);
    }

    // Covers: specs/formats/native-assets.md §2.2 r1
    // Covers: specs/formats/native-assets.md §7.1 r1
    #[test]
    fn dc6_without_pixels_has_no_png() {
        let d = dc6(&[(0, 0), (0, 0)], 1);
        let files = round_trip(&d, P, &grey_palette()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(toml_of(&files).contains("sheet = false"));
        // a frame 5 x 0 still takes its width in the layout and has no pixels
        let d = dc6(&[(5, 0), (0, 0)], 1);
        let files = round_trip(&d, P, &grey_palette()).unwrap();
        assert_eq!(files.len(), 1);
    }

    // Covers: specs/formats/native-assets.md §7.1 r1
    #[test]
    fn dc6_empty_file_and_row_of_empty_frames() {
        let empty = dc6(&[], 1);
        round_trip(&empty, P, &grey_palette()).unwrap();
        // row 0 all 0 x 0 (height 0), row 1 real
        let d = dc6(&[(0, 0), (2, 2)], 2);
        let files = round_trip(&d, P, &grey_palette()).unwrap();
        let png = files.iter().find(|f| f.path.ends_with(".png")).unwrap();
        let (_, h, _) = png::read_indexed("x", &png.bytes).unwrap();
        assert_eq!(h, 2);
    }

    // Covers: specs/formats/native-assets.md §4.2 r1
    // Covers: specs/formats/native-assets.md §7.1 r2
    #[test]
    fn writing_twice_gives_identical_bytes() {
        let d = vector();
        assert_eq!(
            d.write(P, &grey_palette()).unwrap(),
            d.write(P, &grey_palette()).unwrap()
        );
    }

    fn perturb_png(files: &mut [NativeFile], at: usize) {
        let f = files.iter_mut().find(|f| f.path.ends_with(".png")).unwrap();
        let (w, h, mut px) = png::read_indexed("x", &f.bytes).unwrap();
        px[at] = px[at].wrapping_add(1);
        f.bytes = png::write_indexed("x", w, h, &px, &grey_palette()).unwrap();
    }

    // Covers: specs/formats/native-assets.md §7.1 r3
    // Covers: specs/formats/native-assets.md §4.3
    #[test]
    fn dc6_pixel_perturbation_names_file_direction_frame_pixel() {
        let d = vector();
        let mut files = d.write(P, &grey_palette()).unwrap();
        // sheet is 6 wide; frame (dir 1, frame 1) is at [4, 2, 2, 3]: its pixel (1, 2)
        perturb_png(&mut files, (2 + 2) * 6 + 4 + 1);
        let e = check_files(&d, P, &files).unwrap_err();
        let CheckError::Mismatch(diff) = e else {
            panic!("{e:?}")
        };
        assert_eq!(diff.file, format!("{P}.png"));
        assert_eq!(
            diff.detail,
            "direction 1 frame 1 pixel (1, 2): index 126 != 127"
        );
    }

    // Covers: specs/formats/native-assets.md §7.1 r3
    #[test]
    fn dc6_field_perturbation_names_sidecar_and_field() {
        let d = vector();
        let mut files = d.write(P, &grey_palette()).unwrap();
        let t = files
            .iter_mut()
            .find(|f| f.path.ends_with(".toml"))
            .unwrap();
        let s = String::from_utf8(t.bytes.clone()).unwrap();
        t.bytes = s.replacen("offset_y = 7", "offset_y = 8", 1).into_bytes();
        let CheckError::Mismatch(diff) = check_files(&d, P, &files).unwrap_err() else {
            panic!()
        };
        assert_eq!(diff.file, format!("{P}.toml"));
        assert!(
            diff.detail
                .starts_with("direction 0 frame 0: offset_y 7 != 8"),
            "{diff}"
        );
    }

    // Covers: specs/formats/native-assets.md §2.1 r2
    // Covers: specs/formats/native-assets.md §2.1 r7
    // Covers: specs/formats/native-assets.md §2.1 r6
    // Covers: specs/formats/native-assets.md §7.1 r5
    #[test]
    fn strict_readers_name_the_file() {
        let d = vector();
        let files = d.write(P, &grey_palette()).unwrap();
        let read = |files: &[NativeFile]| Dc6::read(P, &MemStore::from_files(files)).unwrap_err();

        // colour type 2 PNG
        let mut f = files.clone();
        let png = f.iter_mut().find(|f| f.path.ends_with(".png")).unwrap();
        let mut out = Vec::new();
        {
            let mut e = ::png::Encoder::new(&mut out, 6, 5);
            e.set_color(::png::ColorType::Rgb);
            e.set_depth(::png::BitDepth::Eight);
            let mut w = e.write_header().unwrap();
            w.write_image_data(&[0u8; 6 * 5 * 3]).unwrap();
        }
        png.bytes = out;
        let e = read(&f);
        assert_eq!(e.file, format!("{P}.png"));
        assert!(e.message.contains("indexed"), "{e}");

        // unknown native_version
        let mut f = files.clone();
        let t = f.iter_mut().find(|f| f.path.ends_with(".toml")).unwrap();
        t.bytes = String::from_utf8(t.bytes.clone())
            .unwrap()
            .replace("native_version = 1", "native_version = 2")
            .into_bytes();
        let e = read(&f);
        assert_eq!(e.file, format!("{P}.toml"));
        assert!(e.message.contains("native_version"), "{e}");

        // hex in the wrong spelling (upper case)
        let mut f = files.clone();
        let t = f.iter_mut().find(|f| f.path.ends_with(".toml")).unwrap();
        t.bytes = String::from_utf8(t.bytes.clone())
            .unwrap()
            .replace("0xeeeeeeee", "0xEEEEEEEE")
            .into_bytes();
        assert!(read(&f).message.contains("termination"));

        // unknown key
        let mut f = files.clone();
        let t = f.iter_mut().find(|f| f.path.ends_with(".toml")).unwrap();
        t.bytes.extend_from_slice(b"\nextra = 1\n");
        assert!(read(&f).message.contains("unknown key"));

        // a frame resized in the sidecar no longer matches the layout / PNG
        let mut f = files.clone();
        let t = f.iter_mut().find(|f| f.path.ends_with(".toml")).unwrap();
        t.bytes = String::from_utf8(t.bytes.clone())
            .unwrap()
            .replace("rect = [0, 0, 3, 2]", "rect = [0, 0, 4, 2]")
            .into_bytes();
        assert!(read(&f).message.contains("rect"));
    }

    // Covers: specs/formats/native-assets.md §2.1 r4
    #[test]
    fn mod_authored_file_omits_encoding_and_gets_defaults() {
        let d = vector();
        let mut files = d.write(P, &grey_palette()).unwrap();
        let t = files
            .iter_mut()
            .find(|f| f.path.ends_with(".toml"))
            .unwrap();
        let s = String::from_utf8(t.bytes.clone()).unwrap();
        let s: String = s
            .lines()
            .filter(|l| !l.starts_with("next_block") && !l.contains("encoding]"))
            .map(|l| format!("{l}\n"))
            .collect();
        t.bytes = s.into_bytes();
        let back = Dc6::read(P, &MemStore::from_files(&files)).unwrap();
        assert!(back.frames.iter().all(|f| f.next_block == 0));
        assert_eq!(back.frames[0].pixels, d.frames[0].pixels);
    }

    fn dcc_frame(w: u32, h: u32, seed: u8) -> DccFrame {
        DccFrame {
            variable0: 0,
            width: w,
            height: h,
            x_offset: -5,
            y_offset: 9,
            coded_bytes: 77,
            bottom_up: seed.is_multiple_of(2),
            optional_data: vec![0xab, 0x01],
            x_min: -4,
            y_min: -8,
            pixels: (0..w * h)
                .map(|i| (i as u8).wrapping_mul(3).wrapping_add(seed))
                .collect(),
        }
    }

    fn dcc() -> Dcc {
        let dir = |sizes: &[(u32, u32)], s: u8| DccDirection {
            outsize_coded: 1234,
            compression_flags: 0x03,
            x_min: -10,
            y_min: -20,
            width: 30,
            height: 40,
            frames: sizes
                .iter()
                .enumerate()
                .map(|(i, &(w, h))| dcc_frame(w, h, s + i as u8))
                .collect(),
            pcd_leftover_bits: 5,
        };
        Dcc {
            version: 6,
            frames_per_direction: 2,
            tag: 1,
            final_dc6_size: 5555,
            directions: vec![dir(&[(4, 3), (2, 2)], 1), dir(&[(0, 0), (5, 1)], 9)],
        }
    }

    // Covers: specs/formats/native-assets.md §2.2 r2
    // Covers: specs/formats/native-assets.md §7.1 r1
    // Covers: specs/formats/native-assets.md §2.1 r4
    #[test]
    fn dcc_round_trip_with_encoding_fields() {
        let d = dcc();
        let files = round_trip(&d, "data/global/chars/x.dcc", &grey_palette()).unwrap();
        let t = toml_of(&files);
        assert!(t.contains("compression_flags = \"0x03\""));
        assert!(t.contains("optional_data = \"ab01\""));
        // an empty direction (no frames) and one with only empty frames
        let mut e = d.clone();
        e.frames_per_direction = 0;
        for dir in &mut e.directions {
            dir.frames.clear();
        }
        round_trip(&e, "data/global/chars/e.dcc", &grey_palette()).unwrap();
        let mut z = d;
        for dir in &mut z.directions {
            for f in &mut dir.frames {
                f.width = 0;
                f.height = 0;
                f.pixels.clear();
            }
        }
        let files = round_trip(&z, "data/global/chars/z.dcc", &grey_palette()).unwrap();
        assert_eq!(files.len(), 1);
    }

    // Covers: specs/formats/native-assets.md §7.1 r3
    #[test]
    fn dcc_perturbations_report_the_first_difference() {
        let d = dcc();
        let p = "data/global/chars/x.dcc";
        let mut files = d.write(p, &grey_palette()).unwrap();
        // sheet row 0 is 6 wide x 3; frame (0,1) at [4,0,2,2]; pixel (1,1)
        perturb_png(&mut files, 6 + 4 + 1);
        let CheckError::Mismatch(diff) = check_files(&d, p, &files).unwrap_err() else {
            panic!()
        };
        assert_eq!(diff.file, format!("{p}.png"));
        assert!(
            diff.detail.starts_with("direction 0 frame 1 pixel (1, 1)"),
            "{diff}"
        );

        // an encoding-only field is compared too
        let mut files = d.write(p, &grey_palette()).unwrap();
        let t = files
            .iter_mut()
            .find(|f| f.path.ends_with(".toml"))
            .unwrap();
        t.bytes = String::from_utf8(t.bytes.clone())
            .unwrap()
            .replacen("pcd_leftover_bits = 5", "pcd_leftover_bits = 6", 1)
            .into_bytes();
        let CheckError::Mismatch(diff) = check_files(&d, p, &files).unwrap_err() else {
            panic!()
        };
        assert!(diff.detail.contains("direction 0") && diff.detail.contains("pcd_leftover_bits"));
    }
}

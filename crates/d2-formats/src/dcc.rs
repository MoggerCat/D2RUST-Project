// Spec: specs/formats/dcc.md
//! DCC animations: cell-based compressed 8-bit indexed frames.

use crate::cursor::{invalid, FormatError};

const FORMAT: &str = "dcc";
const SIGNATURE: u8 = 0x74;
const MAX_FRAMES: u32 = 256;
const MAX_DIR_PIXELS: u64 = 0x100_0000;
const ENCODED_BITS: [u32; 16] = [0, 1, 2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 26, 28, 30, 32];

/// LSB-first bit cursor over `data[.. end_bit]`.
#[derive(Clone)]
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            end: data.len() * 8,
        }
    }

    /// A sub-stream of the next `len` bits; this cursor skips past it.
    fn take(&mut self, len: usize, what: &str) -> Result<Bits<'a>, FormatError> {
        if self.pos + len > self.end {
            return Err(invalid(
                FORMAT,
                format!("{what} stream extends past the data"),
            ));
        }
        let sub = Bits {
            data: self.data,
            pos: self.pos,
            end: self.pos + len,
        };
        self.pos += len;
        Ok(sub)
    }

    fn remaining(&self) -> usize {
        self.end - self.pos
    }

    fn read(&mut self, n: u32) -> Result<u32, FormatError> {
        if n == 0 {
            return Ok(0);
        }
        if self.pos + n as usize > self.end {
            return Err(invalid(FORMAT, "bit stream ended early"));
        }
        let byte = self.pos / 8;
        let shift = self.pos % 8;
        let mut v: u64 = 0;
        for i in 0..5 {
            if let Some(&b) = self.data.get(byte + i) {
                v |= u64::from(b) << (8 * i);
            }
        }
        self.pos += n as usize;
        Ok(((v >> shift) & ((1u64 << n) - 1)) as u32)
    }

    fn read_signed(&mut self, n: u32) -> Result<i32, FormatError> {
        let v = self.read(n)?;
        if n == 0 || n == 32 {
            return Ok(v as i32);
        }
        let sign = 1u32 << (n - 1);
        Ok(((v ^ sign).wrapping_sub(sign)) as i32)
    }

    fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DccFrame {
    pub variable0: u32,
    pub width: u32,
    pub height: u32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub coded_bytes: u32,
    pub bottom_up: bool,
    pub optional_data: Vec<u8>,
    /// Box in direction space: `x_min..=x_max`, `y_min..=y_max`.
    pub x_min: i32,
    pub y_min: i32,
    /// Palette indices, `width × height`, top row first; 0 = transparent.
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DccDirection {
    pub outsize_coded: u32,
    pub compression_flags: u8,
    pub x_min: i32,
    pub y_min: i32,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<DccFrame>,
    /// Unread bits at the end of the pixel-code-and-displacement stream.
    pub pcd_leftover_bits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dcc {
    pub version: u8,
    pub frames_per_direction: u32,
    pub tag: u32,
    pub final_dc6_size: u32,
    pub directions: Vec<DccDirection>,
}

impl Dcc {
    pub fn parse(data: &[u8]) -> Result<Dcc, FormatError> {
        let mut c = crate::cursor::Cursor::new(data, FORMAT);
        let signature = c.u8()?;
        if signature != SIGNATURE {
            return Err(invalid(FORMAT, format!("signature {signature:#x}")));
        }
        let version = c.u8()?;
        let directions = usize::from(c.u8()?);
        let frames_per_direction = c.u32()?;
        if frames_per_direction > MAX_FRAMES {
            return Err(invalid(FORMAT, format!("{frames_per_direction} frames")));
        }
        let tag = c.u32()?;
        let final_dc6_size = c.u32()?;
        let mut offsets = (0..directions)
            .map(|_| c.u32().map(|o| o as usize))
            .collect::<Result<Vec<_>, _>>()?;
        offsets.push(data.len());
        if offsets[0] < c.pos() || offsets.windows(2).any(|w| w[1] < w[0]) {
            return Err(invalid(FORMAT, "direction offsets out of order"));
        }
        let dirs = (0..directions)
            .map(|d| {
                decode_direction(
                    &data[offsets[d]..offsets[d + 1]],
                    frames_per_direction as usize,
                )
                .map_err(|e| invalid(FORMAT, format!("direction {d}: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Dcc {
            version,
            frames_per_direction,
            tag,
            final_dc6_size,
            directions: dirs,
        })
    }
}

/// One frame cell: position relative to the direction box, size, and its
/// direction cell.
#[derive(Clone, Copy)]
struct Cell {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    dcell: usize,
}

/// Cell sizes along one axis (spec §Cells).
fn axis_cells(offset: usize, len: usize) -> Vec<usize> {
    let first = 4 - offset % 4;
    if len <= first + 1 {
        return vec![len];
    }
    let rem = len - first;
    let mut cells = vec![first];
    match rem % 4 {
        0 => cells.extend(std::iter::repeat_n(4, rem / 4)),
        1 => {
            cells.extend(std::iter::repeat_n(4, rem / 4 - 1));
            cells.push(5);
        }
        r => {
            cells.extend(std::iter::repeat_n(4, rem / 4));
            cells.push(r);
        }
    }
    cells
}

struct Entry {
    frame: usize,
    cell: usize,
    v: [u8; 4],
}

fn decode_direction(data: &[u8], frame_count: usize) -> Result<DccDirection, FormatError> {
    let mut b = Bits::new(data);
    let outsize_coded = b.read(32)?;
    let compression_flags = b.read(2)? as u8;
    let mut codes = [0u32; 7];
    for code in &mut codes {
        *code = ENCODED_BITS[b.read(4)? as usize];
    }
    let [var0_bits, width_bits, height_bits, x_bits, y_bits, optional_bits, coded_bits] = codes;

    struct Header {
        variable0: u32,
        width: u32,
        height: u32,
        x_offset: i32,
        y_offset: i32,
        optional: u32,
        coded_bytes: u32,
        bottom_up: bool,
    }
    let mut headers = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        headers.push(Header {
            variable0: b.read(var0_bits)?,
            width: b.read(width_bits)?,
            height: b.read(height_bits)?,
            x_offset: b.read_signed(x_bits)?,
            y_offset: b.read_signed(y_bits)?,
            optional: b.read(optional_bits)?,
            coded_bytes: b.read(coded_bits)?,
            bottom_up: b.read(1)? == 1,
        });
    }
    let mut optional_data = vec![Vec::new(); frame_count];
    if headers.iter().any(|h| h.optional > 0) {
        b.align();
        for (f, h) in headers.iter().enumerate() {
            let start = b.pos / 8;
            let end = start + h.optional as usize;
            optional_data[f] = data
                .get(start..end)
                .ok_or_else(|| invalid(FORMAT, "optional data past the end"))?
                .to_vec();
            b.pos = end * 8;
        }
        if b.pos > b.end {
            return Err(invalid(FORMAT, "optional data past the end"));
        }
    }

    let equal_size = if compression_flags & 2 != 0 {
        b.read(20)?
    } else {
        0
    } as usize;
    let mask_size = b.read(20)? as usize;
    let (encoding_size, raw_size) = if compression_flags & 1 != 0 {
        (b.read(20)? as usize, b.read(20)? as usize)
    } else {
        (0, 0)
    };
    let mut pv = Vec::with_capacity(256);
    for i in 0..=255u8 {
        if b.read(1)? == 1 {
            pv.push(i);
        }
    }
    let mut equal = b.take(equal_size, "equal cells")?;
    let mut mask = b.take(mask_size, "pixel mask")?;
    let mut encoding = b.take(encoding_size, "encoding type")?;
    let mut raw = b.take(raw_size, "raw pixel")?;
    let mut pcd = b.take(b.remaining(), "pixel code")?;

    // Boxes.
    if headers.is_empty() {
        return Ok(DccDirection {
            outsize_coded,
            compression_flags,
            x_min: 0,
            y_min: 0,
            width: 0,
            height: 0,
            frames: Vec::new(),
            pcd_leftover_bits: pcd.remaining(),
        });
    }
    let mut boxes = Vec::with_capacity(frame_count);
    for h in &headers {
        if h.width == 0 || h.height == 0 {
            return Err(invalid(
                FORMAT,
                format!("frame size {}x{}", h.width, h.height),
            ));
        }
        let x_min = i64::from(h.x_offset);
        let y_min = if h.bottom_up {
            i64::from(h.y_offset)
        } else {
            i64::from(h.y_offset) - i64::from(h.height) + 1
        };
        boxes.push((x_min, y_min, i64::from(h.width), i64::from(h.height)));
    }
    let dx_min = boxes.iter().map(|b| b.0).min().unwrap();
    let dy_min = boxes.iter().map(|b| b.1).min().unwrap();
    let dx_max = boxes.iter().map(|b| b.0 + b.2).max().unwrap();
    let dy_max = boxes.iter().map(|b| b.1 + b.3).max().unwrap();
    let (dir_w, dir_h) = ((dx_max - dx_min) as u64, (dy_max - dy_min) as u64);
    if dir_w * dir_h > MAX_DIR_PIXELS {
        return Err(invalid(FORMAT, format!("direction box {dir_w}x{dir_h}")));
    }
    let (dir_w, dir_h) = (dir_w as usize, dir_h as usize);
    let cells_w = dir_w.div_ceil(4);
    let cells_h = dir_h.div_ceil(4);

    // Frame cell layouts.
    let layouts: Vec<Vec<Cell>> = boxes
        .iter()
        .map(|&(x, y, w, h)| {
            let fx = (x - dx_min) as usize;
            let fy = (y - dy_min) as usize;
            let xs = axis_cells(fx, w as usize);
            let ys = axis_cells(fy, h as usize);
            let mut cells = Vec::with_capacity(xs.len() * ys.len());
            let mut cy = fy;
            for (j, &ch) in ys.iter().enumerate() {
                let mut cx = fx;
                for (i, &cw) in xs.iter().enumerate() {
                    let dcell = (fy / 4 + j) * cells_w + fx / 4 + i;
                    cells.push(Cell {
                        x: cx,
                        y: cy,
                        w: cw,
                        h: ch,
                        dcell,
                    });
                    cx += cw;
                }
                cy += ch;
            }
            cells
        })
        .collect();

    // Stage 1: cell colors.
    let mut latest: Vec<Option<[u8; 4]>> = vec![None; cells_w * cells_h];
    let mut queue: Vec<Entry> = Vec::new();
    for (f, cells) in layouts.iter().enumerate() {
        for (ci, cell) in cells.iter().enumerate() {
            let previous = latest[cell.dcell];
            let pixel_mask = if previous.is_some() {
                let is_equal = equal_size > 0 && equal.read(1)? == 1;
                if is_equal {
                    continue;
                }
                mask.read(4)?
            } else {
                0xF
            };
            let n = pixel_mask.count_ones();
            let is_raw = n > 0 && encoding_size > 0 && encoding.read(1)? == 1;
            let mut stack = [0u8; 4];
            let mut pushed = 0;
            let mut last = 0u8;
            for _ in 0..n {
                let code = if is_raw {
                    raw.read(8)? as u8
                } else {
                    let mut code = u32::from(last);
                    loop {
                        let disp = pcd.read(4)?;
                        code += disp;
                        if disp != 15 {
                            break;
                        }
                    }
                    code as u8
                };
                if code == last {
                    break;
                }
                stack[pushed] = code;
                pushed += 1;
                last = code;
            }
            let prev = previous.unwrap_or([0; 4]);
            let mut v = [0u8; 4];
            for (i, value) in v.iter_mut().enumerate() {
                *value = if pixel_mask & (1 << i) != 0 {
                    if pushed > 0 {
                        pushed -= 1;
                        stack[pushed]
                    } else {
                        0
                    }
                } else {
                    prev[i]
                };
            }
            latest[cell.dcell] = Some(v);
            queue.push(Entry {
                frame: f,
                cell: ci,
                v,
            });
        }
    }

    // Stage 2: build frames.
    let mut buffer = vec![0u8; dir_w * dir_h];
    let mut last_rect: Vec<Option<(usize, usize, usize, usize)>> = vec![None; cells_w * cells_h];
    let mut next = 0;
    let mut frames = Vec::with_capacity(frame_count);
    for (f, cells) in layouts.iter().enumerate() {
        let mut out = vec![0u8; dir_w * dir_h];
        for (ci, cell) in cells.iter().enumerate() {
            let rect = (cell.x, cell.y, cell.w, cell.h);
            if next < queue.len() && queue[next].frame == f && queue[next].cell == ci {
                let v = queue[next].v;
                next += 1;
                if v[0] == v[1] {
                    for y in cell.y..cell.y + cell.h {
                        buffer[y * dir_w + cell.x..y * dir_w + cell.x + cell.w].fill(v[0]);
                    }
                } else {
                    let bits = if v[1] == v[2] { 1 } else { 2 };
                    for y in cell.y..cell.y + cell.h {
                        for x in cell.x..cell.x + cell.w {
                            buffer[y * dir_w + x] = v[pcd.read(bits)? as usize];
                        }
                    }
                }
                copy_rect(&buffer, &mut out, dir_w, rect);
            } else {
                match last_rect[cell.dcell] {
                    Some((lx, ly, lw, lh)) if lw == cell.w && lh == cell.h => {
                        // Read the whole source before writing (overlap-safe).
                        let mut tmp = Vec::with_capacity(lw * lh);
                        for y in ly..ly + lh {
                            tmp.extend_from_slice(&buffer[y * dir_w + lx..y * dir_w + lx + lw]);
                        }
                        for (row, y) in (cell.y..cell.y + cell.h).enumerate() {
                            buffer[y * dir_w + cell.x..y * dir_w + cell.x + cell.w]
                                .copy_from_slice(&tmp[row * lw..(row + 1) * lw]);
                        }
                        copy_rect(&buffer, &mut out, dir_w, rect);
                    }
                    _ => {
                        for y in cell.y..cell.y + cell.h {
                            buffer[y * dir_w + cell.x..y * dir_w + cell.x + cell.w].fill(0);
                        }
                    }
                }
            }
            last_rect[cell.dcell] = Some(rect);
        }

        // Crop to the frame box and map codes to palette indices.
        let h = &headers[f];
        let (bx, by, bw, bh) = boxes[f];
        let fx = (bx - dx_min) as usize;
        let fy = (by - dy_min) as usize;
        let (bw, bh) = (bw as usize, bh as usize);
        let mut pixels = Vec::with_capacity(bw * bh);
        for y in fy..fy + bh {
            for &code in &out[y * dir_w + fx..y * dir_w + fx + bw] {
                let idx = *pv.get(usize::from(code)).ok_or_else(|| {
                    invalid(
                        FORMAT,
                        format!("pixel code {code} with {} colors", pv.len()),
                    )
                })?;
                pixels.push(idx);
            }
        }
        frames.push(DccFrame {
            variable0: h.variable0,
            width: h.width,
            height: h.height,
            x_offset: h.x_offset,
            y_offset: h.y_offset,
            coded_bytes: h.coded_bytes,
            bottom_up: h.bottom_up,
            optional_data: std::mem::take(&mut optional_data[f]),
            x_min: bx as i32,
            y_min: by as i32,
            pixels,
        });
    }

    for (stream, name) in [
        (&equal, "equal cells"),
        (&mask, "pixel mask"),
        (&encoding, "encoding type"),
        (&raw, "raw pixel"),
    ] {
        if stream.remaining() != 0 {
            return Err(invalid(
                FORMAT,
                format!("{name} stream has {} unread bits", stream.remaining()),
            ));
        }
    }
    Ok(DccDirection {
        outsize_coded,
        compression_flags,
        x_min: dx_min as i32,
        y_min: dy_min as i32,
        width: dir_w as u32,
        height: dir_h as u32,
        frames,
        pcd_leftover_bits: pcd.remaining(),
    })
}

fn copy_rect(
    src: &[u8],
    dst: &mut [u8],
    stride: usize,
    (x, y, w, h): (usize, usize, usize, usize),
) {
    for row in y..y + h {
        let at = row * stride + x;
        dst[at..at + w].copy_from_slice(&src[at..at + w]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_sizes() {
        assert_eq!(axis_cells(0, 1), [1]);
        assert_eq!(axis_cells(0, 9), [4, 5]);
        assert_eq!(axis_cells(2, 10), [2, 4, 4]);
        assert_eq!(axis_cells(3, 7), [1, 4, 2]);
        assert_eq!(axis_cells(3, 2), [2]);
        assert_eq!(axis_cells(0, 4), [4]);
        assert_eq!(axis_cells(0, 8), [4, 4]);
        for off in 0..4 {
            for len in 1..40 {
                assert_eq!(
                    axis_cells(off, len).iter().sum::<usize>(),
                    len,
                    "{off} {len}"
                );
            }
        }
    }

    #[test]
    fn signed_fields() {
        // 0b1111 as a 4-bit signed field is -1; 0b0111 is 7.
        let mut b = Bits::new(&[0x7F]);
        assert_eq!(b.read_signed(4).unwrap(), -1);
        assert_eq!(b.read_signed(4).unwrap(), 7);
    }

    /// Writes LSB-first bit fields.
    #[derive(Default)]
    struct BitWriter {
        bytes: Vec<u8>,
        bits: usize,
    }
    impl BitWriter {
        fn put(&mut self, value: u32, n: u32) {
            for i in 0..n {
                if self.bits.is_multiple_of(8) {
                    self.bytes.push(0);
                }
                if value >> i & 1 == 1 {
                    *self.bytes.last_mut().unwrap() |= 1 << (self.bits % 8);
                }
                self.bits += 1;
            }
        }
    }

    /// One direction, one 4×4 frame at (0, -3) top-down: a single cell with
    /// colors {10, 20}: the left half 10, the right half 20.
    fn one_frame_file() -> Vec<u8> {
        let mut w = BitWriter::default();
        w.put(0, 32); // outsize coded
        w.put(0, 2); // flags: no equal cells, no raw encoding
                     // width codes: variable0 0 bits, width 4 bits (code 3), height 4 bits,
                     // x 4 bits, y 4 bits, optional 0, coded 0
        for code in [0, 3, 3, 3, 3, 0, 0] {
            w.put(code, 4);
        }
        w.put(4, 4); // width
        w.put(4, 4); // height
        w.put(0, 4); // x offset
        w.put(3, 4); // y offset (y_max = 3, so y_min = 0)
        w.put(0, 1); // top-down
        w.put(0, 20); // pixel mask stream size (first touch needs none)
                      // pixel values: palette indices 0, 10, 20 present (codes 0, 1, 2)
        for i in 0..256u32 {
            w.put(u32::from(i == 0 || i == 10 || i == 20), 1);
        }
        // Stage 1, PCD: codes pushed 1 then 2, then a repeat (disp 0) stops.
        w.put(1, 4); // code 1
        w.put(1, 4); // code 2
        w.put(0, 4); // code 2 again: stop
                     // v = [2, 1, 0, 0] (pops 2 then 1, then zeros): v0 != v1, v1 != v2,
                     // so 2 bits per pixel. Pixels: indices 1 (code 1) on the left half,
                     // 0 (code 2) on the right half.
        for _y in 0..4 {
            for x in 0..4 {
                w.put(if x < 2 { 1 } else { 0 }, 2);
            }
        }
        let dir = w.bytes;
        let mut file = vec![SIGNATURE, 6, 1];
        file.extend_from_slice(&1u32.to_le_bytes()); // frames per direction
        file.extend_from_slice(&1u32.to_le_bytes()); // tag
        file.extend_from_slice(&0u32.to_le_bytes()); // final dc6 size
        file.extend_from_slice(&19u32.to_le_bytes()); // direction offset
        file.extend(dir);
        file
    }

    #[test]
    fn decodes_synthetic_frame() {
        let dcc = Dcc::parse(&one_frame_file()).unwrap();
        let frame = &dcc.directions[0].frames[0];
        assert_eq!((frame.width, frame.height), (4, 4));
        let expected: Vec<u8> = (0..16).map(|i| if i % 4 < 2 { 10 } else { 20 }).collect();
        assert_eq!(frame.pixels, expected);
        assert!(dcc.directions[0].pcd_leftover_bits < 8);
    }

    #[test]
    fn rejects_bad_signature() {
        let mut data = one_frame_file();
        data[0] = 0x75;
        assert!(Dcc::parse(&data).is_err());
    }
}

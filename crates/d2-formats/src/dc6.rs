// Spec: specs/formats/dc6.md
//! DC6 sprites: run-length encoded 8-bit indexed frames.

use crate::cursor::{invalid, Cursor, FormatError};

const FORMAT: &str = "dc6";
const MAX_FRAMES: u64 = 0x1_0000;
const MAX_PIXELS: u64 = 0x100_0000;
/// Implementation limit on the pixels of all frames together: frame
/// sizes aren't bounded by the encoded data (an empty encoding is a
/// transparent frame), and frame pointers may repeat.
const MAX_TOTAL_PIXELS: u64 = 0x400_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dc6Header {
    pub version: i32,
    pub flags: u32,
    pub encoding: u32,
    pub termination: [u8; 4],
    pub directions: u32,
    pub frames_per_direction: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dc6Frame {
    pub flip: u32,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub unknown: u32,
    pub next_block: u32,
    /// Palette indices, `width × height`, top row first; 0 = transparent.
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dc6 {
    pub header: Dc6Header,
    /// Direction-major: frame `f` of direction `d` is `frames[d * F + f]`.
    pub frames: Vec<Dc6Frame>,
}

impl Dc6 {
    pub fn parse(data: &[u8]) -> Result<Dc6, FormatError> {
        let mut c = Cursor::new(data, FORMAT);
        let version = c.i32()?;
        if version != 6 {
            return Err(invalid(FORMAT, format!("version {version}, expected 6")));
        }
        let flags = c.u32()?;
        let encoding = c.u32()?;
        let mut termination = [0u8; 4];
        termination.copy_from_slice(c.bytes(4)?);
        let directions = c.u32()?;
        let frames_per_direction = c.u32()?;
        let total = u64::from(directions) * u64::from(frames_per_direction);
        if total > MAX_FRAMES {
            return Err(invalid(FORMAT, format!("{total} frames")));
        }
        let pointers = (0..total).map(|_| c.u32()).collect::<Result<Vec<_>, _>>()?;
        let mut pixel_budget = MAX_TOTAL_PIXELS;
        let frames = pointers
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                decode_frame(data, p as usize, &mut pixel_budget)
                    .map_err(|e| invalid(FORMAT, format!("frame {i}: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
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

    pub fn frame(&self, direction: usize, frame: usize) -> Option<&Dc6Frame> {
        let f = self.header.frames_per_direction as usize;
        if frame >= f {
            return None;
        }
        self.frames
            .get(direction.checked_mul(f)?.checked_add(frame)?)
    }
}

fn decode_frame(data: &[u8], at: usize, pixel_budget: &mut u64) -> Result<Dc6Frame, FormatError> {
    let mut c = Cursor::at(data, at, FORMAT);
    let flip = c.u32()?;
    let width = c.u32()?;
    let height = c.u32()?;
    let offset_x = c.i32()?;
    let offset_y = c.i32()?;
    let unknown = c.u32()?;
    let next_block = c.u32()?;
    let length = c.u32()? as usize;
    let size = u64::from(width) * u64::from(height);
    if size > MAX_PIXELS {
        return Err(invalid(FORMAT, format!("frame size {width}x{height}")));
    }
    *pixel_budget = pixel_budget.checked_sub(size).ok_or_else(|| {
        invalid(
            FORMAT,
            format!("frames add up to more than {MAX_TOTAL_PIXELS} pixels"),
        )
    })?;
    let encoded = c.bytes(length)?;
    let pixels = decode_pixels(encoded, width as usize, height as usize, flip != 0)?;
    Ok(Dc6Frame {
        flip,
        width,
        height,
        offset_x,
        offset_y,
        unknown,
        next_block,
        pixels,
    })
}

/// Decodes the RLE pixel data of one frame (spec §Pixel decoding).
fn decode_pixels(
    encoded: &[u8],
    width: usize,
    height: usize,
    top_down: bool,
) -> Result<Vec<u8>, FormatError> {
    let mut pixels = vec![0u8; width * height];
    // Row counter in encoding order; mapped to y below.
    let mut row = 0usize;
    let mut x = 0usize;
    let mut i = 0;
    while i < encoded.len() {
        let b = encoded[i];
        i += 1;
        if b == 0x80 {
            x = 0;
            row += 1;
        } else if b & 0x80 != 0 {
            x += usize::from(b & 0x7F);
        } else {
            let n = usize::from(b);
            let run = encoded
                .get(i..i + n)
                .ok_or_else(|| invalid(FORMAT, "pixel run past end of data"))?;
            i += n;
            if n == 0 {
                continue;
            }
            if row >= height || x + n > width {
                return Err(invalid(
                    FORMAT,
                    format!("pixels outside the frame at row {row}, x {x}"),
                ));
            }
            let y = if top_down { row } else { height - 1 - row };
            pixels[y * width + x..y * width + x + n].copy_from_slice(run);
            x += n;
        }
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/formats/dc6.md §pixel-decoding
    #[test]
    fn spec_vector_bottom_up() {
        let px = decode_pixels(&[0x02, 5, 6, 0x80, 0x82, 0x01, 7, 0x80], 3, 2, false).unwrap();
        assert_eq!(px, [0, 0, 7, 5, 6, 0]);
    }

    // Covers: specs/formats/dc6.md §pixel-decoding
    #[test]
    fn top_down_when_flipped() {
        let px = decode_pixels(&[0x02, 5, 6, 0x80, 0x82, 0x01, 7, 0x80], 3, 2, true).unwrap();
        assert_eq!(px, [5, 6, 0, 0, 0, 7]);
    }

    // Covers: specs/formats/dc6.md §pixel-decoding
    #[test]
    fn errors() {
        assert!(
            decode_pixels(&[0x03, 1, 2, 3], 2, 1, false).is_err(),
            "row overflow"
        );
        assert!(
            decode_pixels(&[0x02, 1], 2, 1, false).is_err(),
            "run past data"
        );
        assert!(
            decode_pixels(&[0x80, 0x01, 9], 1, 1, false).is_err(),
            "row past last"
        );
        assert!(
            decode_pixels(&[0x80, 0x80], 1, 1, false).is_ok(),
            "empty rows are fine"
        );
    }

    fn file(frames: &[(u32, u32, &[u8])]) -> Vec<u8> {
        let mut d = Vec::new();
        for v in [6i32, 1, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&[0xEE; 4]);
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&(frames.len() as u32).to_le_bytes());
        let mut at = d.len() + 4 * frames.len();
        let mut body = Vec::new();
        for (w, h, enc) in frames {
            d.extend_from_slice(&(at as u32).to_le_bytes());
            for v in [0u32, *w, *h, 0, 0, 0, 0, enc.len() as u32] {
                body.extend_from_slice(&v.to_le_bytes());
            }
            body.extend_from_slice(enc);
            body.extend_from_slice(&[0xEE; 3]);
            at += 32 + enc.len() + 3;
        }
        d.extend(body);
        d
    }

    // Covers: specs/formats/dc6.md §file-header-24-bytes, §frame
    #[test]
    fn whole_file() {
        let data = file(&[(1, 1, &[0x01, 42, 0x80]), (0, 0, &[])]);
        let dc6 = Dc6::parse(&data).unwrap();
        assert_eq!(dc6.frames.len(), 2);
        assert_eq!(dc6.frame(0, 0).unwrap().pixels, [42]);
        assert!(dc6.frame(0, 1).unwrap().pixels.is_empty());
        assert!(dc6.frame(0, 2).is_none());
    }

    // Covers: specs/formats/dc6.md §file-header-24-bytes
    #[test]
    fn bad_version() {
        let mut data = file(&[(1, 1, &[0x01, 42])]);
        data[0] = 5;
        assert!(Dc6::parse(&data).is_err());
    }

    #[test]
    fn regress_frame_overflow() {
        // `direction * F + frame` overflowed for a huge direction (panic
        // in debug builds).
        let dc6 = Dc6::parse(&file(&[(1, 1, &[0x01, 42]), (0, 0, &[])])).unwrap();
        assert!(dc6.frame(usize::MAX, 0).is_none());
        assert!(dc6.frame(usize::MAX / 2, 1).is_none());
    }

    #[test]
    fn regress_repeated_large_frames() {
        // Five pointers to one 4096×4096 frame with no encoded data: each
        // 32-byte header is 16M pixels, and up to 0x10000 pointers could
        // repeat it (1 TB). The whole-file budget (64M) stops the fifth.
        let mut data = file(&[(4096, 4096, &[])]);
        let pointer = data[24..28].to_vec();
        data[20..24].copy_from_slice(&5u32.to_le_bytes());
        for _ in 0..4 {
            data.splice(28..28, pointer.iter().copied());
        }
        // The pointer table grew by 16 bytes: move the frame pointers.
        let at = u32::from_le_bytes(pointer.try_into().unwrap()) + 16;
        for i in 0..5 {
            data[24 + 4 * i..28 + 4 * i].copy_from_slice(&at.to_le_bytes());
        }
        let err = Dc6::parse(&data).unwrap_err();
        assert!(err.to_string().contains("frame 4:"), "{err}");
        data[20..24].copy_from_slice(&4u32.to_le_bytes());
        data.drain(40..44);
        for i in 0..4 {
            data[24 + 4 * i..28 + 4 * i].copy_from_slice(&(at - 4).to_le_bytes());
        }
        assert_eq!(Dc6::parse(&data).unwrap().frames.len(), 4);
    }

    mod robust {
        use super::*;
        use crate::robust::{bounded, mutated};
        use crate::robust_tests::config;
        use proptest::prelude::*;

        fn valid() -> Vec<u8> {
            file(&[
                (1, 1, &[0x01, 42, 0x80]),
                (3, 2, &[0x02, 5, 6, 0x80, 0x82, 0x01, 7, 0x80]),
            ])
        }

        #[test]
        fn builder_is_valid() {
            assert!(Dc6::parse(&valid()).is_ok());
        }

        proptest! {
            #![proptest_config(config(64))]

            #[test]
            fn mutated_file(data in mutated(valid()), d in any::<usize>(), f in any::<usize>()) {
                bounded(move || {
                    if let Ok(dc6) = Dc6::parse(&data) {
                        let _ = dc6.frame(d, f);
                        let _ = dc6.frame(d % 4, f % 4);
                    }
                });
            }
        }
    }
}

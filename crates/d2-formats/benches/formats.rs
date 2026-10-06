// Spec: specs/formats/mpq.md §1, §5–§9; specs/formats/dc6.md; specs/formats/dcc.md
//! Performance baselines of `d2-formats` (criterion;
//! `docs/handoff/bench-baselines.md`): MPQ open / read / decompress of a
//! synthetic archive, DC6 and DCC decode of synthetic frames. Not run in
//! CI; `cargo bench -p d2-formats`.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

use d2_formats::dc6::Dc6;
use d2_formats::dcc::Dcc;
use d2_formats::mpq::{bench_fixtures, Archive};

fn bench_mpq(c: &mut Criterion) {
    let bytes = bench_fixtures::archive();
    let path = std::env::temp_dir().join(format!("d2-formats-bench-{}.mpq", std::process::id()));
    std::fs::write(&path, &bytes).expect("write archive");
    let files = bench_fixtures::files();
    let archive = Archive::open(&path).expect("open");
    for (name, content) in &files {
        assert_eq!(&archive.read(name).expect("read"), content, "{name}");
    }
    let mut g = c.benchmark_group("mpq");
    g.bench_function("open_archive", |b| {
        b.iter(|| black_box(Archive::open(&path).expect("open")))
    });
    let (big, big_content) = &files[0];
    g.throughput(Throughput::Bytes(big_content.len() as u64));
    g.bench_function("read_256k_huffman_encrypted_sectors", |b| {
        b.iter(|| black_box(archive.read(big).expect("read")))
    });
    let (plain, plain_content) = &files[1];
    g.throughput(Throughput::Bytes(plain_content.len() as u64));
    g.bench_function("read_64k_plain", |b| {
        b.iter(|| black_box(archive.read(plain).expect("read")))
    });
    g.finish();
    let _ = std::fs::remove_file(&path);
}

// ---- DC6 ---------------------------------------------------------------------

const DC6_FRAME: u32 = 64;
const DC6_FRAMES: usize = 32;

/// A DC6 of `DC6_FRAMES` 64 × 64 frames: every row skips 4 transparent
/// pixels, then 60 literal pixels (`dc6.md` pixel decoding).
fn dc6_file() -> Vec<u8> {
    let mut rows = Vec::new();
    for _ in 0..DC6_FRAME {
        rows.extend_from_slice(&[0x84, 60]);
        rows.extend((0..60u32).map(|i| 1 + (i % 250) as u8));
        rows.push(0x80);
    }
    let mut d = Vec::new();
    for v in [6i32, 1, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0xEE; 4]);
    d.extend_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&(DC6_FRAMES as u32).to_le_bytes());
    let mut at = d.len() + 4 * DC6_FRAMES;
    let mut body = Vec::new();
    for _ in 0..DC6_FRAMES {
        d.extend_from_slice(&(at as u32).to_le_bytes());
        for v in [0u32, DC6_FRAME, DC6_FRAME, 0, 0, 0, 0, rows.len() as u32] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&rows);
        body.extend_from_slice(&[0xEE; 3]);
        at += 32 + rows.len() + 3;
    }
    d.extend(body);
    d
}

// ---- DCC ---------------------------------------------------------------------

/// LSB-first bit fields.
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
                *self.bytes.last_mut().expect("byte") |= 1 << (self.bits % 8);
            }
            self.bits += 1;
        }
    }
}

const DCC_SIDE: u32 = 32;
const DCC_DIRECTIONS: usize = 8;

/// One direction of one `DCC_SIDE`² bottom-up frame, 32-bit header
/// fields, palette {0}, every first-touch cell code 0 (`dcc.md`).
fn dcc_direction() -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(0, 32); // outsize coded
    w.put(0, 2); // flags
    for code in [0, 15, 15, 15, 15, 0, 0] {
        w.put(code, 4);
    }
    for v in [DCC_SIDE, DCC_SIDE, 0, 0] {
        w.put(v, 32); // width, height, x, y
    }
    w.put(1, 1); // bottom-up
    w.put(0, 20); // pixel mask stream size
    for i in 0..256u32 {
        w.put(u32::from(i == 0), 1);
    }
    for _ in 0..(DCC_SIDE / 4) * (DCC_SIDE / 4) {
        w.put(0, 4);
    }
    w.bytes
}

fn dcc_file() -> Vec<u8> {
    let dir = dcc_direction();
    let mut file = vec![0x74, 6, DCC_DIRECTIONS as u8];
    file.extend_from_slice(&1u32.to_le_bytes());
    file.extend_from_slice(&1u32.to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    let mut at = file.len() + 4 * DCC_DIRECTIONS;
    for _ in 0..DCC_DIRECTIONS {
        file.extend_from_slice(&(at as u32).to_le_bytes());
        at += dir.len();
    }
    for _ in 0..DCC_DIRECTIONS {
        file.extend_from_slice(&dir);
    }
    file
}

fn bench_sprites(c: &mut Criterion) {
    let dc6 = dc6_file();
    let parsed = Dc6::parse(&dc6).expect("dc6");
    assert_eq!(parsed.frames.len(), DC6_FRAMES);
    let dcc = dcc_file();
    let parsed = Dcc::parse(&dcc).expect("dcc");
    assert_eq!(parsed.directions.len(), DCC_DIRECTIONS);
    let mut g = c.benchmark_group("sprites");
    g.throughput(Throughput::Elements(DC6_FRAMES as u64));
    g.bench_function("dc6_parse_32_frames_64x64", |b| {
        b.iter(|| black_box(Dc6::parse(black_box(&dc6)).expect("dc6")))
    });
    g.throughput(Throughput::Elements(DCC_DIRECTIONS as u64));
    g.bench_function("dcc_parse_8_directions_32x32", |b| {
        b.iter(|| black_box(Dcc::parse(black_box(&dcc)).expect("dcc")))
    });
    g.finish();
}

criterion_group!(benches, bench_mpq, bench_sprites);
criterion_main!(benches);

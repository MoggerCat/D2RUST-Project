// Spec: specs/render/capture.md (§5–§8, Test vectors)
//! The `scene` case kind on synthetic captures generated here (never game
//! pixels): the `frames-raw-1` reader, the PNG reader against the
//! recorder's own bytes, the comparison of §6 with its perturbation, and
//! the stability rule of §7 with its perturbation.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::capture::{self, FrameBody};
use super::capture_case::{self, SceneJob, SceneNotWired, SceneOutcome, SceneSource};
use super::case::{self, CaptureCheck, CaseKind, RawRef, SceneCase};
use super::*;

/// Fixture frame size (the reader takes any GDI size; the recorder refuses
/// all but 800 × 600 unless `--allow-any-size`).
const W: u32 = 32;
const H: u32 = 24;

/// A fresh, empty directory for one test.
fn tmp(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("d2rs-capture-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The fixture scene of a recorded state: one 4 × 4 sprite filled with
/// `player.cur`, drawn at the player's client position minus the unit
/// origin. A stand-in for the world-view rules, not a rule.
fn fixture_built(c: &capture::Captured) -> Built {
    let p = c
        .state
        .player
        .as_ref()
        .expect("fixture frames have a player");
    let [px, py] = p.client.expect("fixture player has a path");
    let [ox, oy] = c.state.unit_origin;
    let frames = vec![FrameImage {
        width: 4,
        height: 4,
        pixels: vec![p.cur as u8; 16],
    }];
    let items = vec![DrawItem::new(FrameId(0), px - ox, py - oy)];
    Built {
        items,
        frames,
        maps: MapTable::new(),
        palette: synthetic_palette(),
        view: Rect::new(0, 0, W, H),
    }
}

struct Fixture;

impl SceneSource for Fixture {
    fn scene(&mut self, job: &SceneJob<'_>) -> SceneOutcome {
        SceneOutcome::Built(Box::new(fixture_built(job.captured)))
    }
}

/// A GPU half that returns the CPU reference (the GPU's contract), so the
/// GPU branch of the comparison runs without an adapter.
struct CpuAsGpu;

impl GpuCompositor for CpuAsGpu {
    fn compose(&mut self, job: &GpuJob<'_>) -> GpuOutcome {
        let indices = scene::compose(job.items, job.frames, job.maps, job.view).unwrap();
        let rgba = scene::to_rgba(&indices, job.palette);
        GpuOutcome::Image { indices, rgba }
    }
}

/// One frame of a fixture recording.
#[derive(Clone)]
struct Shot {
    tick: i64,
    draw: u32,
    /// Player position (client) and `+0x44`.
    pos: (i32, i32),
    cur: i32,
    clear_counter: i32,
    /// Overrides the composed image (an unstable capture).
    pixels: Option<Vec<u8>>,
}

fn shot(tick: i64, draw: u32, pos: (i32, i32), cur: i32) -> Shot {
    Shot {
        tick,
        draw,
        pos,
        cur,
        clear_counter: 0,
        pixels: None,
    }
}

/// 8-bit palettized PNG, filter 0 on every row: the layout of
/// `record_frames.py` `png_bytes` (IHDR, PLTE, IDAT, IEND).
fn png(width: u32, height: u32, pixels: &[u8], palette: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width, height);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(palette.to_vec());
    enc.set_filter(png::Filter::NoFilter);
    let mut w = enc.write_header().unwrap();
    w.write_image_data(pixels).unwrap();
    w.finish().unwrap();
    out
}

fn frame_record(s: &Shot, pixels: &[u8], palette: &[u8]) -> Value {
    json!({
        "k": "frame", "f": s.tick, "video_type": 1, "w": W, "h": H,
        "index_sha256": capture::sha256_hex(pixels),
        "palette_sha256": capture::sha256_hex(palette),
        "draw": s.draw, "open_mode": 0, "shift_x": 0,
        "unit_origin": [0, 0], "shake": [0, 0, 0],
        "clear_counter": s.clear_counter, "res_mode": 2,
        "view_rect": [0, 0, W, H], "tile_origin": [0, 0],
        "player": {"type": 0, "mode": 1, "cur": s.cur,
                   "fixed": [s.pos.0 as u32 * 65536, s.pos.1 as u32 * 65536],
                   "client": [s.pos.0, s.pos.1]},
        "image": format!("frame-{:07}.png", s.draw),
    })
}

/// Writes `<dir>/<stamp>-frames.jsonl` (header, game, ticks, frames,
/// footer, as the recorder emits them) and the PNGs to `<images>/`, each
/// image composed by [`fixture_built`] unless the shot overrides it.
fn record(dir: &Path, stamp: &str, images: &Path, shots: &[Shot], refused: usize) -> PathBuf {
    std::fs::create_dir_all(images).unwrap();
    let palette = capture::palette_bytes(&synthetic_palette());
    let mut lines = vec![
        json!({"k": "header", "format": "frames-raw-1", "tool": "trace-recorder record_frames 0.1.0",
               "date": "2026-10-06", "game_exe_sha256": "0".repeat(64), "args": ["-w", "-ns"],
               "snap_every": 0}),
        json!({"k": "game", "g": "0x1234", "frame_before": 0}),
    ];
    let mut ticks = Vec::new();
    for _ in 0..refused {
        lines.push(
            json!({"k": "frame", "f": null, "video_type": 3, "w": 640, "h": 480,
                          "refused": "needs GDI (-w) at 800x600 (capture.md §1)"}),
        );
    }
    for s in shots {
        if ticks.last() != Some(&s.tick) {
            ticks.push(s.tick);
            lines.push(json!({"k": "tick", "f": s.tick}));
        }
        let rec = frame_record(s, &[], &palette);
        let c = capture::parse_raw(&raw_text(&[
            lines[0].clone(),
            rec,
            json!({"k": "footer", "ticks": 0, "counts": {"frame": 1}, "notes": []}),
        ]))
        .unwrap();
        let built = fixture_built(c.frames[0].captured().unwrap());
        let mut pixels =
            scene::compose(&built.items, &built.frames, &built.maps, built.view).unwrap();
        if s.clear_counter > 0 {
            pixels.fill(0);
        }
        if let Some(p) = &s.pixels {
            pixels = p.clone();
        }
        std::fs::write(
            images.join(format!("frame-{:07}.png", s.draw)),
            png(W, H, &pixels, &palette),
        )
        .unwrap();
        lines.push(frame_record(s, &pixels, &palette));
    }
    let n_frames = shots.len() + refused;
    lines.push(json!({"k": "footer", "ticks": ticks.len(),
                      "counts": {"game": 1, "tick": ticks.len(), "frame": n_frames},
                      "notes": ["time limit 5.0s reached"]}));
    let path = dir.join(format!("{stamp}-frames.jsonl"));
    std::fs::write(&path, raw_text(&lines)).unwrap();
    path
}

fn raw_text(lines: &[Value]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// A standard recording: two state keys seen three times each (stable),
/// a moving player in between.
fn standard(dir: &Path) -> (PathBuf, PathBuf) {
    let images = dir.join("captures");
    let shots = vec![
        shot(100, 500, (3, 4), 9),
        shot(101, 501, (3, 4), 9),
        shot(102, 502, (3, 4), 9),
        shot(103, 503, (7, 4), 9),
        shot(104, 504, (10, 8), 12),
        shot(105, 505, (10, 8), 12),
        shot(106, 506, (10, 8), 12),
    ];
    (record(dir, "20261006-120000", &images, &shots, 1), images)
}

fn scene_case(raw: &Path, images: &Path, check: CaptureCheck) -> SceneCase {
    SceneCase {
        raw: RawRef::Path(raw.display().to_string()),
        images: Some(images.display().to_string()),
        check,
        draws: Vec::new(),
    }
}

fn run(
    c: &SceneCase,
    n: usize,
    source: &mut dyn SceneSource,
    gpu: &mut dyn GpuCompositor,
) -> CaseReport {
    capture_case::run_capture("t", c, Path::new("/"), n, source, gpu)
}

fn has(r: &CaseReport, text: &str) -> bool {
    r.lines.iter().any(|l| l.contains(text))
}

// Covers: specs/render/capture.md §5
#[test]
fn reads_the_recorders_own_png_bytes() {
    // `png_bytes(5, 3, bytes((7 * i) & 0xFF for i in range(15)),
    // bytes(range(256)) * 3)` of record_frames.py (its selftest image),
    // byte for byte.
    let mut hex = String::from(concat!(
        "89504e470d0a1a0a0000000d49484452000000050000000308030000006ce835ca",
        "00000300504c5445"
    ));
    for _ in 0..3 {
        hex.extend((0..=255u8).map(|b| format!("{b:02x}")));
    }
    hex.push_str(concat!(
        "f650dfb70000001a49444154789c636060e713956150d632b4b06770f30d894e02",
        "0010e302e0fcdb99780000000049454e44ae426082"
    ));
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    let img = capture::decode_png(&bytes).unwrap();
    let pixels: Vec<u8> = (0..15u32).map(|i| (7 * i) as u8).collect();
    assert_eq!((img.width, img.height), (5, 3));
    assert_eq!(img.indices, pixels);
    assert_eq!(img.palette.len(), 768);
    // The hashes the recorder prints for the same image (hashlib).
    assert_eq!(
        capture::sha256_hex(&img.indices),
        "0e67afc0c726b49d7edd84e83cec79b68167f04a0972da170970e432f8ae7065"
    );
    assert_eq!(
        capture::sha256_hex(&img.palette),
        "f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363"
    );
}

// Covers: specs/render/capture.md §5
#[test]
fn png_reader_refuses_non_capture_pngs() {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 2, 2);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&[0; 4])
        .unwrap();
    assert!(capture::decode_png(&out)
        .unwrap_err()
        .contains("palettized"));
    // A 16-entry palette is not a capture's 256.
    let short = png(2, 2, &[0; 4], &[0; 48]);
    assert!(capture::decode_png(&short).unwrap_err().contains("768"));
}

// Covers: specs/render/capture.md §5
#[test]
fn raw_reader_reads_every_field() {
    let dir = tmp("fields");
    let (raw, _) = standard(&dir);
    let r = capture::read_raw(&raw).unwrap();
    assert_eq!(r.header.args, ["-w", "-ns"]);
    assert_eq!(r.games, ["0x1234"]);
    assert_eq!(r.ticks, (100..=106).collect::<Vec<_>>());
    assert_eq!(r.frames.len(), 8);
    assert_eq!(r.footer.notes, ["time limit 5.0s reached"]);
    assert!(matches!(&r.frames[0].body, FrameBody::Refused(_)));
    assert_eq!(r.frames[0].tick, None);
    let f = &r.frames[5];
    let c = f.captured().unwrap();
    assert_eq!(
        (f.tick, f.w, f.h, c.draw),
        (Some(104), W as i32, H as i32, 504)
    );
    let p = c.state.player.as_ref().unwrap();
    assert_eq!(
        (p.mode, p.cur, p.client, p.fixed),
        (1, 12, Some([10, 8]), Some([655360, 524288]))
    );
    assert_eq!(c.state.view_rect, Some([0, 0, W as i32, H as i32]));
    assert_eq!(c.image.as_deref(), Some("frame-0000504.png"));
}

// Covers: specs/render/capture.md §5
#[test]
fn raw_reader_is_strict() {
    let header = r#"{"k": "header", "format": "frames-raw-1", "tool": "t", "date": "d", "game_exe_sha256": "x", "args": []}"#;
    let footer = r#"{"k":"footer","ticks":0,"counts":{},"notes":[]}"#;
    let ok = format!("{header}\n{footer}\n");
    assert!(capture::parse_raw(&ok).is_ok());
    let cases = [
        (
            header.replace("frames-raw-1", "frames-raw-2"),
            "not supported",
        ),
        (
            format!("{header}\n{{\"k\":\"snap\"}}\n{footer}\n"),
            "unknown record kind",
        ),
        (format!("{header}\n"), "no footer"),
        (format!("{footer}\n"), "not the header"),
        (
            format!("{header}\n{footer}\n{footer}\n"),
            "after the footer",
        ),
        (
            format!("{header}\n{{\"k\":\"tick\",\"f\":1}}\n{footer}\n"),
            "counts has no tick",
        ),
        (
            format!(
                "{header}\n{}\n",
                r#"{"k":"footer","ticks":0,"counts":{"frame":2},"notes":[]}"#
            ),
            "counts.frame = 2",
        ),
        (
            format!(
                "{header}\n{}\n{footer}\n",
                r#"{"k":"frame","f":1,"video_type":1,"w":4,"h":4,"index_sha256":"ab"}"#
            ),
            "index_sha256: expected 64",
        ),
        (
            format!(
                "{header}\n{}\n{footer}\n",
                r#"{"k":"frame","f":1,"video_type":3,"w":4,"h":4}"#
            ),
            "video type 3",
        ),
    ];
    for (text, want) in cases {
        let e = capture::parse_raw(&text).unwrap_err().to_string();
        assert!(e.contains(want), "{want:?} not in {e:?}");
    }
    // A missing state field names the line and the key.
    let dir = tmp("strict");
    let (raw, _) = standard(&dir);
    let text = std::fs::read_to_string(&raw)
        .unwrap()
        .replace("\"shift_x\":0,", "");
    let e = capture::parse_raw(&text).unwrap_err().to_string();
    assert!(
        e.contains("shift_x: missing") && e.starts_with("line "),
        "{e}"
    );
}

// Covers: specs/render/capture.md §6
#[test]
fn compare_passes_when_the_reference_equals_the_capture() {
    let dir = tmp("pass");
    let (raw, images) = standard(&dir);
    let c = scene_case(&raw, &images, CaptureCheck::Compare);
    let r = run(&c, 0, &mut Fixture, &mut CpuAsGpu);
    assert_eq!(r.status, Status::Pass, "{:?}", r.lines);
    // Every captured frame but the first.
    assert!(has(&r, "6 frames selected"), "{:?}", r.lines);
    assert!(has(&r, "frames: 6 match, 0 differ"), "{:?}", r.lines);
    assert!(has(&r, "CPU: 0 of 768 bytes differ"));
    assert!(has(&r, "palette: 0 of 256 entries differ"));
    // Without a GPU compositor the case is incomplete, never a pass.
    let r = run(&c, 0, &mut Fixture, &mut NotWired);
    assert_eq!(r.status, Status::GpuNotWired);
    // Without a scene source nothing is compared.
    let r = run(&c, 0, &mut SceneNotWired, &mut CpuAsGpu);
    assert_eq!(r.status, Status::SceneNotWired, "{:?}", r.lines);
    let mut s = Summary::default();
    s.add(&r.status);
    assert_eq!(s.exit_code(), 2);
}

// Covers: specs/render/capture.md §6
#[test]
fn perturb_fails_every_frame_with_exactly_n() {
    let dir = tmp("perturb");
    let (raw, images) = standard(&dir);
    let c = scene_case(&raw, &images, CaptureCheck::Compare);
    for n in [1, 7, 64, (W * H) as usize] {
        let r = run(&c, n, &mut Fixture, &mut CpuAsGpu);
        assert!(matches!(r.status, Status::Fail(_)), "{n}: {:?}", r);
        assert!(
            has(&r, &format!("CPU: {n} of {} bytes differ", W * H)),
            "{n}: {:?}",
            r.lines
        );
        assert!(has(&r, &format!("GPU indices: {n} of")), "{n}");
        assert!(
            has(&r, &format!("GPU: {n} of {} pixels differ", W * H)),
            "{n}"
        );
        assert!(has(&r, "frames: 0 match, 6 differ"), "{n}: {:?}", r.lines);
    }
    let r = run(&c, (W * H) as usize + 1, &mut Fixture, &mut CpuAsGpu);
    assert!(matches!(r.status, Status::Error(_)));
}

// Covers: specs/render/capture.md §6
#[test]
fn a_different_scene_or_palette_fails_with_its_first_difference() {
    struct Shifted;
    impl SceneSource for Shifted {
        fn scene(&mut self, job: &SceneJob<'_>) -> SceneOutcome {
            let mut b = fixture_built(job.captured);
            b.items[0].x += 1;
            SceneOutcome::Built(Box::new(b))
        }
    }
    struct OtherPalette;
    impl SceneSource for OtherPalette {
        fn scene(&mut self, job: &SceneJob<'_>) -> SceneOutcome {
            let mut b = fixture_built(job.captured);
            b.palette.colors[200].g ^= 1;
            SceneOutcome::Built(Box::new(b))
        }
    }
    let dir = tmp("differ");
    let (raw, images) = standard(&dir);
    let mut c = scene_case(&raw, &images, CaptureCheck::Compare);
    c.draws = vec![501];
    let r = run(&c, 0, &mut Shifted, &mut NotWired);
    // A 4-wide sprite moved right by one: columns 3 and 7 change, 4 rows.
    assert!(matches!(r.status, Status::Fail(_)));
    assert!(
        has(
            &r,
            "CPU: 8 of 768 bytes differ, first at (3, 4): expected 9, got 0"
        ),
        "{:?}",
        r.lines
    );
    let r = run(&c, 0, &mut OtherPalette, &mut NotWired);
    assert!(matches!(r.status, Status::Fail(_)));
    assert!(
        has(&r, "palette: 1 of 256 entries differ, first index 200"),
        "{:?}",
        r.lines
    );
    // A scene of another size is an error, not a count.
    struct Small;
    impl SceneSource for Small {
        fn scene(&mut self, job: &SceneJob<'_>) -> SceneOutcome {
            let mut b = fixture_built(job.captured);
            b.view = Rect::new(0, 0, W - 1, H);
            SceneOutcome::Built(Box::new(b))
        }
    }
    assert!(matches!(
        run(&c, 0, &mut Small, &mut NotWired).status,
        Status::Error(_)
    ));
}

// Covers: specs/render/capture.md §6
#[test]
fn a_png_that_disagrees_with_its_record_is_an_error() {
    let dir = tmp("integrity");
    let (raw, images) = standard(&dir);
    let palette = capture::palette_bytes(&synthetic_palette());
    let mut pixels = vec![0u8; (W * H) as usize];
    pixels[5] = 1;
    std::fs::write(
        images.join("frame-0000502.png"),
        png(W, H, &pixels, &palette),
    )
    .unwrap();
    let c = scene_case(&raw, &images, CaptureCheck::Compare);
    // Checked before the scene source is asked.
    let r = run(&c, 0, &mut SceneNotWired, &mut NotWired);
    match &r.status {
        Status::Error(e) => assert!(e.contains("draw 502") && e.contains("index hash"), "{e}"),
        s => panic!("{s:?}"),
    }
    // A selected draw that was not captured is an error.
    let mut c = scene_case(&raw, &images, CaptureCheck::Compare);
    c.draws = vec![999];
    assert!(matches!(
        run(&c, 0, &mut Fixture, &mut NotWired).status,
        Status::Error(_)
    ));
}

// Covers: specs/render/capture.md §6
#[test]
fn the_source_gets_the_previous_draw_when_it_was_captured() {
    struct Seen(Vec<(u32, Option<u8>)>);
    impl SceneSource for Seen {
        fn scene(&mut self, job: &SceneJob<'_>) -> SceneOutcome {
            self.0.push((
                job.captured.draw,
                job.previous.map(|p| p.indices[4 * W as usize + 3]),
            ));
            SceneOutcome::NotWired
        }
    }
    let dir = tmp("previous");
    let (raw, images) = standard(&dir);
    let mut c = scene_case(&raw, &images, CaptureCheck::Compare);
    c.draws = vec![501, 503, 506];
    let mut seen = Seen(Vec::new());
    run(&c, 0, &mut seen, &mut NotWired);
    // Pixel (3, 4) is the sprite of draws 500–502 (index 9), outside the
    // sprite at (7, 4) of 503 and at (10, 8) of 504–506.
    assert_eq!(seen.0, [(501, Some(9)), (503, Some(9)), (506, Some(0))]);
}

// Covers: specs/render/capture.md §4
#[test]
fn frames_sharing_a_tick_are_flagged() {
    let dir = tmp("ticks");
    let images = dir.join("captures");
    let shots = [
        shot(7, 1, (1, 1), 3),
        shot(7, 2, (1, 1), 3),
        shot(8, 3, (1, 1), 3),
    ];
    let raw = record(&dir, "20261006-130000", &images, &shots, 0);
    let r = run(
        &scene_case(&raw, &images, CaptureCheck::Compare),
        0,
        &mut Fixture,
        &mut CpuAsGpu,
    );
    assert!(
        has(&r, "flag: 1 ticks carry more than one frame"),
        "{:?}",
        r.lines
    );
    assert!(has(&r, "tick 7: draws [1, 2]"));
}

// Covers: specs/render/capture.md §7
#[test]
fn stability_holds_on_a_stable_recording() {
    let dir = tmp("stable");
    let (raw, images) = standard(&dir);
    let c = scene_case(&raw, &images, CaptureCheck::Stability);
    let r = run(&c, 0, &mut SceneNotWired, &mut NotWired);
    assert_eq!(r.status, Status::Pass, "{:?}", r.lines);
    assert!(has(&r, "re-hash: 0 of 7 frames differ"));
    assert!(
        has(
            &r,
            "stability: 3 state groups, 2 seen at least twice, 0 with differing frames"
        ),
        "{:?}",
        r.lines
    );
}

// Covers: specs/render/capture.md §7
#[test]
fn stability_perturb_changes_exactly_n_rehashes() {
    let dir = tmp("stable-perturb");
    let (raw, images) = standard(&dir);
    let c = scene_case(&raw, &images, CaptureCheck::Stability);
    for n in 1..=7 {
        let r = run(&c, n, &mut SceneNotWired, &mut NotWired);
        assert!(matches!(r.status, Status::Fail(_)), "{n}");
        assert!(
            has(&r, &format!("re-hash: {n} of 7 frames differ")),
            "{n}: {:?}",
            r.lines
        );
    }
    assert!(matches!(
        run(&c, 8, &mut SceneNotWired, &mut NotWired).status,
        Status::Error(_)
    ));
    // One byte of one saved PNG changed on disk: exactly that frame.
    let palette = capture::palette_bytes(&synthetic_palette());
    let img =
        capture::decode_png(&std::fs::read(images.join("frame-0000505.png")).unwrap()).unwrap();
    let mut px = img.indices;
    px[100] ^= 1;
    std::fs::write(images.join("frame-0000505.png"), png(W, H, &px, &palette)).unwrap();
    let r = run(&c, 0, &mut SceneNotWired, &mut NotWired);
    assert!(has(&r, "re-hash: 1 of 7 frames differ"), "{:?}", r.lines);
    assert!(has(&r, "draw 505: PNG index hash differs"));
}

// Covers: specs/render/capture.md §7
#[test]
fn stability_fails_on_differing_frames_and_too_few_repeats() {
    let dir = tmp("unstable");
    let images = dir.join("captures");
    let mut odd = shot(102, 502, (3, 4), 9);
    let mut px = vec![0u8; (W * H) as usize];
    px[0] = 77;
    odd.pixels = Some(px);
    let shots = vec![
        shot(100, 500, (3, 4), 9),
        shot(101, 501, (3, 4), 9),
        odd,
        shot(104, 504, (10, 8), 12),
        shot(105, 505, (10, 8), 12),
    ];
    let raw = record(&dir, "20261006-140000", &images, &shots, 0);
    let r = run(
        &scene_case(&raw, &images, CaptureCheck::Stability),
        0,
        &mut SceneNotWired,
        &mut NotWired,
    );
    assert!(
        matches!(&r.status, Status::Fail(w) if w.contains("1 state groups with differing frames")),
        "{:?}",
        r
    );
    assert!(
        has(&r, "3 frames, 2 different images, draws [500, 501, 502]"),
        "{:?}",
        r.lines
    );

    // Only one key repeats: §7 needs two.
    let dir = tmp("few");
    let images = dir.join("captures");
    let shots = [
        shot(1, 1, (3, 4), 9),
        shot(2, 2, (3, 4), 9),
        shot(3, 3, (5, 4), 9),
    ];
    let raw = record(&dir, "20261006-150000", &images, &shots, 0);
    let r = run(
        &scene_case(&raw, &images, CaptureCheck::Stability),
        0,
        &mut SceneNotWired,
        &mut NotWired,
    );
    assert!(
        matches!(&r.status, Status::Fail(w) if w.contains("1 state keys seen at least twice")),
        "{:?}",
        r
    );
}

// Covers: specs/render/capture.md §edge-cases-original-bugs
#[test]
fn a_cleared_frame_must_be_all_index_0() {
    let dir = tmp("clear");
    let images = dir.join("captures");
    let mut cleared = shot(3, 3, (3, 4), 9);
    cleared.clear_counter = 1;
    let mut bad = shot(4, 4, (3, 4), 9);
    bad.clear_counter = 1;
    bad.pixels = Some(vec![1u8; (W * H) as usize]);
    let shots = [shot(1, 1, (3, 4), 9), shot(2, 2, (3, 4), 9), cleared, bad];
    let raw = record(&dir, "20261006-160000", &images, &shots, 0);
    let r = run(
        &scene_case(&raw, &images, CaptureCheck::Stability),
        0,
        &mut SceneNotWired,
        &mut NotWired,
    );
    assert!(
        has(
            &r,
            "clear: 1 frames with clear_counter > 0 are not all index 0, draws [4]"
        ),
        "{:?}",
        r.lines
    );
}

// Covers: specs/render/capture.md §8
#[test]
fn latest_raw_and_its_image_directory_resolve() {
    let root = tmp("latest");
    let raw_dir = root.join("traces").join("raw");
    std::fs::create_dir_all(&raw_dir).unwrap();
    for name in [
        "20261006-090000-frames.jsonl",
        "20261007-080000-frames.jsonl",
        "20261008-000000-tick.jsonl",
    ] {
        std::fs::write(raw_dir.join(name), "").unwrap();
    }
    let c = SceneCase {
        raw: RawRef::Latest,
        images: None,
        check: CaptureCheck::Stability,
        draws: Vec::new(),
    };
    let (raw, images) = capture_case::resolve(&root, &c).unwrap();
    assert_eq!(raw, raw_dir.join("20261007-080000-frames.jsonl"));
    assert_eq!(
        images,
        root.join("game").join("captures").join("20261007-080000")
    );
    // No recording: an error, never a pass.
    let empty = tmp("latest-empty");
    std::fs::create_dir_all(empty.join("traces").join("raw")).unwrap();
    assert!(capture_case::resolve(&empty, &c).is_err());
    let r = capture_case::run_capture("t", &c, &empty, 0, &mut SceneNotWired, &mut NotWired);
    assert!(matches!(r.status, Status::Error(_)));
}

// Covers: specs/render/capture.md §8
#[test]
fn the_capture_cases_parse() {
    let cases = load_dir(&capture_case::capture_case_dir()).unwrap();
    let names: Vec<_> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "camera-0001",
            "composition-0001",
            "placement-0001",
            "stability-0001"
        ]
    );
    for c in &cases {
        let CaseKind::Scene(s) = &c.kind else {
            panic!("{} is not a scene case", c.name)
        };
        assert_eq!(s.raw, RawRef::Latest);
        let want = if c.name == "stability-0001" {
            CaptureCheck::Stability
        } else {
            CaptureCheck::Compare
        };
        assert_eq!(s.check, want, "{}", c.name);
    }
}

#[test]
fn scene_case_files_are_strict() {
    let parse = |body: &str| case::parse("x", &format!("version = 1\nkind = \"scene\"\n{body}"));
    let ok =
        parse("raw = \"latest\"\ncheck = \"compare\"\ndraws = [3, 4]\nimages = 'a/b'").unwrap();
    assert_eq!(
        ok.kind,
        CaseKind::Scene(SceneCase {
            raw: RawRef::Latest,
            images: Some("a/b".into()),
            check: CaptureCheck::Compare,
            draws: vec![3, 4],
        })
    );
    for (body, at) in [
        ("check = \"compare\"", "raw"),
        ("raw = \"latest\"", "check"),
        ("raw = \"latest\"\ncheck = \"look\"", "check"),
        (
            "raw = \"latest\"\ncheck = \"stability\"\ndraws = [1]",
            "draws",
        ),
        (
            "raw = \"latest\"\ncheck = \"compare\"\ndraws = [1, 1]",
            "draws[1]",
        ),
        ("raw = \"latest\"\ncheck = \"compare\"\ndraws = []", "draws"),
        (
            "raw = \"latest\"\ncheck = \"compare\"\nview = [0, 0, 1, 1]",
            "view",
        ),
        ("raw = \"\"\ncheck = \"compare\"", "raw"),
    ] {
        assert_eq!(parse(body).unwrap_err().at, at, "{body}");
    }
}

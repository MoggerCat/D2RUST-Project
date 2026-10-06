// Spec: specs/client/render-pipeline.md (A10, Test vectors)
//! The CPU half of every synthetic case file, the M08 perturbation per
//! case, strict case parsing, and the GPU seam's statuses.

use super::case::{self, CaseErrorKind, CaseKind};
use super::*;

fn cases() -> Vec<Case> {
    load_dir(&default_case_dir()).expect("render-cases load")
}

fn synthetic(c: &Case) -> Option<&case::Synthetic> {
    match &c.kind {
        CaseKind::Synthetic(s) => Some(s),
        CaseKind::Map(_) => None,
    }
}

// Covers: specs/client/render-pipeline.md §a10-verify-harness-extension, §a9-gpu-compute-compositor
#[test]
fn every_synthetic_case_passes_its_cpu_half() {
    let cases = cases();
    let synth: Vec<_> = cases
        .iter()
        .filter_map(|c| synthetic(c).map(|s| (c, s)))
        .collect();
    assert!(
        synth.len() >= 8,
        "one case per Test vectors row with a CPU half"
    );
    for (c, s) in synth {
        let r = run_synthetic(&c.name, s, 0, &mut NotWired);
        assert_eq!(r.status, Status::GpuNotWired, "{}: {:?}", c.name, r.lines);
        assert!(
            !s.expects.is_empty(),
            "{}: a case states its expected pixels",
            c.name
        );
    }
}

// Covers: specs/client/render-pipeline.md §a10-verify-harness-extension
#[test]
fn perturb_fails_every_synthetic_case_with_exactly_n() {
    for c in cases() {
        let Some(s) = synthetic(&c) else { continue };
        for n in [1, 7, 64] {
            let (_, _, cpu, _) = run_cpu(s, n).unwrap();
            assert_eq!(cpu.binned.mismatched, n, "{} --perturb {n}", c.name);
            let r = run_synthetic(&c.name, s, n, &mut NotWired);
            assert!(matches!(r.status, Status::Fail(_)), "{}: {:?}", c.name, r);
        }
    }
}

#[test]
fn perturb_counts_exactly_and_rejects_too_many() {
    let view = Rect::new(-2, 3, 7, 5);
    let clean = vec![10u8; 7 * 5 * 4];
    for n in 0..=35 {
        let mut img = clean.clone();
        perturb(&mut img, n).unwrap();
        assert_eq!(compare(&img, &clean, view).unwrap().mismatched, n);
    }
    assert!(perturb(&mut clean.clone(), 36).is_err());
}

#[test]
fn compare_reports_count_and_first_mismatch_in_screen_coords() {
    let view = Rect::new(-2, 3, 4, 2);
    let expected = vec![0u8; 4 * 2 * 4];
    let mut actual = expected.clone();
    actual[5 * 4 + 1] = 9; // pixel (1, 1) of the view
    actual[7 * 4] = 1;
    let m = compare(&actual, &expected, view).unwrap();
    assert_eq!(m.mismatched, 2);
    let f = m.first.unwrap();
    assert_eq!(
        (f.x, f.y, f.expected, f.actual),
        (-1, 4, [0; 4], [0, 9, 0, 0])
    );
    assert!(compare(&actual[4..], &expected, view).is_err());
}

/// A GPU stand-in that returns the CPU image (or a corrupted one).
struct Echo {
    corrupt: usize,
}

impl GpuCompositor for Echo {
    fn compose(&mut self, job: &GpuJob<'_>) -> GpuOutcome {
        let indexed = scene::compose_binned(job.items, job.bins, job.frames, job.maps, job.view);
        let mut rgba = scene::to_rgba(&indexed.unwrap(), job.palette);
        perturb(&mut rgba, self.corrupt).unwrap();
        GpuOutcome::Image(rgba)
    }
}

struct Broken;

impl GpuCompositor for Broken {
    fn compose(&mut self, _job: &GpuJob<'_>) -> GpuOutcome {
        GpuOutcome::Error("no adapter".into())
    }
}

// Covers: specs/client/render-pipeline.md §a10-verify-harness-extension
#[test]
fn gpu_seam_statuses() {
    let cases = cases();
    let c = cases.iter().find(|c| c.name == "synth-opaque").unwrap();
    let s = synthetic(c).unwrap();
    let r = run_synthetic(&c.name, s, 0, &mut Echo { corrupt: 0 });
    assert_eq!(r.status, Status::Pass);
    let r = run_synthetic(&c.name, s, 0, &mut Echo { corrupt: 3 });
    assert_eq!(r.status, Status::Fail("GPU half".into()));
    assert!(r
        .lines
        .iter()
        .any(|l| l.starts_with("GPU: 3 of 1024 pixels differ")));
    let r = run_synthetic(&c.name, s, 0, &mut Broken);
    assert!(matches!(r.status, Status::Error(_)));
    // Not wired is never a pass, and it changes the exit code.
    let mut sum = Summary::default();
    sum.add(&Status::Pass);
    assert_eq!(sum.exit_code(), 0);
    sum.add(&Status::GpuNotWired);
    assert_eq!(sum.exit_code(), 2);
    sum.add(&Status::Fail("x".into()));
    assert_eq!(sum.exit_code(), 1);
}

#[test]
fn map_case_is_todays_default() {
    let cases = cases();
    let c = cases.iter().find(|c| c.name == "map").unwrap();
    assert_eq!(
        c.kind,
        CaseKind::Map(case::MapCase {
            ds1: r"data\global\tiles\ACT1\TOWN\townN1.ds1".into(),
            wall_base: 80,
            view: None,
        })
    );
}

#[test]
fn map_helpers() {
    assert_eq!(
        map::file_stem(r"data\global\tiles\ACT1\TOWN\townN1.ds1"),
        "townN1"
    );
    let b = |x0, y0, x1, y1| crate::map::Bounds { x0, y0, x1, y1 };
    let v = map::full_view(b(-3, -5, 10, 8));
    assert_eq!((v.left, v.top, v.width, v.height), (-3, -5, 14, 14));
    let v = map::full_view(b(0, 0, 20000, 100));
    assert_eq!((v.width, v.left), (map::MAX_TEXTURE_SIDE, 10000 - 4096));
}

const MINIMAL: &str = "version = 1\nkind = \"synthetic\"\n\
    [[frame]]\nwidth = 1\nheight = 1\npixels = [3]\n\
    [[item]]\nframe = 0\nx = 0\ny = 0\n";

fn parse_err(text: &str) -> case::CaseError {
    case::parse("t", text).expect_err(text)
}

// Covers: specs/client/render-pipeline.md §a10-verify-harness-extension
#[test]
fn case_parsing_is_strict() {
    assert!(case::parse("t", MINIMAL).is_ok());
    let e = parse_err(&MINIMAL.replace("version = 1\n", ""));
    assert_eq!(e.kind, CaseErrorKind::MissingVersion);
    let e = parse_err(&MINIMAL.replace("version = 1", "version = 2"));
    assert_eq!(e.kind, CaseErrorKind::UnsupportedVersion(2));
    let e = parse_err(&MINIMAL.replace("version = 1", "version = \"1\""));
    assert_eq!(e.kind, CaseErrorKind::WrongType("an integer"));
    let e = parse_err(&MINIMAL.replace("synthetic", "sprite"));
    assert_eq!(e.kind, CaseErrorKind::UnknownValue("sprite".into()));
    let e = parse_err(&format!("{MINIMAL}colour = 3\n"));
    assert_eq!(
        (e.at.as_str(), e.kind),
        ("item[0].colour", CaseErrorKind::UnknownKey)
    );
    let e = parse_err(&format!("ds1 = 'x'\n{MINIMAL}"));
    assert_eq!((e.at.as_str(), e.kind), ("ds1", CaseErrorKind::UnknownKey));
    let e = parse_err(&MINIMAL.replace("pixels = [3]", "pixels = [3, 4]"));
    assert_eq!(e.at, "frame[0].pixels");
    let e = parse_err(&MINIMAL.replace("pixels = [3]", "pixels = [256]"));
    assert_eq!(
        (e.at.as_str(), e.kind),
        (
            "frame[0].pixels[0]",
            CaseErrorKind::Range {
                value: 256,
                min: 0,
                max: 255
            }
        )
    );
    let e = parse_err(&MINIMAL.replace("pixels = [3]", "pixels = [3]\nfill = 3"));
    assert_eq!(e.at, "frame[0]");
    let e = parse_err(&MINIMAL.replace("x = 0\n", ""));
    assert_eq!(
        (e.at.as_str(), e.kind),
        ("item[0].x", CaseErrorKind::Missing)
    );
    let e = parse_err(&format!("view = [0, 0, 0, 4]\n{MINIMAL}"));
    assert_eq!(e.at, "view[2]");
    let e = parse_err(&format!("view = [0, 0, 4]\n{MINIMAL}"));
    assert_eq!(e.at, "view");
    let e = parse_err(&format!(
        "{MINIMAL}[[map]]\nbase = \"identity\"\nset = [[1, 2], [1, 3]]\n"
    ));
    assert_eq!(e.at, "map[0].set[1]");
    let e = parse_err(&format!("{MINIMAL}[[table]]\nrule = \"mul\"\n"));
    assert_eq!(e.kind, CaseErrorKind::UnknownValue("mul".into()));
    let e = parse_err("version = 1\nkind = \"synthetic\"\n");
    assert_eq!(e.at, "item");
    let e = parse_err("version = 1\nkind = \"map\"\nds1 = 'a'\n");
    assert_eq!(
        (e.at.as_str(), e.kind),
        ("wall_base", CaseErrorKind::Missing)
    );
    let e = parse_err("version = 1\nkind = \"map\"\nds1 = 'a'\nwall_base = 1\nframe = 1\n");
    assert_eq!(
        (e.at.as_str(), e.kind),
        ("frame", CaseErrorKind::UnknownKey)
    );
    let e = parse_err("version = 1\nkind = [");
    assert!(matches!(e.kind, CaseErrorKind::Toml(_)), "{e}");
}

#[test]
fn build_rejects_undefined_references_and_scene_errors() {
    let with = |extra: &str| {
        let c = case::parse("t", &format!("{MINIMAL}{extra}")).unwrap();
        let CaseKind::Synthetic(s) = c.kind else {
            unreachable!()
        };
        s
    };
    let s = with("shade = [0]\n");
    assert!(matches!(
        build(&s),
        Err(BuildError::Undefined { what: "map", .. })
    ));
    let s = with("table = 0\n");
    assert!(matches!(
        build(&s),
        Err(BuildError::Undefined { what: "table", .. })
    ));
    let s = with("[[item]]\nframe = 1\nx = 0\ny = 0\n");
    assert!(matches!(
        build(&s),
        Err(BuildError::Undefined {
            item: 1,
            what: "frame",
            ..
        })
    ));
    let s = with("key = [16, 0, 0, 0]\n");
    assert!(matches!(build(&s), Err(BuildError::Scene { item: 0, .. })));
    let s = with("key = [0, 0, 0, 256]\n");
    assert!(matches!(build(&s), Err(BuildError::Scene { item: 0, .. })));
    let s = with(
        "[[map]]\nbase = \"zero\"\n[[item]]\nframe = 0\nx = 0\ny = 0\nshade = [0, 0, 0, 0, 0]\n",
    );
    assert!(matches!(build(&s), Err(BuildError::Scene { item: 1, .. })));
    // flip_x is reserved: the compositor rejects it, so the case errors.
    let s = with("flip_x = true\n");
    let r = run_synthetic("t", &s, 0, &mut NotWired);
    assert!(matches!(r.status, Status::Error(_)), "{r:?}");
    // An expectation outside the view fails the CPU half.
    let s = with("[[expect]]\nx = 900\ny = 0\nindex = 0\n");
    let r = run_synthetic("t", &s, 0, &mut NotWired);
    assert_eq!(r.status, Status::Fail("CPU half".into()));
}

#[test]
fn a_wrong_expectation_fails_the_cpu_half() {
    let c = case::parse(
        "t",
        &format!("{MINIMAL}[[expect]]\nx = 0\ny = 0\nindex = 4\n"),
    )
    .unwrap();
    let CaseKind::Synthetic(s) = c.kind else {
        unreachable!()
    };
    let r = run_synthetic("t", &s, 0, &mut NotWired);
    assert_eq!(r.status, Status::Fail("CPU half".into()));
    assert!(
        r.lines.iter().any(|l| l == "CPU: expect (0, 0) = 4, got 3"),
        "{r:?}"
    );
}

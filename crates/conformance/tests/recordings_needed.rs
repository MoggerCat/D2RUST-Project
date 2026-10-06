//! The recordings list (`conformance::needs`): every recorder script,
//! test file and test it names exists, every harness with a file-driven
//! test is listed, and the report prints (run with `--nocapture` to see it,
//! or `cargo run -p conformance --bin recordings-needed`).

use std::path::PathBuf;

use conformance::needs::{report, NEEDED};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_named_script_and_test_exists() {
    for n in NEEDED {
        assert_eq!(n.record.is_empty(), n.blocked.is_some(), "{}", n.id);
        for c in n.record {
            let script = c
                .split_whitespace()
                .find(|w| w.ends_with(".py"))
                .unwrap_or_else(|| panic!("{}: no script in {c:?}", n.id));
            assert!(root().join(script).is_file(), "{}: {script} missing", n.id);
            assert!(c.starts_with("py tools/trace-recorder/"), "{}: {c}", n.id);
        }
        let test = root()
            .join("crates/conformance/tests")
            .join(format!("{}.rs", n.test_file));
        let text = std::fs::read_to_string(&test)
            .unwrap_or_else(|e| panic!("{}: {}: {e}", n.id, test.display()));
        assert!(
            text.contains(&format!("fn {}()", n.test)),
            "{}: no fn {} in {}",
            n.id,
            n.test,
            test.display()
        );
    }
}

#[test]
fn every_file_driven_test_is_listed() {
    // Each ignored test in tests/*.rs that reads traces/raw/ has an entry.
    let dir = root().join("crates/conformance/tests");
    let mut listed = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let path = e.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if !l.contains("#[ignore = \"needs traces/raw/") {
                continue;
            }
            let f = lines[i + 1..]
                .iter()
                .find_map(|l| l.trim().strip_prefix("fn "))
                .and_then(|l| l.split('(').next())
                .unwrap();
            assert!(
                NEEDED.iter().any(|n| n.test == f),
                "{}: {f} not in conformance::needs::NEEDED",
                path.display()
            );
            listed += 1;
        }
    }
    assert!(listed >= 5, "{listed}");
}

#[test]
fn report_lists_every_entry() {
    let r = report();
    println!("{r}");
    for n in NEEDED {
        assert!(r.contains(&format!("[{}]", n.id)), "{}", n.id);
    }
    assert!(r.contains("BLOCKED"));
}

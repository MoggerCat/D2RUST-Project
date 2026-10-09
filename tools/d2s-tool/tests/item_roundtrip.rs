// Spec: specs/formats/d2s.md §8.1 (player item list entries); specs/items/bitstream.md §5
//! The items `d2s-tool new` writes are what 1.14d writes when it saves the
//! same character again: every `facts/saves/*.tsv` (`d2s-roundtrip-1`,
//! `tools/cloud-game/d2s_roundtrip.py`) names the `new` arguments and, per
//! item, the entry bytes 1.14d saved; a row marked `equal` must come out of
//! the tool byte for byte. Rows marked `differs` record a game-side change
//! (a flag the game sets) and are not checked.
//!
//! `D2_GAME_DIR=<install> cargo test -p d2s-tool --test item_roundtrip -- --ignored`

use std::collections::BTreeMap;
use std::path::PathBuf;

fn facts() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../facts/saves");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|x| x == "tsv"));
    v.sort();
    v
}

/// (key, bytes hex) of every item entry `dump` prints with the item debug
/// lines on.
fn dumped(save: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let args = vec![
        "dump".to_owned(),
        save.display().to_string(),
        "--items".to_owned(),
    ];
    d2s_tool::cli::run(&args, &mut out).expect("dump");
    let text = String::from_utf8(out).unwrap();
    let mut res = Vec::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        let t = line.trim_start();
        if line.starts_with("  [") {
            // "  [i] code  mode M <loc> | ..."
            let rest = line.split_once("] ").map(|x| x.1).unwrap_or("");
            let head = rest.split(" | ").next().unwrap_or("");
            let mut w = head.split_whitespace();
            let code = w.next().unwrap_or("").to_owned();
            let tail: Vec<&str> = w.collect();
            cur = Some(format!("{code} {}", tail.join(" ")));
        } else if let Some(hex) = t.strip_prefix("bytes ") {
            if let Some(k) = cur.take() {
                res.push((k, hex.to_owned()));
            }
        }
    }
    res
}

#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn tool_items_equal_the_game_resave() {
    if std::env::var_os("D2_GAME_DIR").is_none() {
        eprintln!("D2_GAME_DIR is not set: skipping");
        return;
    }
    let files = facts();
    assert!(!files.is_empty(), "no facts/saves/*.tsv");
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let args = text
            .lines()
            .find_map(|l| l.strip_prefix("# d2s-tool new "))
            .unwrap_or_else(|| panic!("{}: no `# d2s-tool new` line", f.display()));
        let game: BTreeMap<String, (String, String)> = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("item\t"))
            .map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c[0].to_owned(), (c[2].to_owned(), c[3].to_owned()))
            })
            .collect();
        let out = std::env::temp_dir().join(format!(
            "d2s-roundtrip-{}-{}.d2s",
            std::process::id(),
            f.file_stem().unwrap().to_string_lossy()
        ));
        let mut a = vec!["new".to_owned()];
        a.extend(args.split_whitespace().map(str::to_owned));
        a.extend(["-o".to_owned(), out.display().to_string()]);
        d2s_tool::cli::run(&a, &mut Vec::new()).expect("new");
        let ours = dumped(&out);
        let _ = std::fs::remove_file(&out);
        let mut checked = 0;
        for (k, hex) in &ours {
            let (g, result) = game
                .get(k)
                .unwrap_or_else(|| panic!("{}: item {k} not in the facts", f.display()));
            if result == "equal" {
                assert_eq!(hex, g, "{}: item {k}", f.display());
                checked += 1;
            }
        }
        assert!(checked > 0, "{}: no equal row checked", f.display());
        eprintln!("{}: {checked} items equal the 1.14d re-save", f.display());
    }
}

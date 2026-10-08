// Spec: specs/client/ui.md
//! §Test vectors (controls rows) and the §A6 rules, on inline files.

use super::*;

fn err(text: &str) -> ControlsError {
    parse(text).expect_err("file must be rejected")
}

fn ok(text: &str) -> Bindings {
    parse(text).expect("file must parse").1
}

const HEAD: &str = "version = 1\npreset = \"dev\"\n";

// --- §Test vectors ---------------------------------------------------------

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 text
#[test]
fn missing_version_is_line_1() {
    let e = err("preset = \"dev\"\n\n[bindings]\ntoggle_inventory = [\"B\"]\n");
    assert_eq!(e.kind, ErrorKind::MissingVersion);
    assert_eq!(e.line, Some(1));
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r5
#[test]
fn version_2_is_unsupported() {
    let e = err("# header\nversion = 2\npreset = \"dev\"\n[future]\nx = 1\n");
    assert_eq!(e.kind, ErrorKind::UnsupportedVersion(2));
    assert_eq!(e.line, Some(2));
    assert!(e.to_string().contains("unsupported version 2"), "{e}");
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 text
#[test]
fn unknown_action_names_it_and_its_line() {
    let e = err(&format!("{HEAD}\n[bindings]\nfoo = [\"A\"]\n"));
    assert_eq!(e.kind, ErrorKind::UnknownAction("foo".into()));
    assert_eq!(e.line, Some(5));
    assert!(e.to_string().contains("`foo`"), "{e}");
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r2
#[test]
fn two_world_actions_on_i_name_both() {
    // `toggle_inventory` is on `I` in the dev preset.
    let e = err(&format!("{HEAD}\n[bindings]\ntoggle_character = [\"I\"]\n"));
    assert_eq!(
        e.kind,
        ErrorKind::Clash(Clash {
            context: Context::World,
            key: Key::I,
            first: Action::ToggleInventory,
            second: Action::ToggleCharacter,
        })
    );
    assert_eq!(e.line, Some(5));
    let msg = e.to_string();
    assert!(msg.contains("toggle_inventory") && msg.contains("toggle_character"));
    assert!(msg.contains("`world`"), "{msg}");
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r1
#[test]
fn dev_override_moves_inventory_to_b_only() {
    let b = ok(&format!("{HEAD}\n[bindings]\ntoggle_inventory = [\"B\"]\n"));
    assert_eq!(b.inputs(Action::ToggleInventory), &[Key::B]);
    assert_eq!(
        b.action_for(Context::World, Key::B),
        Some(Action::ToggleInventory)
    );
    assert_eq!(b.action_for(Context::World, Key::I), None);
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r1
#[test]
fn unbind_clears_an_action_also_in_bindings() {
    let b = ok(&format!(
        "{HEAD}\n[bindings]\ntoggle_automap_fade = [\"G\"]\n\n[unbind]\nlist = [\"toggle_automap_fade\"]\n"
    ));
    assert_eq!(b.inputs(Action::ToggleAutomapFade), &[] as &[Key]);
    assert_eq!(b.action_for(Context::World, Key::G), None);
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r4
#[test]
fn write_then_parse_keeps_bindings_in_enum_order() {
    // Entries deliberately out of enum order.
    let text = format!(
        "{HEAD}\n[bindings]\nskill_slot_2 = [\"2\", \"F2\"]\ntoggle_inventory = [\"B\"]\nmove_attack = [\"MouseLeft\", \"Space\"]\n\n[unbind]\nlist = [\"clear_screen\", \"belt_slot_1\", \"belt_slot_2\"]\n"
    );
    let (file, b) = parse(&text).unwrap();
    let out = write(&file);
    let (file2, b2) = parse(&out).unwrap();
    assert_eq!(b, b2);
    assert_eq!(write(&file2), out, "writer is a fixed point");
    // Keys appear in Action enum order.
    let pos = |name: &str| out.find(&format!("{name} =")).unwrap();
    assert!(pos("move_attack") < pos("toggle_inventory"));
    assert!(pos("toggle_inventory") < pos("skill_slot_2"));
    let unbind = out.split("[unbind]").nth(1).unwrap();
    assert!(unbind.contains(r#"list = ["clear_screen", "belt_slot_1", "belt_slot_2"]"#));
    assert_eq!(
        out,
        "version = 1\npreset = \"dev\"\n\n[bindings]\n\
         move_attack = [\"MouseLeft\", \"Space\"]\n\
         toggle_inventory = [\"B\"]\n\
         skill_slot_2 = [\"2\", \"F2\"]\n\n\
         [unbind]\nlist = [\"clear_screen\", \"belt_slot_1\", \"belt_slot_2\"]\n"
    );
}

// --- §A6 strictness --------------------------------------------------------

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r1
#[test]
fn minimal_file_is_the_dev_preset() {
    assert_eq!(ok(HEAD), Preset::Dev.bindings().unwrap());
}

#[test]
fn dev_preset_has_no_clash_and_respects_slots() {
    let b = Preset::Dev.bindings().unwrap();
    assert_eq!(b.find_clash(), None);
    for &a in Action::ALL {
        assert!(b.inputs(a).len() <= MAX_INPUTS, "{}", a.name());
    }
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r2
#[test]
fn contexts_are_separate() {
    // Escape is game_menu (world), chat_cancel (chat), panel_close (panel).
    let b = ok(HEAD);
    assert_eq!(
        b.action_for(Context::World, Key::Escape),
        Some(Action::GameMenu)
    );
    assert_eq!(
        b.action_for(Context::Chat, Key::Escape),
        Some(Action::ChatCancel)
    );
    assert_eq!(
        b.action_for(Context::Panel, Key::Escape),
        Some(Action::PanelClose)
    );
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 text, §a6-controls-file-d2controls-1-m20 r3
#[test]
fn rejections_carry_their_line() {
    let cases: &[(&str, usize, ErrorKind)] = &[
        (
            "version = 1\npreset = \"dev\"\ncolour = 3\n",
            3,
            ErrorKind::UnknownKey("colour".into()),
        ),
        (
            "version = 1\npreset = \"dev\"\n[unbind]\nlist = []\nmore = []\n",
            5,
            ErrorKind::UnknownKey("unbind.more".into()),
        ),
        (
            "version = 1\npreset = \"dev\"\n[bindings]\nshow_items = [\"Alt\"]\n",
            4,
            ErrorKind::UnknownInput {
                action: "show_items".into(),
                input: "Alt".into(),
            },
        ),
        (
            "version = 1\npreset = \"dev\"\n[bindings]\nshow_items = [\"a\"]\n",
            4,
            ErrorKind::UnknownInput {
                action: "show_items".into(),
                input: "a".into(),
            },
        ),
        (
            "version = 1\npreset = \"dev\"\n[bindings]\nshow_items = []\n",
            4,
            ErrorKind::EmptyBinding("show_items".into()),
        ),
        (
            "version = 1\npreset = \"dev\"\n[bindings]\nshow_items = [\n  \"A\",\n  \"B\",\n  \"C\",\n]\n",
            7,
            ErrorKind::TooManyInputs("show_items".into()),
        ),
        (
            "version = 1\npreset = \"dev\"\n[bindings]\nshow_items = [\"G\", \"G\"]\n",
            4,
            ErrorKind::RepeatedInput {
                action: "show_items".into(),
                input: "G".into(),
            },
        ),
        (
            "version = 1\npreset = \"dev\"\n[unbind]\nlist = [\"show_items\", \"show_items\"]\n",
            4,
            ErrorKind::RepeatedUnbind("show_items".into()),
        ),
        (
            "version = 1\npreset = \"dev\"\n[unbind]\nlist = [\"nope\"]\n",
            4,
            ErrorKind::UnknownAction("nope".into()),
        ),
        (
            "version = 1\n\npreset = \"mine\"\n",
            3,
            ErrorKind::UnknownPreset("mine".into()),
        ),
        ("version = 1\n", 1, ErrorKind::MissingPreset),
        (
            "version = \"1\"\npreset = \"dev\"\n",
            1,
            ErrorKind::WrongType {
                key: "version".into(),
                expected: "an integer",
            },
        ),
        (
            "version = 1\npreset = \"dev\"\nbindings = 4\n",
            3,
            ErrorKind::WrongType {
                key: "bindings".into(),
                expected: "a table",
            },
        ),
        (
            "version = 1\npreset = \"dev\"\n[bindings]\nshow_items = \"G\"\n",
            4,
            ErrorKind::WrongType {
                key: "show_items".into(),
                expected: "an array of strings",
            },
        ),
        ("version = 0\npreset = \"dev\"\n", 1, ErrorKind::UnsupportedVersion(0)),
    ];
    for (text, line, kind) in cases {
        let e = err(text);
        assert_eq!((&e.kind, e.line), (kind, Some(*line)), "{text:?} -> {e}");
    }
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 text
#[test]
fn duplicate_key_is_a_toml_error_with_line() {
    let e = err(
        "version = 1\npreset = \"dev\"\n[bindings]\nshow_items = [\"G\"]\nshow_items = [\"H\"]\n",
    );
    assert!(matches!(e.kind, ErrorKind::Toml(_)), "{e}");
    assert_eq!(e.line, Some(5));
    let e = err("version = 1\npreset = \"dev\"\n[bindings\n");
    assert!(matches!(e.kind, ErrorKind::Toml(_)), "{e}");
    assert_eq!(e.line, Some(3));
}

#[test]
fn migrate_accepts_only_current_version() {
    let raw = RawFile {
        version: VERSION,
        preset: Named {
            name: "dev".into(),
            line: 2,
        },
        bindings: Vec::new(),
        unbind: Vec::new(),
    };
    assert_eq!(migrate(raw.clone(), 1), Ok(raw.clone()));
    let old = RawFile { version: 0, ..raw };
    assert_eq!(
        migrate(old, 7),
        Err(ControlsError::at(7, ErrorKind::UnsupportedVersion(0)))
    );
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r1
#[test]
fn from_effective_is_minimal_and_round_trips() {
    let mut b = Preset::Dev.bindings().unwrap();
    b.set(Action::ToggleInventory, &[Key::B, Key::I]);
    b.set(Action::ClearScreen, &[]);
    let file = ControlsFile::from_effective(Preset::Dev, &b).unwrap();
    assert_eq!(
        file.bindings,
        vec![(Action::ToggleInventory, vec![Key::B, Key::I])]
    );
    assert_eq!(file.unbind, vec![Action::ClearScreen]);
    assert_eq!(file.effective().unwrap(), b);
    assert_eq!(parse(&write(&file)).unwrap().1, b);
    // The original preset exists (`client/ui.md` §A6): the same bindings
    // written over it round-trip too.
    let over = ControlsFile::from_effective(Preset::Original, &b).unwrap();
    assert_eq!(over.effective().unwrap(), b);
    assert_eq!(parse(&write(&over)).unwrap().1, b);
}

// `preset = "original"` is the original's default key configuration
// (before: rejected as unavailable).
// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 text
#[test]
fn the_original_preset_parses() {
    let b = ok("version = 1\npreset = \"original\"\n");
    assert_eq!(Some(b.clone()), Preset::Original.bindings());
    assert!(b.find_clash().is_none());
    assert_eq!(b.inputs(Action::ToggleCharacter), &[Key::A, Key::C]);
    assert_eq!(
        b.inputs(Action::ToggleAutomap),
        &[Key::Tab, Key::MouseMiddle]
    );
}

/// `ui/controls.md` §B4 r1: the 1140-byte default table built from
/// `key-commands.tsv` (entries in `file_pos` order, slot 1 then slot 0;
/// i32 cmd, u16 key or 0xFFFF, i32 slot).
fn tsv_table() -> Vec<u8> {
    let tsv = include_str!("../../../../specs/ui/key-commands.tsv");
    let mut entries = vec![(0i32, original::UNBOUND, 0i32); original::TABLE_LEN];
    let key = |f: &str| match f {
        "-" => original::UNBOUND,
        f => u16::from_str_radix(&f[2..f.find(':').unwrap()], 16).unwrap(),
    };
    for line in tsv.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        let cmd: i32 = c[0].parse().unwrap();
        let pos: usize = c[8].parse().unwrap();
        entries[2 * pos] = (cmd, key(c[3]), 1);
        entries[2 * pos + 1] = (cmd, key(c[4]), 0);
    }
    let mut out = Vec::new();
    for (cmd, k, slot) in entries {
        out.extend_from_slice(&cmd.to_le_bytes());
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&slot.to_le_bytes());
    }
    out
}

// The `original` preset's bindings, written back into the table form
// (each command's action: slot 1 = first input, slot 0 = second), equal
// the table built from `key-commands.tsv`: 57 commands, 114 bindings.
// Covers: specs/ui/controls.md §b4-original-defaults-check-client-ui-md-b4 r1
#[test]
fn the_original_preset_is_the_tsv_table() {
    let b = Preset::Original.bindings().unwrap();
    let mut t = original::BindingTable::defaults();
    for e in t.0.iter_mut() {
        e.key = original::UNBOUND;
    }
    for e in t.0.iter_mut() {
        let a = keymap::action_of_cmd(e.cmd).expect("every command has an action");
        let i = if e.slot == 1 { 0 } else { 1 };
        e.key = b
            .inputs(a)
            .get(i)
            .map_or(original::UNBOUND, |&k| keymap::key_to_vk(k).unwrap());
    }
    let bytes = t.to_bytes();
    assert_eq!(bytes.len(), original::TABLE_BYTES);
    assert_eq!(bytes, tsv_table());
    // Every command maps to its own action.
    let mut seen: Vec<Action> = (0..57).filter_map(keymap::action_of_cmd).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 57);
}

// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r3
#[test]
fn name_lists_round_trip() {
    for &k in Key::ALL {
        assert_eq!(Key::from_name(k.name()), Some(k));
    }
    for (i, &a) in Action::ALL.iter().enumerate() {
        assert_eq!(Action::from_name(a.name()), Some(a));
        assert_eq!(a.index(), i);
    }
}

// --- M08: the clash check can fail -----------------------------------------

/// For every pair of same-context actions, binding the later one to an
/// input of the earlier one is reported as exactly that pair and input;
/// the same input in another context is not a clash.
// Covers: specs/client/ui.md §a6-controls-file-d2controls-1-m20 r2
#[test]
fn clash_check_catches_every_perturbation() {
    let base = Preset::Dev.bindings().unwrap();
    assert_eq!(base.find_clash(), None);
    let mut checked = 0;
    for (i, &first) in Action::ALL.iter().enumerate() {
        let Some(&key) = base.inputs(first).first() else {
            continue;
        };
        for &second in &Action::ALL[i + 1..] {
            if base.inputs(second).contains(&key) {
                continue; // already shares it in another context
            }
            let mut b = base.clone();
            b.set(second, &[key]);
            let got = b.find_clash();
            if second.context() == first.context() {
                assert_eq!(
                    got,
                    Some(Clash {
                        context: first.context(),
                        key,
                        first,
                        second,
                    }),
                    "{} vs {}",
                    first.name(),
                    second.name()
                );
                checked += 1;
            } else {
                // The perturbed key may still clash inside `second`'s own
                // context; it must never be reported against `first`.
                assert!(got.is_none_or(|c| c.first != first), "{got:?}");
            }
        }
    }
    assert!(checked > 100, "only {checked} perturbations");

    // Through the parser: the line points at the perturbed entry.
    let e = err(&format!("{HEAD}\n[bindings]\nbelt_slot_4 = [\"1\"]\n"));
    assert_eq!(e.line, Some(5));
    assert!(matches!(
        e.kind,
        ErrorKind::Clash(Clash {
            first: Action::BeltSlot1,
            second: Action::BeltSlot4,
            key: Key::Digit1,
            ..
        })
    ));
}

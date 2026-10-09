// Spec: specs/sim/units.md §4.7; specs/render/unit-composite.md §1.1; specs/skills/bodies.md §2.6
//! The animation rate of a cast and the shape it leaves, on the user's
//! install, against 1.14d (`traces/checks/dru-werewolf.check` and
//! `ass-burst-of-speed.check`, run 2026-10-09 under Wine on the
//! `blood-moor-empty` variant): the server's rate is `units.md` §4.7 on
//! the draw identity (`0x00645270`: the Werewolf's state 139 draws the
//! Druid as monster 430), and a state's stat fill re-rates the running
//! mode (`bodies.md` §2.6 "anim refresh").

use d2_client::app::single_player::Character;
use d2_client::app::state_dump::{self, DumpArgs, DumpGame, RunInfo};
use d2_formats::d2s::{self, Body, D2s, Golem, Header, StatEntry, Stats};
use d2_server::adapters::character::LoadContext;

mod app_support;

/// `d2s-tool new --class C --expansion --level 30 --all-skills 20
/// --right-skill R`: life, mana and stamina as the tool writes them.
fn save(name: &[u8], class: u8, stats: [(u16, i32); 10], right: i32) -> Character {
    let mut header = Header::default();
    header.set_name(name).unwrap();
    header.class = class;
    header.level = 30;
    header.status = d2s::status::EXPANSION;
    header.towns = [0x80, 0, 0];
    header.mouse[1] = d2s::Slot::encode(right, false, 0).unwrap();
    let mut stats: Vec<StatEntry> = stats
        .into_iter()
        .map(|(id, value)| StatEntry {
            id,
            layer: 0,
            value,
        })
        .collect();
    // Stat points, skill points, level and its experience.
    for (id, value) in [(4, 145), (5, 29), (12, 30), (13, 4_663_553)] {
        stats.push(StatEntry {
            id,
            layer: 0,
            value,
        });
    }
    stats.sort_by_key(|e| e.id);
    let save = D2s {
        header,
        body: Some(Body {
            stats: Stats::Bits(stats),
            skills: vec![20; 30],
            hireling_items: Some(None),
            golem: Some(Golem {
                flag: 0,
                item: None,
            }),
            ..Body::default()
        }),
    };
    Character::Save(
        Box::new(save),
        LoadContext {
            difficulty: 0,
            map_seed_applies: false,
        },
    )
}

/// The player's (mode, frame count, speed) by frame, after `warp 2`
/// before frame 4 and a right click at (330, 300) before frame 20.
fn player_rates(character: Character, ticks: u32) -> Vec<(u32, String)> {
    player_fields(
        character,
        ticks,
        &[],
        "frame 20; rclick 330 300",
        &["m", "fc", "sp"],
    )
}

/// The player's `fields` by frame after `warp 2` before frame 4, the
/// further `pokes` and the shared `input`.
fn player_fields(
    character: Character,
    ticks: u32,
    pokes: &[&str],
    input: &str,
    fields: &[&str],
) -> Vec<(u32, String)> {
    let mut all = vec![d2_client::app::poke::parse_poke_arg("4 warp 2").unwrap()];
    all.extend(
        pokes
            .iter()
            .map(|p| d2_client::app::poke::parse_poke_arg(p).unwrap()),
    );
    let args = DumpArgs {
        save: None,
        seed: Some(1234),
        difficulty: 0,
        ticks,
        every: 1,
        out: "unused".into(),
        game_dir: None,
        date: Some("2026-10-09".into()),
        pokes: all,
        input: Some(d2_client::world_view::input_script::parse(input).unwrap()),
        packets: None,
        rng: None,
        sends: Vec::new(),
    };
    let mut game = DumpGame::resolve(&args, app_support::game_data(), None).unwrap();
    game.character = character;
    let info = RunInfo {
        tool: "d2-client state-dump test".into(),
        date: "2026-10-09".into(),
        command: "test".into(),
        save: Some("Scn.d2s".into()),
    };
    let mut out = Vec::new();
    state_dump::dump(game, ticks, 1, &info, &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with(r#"{"k":"snap""#) && l.contains(r#"{"ut":0,"#))
        .map(|l| {
            let f: u32 = l[r#"{"k":"snap","f":"#.len()..]
                .split(',')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let p = &l[l.find(r#"{"ut":0,"#).unwrap()..];
            let field = |k: &str| {
                let at = p.find(&format!(r#""{k}":"#)).unwrap() + k.len() + 3;
                p[at..].split([',', '}']).next().unwrap().to_string()
            };
            let text: Vec<String> = fields.iter().map(|k| format!("{k}={}", field(k))).collect();
            (f, text.join(" "))
        })
        .collect()
}

fn at(rates: &[(u32, String)], f: u32) -> &str {
    &rates.iter().find(|(g, _)| *g == f).unwrap().1
}

// Covers: specs/sim/units.md §4.7 text
// Covers: specs/render/unit-composite.md §1.1
// Covers: specs/skills/bodies.md §2.6
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn werewolf_re_rates_the_cast_and_draws_the_druid_as_the_wolf() {
    let stats = [
        (0, 15),
        (1, 20),
        (2, 20),
        (3, 25),
        (6, 25216),
        (7, 25216),
        (8, 19968),
        (9, 19968),
        (10, 28928),
        (11, 28928),
    ];
    let r = player_rates(save(b"ScnDru", 5, stats, 223), 38);
    // 1.14d: the cast (SC, 15 frames) at the Druid's 208; the shape lands
    // at frame 29 and re-rates the running cast to 168; the wolf's
    // neutral: 9 frames at 241.
    assert_eq!(at(&r, 28), "m=10 fc=3840 sp=208");
    assert_eq!(at(&r, 29), "m=10 fc=3840 sp=168");
    assert_eq!(at(&r, 38), "m=1 fc=2304 sp=241");
}

// Covers: specs/sim/units.md §4.7 text
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn burst_of_speed_raises_the_neutral_rate() {
    let stats = [
        (0, 20),
        (1, 25),
        (2, 20),
        (3, 20),
        (6, 27648),
        (7, 27648),
        (8, 17536),
        (9, 17536),
        (10, 33600),
        (11, 33600),
    ];
    let r = player_rates(save(b"ScnAss", 6, stats, 258), 36);
    // 1.14d: the cast at 256; the neutral after it at 194.
    assert_eq!(at(&r, 35), "m=10 fc=4352 sp=256");
    assert_eq!(at(&r, 36), "m=1 fc=2048 sp=194");
}

// Covers: specs/skills/bodies-2b.md §8.10
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn whirlwind_moves_at_the_velocity_its_refresh_sets() {
    let stats = [
        (0, 30),
        (1, 10),
        (2, 20),
        (3, 25),
        (6, 28928),
        (7, 28928),
        (8, 9984),
        (9, 9984),
        (10, 30976),
        (11, 30976),
    ];
    let r = player_fields(
        save(b"ScnBar", 4, stats, 151),
        22,
        &["10 spawn 179 @x+4 @y normal"],
        "frame 20; rclick 544 336",
        &["m", "x", "xf", "yf", "sp"],
    );
    // 1.14d: the start's walk velocity becomes base · 143 / 100 in the
    // aura fill's anim refresh (mode 18, entry flag 1), speed 304; the
    // first step lands at frame 22.
    assert_eq!(at(&r, 21), "m=18 x=5143 xf=32768 yf=32768 sp=304");
    assert_eq!(at(&r, 22), "m=18 x=5144 xf=2119 yf=28650 sp=304");
}

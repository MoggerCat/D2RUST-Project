// Spec: specs/render/draw-order-2.md (§11)
//! Tests from the spec's rules and test vectors (§11, Test vectors).

use super::*;

const F800: FrameSize = FrameSize::D2RS;

fn colors() -> ColorTables {
    ColorTables {
        period0: [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
        period13: [30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41],
        period2: [50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61],
    }
}

fn resources() -> ActResources {
    ActResources {
        splash_frames: [6, 6, 5, 4],
        bubble_frames: [8, 8, 7, 6],
        colors: colors(),
    }
}

fn level(level_id: u32, act: u8, rain: bool, mud: bool) -> LevelWeather {
    LevelWeather {
        level_id,
        act,
        rain,
        mud,
    }
}

fn input(c: u32) -> UpdateInput {
    UpdateInput {
        update_count: c,
        frame: F800,
        camera_delta: (0, 0),
        day_period: 2,
    }
}

fn mode(m: u8) -> OpenMode {
    OpenMode::new(m).unwrap()
}

/// A weather after act load in `lv` (act load draws on a scratch seed).
fn loaded(lv: LevelWeather) -> Weather {
    let mut w = Weather::new();
    let mut s = Seed::new(7, 0);
    w.act_load(
        LocalPlayer {
            seed: &mut s,
            level: lv,
        },
        resources(),
    )
    .unwrap();
    w
}

fn rec(x: i32, y: i32, kind: u8) -> PoolRecord {
    PoolRecord {
        x,
        y,
        kind,
        frame: 0,
        countdown: 2,
    }
}

fn particle(x: i32, y: i32, landed: bool) -> Particle {
    Particle {
        x,
        y,
        ground_y: y,
        shape_10: None,
        shape_14: None,
        phase: 0,
        landed,
        bounces: 3,
        color: 77,
        alpha: 0xFF,
    }
}

fn p9(frame_rate: u32) -> Pass9Input {
    Pass9Input {
        frame: F800,
        mode: OpenMode::NONE,
        frame_rate,
        low_quality: false,
        thunder_sound_starts: true,
    }
}

// Covers: specs/render/draw-order-2.md §11.1
#[test]
fn pools_take_lowest_free_slot() {
    let mut w = Weather::new();
    assert_eq!(w.particles().capacity(), 256);
    assert_eq!(w.splashes().capacity(), 512);
    assert_eq!(w.bubbles().capacity(), 128);
    let mut p: Pool<u8> = Pool::new(3);
    assert_eq!(p.alloc(1), Some(0));
    assert_eq!(p.alloc(2), Some(1));
    assert_eq!(p.alloc(3), Some(2));
    assert_eq!(p.alloc(4), None);
    p.free(1);
    assert_eq!((p.live(), p.highest()), (2, Some(2)));
    assert_eq!(p.alloc(5), Some(1));
    let order: Vec<_> = p.iter().map(|(i, v)| (i, *v)).collect();
    assert_eq!(order, [(0, 1), (1, 5), (2, 3)]);
    p.clear();
    assert_eq!((p.live(), p.highest(), p.iter().count()), (0, None, 0));
    assert!(w.thunder);
    assert_eq!((w.snow_goal_min, w.snow_goal_n), (42, 170));
    w.pools_mut().1.alloc(rec(0, 0, 2));
    assert_eq!(w.splashes().live(), 1);
}

// Covers: specs/render/draw-order-2.md §11.2 r1, §11.2 r2
#[test]
fn rain_off_clears_particles_and_splashes() {
    let mut w = loaded(level(1, 0, false, false));
    w.rain_flag = true;
    w.mud_flag = true;
    let (p, s, b) = w.pools_mut();
    p.alloc(particle(1, 1, false));
    s.alloc(rec(1, 1, 2));
    b.alloc(rec(1, 1, 2));
    let mut seed = Seed::new(5, 0);
    let before = seed;
    // Rain off, mud on: particles and splashes go, bubbles stay.
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: level(1, 0, false, true),
        }),
        &input(1),
    )
    .unwrap();
    assert_eq!((w.particles().live(), w.splashes().live()), (0, 0));
    assert_eq!(w.bubbles().live(), 1);
    assert!(!w.rain_flag && w.mud_flag);
    assert_eq!(w.target, 0);
    assert_eq!(seed, before, "rain off draws nothing");
    // Mud off now: bubbles cleared.
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: level(1, 0, false, false),
        }),
        &input(2),
    )
    .unwrap();
    assert_eq!(w.bubbles().live(), 0);
}

// Covers: specs/render/draw-order-2.md §11.2 r3, §11.2 text
#[test]
fn update_needs_player_and_runs_once_per_update_count() {
    let mut w = loaded(level(1, 0, true, true));
    assert_eq!(w.update(None, &input(1)), Err(WeatherError::Fatal(0x547)));
    let mut seed = Seed::new(99, 0);
    let lv = level(1, 0, true, true);
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(5),
    )
    .unwrap();
    assert_eq!(w.mark, 5);
    let after = seed;
    let state = w.clone();
    // Same update count: a second frame changes nothing.
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(5),
    )
    .unwrap();
    assert_eq!(seed, after);
    assert_eq!(w, state);
}

// Covers: specs/render/draw-order-2.md §11.3 r1, §11.3 r2, §11.3 r3, §11.4 r1, §11.4 r5, §11.8 r1
#[test]
fn first_rain_update_enters_phase_one_and_spawns() {
    let lv = level(1, 0, true, false);
    let mut w = loaded(lv);
    assert_eq!((w.phase, w.countdown), (0, 0));
    let mut seed = Seed::new(0x1234_5678, 0);
    let mut e = seed;
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(1),
    )
    .unwrap();
    // Cycle: D, then 0x004726F0(0) (three raw steps), then the peak.
    let d = e.roll_range(250, 250) as u32;
    let wc = 125 + (e.step() % 375) as i32;
    let lc = 500 + (e.step() % 1_500) as i32;
    let wind = 92 + (e.step() % 71) as i32;
    let peak = e.roll_range(32, 224) as u32;
    assert_eq!((w.phase, w.length, w.countdown), (1, d, d - 1));
    assert_eq!(w.lightning_countdown, lc);
    assert_eq!((w.wind, w.wind_goal), (wind, wind));
    assert_eq!(w.peak, peak);
    let target = peak / d;
    assert_eq!(w.target, target);
    assert_eq!(w.intensity_256, target);
    // Top-up: one rain spawn per missing particle.
    for _ in 0..target {
        let x = e.roll_range(0, 800);
        let g = e.roll_range(40, 600 - 87);
        let y = e.roll_range(-20, g + 20);
        let phase = e.step() & 511;
        let ci = (e.step() % 12) as usize;
        let (_, p) = w.particles().iter().next().unwrap();
        assert_eq!((p.x, p.ground_y, p.y, p.phase), (x, g, y, phase));
        assert_eq!((p.color, p.alpha), (colors().period2[ci], 0xFF));
        assert_eq!((p.bounces, p.landed, p.shape_10), (3, false, None));
    }
    assert_eq!(w.particles().live() as u32, target);
    // Wind already on its goal; retarget countdown −1.
    assert_eq!(w.wind_countdown, wc - 1);
    assert_eq!(seed, e);
}

// Covers: specs/render/draw-order-2.md §11.3 r1, §11.3 r3, §11.8 text
#[test]
fn ramp_then_phase_two_presets() {
    let lv = level(1, 0, true, false);
    let mut w = loaded(lv);
    (w.phase, w.length, w.countdown, w.peak) = (1, 250, 1, 200);
    w.wind_countdown = 50;
    let mut seed = Seed::new(3, 0);
    w.target = 0;
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(1),
    )
    .unwrap();
    assert_eq!((w.countdown, w.target), (0, 200));
    w.particles.clear();
    w.target = 0;
    w.lightning_on = true;
    // Countdown 0: phase 2, D from (3000, 3000), presets: lightning off,
    // one raw step (rain mode), target := peak. Then a 200-particle top-up.
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(2),
    )
    .unwrap();
    // The first update's top-up drew 200 spawns.
    let mut e = Seed::new(3, 0);
    for _ in 0..200 {
        e.roll_range(0, 800);
        let g = e.roll_range(40, 513);
        e.roll_range(-20, g + 20);
        e.step();
        e.step();
    }
    let d = e.roll_range(3_000, 3_000) as u32;
    e.step();
    assert_eq!((w.phase, w.length, w.countdown), (2, d, d - 1));
    assert!(!w.lightning_on);
    assert_eq!(w.target, 200);
    assert_eq!(w.intensity_256, 200);
    // Phase 3 ramp down: peak × countdown / D.
    (w.phase, w.length, w.countdown, w.peak) = (3, 100, 51, 200);
    w.rain_cycle(&mut seed, &lv).unwrap();
    assert_eq!(w.target, 200 * 50 / 100);
}

// Covers: specs/render/draw-order-2.md §11.8 text
#[test]
fn act_load_cycle_tables() {
    let w = loaded(level(1, 0, false, false));
    assert_eq!(w.cycle_min, [7_500, 250, 3_000, 125]);
    assert_eq!(w.cycle_n, [7_500, 250, 3_000, 50]);
    assert_eq!(w.intensity_256, 0);
    assert_eq!(w.resources(), Some(&resources()));
    for act in [2, 4] {
        let w = loaded(level(if act == 4 { 109 } else { 75 }, act, false, false));
        assert_eq!(w.cycle_min[2], 9_000);
    }
    let w = loaded(level(40, 1, false, false));
    assert_eq!(w.cycle_min[2], 3_000);
}

// Covers: specs/render/draw-order-2.md §11.4 r2, §11.4 r3, §11.4 r4
#[test]
fn wind_retarget_and_lightning_timer() {
    let lv = level(1, 0, true, false);
    let mut w = loaded(lv);
    (w.phase, w.length, w.countdown, w.peak) = (0, 7_500, 10, 0);
    (w.wind, w.wind_goal, w.wind_countdown) = (100, 95, 1);
    (w.lightning_on, w.lightning_countdown) = (true, 2);
    let mut seed = Seed::new(11, 0);
    let mut e = seed;
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(1),
    )
    .unwrap();
    assert_eq!(w.wind, 98, "step 2 toward the goal");
    let wc = 125 + (e.step() % 375) as i32;
    let goal = 92 + (e.step() % 71) as i32;
    assert_eq!((w.wind_countdown, w.wind_goal), (wc, goal));
    assert_eq!(seed, e);
    assert_eq!((w.lightning_countdown, w.lightning_trigger), (1, 0));
    (w.wind, w.wind_goal) = (96, 95);
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(2),
    )
    .unwrap();
    assert_eq!(w.wind, 95, "stops on the goal");
    assert_eq!((w.lightning_countdown, w.lightning_trigger), (0, 1));
}

// Covers: specs/render/draw-order-2.md §11.2 r3
#[test]
fn moving_live_particles_is_open_question_3() {
    let lv = level(1, 0, true, false);
    let mut w = loaded(lv);
    w.particles.alloc(particle(5, 5, false));
    let mut seed = Seed::new(1, 0);
    let r = w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &input(1),
    );
    assert!(matches!(r, Err(WeatherError::Open { question: 3, .. })));
}

// Covers: specs/render/draw-order-2.md §11.4 r5
#[test]
fn snow_spawn_is_unspecified_and_day_period_tables() {
    let lv = level(1, 0, true, false);
    let mut w = loaded(lv);
    w.snow_mode = true;
    w.target = 1;
    let mut seed = Seed::new(1, 0);
    let before = seed;
    assert!(matches!(
        w.top_up(&mut seed, &input(1)),
        Err(WeatherError::Unspecified { .. })
    ));
    assert_eq!(seed, before);
    for (period, table, alpha) in [
        (0, colors().period0, 0x7F),
        (1, colors().period13, 0xFF),
        (3, colors().period13, 0xFF),
    ] {
        let mut w = loaded(lv);
        let mut seed = Seed::new(42, 0);
        let mut e = seed;
        let inp = UpdateInput {
            day_period: period,
            ..input(1)
        };
        assert!(w.spawn_particle(&mut seed, &inp).unwrap());
        for _ in 0..4 {
            e.step();
        }
        let i = (e.step() % 12) as usize;
        let (_, p) = w.particles().iter().next().unwrap();
        assert_eq!((p.color, p.alpha), (table[i], alpha));
    }
}

// Covers: specs/render/draw-order-2.md §11.5 r1, §edge-cases-original-bugs
#[test]
fn water_floor_rain_off_draws_once() {
    // Test vector: water floor drawn, rain off → 1 draw, no spawn.
    let mut w = loaded(level(1, 0, false, false));
    let mut ctx = FloorContext::default();
    ctx.begin_frame(100, &w, false);
    assert!(!ctx.splash && !ctx.bubble);
    let mut seed = Seed::new(17, 0);
    let mut e = seed;
    w.water_floor(&ctx, 400, 300, &mut seed);
    e.step();
    assert_eq!(seed, e);
    assert_eq!((w.splashes().live(), w.bubbles().live()), (0, 0));
}

// Covers: specs/render/draw-order-2.md §11.5 text
#[test]
fn floor_context_gates() {
    let mut w = Weather::new();
    let mut ctx = FloorContext::default();
    w.intensity_256 = 255;
    ctx.begin_frame(25, &w, true);
    assert!(!ctx.splash, "int(255/256) = 0");
    assert!(!ctx.bubble, "25 is not > 0 + 25");
    ctx.begin_frame(26, &w, true);
    assert!(ctx.bubble);
    assert_eq!(ctx.last_b, 26);
    w.intensity_256 = 256;
    ctx.begin_frame(26, &w, true);
    assert!(ctx.splash && !ctx.bubble, "flags cleared each frame");
    assert_eq!(ctx.last_s, 26);
    ctx.begin_frame(29, &w, true);
    assert!(!ctx.splash, "29 is not > 26 + 3");
    ctx.begin_frame(30, &w, true);
    assert!(ctx.splash);
    ctx.begin_frame(52, &w, false);
    assert!(!ctx.bubble, "no Mud");
    ctx.begin_frame(52, &w, true);
    assert!(ctx.bubble);
}

/// A seed whose first `roll_range(0, 80)` gives `a`.
fn seed_with_a(a: i32) -> Seed {
    (1..)
        .map(|lo| Seed::new(lo, 0))
        .find(|s| s.clone().roll_range(0, 80) == a)
        .unwrap()
}

// Covers: specs/render/draw-order-2.md §11.5 r2
#[test]
fn splash_spawn_a_zero() {
    // Test vector: a = 0 → b = 0, no x draw, x = X, y = Y.
    let mut w = Weather::new();
    let mut seed = seed_with_a(0);
    let mut e = seed;
    let i = w.spawn_splash(400, 300, &mut seed).unwrap();
    e.step();
    let kind = 2 + (e.step() & 1) as u8;
    assert_eq!(seed, e, "two draws: a and the kind step");
    assert_eq!(*w.splashes().get(i).unwrap(), rec(400, 300, kind));
}

// Covers: specs/render/draw-order-2.md §11.5 r2, §11.5 r3
#[test]
fn splash_and_bubble_spawn_positions() {
    let mut w = Weather::new();
    let mut seed = seed_with_a(30);
    let mut e = seed;
    w.spawn_splash(100, 200, &mut seed).unwrap();
    assert_eq!(e.roll_range(0, 80), 30);
    let b = 80 - 2 * 10;
    let dx = e.roll_range(-b, 2 * b);
    let kind = 2 + (e.step() & 1) as u8;
    let r = *w.splashes().get(0).unwrap();
    assert_eq!(r, rec(100 + dx, 230, kind));
    assert_eq!(seed, e);

    let mut seed = seed_with_a(50);
    let mut e = seed;
    w.spawn_bubble(100, 200, &mut seed).unwrap();
    e.roll_range(0, 80);
    let dx = e.roll_range(-60, 120);
    assert_eq!(*w.bubbles().get(0).unwrap(), rec(100 + dx, 220, 2));
    assert_eq!(seed, e, "bubble: two draws, no kind step");

    // Full pool: nothing, no draw.
    for _ in 1..BUBBLE_SLOTS {
        w.bubbles.alloc(rec(0, 0, 2));
    }
    let before = seed;
    assert_eq!(w.spawn_bubble(1, 1, &mut seed), None);
    assert_eq!(seed, before);
}

// Covers: specs/render/draw-order-2.md §11.5 r2, §11.5 r3
#[test]
fn water_floor_spawns_when_gated() {
    let mut w = Weather::new();
    let ctx = FloorContext {
        bubble: true,
        ..Default::default()
    };
    // Find a seed with r < 100.
    let seed0 = (1..)
        .map(|lo| Seed::new(lo, 0))
        .find(|s| s.clone().roll_range(0, 1_000) < 100)
        .unwrap();
    let mut seed = seed0;
    w.water_floor(&ctx, 10, 20, &mut seed);
    assert_eq!((w.splashes().live(), w.bubbles().live()), (0, 1));
    let mut e = seed0;
    e.roll_range(0, 1_000);
    let mut probe = Weather::new();
    probe.spawn_bubble(10, 20, &mut e);
    assert_eq!(seed, e);
    assert_eq!(probe.bubbles().get(0), w.bubbles().get(0));
}

// Covers: specs/render/draw-order-2.md §11.5 text, §edge-cases-original-bugs
#[test]
fn pool_update_moves_and_ages() {
    let lv = level(1, 0, false, true);
    let mut w = loaded(lv);
    w.bubbles.alloc(rec(100, 100, 2)); // bubble3: 7 frames
    let mut seed = Seed::new(1, 0);
    let mut inp = input(1);
    inp.camera_delta = (4, -2);
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &inp,
    )
    .unwrap();
    let r = *w.bubbles().get(0).unwrap();
    assert_eq!((r.x, r.y, r.frame, r.countdown), (96, 102, 0, 1));
    inp.update_count = 2;
    w.update(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &inp,
    )
    .unwrap();
    let r = *w.bubbles().get(0).unwrap();
    assert_eq!((r.x, r.y, r.frame, r.countdown), (92, 104, 1, 2));
    // 6 more frames (12 updates) → frame 7 ≥ 7: freed.
    for c in 3..=14 {
        inp.update_count = c;
        w.update(
            Some(LocalPlayer {
                seed: &mut seed,
                level: lv,
            }),
            &inp,
        )
        .unwrap();
    }
    assert_eq!(w.bubbles().live(), 0);
}

// Covers: specs/render/draw-order-2.md §11.6
#[test]
fn pass4_order_and_bounds() {
    let mut w = Weather::new();
    w.splashes.alloc(rec(10, 10, 2));
    w.splashes.alloc(rec(799, 552, 3));
    w.splashes.alloc(rec(800, 10, 2)); // x = W: out
    w.splashes.alloc(rec(10, 553, 2)); // y = H − 47: out
    w.splashes.alloc(rec(10, -1, 2)); // y < 0: out
    w.bubbles.alloc(rec(20, 20, 2));
    w.splashes.free(0);
    let d = w.pass4(F800, mode(0), 0);
    let got: Vec<_> = d.iter().map(|d| (d.pool, d.slot, d.x, d.y)).collect();
    assert_eq!(
        got,
        [
            (EnvPool::Splashes, 1, 799, 552),
            (EnvPool::Bubbles, 0, 20, 20)
        ]
    );
    assert_eq!((d[0].kind, d[0].frame), (3, 0));
    assert_eq!(
        EnvPool::Splashes.cel_file(d[0].kind),
        "DATA\\GLOBAL\\UncompOverlays\\Rain4"
    );
    assert_eq!(
        EnvPool::Bubbles.cel_file(2),
        "DATA\\GLOBAL\\UncompOverlays\\bubble3"
    );
    assert_eq!(
        (EnvPool::Splashes.major(), EnvPool::Bubbles.major()),
        (0, 1)
    );
    assert_eq!((PoolDraw::LIGHT, PoolDraw::DRAW_MODE), (-1, 3));
    // Open mode 1: [0, 400), shiftX −200 added.
    assert_eq!(span(F800, mode(1)), (0, 400));
    assert_eq!(span(F800, mode(2)), (400, 800));
    assert_eq!(span(F800, mode(3)), (0, 800));
    let mut w = Weather::new();
    w.splashes.alloc(rec(150, 10, 2)); // −50: out
    w.splashes.alloc(rec(300, 10, 2)); // 100: in
    w.splashes.alloc(rec(600, 10, 2)); // 400 = R: out
    let d = w.pass4(F800, mode(1), -200);
    let got: Vec<_> = d.iter().map(|d| (d.slot, d.x)).collect();
    assert_eq!(got, [(1, 100)]);
}

// Covers: specs/render/draw-order-2.md §11.7 r1, §11.7 r2, §edge-cases-original-bugs
#[test]
fn flash_needs_ten_frames_per_second() {
    // Test vector: frame rate 9 / 10, open mode 0, 800 × 600.
    let lv = level(1, 0, false, false);
    for (fps, flash) in [(9, false), (10, true)] {
        let mut w = Weather::new();
        w.start_lightning(true);
        let mut seed = Seed::new(1, 0);
        let out = w
            .pass9(
                Some(LocalPlayer {
                    seed: &mut seed,
                    level: lv,
                }),
                &p9(fps),
            )
            .unwrap();
        let want: &[SkyDraw] = if flash {
            &[SkyDraw::Flash {
                x0: 0,
                y0: 0,
                x1: 800,
                y1: 553,
            }]
        } else {
            &[]
        };
        assert_eq!(out.draws, want);
        assert_eq!((w.lightning_phase, w.lightning_countdown), (1, 3));
    }
    let mut w = Weather::new();
    assert_eq!(w.pass9(None, &p9(30)), Err(WeatherError::Fatal(0x573)));
}

// Covers: specs/render/draw-order-2.md §11.7 r2, §11.7 text
#[test]
fn thunder_first_strike_silent() {
    let lv = level(1, 0, false, false);
    let mut w = Weather::new();
    w.start_lightning(false);
    assert!(w.lightning_on && !w.thunder && w.lightning_trigger == 1);
    w.lightning_phase = 1;
    let mut seed = Seed::new(8, 0);
    let mut e = seed;
    let out = w
        .pass9(
            Some(LocalPlayer {
                seed: &mut seed,
                level: lv,
            }),
            &p9(30),
        )
        .unwrap();
    assert_eq!(w.lightning_countdown, e.roll_range(500, 1_500));
    assert_eq!(seed, e);
    assert_eq!(out.thunder, None);
    assert!(w.thunder, "a cleared thunder flag is set instead");
    assert_eq!(w.lightning_phase, 0);
    // Second strike: sound 202 with volume and two position rolls.
    w.lightning_phase = 1;
    w.lightning_trigger = 1;
    let out = w
        .pass9(
            Some(LocalPlayer {
                seed: &mut seed,
                level: lv,
            }),
            &p9(30),
        )
        .unwrap();
    let cd = e.roll_range(500, 1_500);
    let volume = e.roll_range(25, 50);
    let px = e.roll_range(-200, 400);
    let py = e.roll_range(-200, 400);
    assert_eq!(w.lightning_countdown, cd);
    assert_eq!(
        out.thunder,
        Some(Thunder {
            sound: 202,
            volume,
            position: Some((px, py))
        })
    );
    assert_eq!(seed, e);
    // Sound not started: no position rolls.
    w.lightning_phase = 1;
    w.lightning_trigger = 1;
    let inp = Pass9Input {
        thunder_sound_starts: false,
        ..p9(30)
    };
    let out = w
        .pass9(
            Some(LocalPlayer {
                seed: &mut seed,
                level: lv,
            }),
            &inp,
        )
        .unwrap();
    e.roll_range(500, 1_500);
    assert_eq!(out.thunder.unwrap().volume, e.roll_range(25, 50));
    assert_eq!(seed, e);
}

// Covers: specs/render/draw-order-2.md §11.7 r3
#[test]
fn particles_draw_only_landed_drops() {
    let lv = level(1, 0, true, false);
    let mut w = Weather::new();
    w.particles.alloc(particle(10, 10, true));
    w.particles.alloc(particle(800, 10, true)); // out of range
    let mut seed = Seed::new(1, 0);
    let out = w
        .pass9(
            Some(LocalPlayer {
                seed: &mut seed,
                level: lv,
            }),
            &p9(30),
        )
        .unwrap();
    assert_eq!(
        out.draws,
        [SkyDraw::Line {
            x0: 10,
            y0: 10,
            x1: 10,
            y1: 10,
            color: 77,
            alpha: 0xFF
        }]
    );
    // Low quality: nothing.
    let lq = Pass9Input {
        low_quality: true,
        ..p9(30)
    };
    let out = w
        .pass9(
            Some(LocalPlayer {
                seed: &mut seed,
                level: lv,
            }),
            &lq,
        )
        .unwrap();
    assert!(out.draws.is_empty());
    // A falling drop needs the float vector.
    w.particles.alloc(particle(20, 20, false));
    let r = w.pass9(
        Some(LocalPlayer {
            seed: &mut seed,
            level: lv,
        }),
        &p9(30),
    );
    assert!(matches!(r, Err(WeatherError::Open { question: 3, .. })));
}

// Covers: specs/render/draw-order-2.md §11.8 r2
#[test]
fn act5_lock_levels() {
    let lv = level(120, 4, true, false);
    let mut w = Weather::new();
    w.particles.alloc(particle(1, 1, true));
    w.lightning_on = true;
    let mut seed = Seed::new(5, 0);
    let mut e = seed;
    w.level_entry(0, &mut seed, &lv).unwrap();
    for _ in 0..4 {
        e.step();
    }
    assert_eq!(
        seed, e,
        "0x00472610 (3 steps) + 1 raw step, no preset draws"
    );
    assert!(w.snow_mode && w.snow_lock && w.size_bump);
    assert_eq!((w.peak, w.target, w.wind, w.wind_goal), (256, 256, 28, 28));
    assert_eq!((w.snow_goal_min, w.snow_goal_n), (28, 28));
    assert!(!w.lightning_on);
    assert_eq!(w.particles().live(), 0, "mode changed: cleared");
    // Locked and phase ≠ 2: the cycle forces phase 2 and the presets.
    w.phase = 0;
    w.target = 0;
    w.rain_cycle(&mut seed, &lv).unwrap();
    assert_eq!((w.phase, w.target), (2, 256));
    w.phase = 3;
    w.snow_lock_hold = 1;
    w.target = 0;
    w.rain_cycle(&mut seed, &lv).unwrap();
    assert_eq!((w.phase, w.target), (3, 0), "held: nothing runs");
}

// Covers: specs/render/draw-order-2.md §11.8 r1, §11.8 r2
#[test]
fn act5_snow_off_roll_and_leaving_act5() {
    let lv = level(110, 4, true, false);
    // A seed whose 4th step mod 100 < 25.
    let seed0 = (1..)
        .map(|lo| Seed::new(lo, 0))
        .find(|s| {
            let mut s = *s;
            (0..3).for_each(|_| {
                s.step();
            });
            s.step() % 100 < 25
        })
        .unwrap();
    let mut w = Weather::new();
    w.snow_mode = true;
    w.particles.alloc(particle(1, 1, true));
    let mut seed = seed0;
    w.level_entry(0, &mut seed, &lv).unwrap();
    assert!(!w.snow_mode && !w.size_bump);
    assert_eq!(w.particles().live(), 0);
    // With arg ≠ 0 the same roll keeps snow on.
    let mut w = Weather::new();
    let mut seed = seed0;
    w.level_entry(1, &mut seed, &lv).unwrap();
    assert!(w.snow_mode && !w.snow_lock);
    // Leaving act V: snow off, lightning off, 0x00472610.
    w.lightning_on = true;
    w.particles.alloc(particle(1, 1, true));
    let mut e = seed;
    w.level_entry(0, &mut seed, &level(1, 0, true, false))
        .unwrap();
    let wc = 125 + (e.step() % 375) as i32;
    let lc = 500 + (e.step() % 1_500) as i32;
    let wind = 92 + (e.step() % 71) as i32;
    assert_eq!(seed, e);
    assert_eq!(
        (w.wind_countdown, w.lightning_countdown, w.wind),
        (wc, lc, wind)
    );
    assert!(!w.snow_mode && !w.lightning_on);
    assert_eq!(w.particles().live(), 0);
}

// Covers: specs/render/draw-order-2.md §11.8 text
#[test]
fn level_presets_snow_levels() {
    let mut w = Weather::new();
    w.snow_mode = true;
    let mut seed = Seed::new(9, 0);
    let mut e = seed;
    w.level_presets(&mut seed, &level(117, 4, true, false));
    assert_eq!(w.peak, e.roll_range(40, 112) as u32);
    assert_eq!(
        (w.snow_goal_min, w.snow_goal_n, w.target),
        (170, 56, w.peak)
    );
    w.level_presets(&mut seed, &level(111, 4, true, false));
    assert_eq!(w.peak, e.roll_range(32, 56) as u32);
    assert_eq!((w.snow_goal_min, w.snow_goal_n), (42, 170));
    // Other level in snow mode: no raw step.
    let before = seed;
    w.level_presets(&mut seed, &level(115, 4, true, false));
    assert_eq!(seed, before);
}

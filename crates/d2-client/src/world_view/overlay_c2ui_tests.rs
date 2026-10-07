// Spec: specs/render/overlay.md
//! Coverage tests (c2-ui-render session).
use super::*;

// Covers: specs/render/overlay.md §2 r9
#[test]
fn overlay_light_rules() {
    let mut row = OverlayRow::default();
    assert_eq!(overlay_light(&row), None);
    row.radius = 5;
    row.init_radius = 5;
    assert_eq!(
        overlay_light(&row),
        Some(OverlayLight {
            init_radius: 5,
            target_radius: None
        })
    );
    row.init_radius = 2;
    assert_eq!(overlay_light(&row).unwrap().target_radius, Some(5));
}

// Covers: specs/render/overlay.md §3 r7
#[test]
fn kind6_clock_is_40ms_per_update() {
    assert_eq!(kind6_clock_ms(0), 0);
    assert_eq!(kind6_clock_ms(25), 1000);
}

// Covers: specs/render/unit-composite.md §5 r4
#[test]
fn overlay_draw_filter() {
    let mut o = Overlay {
        kind: 2,
        frame: 3 * 256,
        frames: 10 * 256,
        pre_draw: true,
        ..Overlay::default()
    };
    assert!(overlay_draws(&o, true));
    assert!(!overlay_draws(&o, false));
    o.pre_draw = false;
    assert!(overlay_draws(&o, false));
    o.frame = 10 * 256;
    assert!(!overlay_draws(&o, false));
    o.frame = 0;
    o.kind = 6;
    o.a = 0;
    assert!(!overlay_draws(&o, false));
    o.kind = 8;
    o.active = false;
    assert!(!overlay_draws(&o, false));
    o.active = true;
    assert!(overlay_draws(&o, false));
}

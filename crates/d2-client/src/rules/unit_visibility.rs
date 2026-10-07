// Spec: specs/client/model.md (§13 visibility predicate)
//! The visibility predicate `0x004DBF20` (`client/model.md` §13 rules 1–5):
//! the screen point of the unit origin, the COF box test, and the cel box
//! test. The cel request and load (rules 3 and 4) are the caller's: it
//! passes `None` when either fails. Plain Rust, integer math.

use d2_formats::cof::Cof;

use super::unit_composite::cof_box_visible;

/// The cel fields the box test reads: width (+4), height (+8), x offset
/// (+0x0C), y offset (+0x10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CelBox {
    pub w: i32,
    pub h: i32,
    pub ox: i32,
    pub oy: i32,
}

/// Rule 1: the screen point of client pixel point (a, b) for the unit
/// origin (`cx`, `cy`) and `shift_x`: X := a − (cx − shift_x),
/// Y := b − (cy − 8).
pub fn screen_point(a: i32, b: i32, cx: i32, cy: i32, shift_x: i32) -> (i32, i32) {
    (a - (cx - shift_x), b - (cy - 8))
}

/// Rule 5, `0x004DAB40(cel, X, Y, 0)`: left := ox + X, top := oy + Y;
/// visible iff left ≤ W, left + w ≥ 0, top − h ≤ H and top + h ≥ 0
/// (signed; the vertical span is [top − h, top + h], as read).
pub fn cel_box_visible(cel: CelBox, x: i32, y: i32, w: i32, h: i32) -> bool {
    let (left, top) = (cel.ox + x, cel.oy + y);
    left <= w && left + cel.w >= 0 && top - cel.h <= h && top + cel.h >= 0
}

/// Rules 1–5. `cel` is the loaded TR cel (`None`: the request or the load
/// of rules 3 / 4 failed → not visible). `w`, `h`: the frame size.
#[allow(clippy::too_many_arguments)]
pub fn unit_visible(
    cof: &Cof,
    cel: Option<CelBox>,
    a: i32,
    b: i32,
    origin: (i32, i32),
    shift_x: i32,
    w: u32,
    h: u32,
) -> bool {
    let (x, y) = screen_point(a, b, origin.0, origin.1, shift_x);
    if !cof_box_visible(cof, x, y, w, h) {
        return false;
    }
    cel.is_some_and(|c| cel_box_visible(c, x, y, w as i32, h as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/client/model.md §13 r1
    #[test]
    fn screen_point_subtracts_the_origin() {
        assert_eq!(screen_point(100, 50, 30, 20, 4), (74, 38));
    }

    // Covers: specs/client/model.md §13 r5
    #[test]
    fn cel_box_edges() {
        let c = CelBox {
            w: 10,
            h: 6,
            ox: -5,
            oy: 0,
        };
        // left = -5 + x; the right edge left + w ≥ 0 fails below x = -5.
        assert!(cel_box_visible(c, -5, 0, 640, 480));
        assert!(!cel_box_visible(c, -6, 0, 640, 480));
        // left ≤ W.
        assert!(cel_box_visible(c, 645, 0, 640, 480));
        assert!(!cel_box_visible(c, 646, 0, 640, 480));
        // top − h ≤ H and top + h ≥ 0 (a 2h-tall span).
        assert!(cel_box_visible(c, 0, 486, 640, 480));
        assert!(!cel_box_visible(c, 0, 487, 640, 480));
        assert!(cel_box_visible(c, 0, -6, 640, 480));
        assert!(!cel_box_visible(c, 0, -7, 640, 480));
    }

    fn cof(x_min: i32, x_max: i32, y_min: i32, y_max: i32) -> Cof {
        Cof {
            layers_count: 0,
            frames: 1,
            directions: 16,
            version: 20,
            unknown: [0; 4],
            x_min,
            x_max,
            y_min,
            y_max,
            animation_rate: 256,
            layers: vec![],
            events: vec![0],
            event_padding: vec![],
            draw_order: vec![],
        }
    }

    // Covers: specs/client/model.md §13 r2, §13 r3, §13 r4
    #[test]
    fn the_predicate_chains_the_cof_box_and_the_cel_box() {
        let c = cof(-30, 30, -90, 0);
        let cel = Some(CelBox {
            w: 20,
            h: 10,
            ox: -10,
            oy: -20,
        });
        // Origin at (400, 300), a = b = origin: X = shift, Y = 8.
        assert!(unit_visible(&c, cel, 400, 300, (400, 300), 0, 800, 600));
        // COF box fails (x_min + X < W − 1 broken) → not visible.
        assert!(!unit_visible(&c, cel, 400, 300, (400, 300), 900, 800, 600));
        // A failed cel request or load → not visible.
        assert!(!unit_visible(&c, None, 400, 300, (400, 300), 0, 800, 600));
    }
}

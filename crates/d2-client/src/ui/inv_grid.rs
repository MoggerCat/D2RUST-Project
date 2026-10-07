// Spec: specs/ui/inventory.md (§1–§6, §8–§10)
//! Inventory grids, tints and item graphics as plain decisions
//! (`ui/inventory.md`): the layout record and its page picks (§1), the five
//! tints and their palette match (§2), which tint an item gets (§3, §4,
//! §6, §9), the equipment-box offsets (§6), the item graphic file and
//! draw (§8) and the grid click → C→S message choice (§10). The owner
//! panel reads the world model and feeds the facts in; nothing here
//! decides an outcome the server does not check again.

use super::geom::Point;

/// A grid layout record (§1 r1, 24 bytes in `inventory.bin`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridRecord {
    pub grid_x: u8,
    pub grid_y: u8,
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
    pub cell_w: u8,
    pub cell_h: u8,
}

impl GridRecord {
    /// Parses the 24-byte record (§1 r1): gridX +0, gridY +1, left +4,
    /// right +8, top +0xC, bottom +0x10, cell width +0x14, height +0x15.
    pub fn parse(b: &[u8; 24]) -> Self {
        let i = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        Self {
            grid_x: b[0],
            grid_y: b[1],
            left: i(4),
            right: i(8),
            top: i(0xC),
            bottom: i(0x10),
            cell_w: b[0x14],
            cell_h: b[0x15],
        }
    }

    /// Cell (c, r): top-left corner and size (§1 r3).
    pub fn cell(&self, c: i32, r: i32) -> (i32, i32, i32, i32) {
        let (cw, ch) = (i32::from(self.cell_w), i32::from(self.cell_h));
        (self.left + cw * c, self.top + ch * r, cw, ch)
    }

    /// A cell is tinted only when its top-left corner is inside
    /// [0, clipW) × [0, clipH) (§1 r3).
    pub fn cell_tinted(&self, c: i32, r: i32, clip_w: i32, clip_h: i32) -> bool {
        let (x, y, _, _) = self.cell(c, r);
        (0..clip_w).contains(&x) && (0..clip_h).contains(&y)
    }

    /// Mouse → cell (§1 r4): unsigned division of the wrapped difference.
    pub fn mouse_cell(&self, p: Point) -> (u32, u32) {
        (
            (p.x.wrapping_sub(self.left) as u32) / u32::from(self.cell_w),
            (p.y.wrapping_sub(self.top) as u32) / u32::from(self.cell_h),
        )
    }

    /// `0x00483AB0` (§4 r1): the mouse is inside the grid rectangle
    /// (left ≤ x < right, top ≤ y ≤ bottom).
    pub fn contains_mouse(&self, p: Point) -> bool {
        self.left <= p.x && p.x < self.right && self.top <= p.y && p.y <= self.bottom
    }
}

/// The unit kinds an owner can have (§1 r2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerKind {
    Player,
    /// A monster (hireling, unit type 1).
    Monster,
    Other,
}

/// The layout record `0x00483FF0` uses (§1 r2): the address of the
/// record in the original; `None` draws nothing.
pub fn record_address(owner: OwnerKind, page: u8, expansion: bool) -> Option<u32> {
    match owner {
        OwnerKind::Monster => Some(0x007B_CB58),
        OwnerKind::Other => None,
        OwnerKind::Player => match page {
            0 => Some(0x007B_CB88),
            1 => Some(0x007B_CB18),
            2 => Some(0x007B_CA30),
            3 => Some(0x007B_CB70),
            4 if expansion => Some(0x007B_CA78),
            4 => Some(0x007B_CB40),
            _ => None,
        },
    }
}

/// The five tints (§2 r1); the number is the index of the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    /// Refused / unusable.
    Refused = 0,
    /// Hovered, fits.
    Fits = 1,
    /// Usable item in a grid.
    Usable = 2,
    /// Swap target.
    Swap = 3,
    /// Unidentified.
    Unidentified = 4,
}

impl Tint {
    /// The (red, green, blue) triple the palette is matched to (§2 r1, r3).
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Tint::Refused => (0x80, 0, 0),
            Tint::Fits => (0, 0x80, 0),
            Tint::Usable => (0, 0, 0x80),
            Tint::Swap => (0x80, 0x80, 0),
            Tint::Unidentified => (0x80, 0x40, 0x40),
        }
    }
}

/// The palette index nearest to `rgb` (`0x00605210`, §2 r1): smallest
/// squared distance over entry bytes 0, 1, 2; the first one on a tie.
pub fn nearest_index(palette: &[[u8; 3]], rgb: (u8, u8, u8)) -> Option<u8> {
    let d = |e: &[u8; 3]| {
        let (r, g, b) = (
            i32::from(e[0]) - i32::from(rgb.0),
            i32::from(e[1]) - i32::from(rgb.1),
            i32::from(e[2]) - i32::from(rgb.2),
        );
        r * r + g * g + b * b
    };
    let mut best: Option<(i32, usize)> = None;
    for (i, e) in palette.iter().enumerate().take(256) {
        let v = d(e);
        if best.is_none_or(|(bv, _)| v < bv) {
            best = Some((v, i));
        }
    }
    best.map(|(_, i)| i as u8)
}

/// The five tint palette indices (`0x00483960`, §2 r1).
pub fn tint_indices(palette: &[[u8; 3]]) -> Option<[u8; 5]> {
    let mut out = [0u8; 5];
    for (i, t) in [
        Tint::Refused,
        Tint::Fits,
        Tint::Usable,
        Tint::Swap,
        Tint::Unidentified,
    ]
    .into_iter()
    .enumerate()
    {
        out[i] = nearest_index(palette, t.rgb())?;
    }
    Some(out)
}

/// The tinted pixel (§2 r3): draw mode 0 is blend kind 2, each pixel
/// becomes `A2[256·d + c]` (`d` destination index, `c` tint index).
pub fn tint_pixel(a2: &[u8], dest: u8, tint_index: u8) -> u8 {
    a2[256 * usize::from(dest) + usize::from(tint_index)]
}

/// Item checks used by the tints (§9).
pub mod checks {
    /// `0x0062A4E0` "usable state" (§9 r1): 1 unless the item data flags
    /// (+0x18) have 0x100 or 0x4000.
    pub fn usable_state(item_flags_18: u32) -> bool {
        item_flags_18 & (0x100 | 0x4000) == 0
    }

    /// `0x004C2240` (§9 r2): the item has state 2, or the local player
    /// has state 54 `uninterruptable`.
    pub fn busy(item_has_state2: bool, player_has_state54: bool) -> bool {
        item_has_state2 || player_has_state54
    }

    /// `0x0062E6F0` / `0x0062E740` (§9 r4): the item type's `Shoots`
    /// (itemtypes +0xC) / `Quiver` (+0xE) link, non-zero for launchers and
    /// their ammo.
    pub fn launcher_or_ammo(shoots: i32, quiver: i32) -> bool {
        shoots != 0 || quiver != 0
    }

    /// The client quest record bits `0x0065C310(record, quest, bit)`.
    pub trait QuestBits {
        fn bit(&self, quest: u8, bit: u8) -> bool;
    }

    /// Quest test `0x00483F80` (§9 r5) by item code: `ass` quest 9 bit 5
    /// clear; `xyz` quest 20 bit 5 clear; `tr2` quest 37 bit 8 clear or
    /// bit 7 set; any other code 0.
    pub fn quest_test(code: &[u8; 3], q: &dyn QuestBits) -> bool {
        match code {
            b"ass" => !q.bit(9, 5),
            b"xyz" => !q.bit(20, 5),
            b"tr2" => !q.bit(37, 8) || q.bit(37, 7),
            _ => false,
        }
    }
}

/// Facts about an item in a grid that the tint reads (§3 r3).
#[derive(Clone, Copy, Debug, Default)]
pub struct GridItemFacts {
    /// `0x0062EAF0` requirement check failed.
    pub requirements_fail: bool,
    /// Item flag 0x4.
    pub flag_4: bool,
    /// `0x004C2240` ≠ 0.
    pub busy: bool,
    /// `0x0062A4E0` = 0.
    pub not_usable: bool,
    /// Item flag 0x10 (identified).
    pub identified: bool,
    /// The class has the quest flag (+0x12A), is `ass`/`xyz`/`tr2` and the
    /// quest test holds.
    pub quest_blocked: bool,
}

/// The tint of a non-hovered grid item (§3 r3).
pub fn grid_item_tint(f: &GridItemFacts) -> Tint {
    if f.requirements_fail || f.flag_4 || f.busy || f.not_usable {
        Tint::Refused
    } else if !f.identified {
        Tint::Unidentified
    } else if f.quest_blocked {
        Tint::Refused
    } else {
        Tint::Usable
    }
}

/// The tint of the hovered item (§3 r2, §6 r4): 1, or 0 when the cursor
/// state is 8 and the item has no `Transmogrify`.
pub fn hovered_tint(cursor_state: u8, transmogrify: bool) -> Tint {
    if cursor_state == 8 && !transmogrify {
        Tint::Refused
    } else {
        Tint::Fits
    }
}

/// The grid hit test the placement tint uses (§4 r1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridHit {
    None,
    /// `0x00483AB0`: the mouse inside the grid rectangle.
    Rect,
    /// `0x00483B10` (trade page 2).
    TradePage2,
    /// `0x00483B40` (stash page 4).
    Stash,
    /// `0x00483BF0` (cube page 3).
    Cube,
}

/// Which hit test applies by inventory mode and page (§4 r1).
pub fn grid_hit_for(mode: u8, page: u8) -> GridHit {
    match mode {
        0x0B => match page {
            1 => GridHit::None,
            2 => GridHit::TradePage2,
            _ => GridHit::Rect,
        },
        0x0C | 0x0D if page == 4 => GridHit::Stash,
        0x0C | 0x0D => GridHit::Rect,
        0x0E if page == 3 => GridHit::Cube,
        0x0E => GridHit::Rect,
        _ => GridHit::None,
    }
}

/// `0x00483C50` draws only when (§4 r2): the cursor cell is ≥ 0 and
/// hover-in-grid is set; not in mode 1 or 0x13 with mouse x < width / 2;
/// not with mouse y ≥ height − 0x27.
#[allow(clippy::too_many_arguments)]
pub fn placement_draws(
    cursor_cell: (i32, i32),
    hover_in_grid: bool,
    mode: u8,
    mouse: Point,
    width: i32,
    height: i32,
) -> bool {
    cursor_cell.0 >= 0
        && cursor_cell.1 >= 0
        && hover_in_grid
        && !(matches!(mode, 1 | 0x13) && mouse.x < width / 2)
        && mouse.y < height - 0x27
}

/// What the placement tint covers (§4 r3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// Tint over the cursor item's footprint.
    Footprint(Tint),
    /// Tint over the item under the cursor item.
    OverItem(Tint),
}

/// The item found under a cursor item that does not fit (`0x0063BB20`).
#[derive(Clone, Copy, Debug)]
pub struct Under {
    pub swap_ok: bool,
    /// Code `box` (a cube).
    pub is_cube: bool,
}

/// The placement tint (§4 r3): fits → 1 over the footprint; else with the
/// item under: 0 when there is none, or when the swap fails and it is not
/// a cube; otherwise 3 over the item under.
pub fn placement_tint(fits: bool, under: Option<Under>) -> Placement {
    if fits {
        return Placement::Footprint(Tint::Fits);
    }
    match under {
        None => Placement::Footprint(Tint::Refused),
        Some(u) if !u.swap_ok && !u.is_cube => Placement::Footprint(Tint::Refused),
        Some(_) => Placement::OverItem(Tint::Swap),
    }
}

/// An equipment box rectangle (§6 r1): left, top, slot width and height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquipBox {
    pub left: i32,
    pub top: i32,
    pub w: i32,
    pub h: i32,
}

/// Where an item of `iw × ih` cells in body location `loc` is drawn (§6
/// r2): x = left + (slotW − cellW·w) / 2, y = top + (slotH − cellH·h) / 2
/// (arithmetic shift), then the per-location offsets. `weapon_swap`
/// (`0x007BCC4C` ≠ 0) never happens in 1.14d.
pub fn equip_draw_point(
    loc: u8,
    b: EquipBox,
    cell: (i32, i32),
    item: (i32, i32),
    weapon_swap: bool,
) -> Point {
    let mut x = b.left + ((b.w - cell.0 * item.0) >> 1);
    let mut y = b.top + ((b.h - cell.1 * item.1) >> 1);
    match loc {
        3 => x += 3,
        8 => {
            x += 2;
            y -= 2;
        }
        2 | 6 | 7 | 10 => y -= 2,
        9 => {
            x += 2;
            y -= 4;
        }
        4 | 5 if weapon_swap => {
            x += 4;
            y -= 4;
        }
        _ => {}
    }
    Point::new(x, y)
}

/// The two-handed ghost (§6 r3): for L 4 (or 5) with an item, when
/// `0x0063D340` = 2 and neither hand item passes `0x0062E740`, the other
/// hand's box gets tint 0 and the item is drawn there too, centred
/// vertically by its height.
pub fn two_handed_ghost(two_hand_state: u32, either_hand_quiver_or_ammo: bool) -> bool {
    two_hand_state == 2 && !either_hand_quiver_or_ammo
}

/// Facts for the tint of an equipped item's own box (§6 r4).
#[derive(Clone, Copy, Debug, Default)]
pub struct EquipItemFacts {
    pub hovered: bool,
    /// Cursor state 8.
    pub cursor_state_8: bool,
    pub transmogrify: bool,
    pub ghost: bool,
    /// `0x0062E6F0` or `0x0062E740`.
    pub shoots_or_quiver: bool,
    pub grid: GridItemFacts,
}

/// The tint of an equipped item's box (§6 r4); `None` = no tint
/// (equipped items never get tint 2).
pub fn equip_item_tint(f: &EquipItemFacts) -> Option<Tint> {
    if f.hovered {
        return Some(hovered_tint(
            if f.cursor_state_8 { 8 } else { 0 },
            f.transmogrify,
        ));
    }
    let g = &f.grid;
    if (!f.ghost && f.shoots_or_quiver) || g.requirements_fail || g.flag_4 || g.busy || g.not_usable
    {
        Some(Tint::Refused)
    } else if !g.identified {
        Some(Tint::Unidentified)
    } else {
        None
    }
}

/// `0x004843E0` (§6 r5): the cursor item can be socketed into the item
/// there.
#[derive(Clone, Copy, Debug, Default)]
pub struct SocketFacts {
    /// Filler type `0x0062BEB0` accepted.
    pub filler: bool,
    pub target_socketed: bool,
    pub target_identified: bool,
    /// Target flag 0x100.
    pub target_flag_100: bool,
    pub filled: u32,
    pub sockets: u32,
    pub mode: u8,
}

pub fn can_socket(f: &SocketFacts) -> bool {
    f.filler
        && f.target_socketed
        && f.target_identified
        && !f.target_flag_100
        && f.filled < f.sockets
        && !matches!(f.mode, 0x0A | 0x0B)
}

/// The hovered equipment box tint with a cursor item (§6 r5): tint 1 when
/// the body location accepts the item and its requirements pass, or the
/// cursor item can be socketed; else 0.
pub fn equip_hover_tint(accepts_and_requirements: bool, socketable: bool) -> Tint {
    if accepts_and_requirements || socketable {
        Tint::Fits
    } else {
        Tint::Refused
    }
}

/// Facts that pick the inventory graphic file (§8 r2).
#[derive(Clone, Copy, Debug, Default)]
pub struct InvFileFacts<'a> {
    pub identified: bool,
    pub quality: u8,
    /// `setitems` row `invfile`; the base record's `setinvfile`.
    pub set_row_invfile: &'a str,
    pub base_setinvfile: &'a str,
    /// The unique's file index; `uniqueitems` row `invfile`; the base
    /// record's `uniqueinvfile`.
    pub unique_file_index: i32,
    pub unique_row_invfile: &'a str,
    pub base_uniqueinvfile: &'a str,
    /// The item type's `VarInvGfx` and `InvGfx1`–`6`.
    pub var_inv_gfx: u8,
    pub inv_gfx: [&'a str; 6],
    /// The item's gfx variant (item data +0x49).
    pub variant: u8,
    /// The base record's `invfile`.
    pub base_invfile: &'a str,
}

/// The inventory graphic name (§8 r2).
pub fn inventory_file<'a>(f: &InvFileFacts<'a>) -> &'a str {
    if f.identified && f.quality == 5 {
        if !f.set_row_invfile.is_empty() {
            return f.set_row_invfile;
        }
        if !f.base_setinvfile.is_empty() {
            return f.base_setinvfile;
        }
    } else if f.identified && f.quality == 7 {
        if f.unique_file_index > 0 && !f.unique_row_invfile.is_empty() {
            return f.unique_row_invfile;
        }
        if !f.base_uniqueinvfile.is_empty() {
            return f.base_uniqueinvfile;
        }
    }
    if f.var_inv_gfx == 0 {
        return f.base_invfile;
    }
    let n = usize::from(f.variant)
        .min(usize::from(f.var_inv_gfx) - 1)
        .min(5);
    f.inv_gfx[n]
}

/// The archive path of an inventory graphic (§8 r2): `DATA\GLOBAL\items\
/// <name>.dc6`.
pub fn inventory_path(name: &str) -> String {
    format!("data\\global\\items\\{name}.dc6")
}

/// The item graphic draw (§8 r4): the cel is drawn at (x, top + h) in mode
/// 1 when the item is ethereal (flag 0x400000), else 5, light 0xFF.
pub fn item_graphic_draw(x: i32, top: i32, frame_h: i32, item_flags: u32) -> (Point, u8) {
    let mode = if item_flags & 0x40_0000 != 0 { 1 } else { 5 };
    (Point::new(x, top + frame_h), mode)
}

/// The item's own overlays are drawn centred on the picture (§8 r5):
/// (x + w / 2, top + h / 2).
pub fn item_overlay_centre(x: i32, top: i32, w: i32, h: i32) -> Point {
    Point::new(x + w / 2, top + h / 2)
}

/// Only an item unit (type 4) has a graphic (§8 r1).
pub fn has_item_graphic(unit_type: u8) -> bool {
    unit_type == 4
}

/// The gold picture frame (§8 r6): every gold pile draws frame 0 of
/// `invgld`, whatever the amount class.
pub fn gold_frame(_amount: u32) -> u32 {
    0
}

/// The gold amount class of §8 r2 (stat 14): 0 (< 100), 1 (100–499), 2
/// (500–4,999), 3 (≥ 5,000).
pub fn gold_class(amount: u32) -> u8 {
    match amount {
        0..=99 => 0,
        100..=499 => 1,
        500..=4999 => 2,
        _ => 3,
    }
}

/// After the grid items (§3 r4): for a hireling owner nothing more is
/// drawn; for the player, an item on the cursor adds the placement tint
/// of §4.
pub fn placement_follows(owner: OwnerKind, cursor_item: bool) -> bool {
    owner == OwnerKind::Player && cursor_item
}

/// The hover state of §5 (`0x007BCBF4`, `0x007BCBE4`, `0x00721E4C` /
/// `0x00721E50`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HoverState {
    /// `0x007BCBF4`: the hovered item.
    pub item: Option<u32>,
    /// `0x007BCBE4`: hover-in-grid.
    pub in_grid: bool,
    /// The cursor cell (§5 r2–r3).
    pub cursor_cell: (i32, i32),
}

impl HoverState {
    /// No cursor item (§5 r1): the item at the mouse cell becomes the
    /// hovered item (`0x007BCBE4` := 1) or none (both cleared). Returns
    /// whether it changed (the hover anchor is then set from its grid
    /// position, `CellGrid::hover_anchor`).
    pub fn without_cursor_item(&mut self, under: Option<u32>) -> bool {
        let changed = self.item != under;
        self.item = under;
        self.in_grid = under.is_some();
        changed
    }

    /// With a cursor item (§5 r2–r3): `cell` is [`CellGrid::cursor_cell`]'s
    /// answer; `None` = the handler returned without any change. Else the
    /// cursor cell is set, hover-in-grid := 1, the hovered item cleared.
    ///
    /// [`CellGrid::cursor_cell`]: crate::ui::widget::CellGrid::cursor_cell
    pub fn with_cursor_item(&mut self, cell: Option<(i32, i32)>) {
        if let Some(c) = cell {
            self.cursor_cell = c;
            self.in_grid = true;
            self.item = None;
        }
    }
}

/// The visibility test of the item graphic `0x004DAB40` (§8 r3): the cel's
/// extent at (x, top + h) must touch [0, W] × [0, H], else nothing is
/// drawn.
// PROVISIONAL (specs/ui/inventory.md §8 r3; REC-60): "touch" is read as
// the closed boxes intersecting; the original's exact comparisons are not
// in the spec.
pub fn item_graphic_visible(
    x: i32,
    top: i32,
    w: i32,
    h: i32,
    screen_w: i32,
    screen_h: i32,
) -> bool {
    x <= screen_w && x + w >= 0 && top <= screen_h && top + h >= 0
}

// ---- §10 grid click → C→S message ------------------------------------

/// The C→S message a grid click sends (§10; ids from
/// `sim/client-messages.tsv`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridMsg {
    /// 0x27 target, used.
    TargetUsed { target: u32, used: u32 },
    /// 0x4C item (cursor state 8).
    Transmog { item: u32 },
    /// 0x33 sell.
    Sell { item: u32 },
    /// 0x63 to belt.
    ToBelt { item: u32 },
    /// 0x19 lift to the cursor.
    Lift { item: u32 },
    /// 0x18 place item at (x, y, page).
    Place { item: u32, x: u32, y: u32, page: u8 },
    /// 0x21 stack cursor onto `under`.
    Stack { cursor: u32, under: u32 },
    /// 0x28 socket.
    Socket { cursor: u32, under: u32 },
    /// 0x29 scroll onto book.
    ScrollBook { cursor: u32, under: u32 },
    /// 0x1F swap.
    Swap {
        cursor: u32,
        under: u32,
        x: u32,
        y: u32,
    },
    /// 0x2A into the cube.
    ToCube { item: u32, cube: u32 },
}

impl GridMsg {
    /// The message id byte.
    pub fn id(&self) -> u8 {
        match self {
            GridMsg::TargetUsed { .. } => 0x27,
            GridMsg::Transmog { .. } => 0x4C,
            GridMsg::Sell { .. } => 0x33,
            GridMsg::ToBelt { .. } => 0x63,
            GridMsg::Lift { .. } => 0x19,
            GridMsg::Place { .. } => 0x18,
            GridMsg::Stack { .. } => 0x21,
            GridMsg::Socket { .. } => 0x28,
            GridMsg::ScrollBook { .. } => 0x29,
            GridMsg::Swap { .. } => 0x1F,
            GridMsg::ToCube { .. } => 0x2A,
        }
    }
}

/// The outcome of a click (§10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridClick {
    pub msg: Option<GridMsg>,
    /// The "cannot" note `0x004CB9C0`.
    pub cannot_note: bool,
    /// The NPC store click `0x004B3870` is to run (no message here).
    pub npc_store_click: bool,
    /// `[0x007BCBEC]` := 1 (after the lift).
    pub lifted_flag: bool,
    /// Return value 1.
    pub consumed: bool,
}

/// The item under a grid cell or under the cursor item.
#[derive(Clone, Copy, Debug)]
pub struct ItemRef {
    pub id: u32,
    /// Code `box`.
    pub is_cube: bool,
    /// The cursor item can stack onto it (`0x0062C850` ≠ 0).
    pub stackable_onto: bool,
    /// Type 18 `book`, and the kind (`0x00627F80`).
    pub book_kind: Option<u32>,
    /// Sellable (`0x0062A130`).
    pub sellable: bool,
    /// Fits the belt (`0x0062BAD0`).
    pub fits_belt: bool,
}

/// Everything §10 reads.
#[derive(Clone, Copy, Debug)]
pub struct ClickCtx {
    pub cursor_state: u8,
    /// The used item for cursor state 6 (`0x004680A0`).
    pub used_item: Option<u32>,
    pub cursor_item: Option<ItemRef>,
    /// Cursor item is itself a cube / a scroll with its kind (type 22).
    pub cursor_scroll_kind: Option<u32>,
    /// The item at the mouse cell (`0x0063BD10`).
    pub under_mouse: Option<ItemRef>,
    /// `0x004C2240` = 0 and the send throttle `0x00486D10` ≠ 0.
    pub ready: bool,
    pub own_player: bool,
    /// The player's own inventory context (`0x00486B30`).
    pub own_inventory_context: bool,
    pub inventory_mode: u8,
    pub page: u8,
    pub shift: bool,
    pub ctrl: bool,
    /// A store open with its NPC.
    pub store_open: bool,
    /// Placement test at the cursor cell: the item under it and the
    /// overlap count.
    pub overlap_item: Option<ItemRef>,
    pub overlap_count: u32,
    /// `0x0063BB20` finds a cube under the footprint.
    pub cube_under_footprint: Option<ItemRef>,
    /// The drop cell from the mouse (`0x00486BD0`) places (placement test
    /// there passes).
    pub drop_cell: Option<(u32, u32)>,
    /// A swap candidate by `0x0063BB20` (single overlap).
    pub swap_ok: bool,
    /// The cursor cell (for swap sends).
    pub cursor_cell: (u32, u32),
    /// Free space on page 3 of the cube grid (`0x0063B850`).
    pub cube_has_room: bool,
    /// The cursor item can fill a socket of the item under it
    /// (`0x004843E0`, §6 r5).
    pub cursor_can_socket: bool,
}

/// `0x0048FFE0` (§10): the message a left click on a page grid sends.
pub fn grid_click(c: &ClickCtx) -> GridClick {
    let mut out = GridClick::default();
    let page_ok = c.page != 1 && c.page != 2;
    // r1 cursor state 6
    if c.cursor_state == 6 {
        if let (Some(u), Some(used)) = (c.under_mouse, c.used_item) {
            if c.ready && c.own_player && page_ok {
                out.msg = Some(GridMsg::TargetUsed { target: u.id, used });
            }
            out.consumed = true;
            return out;
        }
    }
    // r2 cursor state 8
    if c.cursor_state == 8 {
        if let Some(u) = c.under_mouse {
            if c.ready && c.own_player && page_ok {
                out.msg = Some(GridMsg::Transmog { item: u.id });
            }
            out.consumed = true;
            return out;
        }
    }
    match c.cursor_item {
        None => {
            let Some(u) = c.under_mouse else { return out };
            out.consumed = true;
            // r3.1
            if !c.own_inventory_context {
                out.npc_store_click = c.inventory_mode <= 0x12;
                return out;
            }
            // r3.2
            if !c.ready {
                return out;
            }
            // r3.3
            if c.ctrl {
                if c.store_open {
                    if u.sellable {
                        out.msg = Some(GridMsg::Sell { item: u.id });
                    } else {
                        out.cannot_note = true;
                    }
                }
                return out;
            }
            // r3.4 / r3.5
            if c.shift && u.fits_belt && c.page == 0 {
                out.msg = Some(GridMsg::ToBelt { item: u.id });
            } else {
                out.msg = Some(GridMsg::Lift { item: u.id });
                out.lifted_flag = true;
            }
        }
        Some(cur) => {
            out.consumed = true;
            // r4.1
            let mut under = c.overlap_item;
            let mut n = c.overlap_count;
            if n >= 2 {
                if let Some(cube) = c.cube_under_footprint {
                    return into_cube(c, cur, cube, out);
                }
                n = 1;
            }
            // r4.2
            if n == 0 || under.is_none() {
                if c.page == 3 && cur.is_cube {
                    return out;
                }
                if let (Some((x, y)), true) = (c.drop_cell, c.ready) {
                    out.msg = Some(GridMsg::Place {
                        item: cur.id,
                        x,
                        y,
                        page: c.page,
                    });
                }
                return out;
            }
            // r4.3
            let u = under.take().unwrap();
            let mut sent = false;
            if u.stackable_onto {
                if c.ready && c.inventory_mode != 0x0B && c.page != 2 {
                    out.msg = Some(GridMsg::Stack {
                        cursor: cur.id,
                        under: u.id,
                    });
                }
                return out;
            }
            if c.cursor_can_socket && c.ready {
                out.msg = Some(GridMsg::Socket {
                    cursor: cur.id,
                    under: u.id,
                });
                sent = true;
            }
            if !sent {
                if let (Some(sk), Some(bk)) = (c.cursor_scroll_kind, u.book_kind) {
                    if sk == bk && c.ready && c.inventory_mode != 0x0B {
                        out.msg = Some(GridMsg::ScrollBook {
                            cursor: cur.id,
                            under: u.id,
                        });
                        sent = true;
                    }
                }
            }
            if u.is_cube {
                return into_cube(c, cur, u, out);
            }
            if !sent && c.swap_ok && c.ready && c.page != 2 {
                out.msg = Some(GridMsg::Swap {
                    cursor: cur.id,
                    under: u.id,
                    x: c.cursor_cell.0,
                    y: c.cursor_cell.1,
                });
            }
        }
    }
    out
}

/// r4.4: into the cube.
fn into_cube(c: &ClickCtx, cur: ItemRef, cube: ItemRef, mut out: GridClick) -> GridClick {
    if c.ready {
        if c.cube_has_room {
            out.msg = Some(GridMsg::ToCube {
                item: cur.id,
                cube: cube.id,
            });
        } else {
            out.cannot_note = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::CellGrid;
    use crate::ui::WidgetId;

    /// Page-0 record of 800 × 600 (test vectors): left 419, top 315,
    /// 29 × 29 cells, 10 × 4.
    fn rec16() -> GridRecord {
        GridRecord {
            grid_x: 10,
            grid_y: 4,
            left: 419,
            right: 419 + 290,
            top: 315,
            bottom: 315 + 116,
            cell_w: 29,
            cell_h: 29,
        }
    }

    fn grid() -> CellGrid {
        CellGrid::new(WidgetId(1), Point::new(419, 315), 10, 4, 29, 29).unwrap()
    }

    // Covers: specs/ui/inventory.md §1 r1, §1 r3, §1 r4
    #[test]
    fn record_geometry() {
        let mut b = [0u8; 24];
        b[0] = 10;
        b[1] = 4;
        b[4..8].copy_from_slice(&419i32.to_le_bytes());
        b[8..12].copy_from_slice(&709i32.to_le_bytes());
        b[12..16].copy_from_slice(&315i32.to_le_bytes());
        b[16..20].copy_from_slice(&431i32.to_le_bytes());
        b[0x14] = 29;
        b[0x15] = 29;
        assert_eq!(GridRecord::parse(&b), rec16());
        let r = rec16();
        assert_eq!(r.cell(0, 0), (419, 315, 29, 29));
        assert_eq!(r.cell(2, 1), (419 + 58, 315 + 29, 29, 29));
        // a cell is tinted only with its top-left corner inside the clip
        assert!(r.cell_tinted(0, 0, 800, 600));
        assert!(!r.cell_tinted(0, 0, 400, 600));
        assert!(!r.cell_tinted(0, 0, 800, 315));
        assert!(r.cell_tinted(0, 0, 420, 316));
        // mouse → cell, unsigned: left of the grid wraps to a huge value
        assert_eq!(r.mouse_cell(Point::new(500, 340)), (2, 0));
        assert!(r.mouse_cell(Point::new(400, 340)).0 > 1000);
        assert_eq!(
            r.mouse_cell(Point::new(500, 340)),
            grid().mouse_cell(Point::new(500, 340))
        );
        // the same through the widget: footprint of a 2 × 3 item
        assert_eq!(grid().footprint(0, 0, 1, 1, 800, 600).len(), 1);
        assert_eq!(grid().footprint(0, 0, 2, 3, 800, 600).len(), 6);
        // §4 r1 rectangle test: right edge exclusive, bottom inclusive
        assert!(r.contains_mouse(Point::new(419, 315)));
        assert!(!r.contains_mouse(Point::new(709, 315)));
        assert!(r.contains_mouse(Point::new(708, 431)));
        assert!(!r.contains_mouse(Point::new(708, 432)));
    }

    // Covers: specs/ui/inventory.md §1 r2
    #[test]
    fn page_records() {
        let p = OwnerKind::Player;
        assert_eq!(record_address(p, 0, false), Some(0x007B_CB88));
        assert_eq!(record_address(p, 1, false), Some(0x007B_CB18));
        assert_eq!(record_address(p, 2, true), Some(0x007B_CA30));
        assert_eq!(record_address(p, 3, true), Some(0x007B_CB70));
        assert_eq!(record_address(p, 4, false), Some(0x007B_CB40));
        assert_eq!(record_address(p, 4, true), Some(0x007B_CA78));
        assert_eq!(
            record_address(OwnerKind::Monster, 0, true),
            Some(0x007B_CB58)
        );
        assert_eq!(record_address(OwnerKind::Other, 0, true), None);
    }

    // Covers: specs/ui/inventory.md §2 r1, §2 r2, §2 r3
    #[test]
    fn tints_and_palette() {
        assert_eq!(Tint::Refused as u8, 0);
        assert_eq!(Tint::Unidentified as u8, 4);
        assert_eq!(Tint::Refused.rgb(), (0x80, 0, 0));
        assert_eq!(Tint::Fits.rgb(), (0, 0x80, 0));
        assert_eq!(Tint::Usable.rgb(), (0, 0, 0x80));
        assert_eq!(Tint::Swap.rgb(), (0x80, 0x80, 0));
        assert_eq!(Tint::Unidentified.rgb(), (0x80, 0x40, 0x40));
        // a palette where entries 7 and 9 tie for (0x80, 0, 0): the first wins
        let mut pal = vec![[0u8, 0, 0]; 256];
        pal[7] = [0x7F, 0, 0];
        pal[9] = [0x7F, 0, 0];
        pal[3] = [0, 0x80, 0];
        pal[5] = [0, 0, 0x80];
        pal[11] = [0x80, 0x80, 0];
        pal[13] = [0x80, 0x40, 0x40];
        assert_eq!(tint_indices(&pal), Some([7, 3, 5, 11, 13]));
        assert_eq!(nearest_index(&[], (0, 0, 0)), None);
        // draw mode 0: pixel A2[256·d + c]
        let mut a2 = vec![0u8; 65536];
        a2[256 * 40 + 5] = 99;
        assert_eq!(tint_pixel(&a2, 40, 5), 99);
    }

    // Covers: specs/ui/inventory.md §3 r2, §3 r3, §9 r3
    #[test]
    fn item_tints() {
        let ok = GridItemFacts {
            identified: true,
            ..Default::default()
        };
        assert_eq!(grid_item_tint(&ok), Tint::Usable);
        assert_eq!(
            grid_item_tint(&GridItemFacts {
                identified: false,
                ..ok
            }),
            Tint::Unidentified
        );
        for f in [
            GridItemFacts {
                requirements_fail: true,
                ..ok
            },
            GridItemFacts { flag_4: true, ..ok },
            GridItemFacts { busy: true, ..ok },
            GridItemFacts {
                not_usable: true,
                ..ok
            },
            GridItemFacts {
                quest_blocked: true,
                ..ok
            },
            // red wins over unidentified
            GridItemFacts {
                requirements_fail: true,
                identified: false,
                ..ok
            },
        ] {
            assert_eq!(grid_item_tint(&f), Tint::Refused, "{f:?}");
        }
        // hovered: 1, or 0 for a shop cursor over a non-transmogrifiable item
        assert_eq!(hovered_tint(0, false), Tint::Fits);
        assert_eq!(hovered_tint(8, true), Tint::Fits);
        assert_eq!(hovered_tint(8, false), Tint::Refused);
    }

    struct Q(Vec<(u8, u8)>);
    impl checks::QuestBits for Q {
        fn bit(&self, quest: u8, bit: u8) -> bool {
            self.0.contains(&(quest, bit))
        }
    }

    // Covers: specs/ui/inventory.md §9 r1, §9 r2, §9 r5
    #[test]
    fn item_checks() {
        assert!(checks::usable_state(0));
        assert!(!checks::usable_state(0x100));
        assert!(!checks::usable_state(0x4000));
        assert!(checks::usable_state(0x10));
        assert!(!checks::busy(false, false));
        assert!(checks::busy(true, false));
        assert!(checks::busy(false, true));
        let none = Q(vec![]);
        assert!(checks::quest_test(b"ass", &none));
        assert!(!checks::quest_test(b"ass", &Q(vec![(9, 5)])));
        assert!(checks::quest_test(b"xyz", &none));
        assert!(!checks::quest_test(b"xyz", &Q(vec![(20, 5)])));
        assert!(checks::quest_test(b"tr2", &none));
        assert!(!checks::quest_test(b"tr2", &Q(vec![(37, 8)])));
        assert!(checks::quest_test(b"tr2", &Q(vec![(37, 8), (37, 7)])));
        assert!(!checks::quest_test(b"box", &none));
    }

    // Covers: specs/ui/inventory.md §4 r1, §4 r2, §4 r3
    #[test]
    fn placement() {
        assert_eq!(grid_hit_for(0x0B, 1), GridHit::None);
        assert_eq!(grid_hit_for(0x0B, 2), GridHit::TradePage2);
        assert_eq!(grid_hit_for(0x0B, 0), GridHit::Rect);
        assert_eq!(grid_hit_for(0x0C, 4), GridHit::Stash);
        assert_eq!(grid_hit_for(0x0D, 0), GridHit::Rect);
        assert_eq!(grid_hit_for(0x0E, 3), GridHit::Cube);
        assert_eq!(grid_hit_for(0x0E, 0), GridHit::Rect);
        assert_eq!(grid_hit_for(0, 0), GridHit::None);
        let m = Point::new(600, 300);
        assert!(placement_draws((2, 0), true, 0, m, 800, 600));
        assert!(!placement_draws((-1, 0), true, 0, m, 800, 600));
        assert!(!placement_draws((2, 0), false, 0, m, 800, 600));
        assert!(!placement_draws(
            (2, 0),
            true,
            1,
            Point::new(100, 300),
            800,
            600
        ));
        assert!(placement_draws(
            (2, 0),
            true,
            2,
            Point::new(100, 300),
            800,
            600
        ));
        assert!(!placement_draws(
            (2, 0),
            true,
            0,
            Point::new(600, 600 - 0x27),
            800,
            600
        ));
        assert!(placement_draws(
            (2, 0),
            true,
            0,
            Point::new(600, 600 - 0x28),
            800,
            600
        ));
        // fits: tint 1 over the footprint
        assert_eq!(placement_tint(true, None), Placement::Footprint(Tint::Fits));
        // nothing under, or swap fails and not a cube: tint 0
        assert_eq!(
            placement_tint(false, None),
            Placement::Footprint(Tint::Refused)
        );
        let u = |swap_ok, is_cube| Some(Under { swap_ok, is_cube });
        assert_eq!(
            placement_tint(false, u(false, false)),
            Placement::Footprint(Tint::Refused)
        );
        // swap possible, or a cube: tint 3 over the item under
        assert_eq!(
            placement_tint(false, u(true, false)),
            Placement::OverItem(Tint::Swap)
        );
        assert_eq!(
            placement_tint(false, u(false, true)),
            Placement::OverItem(Tint::Swap)
        );
    }

    // Covers: specs/ui/inventory.md §5 r1, §5 r3
    #[test]
    fn hover_and_cursor_cell() {
        let g = grid();
        // anchor of a 2 × 3 item at (1, 0): x = 419 + 29 + 29, top 315,
        // bottom 315 + 87
        assert_eq!(g.hover_anchor(1, 0, 2, 3), (419 + 29 + 29, 315, 315 + 87));
        // 2 × 3 cursor item, graphic 56 × 84, mouse (500, 340) → (2, 0)
        assert_eq!(
            g.cursor_cell(Point::new(500, 340), 2, 3, 56, 84),
            Some((2, 0))
        );
        // mouse (700, 340): c = 9, 2 + 9 > 10 → no change
        assert_eq!(g.cursor_cell(Point::new(700, 340), 2, 3, 56, 84), None);
        // a 1 × 1 item: plain cell
        assert_eq!(
            g.cursor_cell(Point::new(500, 340), 1, 1, 28, 28),
            Some((2, 0))
        );
    }

    // Covers: specs/ui/inventory.md §6 r2, §6 r3, §6 r4, §6 r5
    #[test]
    fn equipment() {
        let b = EquipBox {
            left: 100,
            top: 200,
            w: 58,
            h: 87,
        };
        let at = |loc, w, h| equip_draw_point(loc, b, (29, 29), (w, h), false);
        // 2 × 3 item fills the slot exactly: no centring offset
        assert_eq!(at(1, 2, 3), Point::new(100, 200));
        // 1 × 1 item: (58 − 29) >> 1 = 14, (87 − 29) >> 1 = 29
        assert_eq!(at(1, 1, 1), Point::new(114, 229));
        assert_eq!(at(3, 1, 1), Point::new(117, 229));
        assert_eq!(at(8, 1, 1), Point::new(116, 227));
        for l in [2, 6, 7, 10] {
            assert_eq!(at(l, 1, 1), Point::new(114, 227), "loc {l}");
        }
        assert_eq!(at(9, 1, 1), Point::new(116, 225));
        // hands: no offset unless the weapon-swap state is set (never)
        assert_eq!(at(4, 1, 1), Point::new(114, 229));
        assert_eq!(
            equip_draw_point(4, b, (29, 29), (1, 1), true),
            Point::new(118, 225)
        );
        // negative centring uses an arithmetic shift
        assert_eq!(
            equip_draw_point(
                1,
                EquipBox {
                    left: 0,
                    top: 0,
                    w: 28,
                    h: 28
                },
                (29, 29),
                (1, 1),
                false
            ),
            Point::new(-1, -1)
        );
        // two-handed ghost
        assert!(two_handed_ghost(2, false));
        assert!(!two_handed_ghost(2, true));
        assert!(!two_handed_ghost(1, false));
        // own-box tint
        let base = EquipItemFacts {
            grid: GridItemFacts {
                identified: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(equip_item_tint(&base), None);
        assert_eq!(
            equip_item_tint(&EquipItemFacts {
                hovered: true,
                ..base
            }),
            Some(Tint::Fits)
        );
        assert_eq!(
            equip_item_tint(&EquipItemFacts {
                hovered: true,
                cursor_state_8: true,
                ..base
            }),
            Some(Tint::Refused)
        );
        assert_eq!(
            equip_item_tint(&EquipItemFacts {
                shoots_or_quiver: true,
                ..base
            }),
            Some(Tint::Refused)
        );
        assert_eq!(
            equip_item_tint(&EquipItemFacts {
                shoots_or_quiver: true,
                ghost: true,
                ..base
            }),
            None
        );
        let unid = EquipItemFacts {
            grid: GridItemFacts {
                identified: false,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(equip_item_tint(&unid), Some(Tint::Unidentified));
        // socket test
        let s = SocketFacts {
            filler: true,
            target_socketed: true,
            target_identified: true,
            target_flag_100: false,
            filled: 1,
            sockets: 2,
            mode: 1,
        };
        assert!(can_socket(&s));
        assert!(!can_socket(&SocketFacts { filled: 2, ..s }));
        assert!(!can_socket(&SocketFacts {
            target_flag_100: true,
            ..s
        }));
        assert!(!can_socket(&SocketFacts {
            target_identified: false,
            ..s
        }));
        assert!(!can_socket(&SocketFacts {
            target_socketed: false,
            ..s
        }));
        assert!(!can_socket(&SocketFacts { filler: false, ..s }));
        assert!(!can_socket(&SocketFacts { mode: 0x0A, ..s }));
        assert!(!can_socket(&SocketFacts { mode: 0x0B, ..s }));
        assert_eq!(equip_hover_tint(true, false), Tint::Fits);
        assert_eq!(equip_hover_tint(false, true), Tint::Fits);
        assert_eq!(equip_hover_tint(false, false), Tint::Refused);
    }

    // Covers: specs/ui/inventory.md §8 r1, §8 r2, §8 r4, §8 r5, §8 r6
    #[test]
    fn item_graphic() {
        assert!(has_item_graphic(4));
        assert!(!has_item_graphic(1));
        let base = InvFileFacts {
            identified: true,
            quality: 2,
            var_inv_gfx: 5,
            inv_gfx: ["invrin1", "invrin2", "invrin3", "invrin4", "invrin5", ""],
            variant: 7,
            base_invfile: "invrin",
            ..Default::default()
        };
        // ring with gfx variant 7 (VarInvGfx 5): clamp to 4 → invrin5
        assert_eq!(inventory_file(&base), "invrin5");
        assert_eq!(
            inventory_file(&InvFileFacts { variant: 0, ..base }),
            "invrin1"
        );
        assert_eq!(
            inventory_file(&InvFileFacts {
                var_inv_gfx: 0,
                ..base
            }),
            "invrin"
        );
        // identified unique: row empty, base uniqueinvfile invxyz
        let uq = InvFileFacts {
            quality: 7,
            unique_file_index: 3,
            base_uniqueinvfile: "invxyz",
            ..base
        };
        assert_eq!(inventory_file(&uq), "invxyz");
        assert_eq!(
            inventory_file(&InvFileFacts {
                unique_row_invfile: "invrow",
                ..uq
            }),
            "invrow"
        );
        // file index 0 never reaches the row
        assert_eq!(
            inventory_file(&InvFileFacts {
                unique_row_invfile: "invrow",
                unique_file_index: 0,
                ..uq
            }),
            "invxyz"
        );
        // ... and falls through when uniqueinvfile is empty
        assert_eq!(
            inventory_file(&InvFileFacts {
                base_uniqueinvfile: "",
                unique_file_index: 0,
                ..uq
            }),
            "invrin5"
        );
        // unidentified unique: the plain rule
        assert_eq!(
            inventory_file(&InvFileFacts {
                identified: false,
                ..uq
            }),
            "invrin5"
        );
        // set item
        let st = InvFileFacts {
            quality: 5,
            set_row_invfile: "invset",
            base_setinvfile: "invbase",
            ..base
        };
        assert_eq!(inventory_file(&st), "invset");
        assert_eq!(
            inventory_file(&InvFileFacts {
                set_row_invfile: "",
                ..st
            }),
            "invbase"
        );
        assert_eq!(
            inventory_file(&InvFileFacts {
                set_row_invfile: "",
                base_setinvfile: "",
                ..st
            }),
            "invrin5"
        );
        assert_eq!(inventory_path("invxyz"), "data\\global\\items\\invxyz.dc6");
        // ethereal sword at (419, 315), frame 28 × 84: mode 1 at (419, 399)
        assert_eq!(
            item_graphic_draw(419, 315, 84, 0x40_0000),
            (Point::new(419, 399), 1)
        );
        assert_eq!(
            item_graphic_draw(419, 315, 84, 0),
            (Point::new(419, 399), 5)
        );
        assert_eq!(item_overlay_centre(10, 20, 29, 85), Point::new(24, 62));
        // gold: every pile draws frame 0
        for a in [0, 99, 100, 499, 500, 4999, 5000, 1_000_000] {
            assert_eq!(gold_frame(a), 0);
        }
        assert_eq!(
            [0, 99, 100, 499, 500, 4999, 5000].map(gold_class),
            [0, 0, 1, 1, 2, 2, 3]
        );
    }

    fn item(id: u32) -> ItemRef {
        ItemRef {
            id,
            is_cube: false,
            stackable_onto: false,
            book_kind: None,
            sellable: false,
            fits_belt: false,
        }
    }

    fn ctx() -> ClickCtx {
        ClickCtx {
            cursor_state: 0,
            used_item: None,
            cursor_item: None,
            cursor_scroll_kind: None,
            under_mouse: None,
            ready: true,
            own_player: true,
            own_inventory_context: true,
            inventory_mode: 1,
            page: 0,
            shift: false,
            ctrl: false,
            store_open: false,
            overlap_item: None,
            overlap_count: 0,
            cube_under_footprint: None,
            drop_cell: None,
            swap_ok: false,
            cursor_cell: (0, 0),
            cube_has_room: true,
            cursor_can_socket: false,
        }
    }

    // Covers: specs/ui/inventory.md §10 r1, §10 r2, §10 r3, §10 r5
    #[test]
    fn click_without_cursor_item() {
        // r1 cursor state 6
        let c = ClickCtx {
            cursor_state: 6,
            used_item: Some(9),
            under_mouse: Some(item(5)),
            ..ctx()
        };
        let r = grid_click(&c);
        assert_eq!(r.msg, Some(GridMsg::TargetUsed { target: 5, used: 9 }));
        assert_eq!(r.msg.unwrap().id(), 0x27);
        assert!(r.consumed);
        // page 1 / 2 or not ready: consumed, no message
        for c in [
            ClickCtx { page: 1, ..c },
            ClickCtx { ready: false, ..c },
            ClickCtx {
                own_player: false,
                ..c
            },
        ] {
            let r = grid_click(&c);
            assert_eq!((r.msg, r.consumed), (None, true));
        }
        // r2 cursor state 8
        let c = ClickCtx {
            cursor_state: 8,
            under_mouse: Some(item(5)),
            ..ctx()
        };
        assert_eq!(grid_click(&c).msg, Some(GridMsg::Transmog { item: 5 }));
        assert_eq!(GridMsg::Transmog { item: 5 }.id(), 0x4C);
        // r3.5: plain click lifts, flag set
        let c = ClickCtx {
            under_mouse: Some(item(5)),
            ..ctx()
        };
        let r = grid_click(&c);
        assert_eq!(r.msg, Some(GridMsg::Lift { item: 5 }));
        assert!(r.lifted_flag);
        assert_eq!(r.msg.unwrap().id(), 0x19);
        // r3.4: Shift with a belt-fitting item on page 0: to belt
        let belt = ItemRef {
            fits_belt: true,
            ..item(5)
        };
        let c = ClickCtx {
            under_mouse: Some(belt),
            shift: true,
            ..ctx()
        };
        let r = grid_click(&c);
        assert_eq!(r.msg, Some(GridMsg::ToBelt { item: 5 }));
        assert_eq!(r.msg.unwrap().id(), 0x63);
        assert!(!r.lifted_flag);
        // Shift on another page lifts
        let c = ClickCtx {
            under_mouse: Some(belt),
            shift: true,
            page: 4,
            ..ctx()
        };
        assert_eq!(grid_click(&c).msg, Some(GridMsg::Lift { item: 5 }));
        // r3.3 Ctrl: sell with a store; "cannot" when not sellable; nothing
        // without a store; never lifts
        let sell = ItemRef {
            sellable: true,
            ..item(5)
        };
        let c = ClickCtx {
            under_mouse: Some(sell),
            ctrl: true,
            store_open: true,
            ..ctx()
        };
        assert_eq!(grid_click(&c).msg, Some(GridMsg::Sell { item: 5 }));
        assert_eq!(GridMsg::Sell { item: 5 }.id(), 0x33);
        let c = ClickCtx {
            under_mouse: Some(item(5)),
            ctrl: true,
            store_open: true,
            ..ctx()
        };
        let r = grid_click(&c);
        assert_eq!((r.msg, r.cannot_note), (None, true));
        let c = ClickCtx {
            under_mouse: Some(item(5)),
            ctrl: true,
            ..ctx()
        };
        let r = grid_click(&c);
        assert_eq!((r.msg, r.consumed), (None, true));
        // r3.2 not ready: nothing
        let c = ClickCtx {
            under_mouse: Some(item(5)),
            ready: false,
            ..ctx()
        };
        assert_eq!(grid_click(&c).msg, None);
        // r3.1 not the own inventory context: the NPC store click, no message
        let c = ClickCtx {
            under_mouse: Some(item(5)),
            own_inventory_context: false,
            ..ctx()
        };
        let r = grid_click(&c);
        assert_eq!((r.msg, r.npc_store_click), (None, true));
        let c = ClickCtx {
            inventory_mode: 0x13,
            ..c
        };
        assert!(!grid_click(&c).npc_store_click);
        // nothing under the mouse: not consumed
        assert!(!grid_click(&ctx()).consumed);
    }

    // Covers: specs/ui/inventory.md §10 r4
    #[test]
    fn click_with_cursor_item() {
        let cur = item(1);
        let base = ClickCtx {
            cursor_item: Some(cur),
            ..ctx()
        };
        // r4.2: empty cell (3, 1), page 0 → 0x18 [item, 3, 1, 0]
        let c = ClickCtx {
            drop_cell: Some((3, 1)),
            ..base
        };
        let r = grid_click(&c);
        assert_eq!(
            r.msg,
            Some(GridMsg::Place {
                item: 1,
                x: 3,
                y: 1,
                page: 0
            })
        );
        assert_eq!(r.msg.unwrap().id(), 0x18);
        // a cube on page 3 is refused, consumed
        let cube = ItemRef {
            is_cube: true,
            ..cur
        };
        let c = ClickCtx {
            cursor_item: Some(cube),
            page: 3,
            drop_cell: Some((0, 0)),
            ..base
        };
        let r = grid_click(&c);
        assert_eq!((r.msg, r.consumed), (None, true));
        // r4.3 stack
        let under = ItemRef {
            stackable_onto: true,
            ..item(2)
        };
        let c = ClickCtx {
            overlap_item: Some(under),
            overlap_count: 1,
            ..base
        };
        assert_eq!(
            grid_click(&c).msg,
            Some(GridMsg::Stack {
                cursor: 1,
                under: 2
            })
        );
        assert_eq!(
            GridMsg::Stack {
                cursor: 1,
                under: 2
            }
            .id(),
            0x21
        );
        let c = ClickCtx {
            inventory_mode: 0x0B,
            ..c
        };
        assert_eq!(grid_click(&c).msg, None);
        // socket
        let c = ClickCtx {
            overlap_item: Some(item(2)),
            overlap_count: 1,
            cursor_can_socket: true,
            ..base
        };
        assert_eq!(
            grid_click(&c).msg,
            Some(GridMsg::Socket {
                cursor: 1,
                under: 2
            })
        );
        assert_eq!(
            GridMsg::Socket {
                cursor: 1,
                under: 2
            }
            .id(),
            0x28
        );
        // scroll on a book of the same kind
        let book = ItemRef {
            book_kind: Some(7),
            ..item(2)
        };
        let c = ClickCtx {
            overlap_item: Some(book),
            overlap_count: 1,
            cursor_scroll_kind: Some(7),
            ..base
        };
        assert_eq!(
            grid_click(&c).msg,
            Some(GridMsg::ScrollBook {
                cursor: 1,
                under: 2
            })
        );
        let c = ClickCtx {
            cursor_scroll_kind: Some(8),
            ..c
        };
        assert_eq!(grid_click(&c).msg, None);
        // swap
        let c = ClickCtx {
            overlap_item: Some(item(2)),
            overlap_count: 1,
            swap_ok: true,
            cursor_cell: (4, 2),
            ..base
        };
        assert_eq!(
            grid_click(&c).msg,
            Some(GridMsg::Swap {
                cursor: 1,
                under: 2,
                x: 4,
                y: 2
            })
        );
        assert_eq!(
            GridMsg::Swap {
                cursor: 1,
                under: 2,
                x: 4,
                y: 2
            }
            .id(),
            0x1F
        );
        assert_eq!(grid_click(&ClickCtx { page: 2, ..c }).msg, None);
        // r4.4 into the cube: n ≥ 2 with a cube under the footprint, or the
        // item under is a cube
        let cubei = ItemRef {
            is_cube: true,
            ..item(9)
        };
        let c = ClickCtx {
            overlap_item: Some(item(2)),
            overlap_count: 2,
            cube_under_footprint: Some(cubei),
            ..base
        };
        assert_eq!(
            grid_click(&c).msg,
            Some(GridMsg::ToCube { item: 1, cube: 9 })
        );
        assert_eq!(GridMsg::ToCube { item: 1, cube: 9 }.id(), 0x2A);
        let c = ClickCtx {
            overlap_item: Some(cubei),
            overlap_count: 1,
            ..base
        };
        assert_eq!(
            grid_click(&c).msg,
            Some(GridMsg::ToCube { item: 1, cube: 9 })
        );
        // no room: the "cannot" note
        let c = ClickCtx {
            cube_has_room: false,
            ..c
        };
        let r = grid_click(&c);
        assert_eq!((r.msg, r.cannot_note), (None, true));
        // n ≥ 2 without a cube behaves as n = 1 (swap)
        let c = ClickCtx {
            overlap_item: Some(item(2)),
            overlap_count: 3,
            swap_ok: true,
            ..base
        };
        assert!(matches!(grid_click(&c).msg, Some(GridMsg::Swap { .. })));
    }

    // Covers: specs/ui/inventory.md §3 r4, §5 r1, §5 r2, §b5-cellgrid-answers-client-ui-md-b5
    #[test]
    fn hover_state_and_cellgrid_answers() {
        assert!(placement_follows(OwnerKind::Player, true));
        assert!(!placement_follows(OwnerKind::Player, false));
        assert!(!placement_follows(OwnerKind::Monster, true));
        let g = grid();
        let mut h = HoverState::default();
        // hovering an item sets the flags; the same item again: unchanged
        assert!(h.without_cursor_item(Some(7)));
        assert!(h.in_grid && h.item == Some(7));
        assert!(!h.without_cursor_item(Some(7)));
        // leaving it clears both
        assert!(h.without_cursor_item(None));
        assert!(!h.in_grid && h.item.is_none());
        // with a cursor item: cell set, hover-in-grid := 1, hovered cleared
        h.item = Some(9);
        h.with_cursor_item(g.cursor_cell(Point::new(500, 340), 2, 3, 56, 84));
        assert_eq!((h.cursor_cell, h.in_grid, h.item), ((2, 0), true, None));
        // the handler returned without change: nothing moves
        let before = h;
        h.with_cursor_item(g.cursor_cell(Point::new(700, 340), 2, 3, 56, 84));
        assert_eq!(h, before);
        // CellGrid answers (§B5): cells adjacent, pitch = cell size
        use crate::ui::widget::Cell;
        assert_eq!(
            g.cell_at(Point::new(419, 315)),
            Some(Cell { col: 0, row: 0 })
        );
        assert_eq!(
            g.cell_at(Point::new(419 + 29, 315 + 29)),
            Some(Cell { col: 1, row: 1 })
        );
        assert_eq!(
            g.cell_at(Point::new(419 + 289, 315 + 115)),
            Some(Cell { col: 9, row: 3 })
        );
        assert_eq!(g.cell_at(Point::new(419 + 290, 315)), None);
        assert_eq!(g.cell_at(Point::new(418, 315)), None);
        let r = g.cell_rect(Cell { col: 2, row: 1 }).unwrap();
        assert_eq!((r.x, r.y, r.w, r.h), (419 + 58, 315 + 29, 29, 29));
        assert!(g.cell_rect(Cell { col: 10, row: 0 }).is_none());
        // the item graphic's top-left sits at the cell corner for offsets 0
        assert_eq!(g.item_draw_point(1, 2, 84), Point::new(448, 315 + 58 + 84));
    }

    // Covers: specs/ui/inventory.md §9 r4, §8 r3
    #[test]
    fn launcher_links_and_visibility() {
        assert!(checks::launcher_or_ammo(5, 0));
        assert!(checks::launcher_or_ammo(0, 7));
        assert!(!checks::launcher_or_ammo(0, 0));
        assert!(item_graphic_visible(419, 315, 28, 84, 800, 600));
        assert!(item_graphic_visible(-10, 0, 28, 84, 800, 600));
        assert!(!item_graphic_visible(-40, 0, 28, 84, 800, 600));
        assert!(!item_graphic_visible(900, 0, 28, 84, 800, 600));
        assert!(!item_graphic_visible(0, 700, 28, 84, 800, 600));
    }
}

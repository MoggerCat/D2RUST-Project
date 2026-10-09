// Spec: specs/ui/controls.md (§1–§5, §7 r1–r3)
//! The original's key bindings (`ui/controls.md`): the 114-entry binding
//! table and its lookups (§1), the `.key` files (§2), the command table
//! (§3), keyboard and mouse dispatch (§4), key assignment (§5) and the
//! gates and belt use (§7). Plain Rust; the portable `controls` module
//! (our `d2controls` file) is separate and unchanged.

/// Key 0xFFFF = unbound (§1 r1).
pub const UNBOUND: u16 = 0xFFFF;
/// Entries in the table (§1 r1).
pub const TABLE_LEN: usize = 114;
/// Table size in bytes: 114 × 10 (0x474).
pub const TABLE_BYTES: usize = TABLE_LEN * 10;
/// Commands 0–56 (§1 r4).
pub const COMMAND_COUNT: usize = 57;
/// File version (§2 r1).
pub const FILE_VERSION: u16 = 0x25;
/// `default.key` magic `"WS"` (§2 r2).
pub const DEFAULT_MAGIC: u16 = 0x5357;
/// Size of a character `.key` file (§2 r1): version + table.
pub const CHAR_FILE_BYTES: usize = 2 + TABLE_BYTES;
/// Size of `default.key` (§2 r2).
pub const DEFAULT_FILE_BYTES: usize = 6 + TABLE_BYTES;

pub const MIDDLE: u16 = 0x100;
pub const X1: u16 = 0x101;
pub const X2: u16 = 0x102;
pub const WHEEL_UP: u16 = 0x103;
pub const WHEEL_DOWN: u16 = 0x104;

/// One record of the command table `0x00712698` joined with the default
/// bindings `0x00712220` (`key-commands.tsv`, §3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CmdDef {
    pub cmd: u8,
    /// `string.tbl` id (0: none, command 56).
    pub string_id: u32,
    /// Default key of slot 1 / slot 0.
    pub key1: u16,
    pub key2: u16,
    /// Handler addresses (0 = none).
    pub down: u32,
    pub up: u32,
    /// Runs in key mode 2 (§1 r4).
    pub full_ok: bool,
    /// Position of the command's two default entries (2p slot 1, 2p + 1
    /// slot 0).
    pub file_pos: u8,
    /// Row in the key-config tables (§3.3); −1 = not listed.
    pub menu_classic: i16,
    pub menu_exp: i16,
}

/// The 57 commands (`key-commands.tsv`).
pub const COMMANDS: [CmdDef; COMMAND_COUNT] = [
    CmdDef {
        cmd: 0,
        string_id: 3924,
        key1: 0x0041,
        key2: 0x0043,
        down: 0x00468940,
        up: 0x00000000,
        full_ok: false,
        file_pos: 0,
        menu_classic: 0,
        menu_exp: 0,
    },
    CmdDef {
        cmd: 1,
        string_id: 3925,
        key1: 0x0049,
        key2: 0x0042,
        down: 0x00468950,
        up: 0x00000000,
        full_ok: false,
        file_pos: 1,
        menu_classic: 1,
        menu_exp: 1,
    },
    CmdDef {
        cmd: 2,
        string_id: 3926,
        key1: 0x0050,
        key2: 0xFFFF,
        down: 0x00468960,
        up: 0x00000000,
        full_ok: false,
        file_pos: 2,
        menu_classic: 2,
        menu_exp: 2,
    },
    CmdDef {
        cmd: 3,
        string_id: 3927,
        key1: 0x004D,
        key2: 0xFFFF,
        down: 0x00468980,
        up: 0x00000000,
        full_ok: false,
        file_pos: 3,
        menu_classic: 3,
        menu_exp: 4,
    },
    CmdDef {
        cmd: 4,
        string_id: 3928,
        key1: 0x0051,
        key2: 0xFFFF,
        down: 0x00468990,
        up: 0x00000000,
        full_ok: false,
        file_pos: 4,
        menu_classic: 4,
        menu_exp: 5,
    },
    CmdDef {
        cmd: 5,
        string_id: 3929,
        key1: 0x000D,
        key2: 0xFFFF,
        down: 0x004689B0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 5,
        menu_classic: 26,
        menu_exp: 36,
    },
    CmdDef {
        cmd: 6,
        string_id: 3933,
        key1: 0x0048,
        key2: 0xFFFF,
        down: 0x004689D0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 6,
        menu_classic: 5,
        menu_exp: 6,
    },
    CmdDef {
        cmd: 7,
        string_id: 3930,
        key1: 0x0009,
        key2: 0x0100,
        down: 0x00468A30,
        up: 0x00000000,
        full_ok: false,
        file_pos: 7,
        menu_classic: 33,
        menu_exp: 43,
    },
    CmdDef {
        cmd: 8,
        string_id: 3931,
        key1: 0x0078,
        key2: 0xFFFF,
        down: 0x00468A60,
        up: 0x00000000,
        full_ok: false,
        file_pos: 8,
        menu_classic: 34,
        menu_exp: 44,
    },
    CmdDef {
        cmd: 9,
        string_id: 3983,
        key1: 0x0079,
        key2: 0xFFFF,
        down: 0x00468A70,
        up: 0x00000000,
        full_ok: false,
        file_pos: 9,
        menu_classic: 35,
        menu_exp: 45,
    },
    CmdDef {
        cmd: 10,
        string_id: 3985,
        key1: 0x007A,
        key2: 0xFFFF,
        down: 0x00468A90,
        up: 0x00000000,
        full_ok: false,
        file_pos: 10,
        menu_classic: 36,
        menu_exp: 46,
    },
    CmdDef {
        cmd: 11,
        string_id: 3984,
        key1: 0x007B,
        key2: 0xFFFF,
        down: 0x00468AB0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 11,
        menu_classic: 37,
        menu_exp: 47,
    },
    CmdDef {
        cmd: 12,
        string_id: 3934,
        key1: 0x0054,
        key2: 0xFFFF,
        down: 0x00468AF0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 12,
        menu_classic: 7,
        menu_exp: 8,
    },
    CmdDef {
        cmd: 13,
        string_id: 10833,
        key1: 0x0053,
        key2: 0xFFFF,
        down: 0x00468B00,
        up: 0x00000000,
        full_ok: false,
        file_pos: 13,
        menu_classic: 8,
        menu_exp: 9,
    },
    CmdDef {
        cmd: 14,
        string_id: 3936,
        key1: 0x0070,
        key2: 0xFFFF,
        down: 0x00468B90,
        up: 0x00000000,
        full_ok: false,
        file_pos: 14,
        menu_classic: 9,
        menu_exp: 10,
    },
    CmdDef {
        cmd: 15,
        string_id: 3937,
        key1: 0x0071,
        key2: 0xFFFF,
        down: 0x00468BC0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 15,
        menu_classic: 10,
        menu_exp: 11,
    },
    CmdDef {
        cmd: 16,
        string_id: 3938,
        key1: 0x0072,
        key2: 0xFFFF,
        down: 0x00468BF0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 16,
        menu_classic: 11,
        menu_exp: 12,
    },
    CmdDef {
        cmd: 17,
        string_id: 3939,
        key1: 0x0073,
        key2: 0xFFFF,
        down: 0x00468C20,
        up: 0x00000000,
        full_ok: false,
        file_pos: 17,
        menu_classic: 12,
        menu_exp: 13,
    },
    CmdDef {
        cmd: 18,
        string_id: 3940,
        key1: 0x0074,
        key2: 0xFFFF,
        down: 0x00468C50,
        up: 0x00000000,
        full_ok: false,
        file_pos: 18,
        menu_classic: 13,
        menu_exp: 14,
    },
    CmdDef {
        cmd: 19,
        string_id: 3941,
        key1: 0x0075,
        key2: 0xFFFF,
        down: 0x00468C80,
        up: 0x00000000,
        full_ok: false,
        file_pos: 19,
        menu_classic: 14,
        menu_exp: 15,
    },
    CmdDef {
        cmd: 20,
        string_id: 3942,
        key1: 0x0076,
        key2: 0xFFFF,
        down: 0x00468CB0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 20,
        menu_classic: 15,
        menu_exp: 16,
    },
    CmdDef {
        cmd: 21,
        string_id: 3943,
        key1: 0x0077,
        key2: 0xFFFF,
        down: 0x00468CE0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 45,
        menu_classic: 16,
        menu_exp: 17,
    },
    CmdDef {
        cmd: 22,
        string_id: 3946,
        key1: 0x00C0,
        key2: 0xFFFF,
        down: 0x00468F00,
        up: 0x00000000,
        full_ok: true,
        file_pos: 21,
        menu_classic: 20,
        menu_exp: 29,
    },
    CmdDef {
        cmd: 23,
        string_id: 3947,
        key1: 0x0031,
        key2: 0xFFFF,
        down: 0x00468F40,
        up: 0x00000000,
        full_ok: false,
        file_pos: 22,
        menu_classic: 21,
        menu_exp: 30,
    },
    CmdDef {
        cmd: 24,
        string_id: 3948,
        key1: 0x0032,
        key2: 0xFFFF,
        down: 0x00468F50,
        up: 0x00000000,
        full_ok: false,
        file_pos: 23,
        menu_classic: 22,
        menu_exp: 31,
    },
    CmdDef {
        cmd: 25,
        string_id: 3949,
        key1: 0x0033,
        key2: 0xFFFF,
        down: 0x00468F60,
        up: 0x00000000,
        full_ok: false,
        file_pos: 24,
        menu_classic: 23,
        menu_exp: 32,
    },
    CmdDef {
        cmd: 26,
        string_id: 3950,
        key1: 0x0034,
        key2: 0xFFFF,
        down: 0x00468F70,
        up: 0x00000000,
        full_ok: false,
        file_pos: 25,
        menu_classic: 24,
        menu_exp: 33,
    },
    CmdDef {
        cmd: 27,
        string_id: 3959,
        key1: 0x0060,
        key2: 0xFFFF,
        down: 0x00468F80,
        up: 0x00000000,
        full_ok: true,
        file_pos: 26,
        menu_classic: 39,
        menu_exp: 50,
    },
    CmdDef {
        cmd: 28,
        string_id: 3960,
        key1: 0x0061,
        key2: 0xFFFF,
        down: 0x00468F90,
        up: 0x00000000,
        full_ok: true,
        file_pos: 27,
        menu_classic: 40,
        menu_exp: 51,
    },
    CmdDef {
        cmd: 29,
        string_id: 3961,
        key1: 0x0062,
        key2: 0xFFFF,
        down: 0x00468FA0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 28,
        menu_classic: 41,
        menu_exp: 52,
    },
    CmdDef {
        cmd: 30,
        string_id: 3962,
        key1: 0x0063,
        key2: 0xFFFF,
        down: 0x00468FB0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 29,
        menu_classic: 42,
        menu_exp: 53,
    },
    CmdDef {
        cmd: 31,
        string_id: 3963,
        key1: 0x0064,
        key2: 0xFFFF,
        down: 0x00468FC0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 30,
        menu_classic: 43,
        menu_exp: 54,
    },
    CmdDef {
        cmd: 32,
        string_id: 3964,
        key1: 0x0065,
        key2: 0xFFFF,
        down: 0x00468FD0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 31,
        menu_classic: 44,
        menu_exp: 55,
    },
    CmdDef {
        cmd: 33,
        string_id: 3965,
        key1: 0x0066,
        key2: 0xFFFF,
        down: 0x00468FE0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 32,
        menu_classic: 45,
        menu_exp: 56,
    },
    CmdDef {
        cmd: 34,
        string_id: 3966,
        key1: 0x0011,
        key2: 0xFFFF,
        down: 0x00469000,
        up: 0x004691C0,
        full_ok: false,
        file_pos: 33,
        menu_classic: 27,
        menu_exp: 37,
    },
    CmdDef {
        cmd: 35,
        string_id: 3967,
        key1: 0x0052,
        key2: 0x0102,
        down: 0x00469060,
        up: 0x00000000,
        full_ok: false,
        file_pos: 34,
        menu_classic: 28,
        menu_exp: 38,
    },
    CmdDef {
        cmd: 36,
        string_id: 3968,
        key1: 0x0010,
        key2: 0xFFFF,
        down: 0x00469080,
        up: 0x00469200,
        full_ok: false,
        file_pos: 35,
        menu_classic: 29,
        menu_exp: 39,
    },
    CmdDef {
        cmd: 37,
        string_id: 3969,
        key1: 0x0012,
        key2: 0x0101,
        down: 0x00469090,
        up: 0x00469210,
        full_ok: false,
        file_pos: 36,
        menu_classic: 30,
        menu_exp: 40,
    },
    CmdDef {
        cmd: 38,
        string_id: 3970,
        key1: 0x0020,
        key2: 0xFFFF,
        down: 0x004690A0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 37,
        menu_classic: 49,
        menu_exp: 60,
    },
    CmdDef {
        cmd: 39,
        string_id: 3944,
        key1: 0x0103,
        key2: 0xFFFF,
        down: 0x00469100,
        up: 0x00000000,
        full_ok: false,
        file_pos: 38,
        menu_classic: 17,
        menu_exp: 26,
    },
    CmdDef {
        cmd: 40,
        string_id: 3945,
        key1: 0x0104,
        key2: 0xFFFF,
        down: 0x00469120,
        up: 0x00000000,
        full_ok: false,
        file_pos: 39,
        menu_classic: 18,
        menu_exp: 27,
    },
    CmdDef {
        cmd: 41,
        string_id: 3981,
        key1: 0x004E,
        key2: 0xFFFF,
        down: 0x004691B0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 40,
        menu_classic: 50,
        menu_exp: 61,
    },
    CmdDef {
        cmd: 42,
        string_id: 3971,
        key1: 0x002C,
        key2: 0xFFFF,
        down: 0x00000000,
        up: 0x004FA7A0,
        full_ok: true,
        file_pos: 41,
        menu_classic: 48,
        menu_exp: 59,
    },
    CmdDef {
        cmd: 43,
        string_id: 3982,
        key1: 0x005A,
        key2: 0xFFFF,
        down: 0x00493840,
        up: 0x00000000,
        full_ok: false,
        file_pos: 42,
        menu_classic: 31,
        menu_exp: 41,
    },
    CmdDef {
        cmd: 44,
        string_id: 22726,
        key1: 0x0057,
        key2: 0xFFFF,
        down: 0x00469140,
        up: 0x00000000,
        full_ok: true,
        file_pos: 43,
        menu_classic: -1,
        menu_exp: 34,
    },
    CmdDef {
        cmd: 45,
        string_id: 22725,
        key1: 0x0056,
        key2: 0xFFFF,
        down: 0x00468AD0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 44,
        menu_classic: -1,
        menu_exp: 48,
    },
    CmdDef {
        cmd: 46,
        string_id: 22717,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468D10,
        up: 0x00000000,
        full_ok: false,
        file_pos: 46,
        menu_classic: -1,
        menu_exp: 18,
    },
    CmdDef {
        cmd: 47,
        string_id: 22718,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468D40,
        up: 0x00000000,
        full_ok: false,
        file_pos: 47,
        menu_classic: -1,
        menu_exp: 19,
    },
    CmdDef {
        cmd: 48,
        string_id: 22719,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468D80,
        up: 0x00000000,
        full_ok: false,
        file_pos: 48,
        menu_classic: -1,
        menu_exp: 20,
    },
    CmdDef {
        cmd: 49,
        string_id: 22720,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468DC0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 49,
        menu_classic: -1,
        menu_exp: 21,
    },
    CmdDef {
        cmd: 50,
        string_id: 22721,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468E00,
        up: 0x00000000,
        full_ok: false,
        file_pos: 50,
        menu_classic: -1,
        menu_exp: 22,
    },
    CmdDef {
        cmd: 51,
        string_id: 22722,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468E40,
        up: 0x00000000,
        full_ok: false,
        file_pos: 51,
        menu_classic: -1,
        menu_exp: 23,
    },
    CmdDef {
        cmd: 52,
        string_id: 22723,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468E80,
        up: 0x00000000,
        full_ok: false,
        file_pos: 52,
        menu_classic: -1,
        menu_exp: 24,
    },
    CmdDef {
        cmd: 53,
        string_id: 22724,
        key1: 0xFFFF,
        key2: 0xFFFF,
        down: 0x00468EC0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 53,
        menu_classic: -1,
        menu_exp: 25,
    },
    CmdDef {
        cmd: 54,
        string_id: 22727,
        key1: 0x004F,
        key2: 0xFFFF,
        down: 0x00469170,
        up: 0x00000000,
        full_ok: false,
        file_pos: 54,
        menu_classic: -1,
        menu_exp: 3,
    },
    CmdDef {
        cmd: 55,
        string_id: 11083,
        key1: 0x0067,
        key2: 0xFFFF,
        down: 0x00468FF0,
        up: 0x00000000,
        full_ok: true,
        file_pos: 55,
        menu_classic: 46,
        menu_exp: 57,
    },
    CmdDef {
        cmd: 56,
        string_id: 0,
        key1: 0x001B,
        key2: 0xFFFF,
        down: 0x004690B0,
        up: 0x00000000,
        full_ok: false,
        file_pos: 56,
        menu_classic: -1,
        menu_exp: -1,
    },
];

/// One binding: i32 command, u16 key, i32 slot (§1 r1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub cmd: i32,
    pub key: u16,
    pub slot: i32,
}

/// The live binding table (§1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingTable(pub Vec<Binding>);

/// The commands whose slot-0 entry precedes their slot-1 entry in the
/// compiled table (`controls.md` §3.4; every other command is slot 1
/// first). Measured on `Game.exe` 0x312220: command 1 (`B`, then `I`).
const SLOT0_FIRST: [u8; 1] = [1];

impl BindingTable {
    /// The compiled defaults `0x00712220` (§3, §B4): entries in
    /// `file_pos` order, slot 1 then slot 0, except command 1 (slot 0
    /// first; `controls.md` §3.4, measured on `Game.exe`).
    pub fn defaults() -> BindingTable {
        let mut v = vec![
            Binding {
                cmd: 0,
                key: UNBOUND,
                slot: 0
            };
            TABLE_LEN
        ];
        for c in &COMMANDS {
            let p = usize::from(c.file_pos);
            let one = Binding {
                cmd: i32::from(c.cmd),
                key: c.key1,
                slot: 1,
            };
            let zero = Binding {
                cmd: i32::from(c.cmd),
                key: c.key2,
                slot: 0,
            };
            let (first, second) = if SLOT0_FIRST.contains(&c.cmd) {
                (zero, one)
            } else {
                (one, zero)
            };
            v[2 * p] = first;
            v[2 * p + 1] = second;
        }
        BindingTable(v)
    }

    /// The 1140 table bytes: i32 cmd, u16 key, i32 slot per entry.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(TABLE_BYTES);
        for b in &self.0 {
            out.extend_from_slice(&b.cmd.to_le_bytes());
            out.extend_from_slice(&b.key.to_le_bytes());
            out.extend_from_slice(&b.slot.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<BindingTable> {
        if b.len() != TABLE_BYTES {
            return None;
        }
        Some(BindingTable(
            b.as_chunks::<10>()
                .0
                .iter()
                .map(|e| Binding {
                    cmd: i32::from_le_bytes([e[0], e[1], e[2], e[3]]),
                    key: u16::from_le_bytes([e[4], e[5]]),
                    slot: i32::from_le_bytes([e[6], e[7], e[8], e[9]]),
                })
                .collect(),
        ))
    }

    /// Key of (command, slot) `0x00469AA0`: the first entry with that
    /// command and slot; 0xFFFF if none.
    pub fn key_of(&self, cmd: i32, slot: i32) -> u16 {
        self.0
            .iter()
            .find(|b| b.cmd == cmd && b.slot == slot)
            .map_or(UNBOUND, |b| b.key)
    }

    /// "Is bound" `0x00469D90`.
    pub fn is_bound(&self, cmd: i32, slot: i32) -> bool {
        self.key_of(cmd, slot) != UNBOUND
    }

    /// Unbind (command, slot) `0x00469D70`: every matching entry.
    pub fn unbind(&mut self, cmd: i32, slot: i32) {
        for b in &mut self.0 {
            if b.cmd == cmd && b.slot == slot {
                b.key = UNBOUND;
            }
        }
    }

    /// The character-file acceptance test of §2 r3: every command 0–56
    /// occurs as the command of at least one entry, and no two entries
    /// hold the same key other than 0xFFFF. The slot is not checked.
    pub fn is_valid(&self) -> bool {
        if self.0.len() != TABLE_LEN {
            return false;
        }
        for c in 0..COMMAND_COUNT as i32 {
            if !self.0.iter().any(|b| b.cmd == c) {
                return false;
            }
        }
        for (i, a) in self.0.iter().enumerate() {
            if a.key != UNBOUND && self.0[i + 1..].iter().any(|b| b.key == a.key) {
                return false;
            }
        }
        true
    }
}

/// A character `.key` file (§2 r1): u16 version 0x25 + the table.
pub fn char_file_bytes(t: &BindingTable) -> Vec<u8> {
    let mut v = FILE_VERSION.to_le_bytes().to_vec();
    v.extend(t.to_bytes());
    v
}

/// `default.key` (§2 r2): u16 magic, u16 version, u16 size 0x47A, table.
pub fn default_file_bytes(t: &BindingTable) -> Vec<u8> {
    let mut v = DEFAULT_MAGIC.to_le_bytes().to_vec();
    v.extend(FILE_VERSION.to_le_bytes());
    v.extend((DEFAULT_FILE_BYTES as u16).to_le_bytes());
    v.extend(t.to_bytes());
    v
}

/// The accepted character file: exactly 0x476 bytes, version 0x25, a
/// valid table (§2 r3).
pub fn parse_char_file(b: &[u8]) -> Option<BindingTable> {
    if b.len() != CHAR_FILE_BYTES || u16::from_le_bytes([b[0], b[1]]) != FILE_VERSION {
        return None;
    }
    BindingTable::from_bytes(&b[2..]).filter(BindingTable::is_valid)
}

/// A `default.key` buffer is used when its magic, version and size match
/// (§2 r3); any other buffer is dropped.
pub fn parse_default_file(b: &[u8]) -> Option<BindingTable> {
    if b.len() != DEFAULT_FILE_BYTES {
        return None;
    }
    let w = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    if w(0) != DEFAULT_MAGIC || w(2) != FILE_VERSION || usize::from(w(4)) != DEFAULT_FILE_BYTES {
        return None;
    }
    BindingTable::from_bytes(&b[6..])
}

/// What the game-start load produced (§2 r3, r4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyLoad {
    pub table: BindingTable,
    /// Both files are written (§2 r4): only after the fallback path.
    pub write_files: bool,
}

/// `0x0046AAE0` (§2 r3): the character file, else `<save>default.key`
/// (read in full), else the archive `default.key`; the compiled defaults
/// stay when the chosen buffer is invalid. `None` = the file could not be
/// opened or read.
pub fn load_at_start(
    char_file: Option<&[u8]>,
    save_default: Option<&[u8]>,
    archive_default: Option<&[u8]>,
) -> KeyLoad {
    if let Some(t) = char_file.and_then(parse_char_file) {
        return KeyLoad {
            table: t,
            write_files: false,
        };
    }
    // the save-directory file, once read (0x47A bytes), is never followed
    // by the archive file
    let buf = match save_default {
        Some(b) if b.len() == DEFAULT_FILE_BYTES => Some(b),
        _ => archive_default,
    };
    let table = buf
        .and_then(parse_default_file)
        .unwrap_or_else(BindingTable::defaults);
    KeyLoad {
        table,
        write_files: true,
    }
}

// ---- §4.1 keyboard ----------------------------------------------------

/// The first entry (table order) whose key equals `vk`, whose command is
/// < 0x39 and has a down handler (§4.1 r2); `key_mode` 2 lets only
/// commands with the M2 flag run (r4). Auto-repeat is ignored; F4 with Alt
/// does nothing.
pub fn key_down_command(
    t: &BindingTable,
    vk: u16,
    auto_repeat: bool,
    alt_down: bool,
    key_mode: u8,
) -> Option<u8> {
    if auto_repeat {
        return None;
    }
    let b = t.0.iter().find(|b| {
        b.key == vk && (0..0x39).contains(&b.cmd) && COMMANDS[b.cmd as usize].down != 0
    })?;
    if vk == 0x73 && alt_down {
        return None;
    }
    let c = &COMMANDS[b.cmd as usize];
    (key_mode != 2 || c.full_ok).then_some(c.cmd)
}

/// Key up (§4.1 r3): the first entry whose key equals `vk` and whose
/// command has an up handler; the repeat bit is not tested.
pub fn key_up_command(t: &BindingTable, vk: u16, key_mode: u8) -> Option<u8> {
    let b = t
        .0
        .iter()
        .find(|b| b.key == vk && (0..0x39).contains(&b.cmd) && COMMANDS[b.cmd as usize].up != 0)?;
    let c = &COMMANDS[b.cmd as usize];
    (key_mode != 2 || c.full_ok).then_some(c.cmd)
}

/// Events that change the key mode (§4.1 r5, §7 r4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyModeEvent {
    GameStart,
    GameEnd,
    UiOpen(u8),
    UiClose(u8),
    KeyConfigOpen,
    /// `0x00453EE0` (gold dialog closes) / `0x00454150` (opens).
    LatchClear,
    LatchSet,
}

/// The key mode after an event: (mode, keep key-up). `None` = unchanged.
/// `open` tells whether a UI state is open (for the chat close rule).
pub fn key_mode_for(ev: KeyModeEvent, open: &dyn Fn(u8) -> bool) -> Option<(u8, bool)> {
    use KeyModeEvent::*;
    match ev {
        GameStart => Some((1, false)),
        GameEnd => Some((0, false)),
        UiOpen(5) | UiOpen(23) => Some((0, true)),
        UiOpen(12) | UiOpen(25) | UiOpen(26) => Some((2, false)),
        UiClose(12) | UiClose(23) | UiClose(25) | UiClose(26) => Some((1, false)),
        UiClose(5) => {
            if [12, 23, 25, 26, 27, 28, 29, 30, 32]
                .iter()
                .any(|&u| open(u))
            {
                None
            } else {
                Some((1, false))
            }
        }
        KeyConfigOpen => Some((0, false)),
        LatchSet => Some((0, true)),
        LatchClear => Some((1, false)),
        _ => None,
    }
}

/// Windows keys are swallowed (§4.1 r6, `0x0044C5C0`).
pub fn swallow_vk(vk: u16) -> bool {
    matches!(vk, 0x5B..=0x5D)
}

/// `WM_SYSCOMMAND` swallowing (§4.1 r6): `SC_KEYMENU` and `SC_SCREENSAVE`;
/// `SC_MOVE` when `0x004F5A30` is non-zero.
pub fn swallow_syscommand(cmd: u16, f5a30: bool) -> bool {
    match cmd & 0xFFF0 {
        0xF100 | 0xF140 => true,
        0xF010 => f5a30,
        _ => false,
    }
}

// ---- §4.2 mouse buttons and wheel -------------------------------------

/// The command each configurable mouse button / wheel runs (§4.2 r1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseSlots {
    pub middle_down: Option<u8>,
    pub middle_up: Option<u8>,
    pub x1_down: Option<u8>,
    pub x1_up: Option<u8>,
    pub x2_down: Option<u8>,
    pub x2_up: Option<u8>,
    pub wheel_up: Option<u8>,
    pub wheel_down: Option<u8>,
}

/// `0x004694A0` (§4.2 r1): scans the table after a change; a wheel binding
/// to a command with an up handler is removed (key := 0xFFFF); several
/// entries on one button: the last in table order wins.
pub fn scan_mouse_slots(t: &mut BindingTable) -> MouseSlots {
    let mut s = MouseSlots::default();
    for b in &mut t.0 {
        if !(MIDDLE..=WHEEL_DOWN).contains(&b.key) || !(0..0x39).contains(&b.cmd) {
            continue;
        }
        let c = &COMMANDS[b.cmd as usize];
        let down = (c.down != 0).then_some(c.cmd);
        let up = (c.up != 0).then_some(c.cmd);
        match b.key {
            MIDDLE => {
                s.middle_down = down;
                s.middle_up = up;
            }
            X1 => {
                s.x1_down = down;
                s.x1_up = up;
            }
            X2 => {
                s.x2_down = down;
                s.x2_up = up;
            }
            wheel => {
                if c.up != 0 {
                    b.key = UNBOUND;
                } else if wheel == WHEEL_UP {
                    s.wheel_up = down;
                } else {
                    s.wheel_down = down;
                }
            }
        }
    }
    s
}

/// X button events (§4.2 r2): high word of wParam 1 → 0x101, 2 → 0x102.
pub fn x_button_key(wparam: u32) -> Option<u16> {
    match wparam >> 16 {
        1 => Some(X1),
        2 => Some(X2),
        _ => None,
    }
}

/// The wheel accumulator `0x007A06F8` (§4.2 r3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WheelAccumulator(pub i32);

impl WheelAccumulator {
    /// One wheel event while in game: add the signed delta; when |acc| >
    /// 0x77 the handler of the direction runs (key 0x103 for acc > 0,
    /// 0x104 for acc < 0) and acc := 0; one action per event.
    pub fn event(&mut self, delta: i32, in_game: bool) -> Option<u16> {
        if !in_game {
            return None;
        }
        self.0 += delta;
        if self.0.abs() > 0x77 {
            let k = if self.0 > 0 { WHEEL_UP } else { WHEEL_DOWN };
            self.0 = 0;
            Some(k)
        } else {
            None
        }
    }
}

// ---- §4.3 modifiers ---------------------------------------------------

/// The flag word a world action passes to `0x00462D00` (§4.3 r1): 8 when
/// Run held or run lock is set, plus 4 when Stand Still is held.
pub fn world_action_flags(run_held: bool, run_lock: bool, stand_still: bool) -> u32 {
    (if run_held || run_lock { 8 } else { 0 }) + if stand_still { 4 } else { 0 }
}

/// `WM_ACTIVATEAPP` with wParam = 0 clears Run held when the key of
/// command 34 is bound and not physically down (§4.3 r3).
pub fn deactivate_clears_run(t: &BindingTable, key_down: &dyn Fn(u16) -> bool) -> bool {
    [1, 0].iter().any(|&slot| {
        let k = t.key_of(34, slot);
        k != UNBOUND && !key_down(k)
    })
}

/// Stand Still re-sampled from the keys bound to command 36, both slots
/// (§4.3 r2).
pub fn stand_still_sample(t: &BindingTable, key_down: &dyn Fn(u16) -> bool) -> bool {
    [1, 0].iter().any(|&slot| {
        let k = t.key_of(36, slot);
        k != UNBOUND && key_down(k)
    })
}

// ---- §5 key assignment ------------------------------------------------

/// The error strings of the key-config screen (§5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignError {
    /// `string.tbl` 3979 `CantAssignKey`.
    CantAssignKey,
    /// 3978 `CantAssignMW`.
    CantAssignMw,
}

impl AssignError {
    pub fn string_id(self) -> u32 {
        match self {
            AssignError::CantAssignKey => 3979,
            AssignError::CantAssignMw => 3978,
        }
    }
}

/// `0x00469AE0` (§5 r1): which keys the screen accepts.
pub fn key_allowed(k: u16) -> bool {
    if k >= 0x100 {
        return true;
    }
    matches!(k,
        0x30..=0x39 | 0x41..=0x5A
        | 8 | 9 | 0xC | 0xD | 0x10..=0x14 | 0x20..=0x24 | 0x2A | 0x2C..=0x2F
        | 0x60..=0x87 | 0x91 | 0xBA..=0xC0 | 0xDB..=0xDE)
}

/// Assign key `k` to (command, slot) `0x00469C20` (§5): command < 0x38,
/// key allowed; the wheel on a command with an up handler and Print
/// Screen on a command with a down handler are refused; then the first
/// other entry holding `k` is unbound and `k` is written to the entry (if
/// there is none, `k` is written back where it was).
pub fn assign_key(t: &mut BindingTable, cmd: i32, slot: i32, k: u16) -> Result<(), AssignError> {
    if !(0..0x38).contains(&cmd) || !key_allowed(k) {
        return Err(AssignError::CantAssignKey);
    }
    let c = &COMMANDS[cmd as usize];
    if matches!(k, WHEEL_UP | WHEEL_DOWN) && c.up != 0 {
        return Err(AssignError::CantAssignMw);
    }
    if k == 0x2C && c.down != 0 {
        return Err(AssignError::CantAssignKey);
    }
    let target = t.0.iter().position(|b| b.cmd == cmd && b.slot == slot);
    let other =
        t.0.iter()
            .enumerate()
            .find(|(i, b)| b.key == k && Some(*i) != target)
            .map(|(i, _)| i);
    if let Some(o) = other {
        t.0[o].key = UNBOUND;
    }
    match target {
        Some(i) => t.0[i].key = k,
        None => {
            if let Some(o) = other {
                t.0[o].key = k;
            }
        }
    }
    Ok(())
}

// ---- §3.1 / §3.2 / §7 -------------------------------------------------

/// Skill hotkey of a command (§3): cmds 14–21 → 0–7, 46–53 → 8–15.
pub fn hotkey_of_command(cmd: u8) -> Option<u8> {
    match cmd {
        14..=21 => Some(cmd - 14),
        46..=53 => Some(cmd - 46 + 8),
        _ => None,
    }
}

/// What a skill hotkey does (§3.1 r1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    /// Ui 3 (skill speed bar) open: assign the hovered skill to k.
    Assign(u8),
    /// Otherwise use it.
    Use(u8),
}

pub fn hotkey_action(k: u8, speed_bar_open: bool) -> HotkeyAction {
    if speed_bar_open {
        HotkeyAction::Assign(k)
    } else {
        HotkeyAction::Use(k)
    }
}

/// The use result of hotkey k (§3.1 r2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotkeyUse {
    /// The skill sent (`0x004786A0` with `0x007C0768[k]`).
    pub send_value: u32,
    /// The skill becomes the left skill (flag ≠ 0), else the right one.
    pub left: bool,
}

/// `0x004AA030` (§3.1 r2): nothing when the hotkey's skill is −1 or
/// `0x004A9FC0` refuses; else the last hotkey := k.
pub fn hotkey_use(
    k: usize,
    skill: &[i32; 16],
    send: &[u32; 16],
    left_flag: &[u8; 16],
    allowed: bool,
    last_hotkey: &mut usize,
) -> Option<HotkeyUse> {
    if skill[k] == -1 || !allowed {
        return None;
    }
    *last_hotkey = k;
    Some(HotkeyUse {
        send_value: send[k],
        left: left_flag[k] != 0,
    })
}

/// The gate of a handler (§7 r1): it does nothing while the client's exit
/// flag is set or there is no local player or it is dead (mode 0x11).
pub fn gate_blocks(exit_flag: bool, player_mode: Option<u8>) -> bool {
    exit_flag || player_mode.is_none_or(|m| m == 0x11)
}

/// Facts of a belt-use attempt `0x00498A90` (§7 r3).
#[derive(Clone, Copy, Debug)]
pub struct BeltUseFacts {
    pub expansion: bool,
    pub shift: bool,
    pub cursor_item: bool,
    pub ui9_open: bool,
    /// The item in belt slot c (GUID), if any.
    pub item: Option<u32>,
    /// `quest` (+0x12A) and `unique` (+0x129) are not 1 (`0x00498A20`).
    pub passes_quest_unique: bool,
    /// `useable` (+0x11D) is 1.
    pub useable: bool,
    /// `0x004C2240(item)` ≠ 0.
    pub busy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeltUse {
    Nothing,
    /// `0x0049FF90` and stop.
    QuestUniqueHandler,
    /// The player event sound `0x004CB9C0` and stop.
    NotUseableSound,
    /// C→S 0x26 [item, shift 0 or 0x8000, 0].
    Send {
        item: u32,
        shift: u32,
    },
}

pub fn belt_use(f: &BeltUseFacts) -> BeltUse {
    let shift = f.shift && f.expansion;
    if f.cursor_item || f.ui9_open {
        return BeltUse::Nothing;
    }
    let Some(item) = f.item else {
        return BeltUse::Nothing;
    };
    if !f.passes_quest_unique {
        return BeltUse::QuestUniqueHandler;
    }
    if !f.useable {
        return BeltUse::NotUseableSound;
    }
    if f.busy {
        return BeltUse::Nothing;
    }
    BeltUse::Send {
        item,
        shift: if shift { 0x8000 } else { 0 },
    }
}

/// A belt key (§3.2 r1, §7 r2): only when the column-ready byte is 1.
/// Returns the shift flag (`GetAsyncKeyState(VK_SHIFT) & 0x8000`).
pub fn belt_key(column_ready: u8, shift_state: i16) -> Option<bool> {
    (column_ready == 1).then_some(shift_state as u16 & 0x8000 != 0)
}

/// One row of the key-config menu tables (§3.3): command (57 =
/// separator) and its string id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuRow {
    pub cmd: i32,
    pub string_id: u32,
}

/// Separator row command (§3.3).
pub const SEPARATOR: i32 = 57;

/// The key-config table of a game (§3.3): 62 rows for the expansion (separators at 7, 28, 35, 42, 49, 58), 51 for classic
/// (6, 19, 25, 32, 38, 47).
pub fn menu_table(expansion: bool) -> Vec<MenuRow> {
    let (len, seps): (usize, &[usize]) = if expansion {
        (62, &[7, 28, 35, 42, 49, 58])
    } else {
        (51, &[6, 19, 25, 32, 38, 47])
    };
    let mut rows = vec![
        MenuRow {
            cmd: SEPARATOR,
            string_id: 0
        };
        len
    ];
    for c in &COMMANDS {
        let r = if expansion {
            c.menu_exp
        } else {
            c.menu_classic
        };
        if r >= 0 {
            rows[r as usize] = MenuRow {
                cmd: i32::from(c.cmd),
                string_id: c.string_id,
            };
        }
    }
    debug_assert!(seps.iter().all(|&s| rows[s].cmd == SEPARATOR));
    rows
}

// ---- §7 r5 pointer buttons ---------------------------------------------

/// A pointer button (`PointerButton` of the UI, `UiFrame::unhandled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// What a pointer button means for the d2rs input (§7 r5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerMeaning {
    /// Left and Right are the fixed world buttons of §6, never
    /// rebindable.
    WorldLeft,
    WorldRight,
    /// Middle and the X buttons run whatever command is bound to keys
    /// 0x100–0x102 (§4.2); the default middle button is command 7.
    Command(u8),
    /// Bound to nothing.
    Unbound,
}

/// The meaning of `b` under the table `t`.
pub fn pointer_meaning(t: &BindingTable, b: Button) -> PointerMeaning {
    let key = match b {
        Button::Left => return PointerMeaning::WorldLeft,
        Button::Right => return PointerMeaning::WorldRight,
        Button::Middle => MIDDLE,
        Button::X1 => X1,
        Button::X2 => X2,
    };
    t.0.iter()
        .rev()
        .find(|e| e.key == key && (0..0x39).contains(&e.cmd))
        .map_or(PointerMeaning::Unbound, |e| {
            PointerMeaning::Command(e.cmd as u8)
        })
}

/// Which modifier a command gives (§7 r5): Shift / Ctrl / Alt meaning
/// comes only from the bindings of commands 36 / 34 / 37, except right up,
/// which reads the event's MK_SHIFT / MK_CONTROL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    StandStill,
    Run,
    ShowItems,
}

pub fn modifier_command(m: Modifier) -> u8 {
    match m {
        Modifier::StandStill => 36,
        Modifier::Run => 34,
        Modifier::ShowItems => 37,
    }
}

#[cfg(test)]
mod tests;

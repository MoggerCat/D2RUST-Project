// Spec: specs/ui/control-panel.md
//! The control panel overlays (`ui/panels.md` §6 owns the border and the
//! base art, [`super::border`]; this module owns everything drawn on top
//! of it and the control panel input): §1 the draw order, §2 the art
//! files; the rest is in the submodules.
//!
//! | Module | Rules |
//! |---|---|
//! | [`globes`] | §3 globes, §4 experience and stamina bars |
//! | [`belt`] | §5 the belt |
//! | [`buttons`] | §6 run / menu buttons, §7 skill buttons, §8 new stats / skills |
//! | [`minipanel`] | §9 the mini panel |
//! | [`input`] | §10 mouse input |

pub mod belt;
pub mod buttons;
pub mod globes;
pub mod input;
pub mod minipanel;

/// The steps of the control panel draw, in order (§1 r1–r5; the border and
/// the base art are `ui/panels.md` §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawStep {
    /// Resolution mode 2 only: the left / right border (r1).
    Border,
    /// `0x004983D0` (r2).
    BaseArt,
    LifeGlobe,
    ManaGlobe,
    ExperienceBar,
    RunButton,
    StaminaBar,
    MenuButton,
    Belt,
    SkillButtons,
    BeltItemHover,
    /// The level-change timer (r4).
    LevelTimer,
    /// The mini-panel menu button tool tip `0x00498340` (r5, §6 r4).
    MenuTip,
}

/// §1 r1–r5: step 7 of the UI pass (`0x00499450`).
pub const DRAW_ORDER: [DrawStep; 13] = [
    DrawStep::Border,
    DrawStep::BaseArt,
    DrawStep::LifeGlobe,
    DrawStep::ManaGlobe,
    DrawStep::ExperienceBar,
    DrawStep::RunButton,
    DrawStep::StaminaBar,
    DrawStep::MenuButton,
    DrawStep::Belt,
    DrawStep::SkillButtons,
    DrawStep::BeltItemHover,
    DrawStep::LevelTimer,
    DrawStep::MenuTip,
];

/// Step 8 of the UI pass (`ui/panels.md` §5), in its order (§1 text).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step8 {
    /// `[6]` → `0x004A6B30`, else `0x004A6A70`.
    NewStatsButton,
    /// `[7]` → `0x004A6E60`, else `0x004A6DA0`.
    NewSkillsButton,
    /// `0x00498120` (§3 r6).
    LifeManaNumbers,
    /// `0x0047F710`, with state 0x15 open (§9).
    MiniPanel,
}

pub const STEP8: [Step8; 4] = [
    Step8::NewStatsButton,
    Step8::NewSkillsButton,
    Step8::LifeManaNumbers,
    Step8::MiniPanel,
];

/// Whether a step 8 entry draws (`state_open(ui)`): the mini panel only
/// with state 0x15 open; the others always (their own gates are §8).
pub fn step8_runs(step: Step8, state_open: &dyn Fn(u8) -> bool) -> bool {
    match step {
        Step8::MiniPanel => state_open(0x15),
        _ => true,
    }
}

/// §1 r1: the border sides at resolution mode 2 only: left when the open
/// mode is 2 or 3, right when it is 1 or 3 (`ui/panels.md` §6 r1).
pub fn border_sides(res_mode: u8, open_mode: u8) -> (bool, bool) {
    if res_mode != 2 {
        return (false, false);
    }
    (matches!(open_mode, 2 | 3), matches!(open_mode, 1 | 3))
}

/// The level-change timer (§1 r4): when P exists and its level id
/// (`0x0061A1B0(P)`) differs from `[0x007BEFF0]`, `[0x007BEFF0]` := it and
/// `[0x007BEFEC]` := 60 (§Open questions 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelTimer {
    /// `[0x007BEFF0]`.
    pub level_id: u32,
    /// `[0x007BEFEC]`.
    pub timer: u32,
}

/// `[0x007BEFEC]` after a level change.
pub const LEVEL_TIMER_START: u32 = 60;

impl LevelTimer {
    pub fn update(&mut self, player_level: Option<u32>) {
        if let Some(l) = player_level {
            if l != self.level_id {
                self.level_id = l;
                self.timer = LEVEL_TIMER_START;
            }
        }
    }
}

/// Every cel draw of the control panel is `CelDraw` (`0x004F6480`) with
/// light 0xFF and draw mode 5 unless a rule says otherwise (§2).
pub const CEL_LIGHT: u8 = 0xFF;
pub const CEL_MODE: u8 = 5;

/// The art loaded by `0x004967F0` (`0x004520C0`, `"%s\UI\"` + name), §2.
pub const CONTROL_ART: [&str; 5] = [
    "Panel\\Hlthmana",
    "Panel\\overlap",
    "Panel\\runbutton",
    "Panel\\menubutton",
    "Panel\\ctrlpnl_popbelt",
];
/// `Panel\Level` and `Panel\Levelsocket` (`0x004A6460`).
pub const LEVEL_ART: [&str; 2] = ["Panel\\Level", "Panel\\Levelsocket"];
/// Loaded there and drawn elsewhere.
pub const OTHER_ART: [&str; 9] = [
    "Panel\\btn",
    "InvChar6",
    "InvChar",
    "Arrow",
    "Invwarn",
    "DuroIcon",
    "GemSocket",
    "menupanel",
    "skillpoints",
];
/// `hostilepic` is the tenth of the list.
pub const HOSTILE_PIC: &str = "hostilepic";

/// The mini panel art (`0x0047F0C0`, §9 r1): `Panel\minipanel` /
/// `Panel\minipanel_s` and `Panel\minipanelbtn`.
pub fn mini_panel_art(multiplayer: bool) -> [&'static str; 2] {
    [
        if multiplayer {
            "Panel\\minipanel"
        } else {
            "Panel\\minipanel_s"
        },
        "Panel\\minipanelbtn",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/ui/control-panel.md §1 r3
    #[test]
    fn step7_order() {
        // After the border and the base art: life globe, mana globe,
        // experience bar, run button, stamina bar, menu button, belt, skill
        // buttons, belt item hover text.
        assert_eq!(
            &DRAW_ORDER[2..11],
            &[
                DrawStep::LifeGlobe,
                DrawStep::ManaGlobe,
                DrawStep::ExperienceBar,
                DrawStep::RunButton,
                DrawStep::StaminaBar,
                DrawStep::MenuButton,
                DrawStep::Belt,
                DrawStep::SkillButtons,
                DrawStep::BeltItemHover,
            ]
        );
    }

    // Covers: specs/ui/control-panel.md §1 r1, §1 r5
    #[test]
    fn border_and_menu_tip_in_order() {
        // The border is the first step, only at resolution mode 2: left
        // when the open mode is 2 or 3, right when it is 1 or 3.
        assert_eq!(DRAW_ORDER[0], DrawStep::Border);
        assert_eq!(border_sides(0, 3), (false, false));
        assert_eq!(border_sides(2, 0), (false, false));
        assert_eq!(border_sides(2, 1), (false, true));
        assert_eq!(border_sides(2, 2), (true, false));
        assert_eq!(border_sides(2, 3), (true, true));
        // The menu button tool tip closes the step (§6 r4 draws it).
        assert_eq!(DRAW_ORDER[12], DrawStep::MenuTip);
        assert_eq!(DRAW_ORDER[11], DrawStep::LevelTimer);
    }

    #[test]
    fn base_art_follows_the_border() {
        assert_eq!(DRAW_ORDER[1], DrawStep::BaseArt);
    }

    // Covers: specs/ui/control-panel.md §1 r4
    #[test]
    fn level_change_timer() {
        let mut t = LevelTimer::default();
        // No player: unchanged.
        t.update(None);
        assert_eq!(t, LevelTimer::default());
        // A level that differs: id stored, timer 60.
        t.update(Some(1));
        assert_eq!(
            t,
            LevelTimer {
                level_id: 1,
                timer: 60
            }
        );
        // The same level again: the timer is left (the reader counts it
        // down).
        t.timer = 30;
        t.update(Some(1));
        assert_eq!(t.timer, 30);
        t.update(Some(2));
        assert_eq!(
            t,
            LevelTimer {
                level_id: 2,
                timer: 60
            }
        );
        // Level id 0 differs from the initial 0? No: equal, no timer.
        let mut z = LevelTimer::default();
        z.update(Some(0));
        assert_eq!(z.timer, 0);
    }

    // Covers: specs/ui/control-panel.md §1 text
    #[test]
    fn step8_order_and_mini_panel_gate() {
        assert_eq!(
            STEP8,
            [
                Step8::NewStatsButton,
                Step8::NewSkillsButton,
                Step8::LifeManaNumbers,
                Step8::MiniPanel
            ]
        );
        let none = |_: u8| false;
        assert!(step8_runs(Step8::NewStatsButton, &none));
        assert!(step8_runs(Step8::LifeManaNumbers, &none));
        assert!(!step8_runs(Step8::MiniPanel, &none));
        assert!(step8_runs(Step8::MiniPanel, &|s| s == 0x15));
    }

    // Covers: specs/ui/control-panel.md §2
    #[test]
    fn art_files() {
        assert_eq!(
            CONTROL_ART,
            [
                "Panel\\Hlthmana",
                "Panel\\overlap",
                "Panel\\runbutton",
                "Panel\\menubutton",
                "Panel\\ctrlpnl_popbelt"
            ]
        );
        assert_eq!(LEVEL_ART, ["Panel\\Level", "Panel\\Levelsocket"]);
        assert_eq!(
            mini_panel_art(true),
            ["Panel\\minipanel", "Panel\\minipanelbtn"]
        );
        assert_eq!(
            mini_panel_art(false),
            ["Panel\\minipanel_s", "Panel\\minipanelbtn"]
        );
        // Also loaded there and drawn elsewhere.
        assert!(OTHER_ART.contains(&"InvChar6") && OTHER_ART.contains(&"skillpoints"));
        assert_eq!(HOSTILE_PIC, "hostilepic");
        // Every cel draw: light 0xFF and draw mode 5.
        assert_eq!((CEL_LIGHT, CEL_MODE), (0xFF, 5));
    }
}

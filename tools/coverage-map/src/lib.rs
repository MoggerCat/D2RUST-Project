//! Behaviour coverage counters (tools/coverage-map/README.md).
//!
//! `d2-sim` calls [`hit`] at its choke points (skill use, monster
//! creation, AI function, object operate, item creation, NPC topic, quest
//! step, level entry, missile creation, state applied) only when built
//! with its `coverage-map` feature, which is off by default. A call copies
//! ids the game computed anyway into a per-thread counter: nothing here is
//! read back by the game, so no outcome depends on it (CLAUDE.md rule 6).
//!
//! Counting is on only when `D2_COVERAGE_DIR` names a directory. Each
//! thread's counts are written there when the thread ends (test threads)
//! or when [`flush`] is called (a binary's main thread, which Rust does not
//! tear down), as `cov-<pid>-<n>.tsv` in the `coverage-map 1` format:
//!
//! ```text
//! coverage-map 1
//! <category>\t<a>\t<b>\t<count>
//! ```
//!
//! `tools/coverage-map/report.py` merges the files and compares them with
//! the install's tables.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

/// The dump format line.
pub const FORMAT: &str = "coverage-map 1";

/// What a counter counts. `a` / `b` per category:
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cat {
    /// a skill started by a unit: a = skills row, b = unit type.
    Skill,
    /// a monster created: a = monstats row (hcIdx), b = 0.
    Monster,
    /// a monster AI function run: a = AI function id, b = monstats row.
    MonsterAi,
    /// an object operated: a = objects row, b = operate function.
    Object,
    /// an item created: a = items row (combined index), b = quality.
    Item,
    /// an NPC topic or menu: a = monstats row of the NPC, b = topic.
    NpcTopic,
    /// a quest step: a = quest slot, b = flag / state index.
    Quest,
    /// a player entered a level: a = levels row, b = 0.
    Level,
    /// a missile created: a = missiles row, b = 0.
    Missile,
    /// a state applied: a = states row, b = 0.
    State,
}

impl Cat {
    pub const ALL: [Cat; 10] = [
        Cat::Skill,
        Cat::Monster,
        Cat::MonsterAi,
        Cat::Object,
        Cat::Item,
        Cat::NpcTopic,
        Cat::Quest,
        Cat::Level,
        Cat::Missile,
        Cat::State,
    ];

    /// The name written to the dump.
    pub fn name(self) -> &'static str {
        match self {
            Cat::Skill => "skill",
            Cat::Monster => "monster",
            Cat::MonsterAi => "monster-ai",
            Cat::Object => "object",
            Cat::Item => "item",
            Cat::NpcTopic => "npc-topic",
            Cat::Quest => "quest",
            Cat::Level => "level",
            Cat::Missile => "missile",
            Cat::State => "state",
        }
    }
}

type Counts = BTreeMap<(Cat, u32, u32), u64>;

/// A thread's counts; written out when the thread ends.
struct Local(RefCell<Counts>);

impl Drop for Local {
    fn drop(&mut self) {
        write_out(&self.0.borrow());
    }
}

thread_local! {
    static LOCAL: Local = const { Local(RefCell::new(BTreeMap::new())) };
}

static SEQ: AtomicU32 = AtomicU32::new(0);

fn dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        std::env::var_os("D2_COVERAGE_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    })
    .as_ref()
}

/// Whether counting is on (`D2_COVERAGE_DIR` set).
pub fn enabled() -> bool {
    dir().is_some()
}

/// Counts one event. A no-op unless `D2_COVERAGE_DIR` is set.
#[inline]
pub fn hit(cat: Cat, a: u32, b: u32) {
    if dir().is_none() {
        return;
    }
    // During thread teardown the slot may be gone: drop the count.
    let _ = LOCAL.try_with(|l| *l.0.borrow_mut().entry((cat, a, b)).or_insert(0) += 1);
}

/// Writes this thread's counts now and clears them (a binary's main
/// thread, whose thread-locals are not dropped at exit).
pub fn flush() {
    let _ = LOCAL.try_with(|l| {
        let counts = std::mem::take(&mut *l.0.borrow_mut());
        write_out(&counts);
    });
}

/// A copy of this thread's counts (tests).
pub fn snapshot() -> Vec<(Cat, u32, u32, u64)> {
    LOCAL
        .try_with(|l| {
            l.0.borrow()
                .iter()
                .map(|(&(c, a, b), &n)| (c, a, b, n))
                .collect()
        })
        .unwrap_or_default()
}

fn write_out(counts: &Counts) {
    let Some(dir) = dir() else { return };
    if counts.is_empty() {
        return;
    }
    let mut text = String::with_capacity(32 * counts.len() + 16);
    text.push_str(FORMAT);
    text.push('\n');
    for (&(c, a, b), n) in counts {
        text.push_str(&format!("{}\t{a}\t{b}\t{n}\n", c.name()));
    }
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!("cov-{}-{seq}.tsv", std::process::id()));
    // A failed write must not fail the run that is being measured.
    if let Err(e) = std::fs::create_dir_all(dir)
        .and_then(|_| std::fs::File::create(&path))
        .and_then(|mut f| f.write_all(text.as_bytes()))
    {
        eprintln!("coverage-map: {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The only test that reads `D2_COVERAGE_DIR` (it is read once).
    #[test]
    fn counts_reach_the_file_on_flush() {
        let dir = std::env::temp_dir().join(format!("coverage-map-test-{}", std::process::id()));
        std::env::set_var("D2_COVERAGE_DIR", &dir);
        assert!(enabled());
        hit(Cat::Skill, 36, 1);
        hit(Cat::Skill, 36, 1);
        hit(Cat::Level, 2, 0);
        assert_eq!(snapshot().len(), 2);
        flush();
        assert!(snapshot().is_empty());
        let mut text = String::new();
        for e in std::fs::read_dir(&dir).unwrap() {
            text += &std::fs::read_to_string(e.unwrap().path()).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(text, "coverage-map 1\nskill\t36\t1\t2\nlevel\t2\t0\t1\n");
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = Cat::ALL.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Cat::ALL.len());
    }
}

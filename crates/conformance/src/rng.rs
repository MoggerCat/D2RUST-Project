//! Replays `rng_draw` traces (`specs/sim/rng.md`, "Test vectors") through
//! `d2_sim::rng::Seed`: start from `setup.seed`, apply each `op` in order,
//! and require `value` and `state` to match exactly.

use d2_sim::rng::Seed;
use serde_json::Value;

use crate::{Trace, TraceError};

/// Replays every draw of an RNG trace. Returns the number of draws checked.
pub fn replay(trace: &Trace) -> Result<usize, TraceError> {
    let format = |m: String| TraceError::Format(format!("trace {}: {m}", trace.id));
    if trace.spec != "specs/sim/rng.md" {
        return Err(format(format!("spec {:?} is not the RNG spec", trace.spec)));
    }
    if trace.compare_mode != "exact" {
        return Err(format(format!(
            "compare.mode {:?}; RNG traces are exact",
            trace.compare_mode
        )));
    }
    if !trace.inputs.is_empty() {
        return Err(format("RNG traces take no inputs".into()));
    }
    let mut seed = seed_of(&trace.setup["seed"]).map_err(|m| format(format!("setup.seed: {m}")))?;

    for (index, event) in trace.expected.iter().enumerate() {
        let mismatch = |message: String| TraceError::Mismatch {
            id: trace.id.clone(),
            index,
            message,
        };
        if event["kind"] != "rng_draw" {
            return Err(mismatch(format!(
                "kind {}, expected \"rng_draw\"",
                event["kind"]
            )));
        }
        let data = &event["data"];
        let before = seed;
        let value = draw(&mut seed, data).map_err(mismatch)?;
        let want_value = data["value"]
            .as_i64()
            .ok_or_else(|| mismatch("missing value".into()))?;
        let want_state = seed_of(&data["state"]).map_err(|m| mismatch(format!("state: {m}")))?;
        if value != want_value || seed != want_state {
            return Err(mismatch(format!(
                "{} from {before:?}: got value {value}, state {seed:?}; \
                 recorded value {want_value}, state {want_state:?}",
                data["op"]
            )));
        }
    }
    Ok(trace.expected.len())
}

/// Applies one `rng_draw` op and returns its value.
fn draw(seed: &mut Seed, data: &Value) -> Result<i64, String> {
    let int = |key: &str| -> Result<i64, String> {
        data[key]
            .as_i64()
            .ok_or_else(|| format!("missing integer {key}"))
    };
    let i32_arg = |key: &str| -> Result<i32, String> {
        i32::try_from(int(key)?).map_err(|_| format!("{key} out of i32 range"))
    };
    let op = data["op"].as_str().ok_or("missing op")?;
    Ok(match op {
        "step" => seed.step().into(),
        "roll" => seed.roll(i32_arg("n")?).into(),
        "mask" => {
            let n = u32::try_from(int("n")?).map_err(|_| "n out of u32 range")?;
            seed.mask(n).into()
        }
        "mask_range" => {
            let n = u32::try_from(int("n")?).map_err(|_| "n out of u32 range")?;
            seed.mask_range(i32_arg("min")?, n).into()
        }
        "roll_range" => seed.roll_range(i32_arg("min")?, i32_arg("n")?).into(),
        other => return Err(format!("unknown op {other:?}")),
    })
}

/// `{ "lo": u32, "hi": u32 }`.
fn seed_of(v: &Value) -> Result<Seed, String> {
    let word = |key: &str| {
        v[key]
            .as_u64()
            .and_then(|x| u32::try_from(x).ok())
            .ok_or_else(|| format!("missing or out-of-range {key}"))
    };
    Ok(Seed::new(word("lo")?, word("hi")?))
}

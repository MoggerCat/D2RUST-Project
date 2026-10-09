# q-fix-depcheck — handoff

Done: nothing to change. On the merged staging base the RNG trace is already
behind the d2-sim `rng-trace` feature (off by default; `debug/mod.rs` gates
`rng_trace`, so the default build has no `thread_local`). d2-client enables it
only via its own `rng-trace = ["d2-sim/rng-trace"]` feature (state-dump --rng).

Verified:
- `cargo run -p depcheck` -> OK (9 crates, d2-sim determinism lint clean)
- `cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings` -> clean
- `cargo clippy -p d2-sim --features rng-trace --all-targets -- -D warnings` -> clean
- `cargo test -p d2-sim --features rng-trace --lib rng_trace` -> 4 passed

Open: none. If CI depcheck is still red, it ran on a base without the staging merge.

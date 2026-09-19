# Journal

## 2026-09-18 — local env: root-owned target/ (resolved), broken default nightly

`target/` briefly had ~1862 files owned by `root` (dated 2026-09-13), from
some earlier `sudo cargo build`, blocking normal builds/tests
(`Permission denied` writing `target/debug/.fingerprint/...`). Worked
around at the time with a scratch `CARGO_TARGET_DIR`. By the next check in
the same session `target/` was back to being fully user-owned — resolved
externally, not by this session. If it recurs: `sudo chown -R
$(id -u):$(id -g) target/` (or `sudo rm -rf target/`, it's disposable
build cache).

Separately: the default toolchain (`rustup show` → `nightly-x86_64-unknown-linux-gnu`,
snapshot dated 2026-09-17) cannot compile `libc`'s build script —
`cannot find function, tuple struct or tuple variant 'Some' in this scope`
in `libc-0.2.189/build.rs`. Reproduced on a clean `git stash`, so it's a
broken nightly snapshot, not a repo bug. `cargo +stable build/test` works
fine. Until nightly is fixed upstream or re-pinned, use `+stable` for
anything that needs to actually compile.

//! The plain `nihilurk` command. It plays no part of the game itself -- it
//! reads the environment's locale, `exec`s the matching `nihilurk-<lang>`
//! binary installed next to it, and forwards every argument untouched.
//! Falls back to English for anything it doesn't recognize, including "no
//! locale set at all", so `nihilurk` always does *something* rather than
//! refusing to start.
//!
//! Deliberately has no dependency on `models` or `strings`: which language a
//! *binary* was built in is a compile-time fact about that binary, not
//! something this dispatcher needs to know or link against.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const SUPPORTED: &[&str] = &["en", "pt", "es", "ht"];

/// The first two letters of `$LC_ALL`, then `$LANG`, then `$LANGUAGE`,
/// lowercased -- e.g. `pt_BR.UTF-8` and `pt-BR` both read as `pt`. Falls back
/// to `"en"` when none of those are set, aren't long enough to hold a
/// language code, or name a language nihilurk doesn't ship.
fn locale() -> &'static str {
    let raw = ["LC_ALL", "LANG", "LANGUAGE"]
        .into_iter()
        .find_map(|key| env::var(key).ok())
        .unwrap_or_default();
    let prefix = raw.get(0..2).unwrap_or_default().to_lowercase();
    SUPPORTED
        .iter()
        .find(|&&lang| lang == prefix)
        .copied()
        .unwrap_or("en")
}

fn sibling(name: &str) -> PathBuf {
    let mut path = env::current_exe().expect("this binary's own path is readable");
    path.set_file_name(name);
    path
}

fn main() {
    let target = sibling(&format!("nihilurk-{}", locale()));
    let args: Vec<String> = env::args().skip(1).collect();

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = Command::new(&target).args(&args).exec();
        eprintln!("nihilurk: couldn't run {}: {err}", target.display());
        std::process::exit(1);
    }

    #[cfg(not(unix))]
    {
        let status = Command::new(&target)
            .args(&args)
            .status()
            .unwrap_or_else(|err| panic!("couldn't run {}: {err}", target.display()));
        std::process::exit(status.code().unwrap_or(1));
    }
}

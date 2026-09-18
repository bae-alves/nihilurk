//! nihilurk-compat -- the reporting half of the compatibility pipeline.
//!
//!     nihilurk-compat                 the report (same as `report`)
//!     nihilurk-compat report          the matrix, as text
//!     nihilurk-compat gate            exit non-zero if a machine cannot run nihilurk
//!
//! It measures nothing itself. `compat/cross_build.sh` builds the game for
//! every machine in the matrix and `compat/run_check.sh` checks that each
//! machine's image has a shell and that the built binary starts and runs;
//! both write their findings into `target/compat/`, and this reads them
//! back. The split is deliberate -- see `results.rs` -- and it is what lets
//! the report be read over a run that finished last week, or one that is
//! still going.
//!
//! # What is being claimed
//!
//! If it builds, the image has a shell, and the target is std (every `linux`
//! row here is), nihilurk can run there. There is no performance band and no
//! resource-capped stress run: nihilurk has no workload heavy enough to need
//! one -- every feel-layer effect is bounded, and cross-compiling Rust asks
//! more of a machine than nihilurk's own frame loop ever will. See
//! `docs/explanation/cross-platform-testing.md`.

mod matrix;
mod results;

use std::fmt::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::matrix::{Class, Exec, Row};
use crate::results::{Results, Status};

/// Where the shell half writes, relative to the workspace root.
const DEFAULT_DIR: &str = "target/compat";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }

    let opts = match Options::parse(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("nihilurk-compat: {e}");
            return ExitCode::FAILURE;
        }
    };

    let results = Results::load(&opts.dir);
    match opts.mode {
        Mode::Report => report(&results),
        Mode::Gate => gate(&results),
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Report,
    /// Read the results and say yes or no, in an exit code. Prints one line.
    Gate,
}

struct Options {
    mode: Mode,
    dir: PathBuf,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut o = Options {
            mode: Mode::Report,
            dir: PathBuf::from(DEFAULT_DIR),
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "report" => o.mode = Mode::Report,
                "gate" => o.mode = Mode::Gate,
                "--dir" => {
                    let value = it.next().ok_or("--dir needs a path")?;
                    o.dir = PathBuf::from(value);
                }
                other => return Err(format!("unknown argument `{other}` (try --help)")),
            }
        }
        Ok(o)
    }
}

fn print_help() {
    println!(
        "\
nihilurk-compat -- does nihilurk run on the machines it claims to?

USAGE
    nihilurk-compat [report] [--dir DIR]   the matrix, as text   [default]
    nihilurk-compat gate [--dir DIR]       one line and an exit code: 1 if
                                       any machine cannot run nihilurk

OPTIONS
    --dir <DIR>   where the result files are   [default: target/compat]
    -h, --help    this

WHERE THE NUMBERS COME FROM
    ./compat/cross_build.sh   builds the game for every machine, and
                              reports the size and the blame
    ./compat/run_check.sh     checks each machine's image has a shell and
                              that the binary actually starts there
    ./compat/nostd_check.sh   the bare-metal rows: particle-core compiled
                              for microcontrollers

    or ./compat_test.sh to do all three and print this.

WHAT IS BEING CLAIMED
    if it builds, the image has a shell, and the target is std, nihilurk can
    run there. Nothing here drives the game under load or caps a container's
    CPU or memory -- there is no stress test in this pipeline any more."
    );
}

// ---------------------------------------------------------------------------
// Plain-text report
// ---------------------------------------------------------------------------

/// The whole matrix as text. This is what `compat_test.sh` prints at the end.
fn report(results: &Results) -> ExitCode {
    let mut out = String::new();
    let w = &mut out;
    let linux = matrix::selected(&[], Some(Class::Linux));
    let bare = matrix::selected(&[], Some(Class::Bare));

    let _ = writeln!(w, "\n=== does nihilurk run here? ===\n");
    if results.is_empty() {
        let _ = writeln!(
            w,
            "  nothing checked yet in {}.\n\n  ./compat_test.sh    build the matrix, check it, and come back here\n",
            results.dir.display()
        );
        print!("{out}");
        return ExitCode::SUCCESS;
    }

    let _ = writeln!(
        w,
        "  {:<9} {:<7} {:>10}  {}",
        "machine", "exec", "binary", "verdict"
    );
    for row in &linux {
        let check = results.check(&row.id);
        let status = check.map(|c| c.status).unwrap_or(Status::Skipped);
        let footprint = results.footprint(&row.id).unwrap_or_default();
        let _ = writeln!(
            w,
            "  {:<9} {:<7} {:>10}  {}",
            row.id,
            exec_word(row.exec),
            cell(footprint.game, bytes),
            status.label(),
        );
    }

    let _ = writeln!(
        w,
        "\n  `binary` is engine, --release: one static musl file, no libc to install."
    );

    report_bare(w, &bare);
    report_failures(w, results, &linux);

    let broken: Vec<&str> = results
        .checks
        .iter()
        .filter(|c| c.status.is_failure())
        .map(|c| c.id.as_str())
        .collect();
    let _ = writeln!(w);
    if broken.is_empty() {
        let _ = writeln!(w, "  VERDICT  nihilurk runs on every machine checked");
    } else {
        let _ = writeln!(
            w,
            "  VERDICT  nihilurk does not run on: {}",
            broken.join(", ")
        );
    }
    let _ = writeln!(w);
    print!("{out}");
    ExitCode::SUCCESS
}

/// The microcontroller rows. They are not a game and never claimed to be.
fn report_bare(w: &mut String, bare: &[Row]) {
    if bare.is_empty() {
        return;
    }
    let _ = writeln!(
        w,
        "\n  BARE METAL (particle-core only -- compiled, never executed)\n"
    );
    for row in bare {
        let _ = writeln!(w, "  {:<9} {:<32}  {}", row.id, row.target, row.note);
    }
    let _ = writeln!(
        w,
        "\n  These rows prove the particle arithmetic compiles with no operating\n  \
         system under it. They do not run nihilurk: crossterm needs a terminal, and\n  \
         a microcontroller has none. ./compat/nostd_check.sh is what checks them."
    );
}

/// Where to go when a row did not check out cleanly. A skipped row was never
/// attempted, so there is no log worth reading and listing it here would
/// send someone chasing nothing.
fn report_failures(w: &mut String, results: &Results, linux: &[Row]) {
    let broken: Vec<&Row> = linux
        .iter()
        .filter(|row| {
            results
                .check(&row.id)
                .is_some_and(|c| c.status.is_failure())
        })
        .collect();
    if broken.is_empty() {
        return;
    }
    let _ = writeln!(w, "\n  WHAT TO LOOK AT\n");
    for row in broken {
        let check = results.check(&row.id).unwrap();
        let _ = writeln!(
            w,
            "  {:<9} {:<9} {:<32}  {}/run-{}.log",
            row.id,
            check.status.name(),
            check.target,
            results.dir.display(),
            row.id,
        );
        let _ = writeln!(
            w,
            "  {:<9} {:<9}  ran in {} on {}",
            "", "", row.image, row.platform
        );
    }
}

fn exec_word(exec: Exec) -> &'static str {
    match exec {
        Exec::Native => "native",
        Exec::Qemu => "qemu",
        Exec::None => "-",
    }
}

fn cell(value: u64, f: fn(u64) -> String) -> String {
    match value {
        0 => "-".to_string(),
        n => f(n),
    }
}

pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    match unit {
        0 => format!("{n} B"),
        _ => format!("{value:.1} {}", UNITS[unit]),
    }
}

// ---------------------------------------------------------------------------
// Gate
// ---------------------------------------------------------------------------

/// One line and an exit code, for CI and for `compat_test.sh`'s last stage.
fn gate(results: &Results) -> ExitCode {
    if results.checks.is_empty() {
        println!("nihilurk-compat: nothing checked -- run ./compat_test.sh");
        return ExitCode::FAILURE;
    }
    let broken: Vec<&str> = results
        .checks
        .iter()
        .filter(|c| c.status.is_failure())
        .map(|c| c.id.as_str())
        .collect();

    if broken.is_empty() {
        println!("nihilurk-compat: nihilurk runs on every machine in the matrix");
        return ExitCode::SUCCESS;
    }
    println!(
        "nihilurk-compat: nihilurk does not run on: {}",
        broken.join(", ")
    );
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_subcommands_parse() {
        let rep = Options::parse(&[]).expect("no args is the report");
        assert_eq!(rep.mode, Mode::Report);
        let rep = Options::parse(&["report".to_string()]).expect("report");
        assert_eq!(rep.mode, Mode::Report);
        let gate = Options::parse(&["gate".to_string()]).expect("gate");
        assert_eq!(gate.mode, Mode::Gate);
    }

    #[test]
    fn an_unknown_flag_is_an_error_rather_than_being_ignored() {
        assert!(Options::parse(&["--densiy".to_string()]).is_err());
    }
}

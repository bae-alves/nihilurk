//! The listing flags print and exit without the alternate screen, so they get
//! piped into `head`. A reader that hangs up early is not an error.

use std::process::{Command, Stdio};

/// Runs `nihilurk <flag>` with stdout wired to a pipe nobody reads, so the
/// first write fails with a broken pipe.
fn run_into_closed_pipe(flag: &str) -> (bool, String) {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let out = Command::new(env!("CARGO_BIN_EXE_nihilurk"))
        .arg(flag)
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("spawn nihilurk");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_listing_into_a_closed_pipe_exits_quietly() {
    for flag in ["-content", "-help", "-scores"] {
        let (ok, stderr) = run_into_closed_pipe(flag);
        assert!(ok, "{flag} failed: {stderr}");
        assert!(stderr.is_empty(), "{flag} spoke on stderr: {stderr}");
    }
}

#[test]
fn leaderboard_longhand_prints_what_scores_prints() {
    let dir = std::env::temp_dir().join(format!("nihilurk-lb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let run = |flag: &str| {
        Command::new(env!("CARGO_BIN_EXE_nihilurk"))
            .arg(flag)
            .current_dir(&dir)
            .output()
            .expect("spawn nihilurk")
    };
    let short = run("-scores");
    let long = run("--leaderboard");
    std::fs::remove_dir_all(&dir).ok();
    assert!(long.status.success());
    assert_eq!(short.stdout, long.stdout);
}

#[test]
fn bad_arguments_are_refused() {
    let cases: [(&[&str], &str); 8] = [
        (&["-bogus"], "-bogus"),
        (&["-s", "-5"], "-s -5"),
        (&["-s", "abc"], "-s abc"),
        (&["-s"], "-s"),
        (&["--anim-rate", "fast"], "-anim-rate fast"),
        (&["-anim-rate", "NaN"], "-anim-rate NaN"),
        (&["-b", "-am"], "-b -am"),
        (&["-pride"], "-pride"),
    ];
    for (args, quoted) in cases {
        let out = Command::new(env!("CARGO_BIN_EXE_nihilurk"))
            .args(args)
            .output()
            .expect("spawn nihilurk");
        assert_eq!(
            String::from_utf8_lossy(&out.stderr).trim(),
            format!("invalid argument {quoted}. Please see -help for more information"),
            "{args:?}"
        );
    }
}

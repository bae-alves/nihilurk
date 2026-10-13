//! `-keymap` refuses bad input before the terminal is touched.

use std::process::Command;

fn run(args: &[&str], data: &std::path::Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_nihilurk"))
        .args(args)
        .env("NIHILURK_DATA", data)
        .output()
        .expect("spawn nihilurk");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nihilurk-keymap-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_keymap_flag_without_a_path_is_an_invalid_argument() {
    let err = run(&["-keymap"], &scratch("novalue"));
    assert!(err.contains("invalid argument -keymap"), "{err}");
}

#[test]
fn a_missing_keymap_file_is_named() {
    let err = run(&["-keymap", "/nonexistent/keymap.txt"], &scratch("missing"));
    assert!(err.contains("/nonexistent/keymap.txt"), "{err}");
}

#[test]
fn a_bad_line_is_named_in_the_auto_loaded_file() {
    let data = scratch("badline");
    std::fs::write(data.join("keymap.txt"), "hj\nabc\n").unwrap();
    let err = run(&[], &data);
    assert!(
        err.contains("keymap.txt") && err.contains("line 2"),
        "{err}"
    );
}

//! Keeps the docs from stating the wrong number of schedule steps or intent
//! queues.
//!
//! Both counts went stale in several files at once, and nothing noticed. The
//! pre-commit hook asks for a docs page when either changes; this checks what
//! the pages say. It reads a number written as a word ("seventeen") or in
//! digits next to the phrases the docs use, and compares it to the code.
//!
//! It only sees those phrases. `the_scan_finds_the_counts_the_docs_state`
//! fails if the pages stop using them, so a rewording cannot turn the check
//! off without anyone noticing.

use std::path::{Path, PathBuf};

use models::turn_schedule;

const WORDS: [&str; 31] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "twenty-one",
    "twenty-two",
    "twenty-three",
    "twenty-four",
    "twenty-five",
    "twenty-six",
    "twenty-seven",
    "twenty-eight",
    "twenty-nine",
    "thirty",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn markdown_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("docs directory is readable") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("docs"), &mut files);
    for top in ["README.md", "MANUAL.md", "scs.md"] {
        files.push(root().join(top));
    }
    files
}

fn as_number(word: &str) -> Option<usize> {
    let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
    WORDS
        .iter()
        .position(|w| *w == word)
        .or_else(|| word.parse().ok())
}

fn word_before(text: &str, at: usize) -> Option<&str> {
    text[..at].split(' ').next_back().filter(|w| !w.is_empty())
}

fn word_after(text: &str, at: usize) -> Option<&str> {
    text[at..].split(' ').find(|w| !w.is_empty())
}

fn after_is(text: &str, at: usize, within: usize, needle: &str) -> bool {
    let end = (at + within).min(text.len());
    let end = (end..=text.len())
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(text.len());
    text[at..end].contains(needle)
}

fn before_is(text: &str, at: usize, within: usize, needle: &str) -> bool {
    let start = at.saturating_sub(within);
    let start = (0..=start)
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    text[start..at].contains(needle)
}

struct Claim {
    file: String,
    said: usize,
    wanted: usize,
    what: &'static str,
}

fn claims_in(file: &str, raw: &str, steps: usize, queues: usize) -> Vec<Claim> {
    let text = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut claims = Vec::new();
    let mut claim = |said: usize, wanted: usize, what: &'static str| {
        claims.push(Claim {
            file: file.to_string(),
            said,
            wanted,
            what,
        })
    };

    for (at, _) in text.match_indices(" schedule steps") {
        if let Some(n) = word_before(&text, at).and_then(as_number) {
            claim(n, steps, "schedule steps");
        }
    }
    for (at, _) in text.match_indices(" steps") {
        if before_is(&text, at, 40, "schedule")
            && !text[..at].ends_with(" schedule")
            && let Some(n) = word_before(&text, at).and_then(as_number)
        {
            claim(n, steps, "schedule steps");
        }
    }
    for needle in [" intent queues", " queues"] {
        for (at, _) in text.match_indices(needle) {
            if let Some(n) = word_before(&text, at).and_then(as_number) {
                claim(n, queues, "intent queues");
            }
        }
    }
    for lead in [" of nihilurk's ", " of its ", " of the "] {
        for (at, _) in text.match_indices(lead) {
            let b_at = at + lead.len();
            let about_steps = after_is(&text, b_at, 40, "schedule steps")
                || after_is(&text, b_at, 40, "&mut world");
            if !about_steps {
                continue;
            }
            if let (Some(a), Some(b)) = (
                word_before(&text, at).and_then(as_number),
                word_after(&text, b_at).and_then(as_number),
            ) {
                claim(b, steps, "schedule steps");
                claim(a, steps - 1, "steps that take &mut World");
            }
        }
    }
    claims
}

fn real_counts() -> (usize, usize) {
    let steps = turn_schedule().graph().systems().count();
    let components = std::fs::read_to_string(root().join("models/src/components.rs"))
        .expect("components.rs is readable");
    let queues = components
        .lines()
        .filter(|l| l.starts_with("pub struct ") && l.contains("Queue"))
        .count();
    (steps, queues)
}

fn every_claim() -> Vec<Claim> {
    let (steps, queues) = real_counts();
    markdown_files()
        .iter()
        .flat_map(|path| {
            let raw = std::fs::read_to_string(path).expect("markdown file is readable");
            let name = path
                .strip_prefix(root())
                .unwrap_or(path)
                .display()
                .to_string();
            claims_in(&name, &raw, steps, queues)
        })
        .collect()
}

#[test]
fn no_page_states_a_wrong_step_or_queue_count() {
    let wrong: Vec<String> = every_claim()
        .into_iter()
        .filter(|c| c.said != c.wanted)
        .map(|c| {
            format!(
                "{}: says {} {}, the code has {}",
                c.file, c.said, c.what, c.wanted
            )
        })
        .collect();
    assert!(wrong.is_empty(), "stale counts:\n{}", wrong.join("\n"));
}

#[test]
fn the_scan_finds_the_counts_the_docs_state() {
    let claims = every_claim();
    let steps = claims.iter().filter(|c| c.what == "schedule steps").count();
    let queues = claims.iter().filter(|c| c.what == "intent queues").count();
    assert!(steps >= 4, "the scan only found {steps} step counts");
    assert!(queues >= 3, "the scan only found {queues} queue counts");
}

#[test]
fn a_stale_sentence_is_caught() {
    let stale = claims_in(
        "x.md",
        "Fifteen of nihilurk's sixteen schedule steps take `&mut World`. All four queues drain.",
        17,
        5,
    );
    let wrong: Vec<&str> = stale
        .iter()
        .filter(|c| c.said != c.wanted)
        .map(|c| c.what)
        .collect();
    for kind in [
        "schedule steps",
        "steps that take &mut World",
        "intent queues",
    ] {
        assert!(wrong.contains(&kind), "{kind} not caught: {wrong:?}");
    }
}

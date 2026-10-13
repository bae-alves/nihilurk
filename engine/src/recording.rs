//! The recording file: what a new run needs to be played again, then every
//! event the game acted on (see [`crate::input`]).
//!
//! Plain text, one fact per line, so two recordings of one run can be diffed.
//! A run resumed from a save has no recording: a seed and keys only describe a
//! run from its first turn.

use crate::input;
use crossterm::event::Event;
use std::path::{Path, PathBuf};

const MAGIC: &str = "nihilurk-rec 1";
const EVENTS: &str = "events";

/// The environment variables that change what a new run builds. Recorded when
/// set, so a run started with `NIHILURK_SPAWN` replays with it.
pub const SHAPING_ENV: [&str; 3] = ["NIHILURK_SPAWN", "NIHILURK_LEVEL", "NIHILURK_MAGICMAP"];

#[derive(Debug, Default, PartialEq)]
pub struct Header {
    pub version: String,
    pub seed: u64,
    pub name: String,
    /// The body flag as typed, `-b lurk` or `-am goblin`.
    pub body: Option<String>,
    pub endless: bool,
    pub fast: bool,
    pub nobones: bool,
    pub env: Vec<(String, String)>,
    pub keymap: Option<String>,
}

impl Header {
    /// The header's lines, ending at the marker the events follow.
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "{MAGIC}\nversion {}\nseed {}\nname {}\n",
            self.version, self.seed, self.name
        );
        if let Some(body) = &self.body {
            out += &format!("body {body}\n");
        }
        if self.endless {
            out += "endless\n";
        }
        if self.fast {
            out += "fast\n";
        }
        if self.nobones {
            out += "nobones\n";
        }
        for (key, value) in &self.env {
            out += &format!("env {key}={value}\n");
        }
        for line in self.keymap.iter().flat_map(|k| k.lines()) {
            out += &format!("keymap {line}\n");
        }
        out + EVENTS + "\n"
    }
}

/// A recording read back: its header and its events in order.
pub fn parse(text: &str) -> Result<(Header, Vec<(u64, Event)>), String> {
    let mut lines = text.lines();
    match lines.next() {
        Some(MAGIC) => {}
        Some(other) if other.starts_with("nihilurk-rec ") => {
            return Err(format!("recording format '{other}' is not '{MAGIC}'"));
        }
        _ => return Err("not a nihilurk recording".into()),
    }
    let mut header = Header::default();
    let mut keymap: Vec<&str> = Vec::new();
    let mut seen_events = false;
    for line in lines.by_ref() {
        if line == EVENTS {
            seen_events = true;
            break;
        }
        let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "version" => header.version = rest.to_string(),
            "seed" => header.seed = rest.parse().map_err(|_| format!("bad seed '{rest}'"))?,
            "name" => header.name = rest.to_string(),
            "body" => header.body = Some(rest.to_string()),
            "endless" => header.endless = true,
            "fast" => header.fast = true,
            "nobones" => header.nobones = true,
            "env" => {
                let (k, v) = rest
                    .split_once('=')
                    .ok_or(format!("bad env line '{line}'"))?;
                header.env.push((k.to_string(), v.to_string()));
            }
            "keymap" => keymap.push(rest),
            _ => return Err(format!("unknown header line '{line}'")),
        }
    }
    if !seen_events {
        return Err("recording ends before its events".into());
    }
    if !keymap.is_empty() {
        header.keymap = Some(keymap.join("\n"));
    }
    let events = lines
        .enumerate()
        .map(|(i, line)| {
            input::decode(line).ok_or(format!("bad event on line {}: '{line}'", i + 1))
        })
        .collect::<Result<_, _>>()?;
    Ok((header, events))
}

/// Where a run's recording goes: `recordings/NAME-SEED.rec` in `data_dir`, the
/// name cut down to characters safe in a file name.
pub fn path_for(data_dir: &Path, name: &str, seed: u64) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    data_dir
        .join("recordings")
        .join(format!("{safe}-{seed}.rec"))
}

/// Opens `path` for a new run, writes `header` and returns the file to log
/// events to. Best effort by the caller: a run on a read-only disk still plays.
pub fn start(path: &Path, header: &Header) -> std::io::Result<std::fs::File> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::File::create(path)?;
    file.write_all(header.to_text().as_bytes())?;
    file.flush()?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> Header {
        Header {
            version: "0.5.1".into(),
            seed: 42,
            name: "BAE".into(),
            body: Some("-am goblin".into()),
            endless: true,
            fast: true,
            nobones: true,
            env: vec![("NIHILURK_LEVEL".into(), "island".into())],
            keymap: Some("ab\ncd".into()),
        }
    }

    #[test]
    fn a_header_and_its_events_read_back_as_written() {
        let text = full().to_text() + "2 k c104 0\n5 m R 3 4 0\n";
        let (header, events) = parse(&text).unwrap();
        assert_eq!(header, full());
        assert_eq!(events.iter().map(|e| e.0).collect::<Vec<_>>(), [2, 5]);
    }

    #[test]
    fn a_bare_header_has_no_optional_lines() {
        let bare = Header {
            version: "v".into(),
            name: "N".into(),
            ..Header::default()
        };
        assert_eq!(parse(&bare.to_text()).unwrap(), (bare, vec![]));
    }

    #[test]
    fn a_recording_from_another_format_is_refused() {
        assert!(parse("nihilurk-rec 2\n").unwrap_err().contains("format"));
        assert!(parse("hello\n").is_err());
        assert!(
            parse("nihilurk-rec 1\nseed 1\n")
                .unwrap_err()
                .contains("events")
        );
        assert!(
            parse(&(full().to_text() + "junk\n"))
                .unwrap_err()
                .contains("line 1")
        );
    }

    #[test]
    fn a_name_cannot_climb_out_of_the_recordings_dir() {
        let path = path_for(Path::new("/d"), "../x y", 7);
        assert_eq!(path, Path::new("/d/recordings/___x_y-7.rec"));
    }
}

//! Key remapping. `keymap.txt` holds one `from to` pair per line, two
//! characters with no separator (Brogue's format): pressing `from` acts as
//! `to`. Lookup is a single pass, so `hj` plus `jh` swaps and never chains.

use bevy_ecs::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Pressed key to command. Empty means no remapping.
#[derive(Resource, Clone, Debug, Default)]
pub struct KeyMap(HashMap<char, char>);

impl KeyMap {
    /// True when no key is remapped.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The command a pressed key stands for. An uppercase letter with no line
    /// of its own follows its lowercase line, so Shift-run keeps working.
    pub fn apply(&self, pressed: char) -> char {
        if let Some(&to) = self.0.get(&pressed) {
            return to;
        }
        match self.0.get(&pressed.to_ascii_lowercase()) {
            Some(to) if pressed.is_ascii_uppercase() => to.to_ascii_uppercase(),
            _ => pressed,
        }
    }

    /// The key that now produces `command`, for help text. A command nothing
    /// maps to shows as itself.
    pub fn show(&self, command: char) -> char {
        let find = |c: char| {
            self.0
                .iter()
                .find(|&(_, &to)| to == c)
                .map(|(&from, _)| from)
        };
        find(command)
            .or_else(|| {
                let lower = command.to_ascii_lowercase();
                find(lower)
                    .filter(|_| command.is_ascii_uppercase())
                    .map(|from| from.to_ascii_uppercase())
            })
            .unwrap_or(command)
    }
}

/// Parses the text of a `keymap.txt`; errors name the line.
pub fn parse_keymap(text: &str) -> Result<KeyMap, String> {
    let mut map = HashMap::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let chars: Vec<char> = line.trim_end_matches('\r').chars().collect();
        let [from, to] = chars[..] else {
            return Err(format!("line {n}: expected two characters, got '{line}'"));
        };
        if from.is_whitespace() || to.is_whitespace() {
            return Err(format!("line {n}: a space cannot be a key"));
        }
        if map.insert(from, to).is_some() {
            return Err(format!("line {n}: '{from}' is already mapped"));
        }
    }
    Ok(KeyMap(map))
}

/// Reads and parses a keymap file; a missing file is an error.
pub fn load_keymap(path: &Path) -> Result<KeyMap, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_keymap(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The auto-loaded file: absent is fine, unreadable or malformed is not.
pub fn load_keymap_if_present(path: &Path) -> Result<KeyMap, String> {
    if path.exists() {
        load_keymap(path)
    } else {
        Ok(KeyMap::default())
    }
}

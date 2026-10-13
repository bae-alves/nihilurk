//! The run dump: a plain-text record of how a run ended — the final screen,
//! the pack, the kills and the message log — written to `morgue/` in the data
//! dir the moment the run ends, for sharing. Best-effort like the leaderboard:
//! a dump that can't be written never holds up the end screens.

use std::io::Write;
use std::path::{Path, PathBuf};

use bevy_ecs::prelude::{Entity, With, World};
use models::leaderboard::{Outcome, file_stamp, format_timestamp, now_epoch_secs};
use models::*;

use crate::view::{is_worn, pack_name, row_text};

/// Writes the dump for a run that just ended as `outcome`, with `screen_text`
/// as its final screen, and returns where it went.
pub fn write(world: &mut World, screen_text: &str, outcome: Outcome) -> std::io::Result<PathBuf> {
    write_in(&data_dir(), world, screen_text, outcome, now_epoch_secs())
}

/// [`write`] into `dir`'s `morgue/`, stamped `epoch_secs`. Each dump is a new
/// file: a name and a second already taken is an error, never an overwrite.
fn write_in(
    dir: &Path,
    world: &mut World,
    screen_text: &str,
    outcome: Outcome,
    epoch_secs: u64,
) -> std::io::Result<PathBuf> {
    let name = file_safe(&world.resource::<PlayerName>().what);
    let morgue = dir.join("morgue");
    let path = morgue.join(format!("{name}-{}.txt", file_stamp(epoch_secs)));
    let text = render(world, screen_text, outcome, &format_timestamp(epoch_secs));
    std::fs::create_dir_all(&morgue)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?
        .write_all(text.as_bytes())?;
    Ok(path)
}

/// A player name cut down to what is safe in a file name on every OS: letters
/// and digits stay, `-` and `_` stay, everything else becomes `_`. A name is
/// typed by the player, so `../x` must not climb out of `morgue/`.
fn file_safe(name: &str) -> String {
    name.chars()
        .map(|c| match c.is_alphanumeric() || c == '-' || c == '_' {
            true => c,
            false => '_',
        })
        .collect()
}

/// The dump's text: who, how it ended, when, then the final screen, the pack,
/// the kills (most-killed first) and the message log.
fn render(world: &mut World, screen_text: &str, outcome: Outcome, when: &str) -> String {
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next();
    let score = player
        .and_then(|p| world.get::<Score>(p))
        .map_or(0, |s| s.value);
    let items = player
        .and_then(|p| world.get::<Backpack>(p))
        .map(|b| b.items.clone())
        .unwrap_or_default();
    let pack = items.iter().enumerate().map(|(i, &item)| {
        let letter = (b'a' + i as u8) as char;
        row_text(letter, &pack_name(world, item), is_worn(world, item))
            .trim()
            .to_string()
    });

    let kills = &world.resource::<Kills>().by_name;
    let mut tally: Vec<(&String, &u32)> = kills.iter().collect();
    tally.sort_by_key(|&(_, count)| std::cmp::Reverse(*count));
    let total = tally.iter().map(|(_, count)| **count).sum();
    let width = tally.first().map_or(0, |(_, c)| c.to_string().len());

    let mut lines = vec![
        strings::morgue_header(
            &world.resource::<PlayerName>().what,
            env!("CARGO_PKG_VERSION"),
        ),
        strings::morgue_summary(
            &outcome.to_string(),
            &world.resource::<Ending>().cause,
            world.resource::<Depth>().what,
            score,
        ),
        strings::morgue_ended(when),
        String::new(),
        strings::morgue_screen_title().to_string(),
    ];
    lines.extend(screen_text.lines().map(str::to_string));
    lines.push(strings::morgue_pack_title().to_string());
    lines.extend(pack);
    lines.push(strings::morgue_kills_title(total));
    lines.extend(
        tally
            .iter()
            .map(|(name, count)| format!("{count:>width$} {name}")),
    );
    if outcome == Outcome::Win {
        let kept = models::conduct::kept_mask(world);
        lines.push(strings::morgue_conducts_title(
            models::conduct::bonus_percent(kept),
        ));
        lines.extend(
            Conduct::ALL
                .iter()
                .enumerate()
                .filter(|(_, c)| kept & c.bit() != 0)
                .map(|(i, _)| strings::conduct_name(i).to_string()),
        );
    }
    lines.push(strings::morgue_messages_title().to_string());
    lines.extend(world.resource::<GameLog>().history.iter().cloned());
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_world() -> World {
        let mut w = World::new();
        w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
        w.insert_resource(RngSeed(1));
        w.init_resource::<GameLog>();
        w.init_resource::<UseQueue>();
        w.init_resource::<ThrowQueue>();
        w.init_resource::<AttackQueue>();
        w.init_resource::<Ending>();
        w.init_resource::<PlayerTempo>();
        w.insert_resource(PlayerName {
            what: "../fixture name".into(),
        });
        initialize_world(&mut w);
        w.resource_mut::<Ending>().cause = "Slain by a fixture".into();
        w.resource_mut::<GameLog>().add("fixture log line");
        let mut kills = w.resource_mut::<Kills>();
        kills.by_name.insert("fixture-moth".into(), 1);
        kills.by_name.insert("fixture-gnat".into(), 2);

        let trinket = w
            .spawn(Name {
                what: "fixture trinket".into(),
            })
            .id();
        let player = w.query_filtered::<Entity, With<Player>>().single(&w);
        let mut pack = w.get_mut::<Backpack>(player).unwrap();
        pack.items.clear();
        pack.items.push(trinket);
        w
    }

    #[test]
    fn the_dump_says_how_the_run_ended_and_what_was_left() {
        let mut w = fixture_world();
        let text = render(
            &mut w,
            "fixture screen row\n",
            Outcome::LoseDescent,
            "2026-10-12 14:15",
        );
        let lines: Vec<&str> = text.lines().collect();

        assert!(lines[1].contains("Slain by a fixture"), "{text}");
        assert!(lines.contains(&"Ended 2026-10-12 14:15 UTC"), "{text}");
        assert!(lines.contains(&"fixture screen row"), "{text}");
        assert!(lines.contains(&"a) fixture trinket"), "{text}");
        assert!(lines.contains(&"-- Kills (3) --"), "{text}");
        let gnat = lines.iter().position(|l| l.ends_with("2 fixture-gnat"));
        let moth = lines.iter().position(|l| l.ends_with("1 fixture-moth"));
        assert!(gnat.is_some() && gnat < moth, "most-killed first:\n{text}");
        assert!(lines.contains(&"fixture log line"), "{text}");
    }

    #[test]
    fn a_won_dump_lists_the_conducts_kept_and_a_lost_one_does_not() {
        let mut w = fixture_world();
        models::conduct::broke(&mut w, models::Conduct::Potions);

        let won = render(&mut w, "", Outcome::Win, "2026-10-12 14:15");
        let lost = render(&mut w, "", Outcome::LoseDescent, "2026-10-12 14:15");

        assert!(won.contains("-- Conducts (+120%) --"), "{won}");
        assert!(won.contains("no scrolls"), "{won}");
        assert!(!won.contains("no potions"), "{won}");
        assert!(!lost.contains("Conducts"), "{lost}");
    }

    #[test]
    fn a_dump_lands_in_morgue_under_a_safe_name_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("nihilurk-morgue-{}", std::process::id()));
        let mut w = fixture_world();

        let path = write_in(&dir, &mut w, "", Outcome::Win, 0).unwrap();
        assert_eq!(
            path,
            dir.join("morgue")
                .join("___fixture_name-19700101-000000.txt")
        );
        assert!(std::fs::read_to_string(&path).unwrap().contains("WIN"));
        assert!(write_in(&dir, &mut w, "", Outcome::Win, 0).is_err());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

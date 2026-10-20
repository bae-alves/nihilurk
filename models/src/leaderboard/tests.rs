use super::*;

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "nihilurk-leaderboard-unit-{}-{tag}.sav",
            std::process::id()
        ))
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn each_board_has_its_own_file() {
    let paths = [Board::Normal, Board::Fast, Board::Endless].map(path);
    assert_eq!(paths[0].file_name().unwrap(), "leaderboard.sav");
    assert!(paths[0] != paths[1] && paths[1] != paths[2] && paths[0] != paths[2]);
}

#[test]
fn a_run_belongs_to_the_board_of_its_mode() {
    let mut w = bevy_ecs::world::World::new();
    assert_eq!(Board::of(&w), Board::Normal);
    w.insert_resource(crate::Floors::new(6));
    assert_eq!(Board::of(&w), Board::Fast);
    w.insert_resource(crate::Endless { enabled: true });
    assert_eq!(Board::of(&w), Board::Endless);
}

#[test]
fn a_recorded_run_round_trips_in_score_order() {
    let path = temp_path("roundtrip");
    let _ = std::fs::remove_file(&path);

    record_to(&path, "GUEST", Outcome::LoseDescent, 100, 0, 1).unwrap();
    record_to(&path, "BAE", Outcome::Win, 4200, 0, 2).unwrap();
    record_to(&path, "AL-OK", Outcome::LoseAscent, 900, 0, 3).unwrap();

    let entries: Vec<_> = all(&path)
        .unwrap()
        .into_iter()
        .map(|e| (e.name, e.outcome, e.score))
        .collect();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        entries,
        vec![
            ("BAE".to_string(), Outcome::Win, 4200),
            ("AL-OK".to_string(), Outcome::LoseAscent, 900),
            ("GUEST".to_string(), Outcome::LoseDescent, 100),
        ]
    );
}

#[test]
fn the_file_never_grows_past_the_store_limit() {
    let path = temp_path("cap");
    let _ = std::fs::remove_file(&path);

    for i in 0..(LEADERBOARD_STORE_LIMIT + 5) {
        record_to(&path, "P", Outcome::Win, i as i64, 0, 0).unwrap();
    }

    let entries = all(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(entries.len(), LEADERBOARD_STORE_LIMIT);
    assert_eq!(entries.last().unwrap().score, 5);
}

#[test]
fn a_conduct_mask_round_trips_with_its_run() {
    let path = temp_path("mask");
    let _ = std::fs::remove_file(&path);

    record_to(&path, "BAE", Outcome::Win, 4200, 0b101, 2).unwrap();

    let masks: Vec<u16> = all(&path)
        .unwrap()
        .into_iter()
        .map(|e| e.conducts)
        .collect();
    let _ = std::fs::remove_file(&path);
    assert_eq!(masks, vec![0b101]);
}

#[test]
fn a_version_one_board_is_read_kept_and_upgraded_on_the_next_write() {
    let path = temp_path("v1");
    let old = vec![EntryV1 {
        name: "OLD".into(),
        outcome: Outcome::Win,
        score: 777,
        epoch_secs: 5,
    }];
    std::fs::write(&path, postcard::to_allocvec(&(1u16, old)).unwrap()).unwrap();

    let read: Vec<_> = all(&path)
        .unwrap()
        .into_iter()
        .map(|e| (e.name, e.score, e.conducts))
        .collect();
    assert_eq!(read, vec![("OLD".to_string(), 777, 0)]);

    record_to(&path, "NEW", Outcome::Win, 900, 0b11, 6).unwrap();
    let after: Vec<_> = all(&path)
        .unwrap()
        .into_iter()
        .map(|e| (e.name, e.conducts))
        .collect();
    let version = postcard::take_from_bytes::<u16>(&std::fs::read(&path).unwrap())
        .unwrap()
        .0;
    let _ = std::fs::remove_file(&path);
    assert_eq!(version, LEADERBOARD_VERSION);
    assert_eq!(
        after,
        vec![("NEW".to_string(), 0b11), ("OLD".to_string(), 0)]
    );
}

#[test]
fn a_name_with_an_escape_reads_back_without_it() {
    let path = temp_path("escape");
    let _ = std::fs::remove_file(&path);
    record_to(&path, "BAE\x1b[31m", Outcome::Win, 1, 0, 0).unwrap();

    let entries = all(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{path}.lock"));

    assert_eq!(entries[0].name, "BAE[31m");
}

#[test]
fn a_missing_leaderboard_is_an_empty_one() {
    assert!(all(temp_path("missing")).unwrap().is_empty());
}

#[test]
fn a_corrupt_leaderboard_is_refused_and_left_alone() {
    let path = temp_path("corrupt");
    std::fs::write(&path, b"\xff\xff\xff not a leaderboard").unwrap();

    assert!(all(&path).is_err());
    assert!(record_to(&path, "BAE", Outcome::Win, 1, 0, 0).is_err());

    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"\xff\xff\xff not a leaderboard"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_leaderboard_from_another_version_is_refused_and_left_alone() {
    let path = temp_path("version");
    let theirs: Vec<Entry> = Vec::new();
    let bytes = postcard::to_allocvec(&(LEADERBOARD_VERSION + 1, theirs)).unwrap();
    std::fs::write(&path, &bytes).unwrap();

    assert!(record_to(&path, "BAE", Outcome::Win, 1, 0, 0).is_err());

    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn runs_ending_at_once_all_make_the_board() {
    for round in 0..5 {
        let path = temp_path(&format!("race-{round}"));
        let _ = std::fs::remove_file(&path);
        let start = std::sync::Barrier::new(LEADERBOARD_STORE_LIMIT);

        std::thread::scope(|s| {
            for score in 0..LEADERBOARD_STORE_LIMIT as i64 {
                let (path, start) = (&path, &start);
                s.spawn(move || {
                    start.wait();
                    record_to(path, "P", Outcome::Win, score, 0, 0).unwrap();
                });
            }
        });

        let entries = all(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{path}.lock"));
        assert_eq!(
            entries.len(),
            LEADERBOARD_STORE_LIMIT,
            "round {round} lost a score"
        );
    }
}

#[test]
fn known_epoch_seconds_format_to_their_known_utc_date() {
    assert_eq!(format_timestamp(0), "1970-01-01 00:00");
    assert_eq!(format_timestamp(1_704_067_200), "2024-01-01 00:00");
    assert_eq!(format_timestamp(1_735_689_540), "2024-12-31 23:59");
}

#[test]
fn a_file_stamp_sorts_by_time_and_keeps_the_seconds() {
    assert_eq!(file_stamp(0), "19700101-000000");
    assert_eq!(file_stamp(1_735_689_599), "20241231-235959");
}

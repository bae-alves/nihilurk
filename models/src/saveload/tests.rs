use super::*;
use rand::SeedableRng;
use std::io::Write;

#[test]
fn every_saved_colour_survives_the_byte_codec() {
    use Color::*;
    let palette = [
        Black,
        DarkGrey,
        Red,
        DarkRed,
        Green,
        DarkGreen,
        Yellow,
        DarkYellow,
        Blue,
        DarkBlue,
        Magenta,
        DarkMagenta,
        Cyan,
        DarkCyan,
        White,
        Grey,
    ];
    for (n, c) in palette.into_iter().enumerate() {
        assert_eq!(color_to_u8(&c), n as u8);
        assert_eq!(u8_to_color(n as u8), c);
    }
}

#[cfg(unix)]
#[test]
fn a_planted_symlink_is_replaced_not_written_through() {
    let dir = std::env::temp_dir().join(format!("nihilurk-fresh-unit-{}-link", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let victim = dir.join("victim");
    std::fs::write(&victim, b"mine").unwrap();
    let path = dir.join("run.sav");
    std::os::unix::fs::symlink(&victim, with_suffix(&path, ".tmp")).unwrap();

    write_atomically(&path, |w| w.write_all(b"the run")).unwrap();

    assert_eq!(std::fs::read(&victim).unwrap(), b"mine");
    assert_eq!(std::fs::read(&path).unwrap(), b"the run");

    std::os::unix::fs::symlink(&victim, dir.join("rec")).unwrap();
    create_fresh(&dir.join("rec"))
        .unwrap()
        .write_all(b"x")
        .unwrap();
    assert_eq!(std::fs::read(&victim).unwrap(), b"mine");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_is_thirteen_characters_without_control_characters() {
    assert_eq!(name_fault("ABCDEFGHIJKLM"), None);
    assert_eq!(name_fault("Zoë Júnior"), None);
    assert_eq!(name_fault("ABCDEFGHIJKLMN"), Some(NameFault::TooLong));
    assert_eq!(name_fault("bae\x1b[31m"), Some(NameFault::ControlChar));
    assert_eq!(name_fault("a\tb"), Some(NameFault::ControlChar));
    assert_eq!(clean_name("ab\x1bcdefghijklmnop"), "abcdefghijklm");
}

#[test]
fn a_windows_device_name_is_no_name() {
    for name in ["nul", "NUL", "Con", "prn", "aux", "com1", "LPT9"] {
        assert_eq!(name_fault(name), Some(NameFault::Reserved), "{name}");
        assert!(!file_safe(name).eq_ignore_ascii_case(name), "{name}");
    }
    assert_eq!(file_safe("nul"), "_nul");
    assert_eq!(name_fault("null"), None);
    assert_eq!(name_fault("com10"), None);
    assert_eq!(name_fault("console"), None);
}

#[test]
fn a_file_name_keeps_words_and_loses_paths_and_length() {
    assert_eq!(file_safe("Zoë-9_x"), "Zoë-9_x");
    assert_eq!(file_safe("/etc/passwd"), "_etc_passwd");
    assert_eq!(file_safe("..\\..\\x"), "______x");
    assert_eq!(file_safe(&"a".repeat(200)).chars().count(), 64);
}

fn atomic_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "nihilurk-atomic-unit-{}-{tag}.sav",
            std::process::id()
        ))
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_write_that_fails_halfway_leaves_the_old_file_whole() {
    let path = atomic_path("fails");
    std::fs::write(&path, b"the old run").unwrap();

    let result = write_atomically(&path, |w| {
        w.write_all(b"half a")?;
        w.flush()?;
        Err(std::io::Error::other("killed mid-write"))
    });

    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"the old run");
    assert!(!std::path::Path::new(&format!("{path}.tmp")).exists());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_first_write_makes_the_directory_it_lands_in() {
    let dir = std::env::temp_dir().join(format!("nihilurk-atomic-unit-{}-dir", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("nested").join("lurk.sav");

    write_atomically(&path, |w| w.write_all(b"a run")).unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"a run");
    let _ = std::fs::remove_dir_all(&dir);
}

fn resolve(os: &str, vars: &[(&str, &str)]) -> std::path::PathBuf {
    data_dir_from(os, |key| {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| std::ffi::OsString::from(v))
    })
}

#[test]
fn the_data_dir_follows_each_os_convention() {
    use std::path::PathBuf;
    let xdg = std::env::temp_dir();
    assert_eq!(
        resolve(
            "linux",
            &[
                ("XDG_DATA_HOME", xdg.to_str().unwrap()),
                ("HOME", "/home/b")
            ]
        ),
        xdg.join("nihilurk")
    );
    assert_eq!(
        resolve("freebsd", &[("HOME", "/home/b")]),
        PathBuf::from("/home/b")
            .join(".local/share")
            .join("nihilurk")
    );
    assert_eq!(
        resolve("macos", &[("HOME", "/Users/b"), ("XDG_DATA_HOME", "/xdg")]),
        PathBuf::from("/Users/b")
            .join("Library/Application Support")
            .join("nihilurk")
    );
    assert_eq!(
        resolve("windows", &[("APPDATA", "C:\\Users\\b\\AppData\\Roaming")]),
        PathBuf::from("C:\\Users\\b\\AppData\\Roaming").join("nihilurk")
    );
}

#[test]
fn nihilurk_data_overrides_the_os_convention() {
    assert_eq!(
        resolve(
            "linux",
            &[("NIHILURK_DATA", "/srv/lurk"), ("HOME", "/home/b")]
        ),
        std::path::PathBuf::from("/srv/lurk")
    );
}

#[test]
fn a_relative_or_empty_variable_is_ignored() {
    assert_eq!(
        resolve(
            "linux",
            &[
                ("NIHILURK_DATA", ""),
                ("XDG_DATA_HOME", "rel"),
                ("HOME", "/home/b")
            ]
        ),
        std::path::PathBuf::from("/home/b")
            .join(".local/share")
            .join("nihilurk")
    );
}

#[test]
fn with_no_home_at_all_the_data_dir_is_the_current_one() {
    assert_eq!(resolve("linux", &[]), std::path::PathBuf::from("."));
}

#[test]
fn a_write_that_succeeds_replaces_the_file_and_leaves_no_temp() {
    let path = atomic_path("succeeds");
    std::fs::write(&path, b"the old run").unwrap();

    write_atomically(&path, |w| w.write_all(b"the new run")).unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"the new run");
    assert!(!std::path::Path::new(&format!("{path}.tmp")).exists());
    let _ = std::fs::remove_file(&path);
}

/// An [`EntitySave`] with every field at its empty default, so a test only
/// has to name the field it actually cares about.
fn blank_entity<'a>() -> EntitySave<'a> {
    EntitySave {
        position: None,
        renderable: None,
        player: false,
        hidden: false,
        invisible: false,
        consume: false,
        amulet: false,
        name: None,
        viewshed: None,
        fighter: None,
        magic: None,
        faction: None,
        backpack: None,
        score: None,
        mob: None,
        item: false,
        value: None,
        potion: None,
        battery: None,
        wand: None,
        ranged: None,
        scroll: None,
        rune: None,
        ring: None,
        equipped: None,
        power_die: None,
        power_bonus: None,
        armor_die: None,
        armor_bonus: None,
        throw_bonus: None,
        max_hp_bonus: None,
        stack: None,
        curse: false,
        megacurse: false,
        known_quality: false,
        vorpal: None,
        effects: Vec::new(),
        speed: None,
        trap: None,
        pickup: None,
        plated: 0,
        forged: 0,
        spellset: None,
        equipped_by: None,
        helper: false,
        mimic: false,
        ice_cube: false,
        aggravated: None,
        alignment: None,
        deck: None,
        content: None,
    }
}

/// A [`SaveGame`] with every field at its empty default.
fn blank_save<'a>(entities: Vec<EntitySave<'a>>) -> SaveGame<'a> {
    SaveGame {
        entities,
        player_name: Cow::Borrowed("X"),
        depth: 1,
        floor_changes: 0,
        rng_seed: 1,
        rng_state: ChaCha12Rng::seed_from_u64(1),
        dark_tiles: FixedBitSet::with_capacity(1),
        dug_tiles: Vec::new(),
        cleared: false,
        spirits_hostile: false,
        monster_body: None,
        kills: Vec::new(),
        floors: FINAL_DEPTH,
        endless: false,
        conducts: 0,
    }
}

/// The bytes of `save` as `save_game` would write them, under `version`.
fn encode(version: u16, save: &SaveGame) -> Vec<u8> {
    postcard::to_allocvec(&(version, save)).unwrap()
}

/// A save is untrusted the moment it can come from anywhere but this
/// build's own [`save_game`] — a shared file, a bug report attachment.
/// `backpack` and `equipped_by` are raw indices into the saved entity
/// list with nothing upstream bounding them, so a crafted save naming an
/// index past the end of the list must fail to load rather than index
/// straight into `new_ents`.
fn crafted_save_with_backpack_index(index: u32) -> Vec<u8> {
    let mut entity = blank_entity();
    entity.backpack = Some(vec![index]);
    encode(SAVE_VERSION, &blank_save(vec![entity]))
}

fn load_bytes(tag: &str, bytes: &[u8]) -> std::io::Result<World> {
    let path = std::env::temp_dir().join(format!(
        "nihilurk-saveload-unit-{}-{tag}.sav",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    let mut world = World::new();
    world.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2)));
    world.insert_resource(RngSeed(2));
    world.init_resource::<GameLog>();
    world.insert_resource(PlayerName { what: "Y".into() });
    let result = load_game(&mut world, path.to_str().unwrap());
    let _ = std::fs::remove_file(&path);
    result.map(|_| world)
}

#[test]
fn an_out_of_range_backpack_index_is_rejected_not_indexed() {
    let bytes = crafted_save_with_backpack_index(1);
    assert!(
        load_bytes("oob", &bytes).is_err(),
        "a crafted save with an out-of-range backpack index must error, not panic"
    );
}

#[test]
fn a_short_dark_room_list_still_lets_a_wand_of_light_work() {
    let mut save = blank_save(vec![blank_entity()]);
    save.dark_tiles = FixedBitSet::with_capacity(1);
    let mut world = load_bytes("dark", &encode(SAVE_VERSION, &save)).unwrap();

    world
        .resource_mut::<Map>()
        .light_tile(crate::map::MAP_WIDTH - 1, crate::map::MAP_HEIGHT - 1);
}

#[test]
fn a_save_from_another_version_is_refused_and_says_which() {
    let other = SAVE_VERSION + 1;
    let bytes = encode(other, &blank_save(vec![blank_entity()]));
    let err = load_bytes("version", &bytes)
        .err()
        .expect("a save under another version must not load");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    let said = err.to_string();
    assert!(
        said.contains(&other.to_string()) && said.contains(&SAVE_VERSION.to_string()),
        "the refusal should name both versions: {said}"
    );
}

#[test]
fn clear_data_leaves_a_save_from_another_version_to_the_loader() {
    let mut save = blank_save(vec![blank_entity()]);
    save.cleared = true;
    let path = std::env::temp_dir().join(format!(
        "nihilurk-saveload-unit-{}-clear-version.sav",
        std::process::id()
    ));
    std::fs::write(&path, encode(SAVE_VERSION + 1, &save)).unwrap();
    let clear = clear_data(path.to_str().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(clear.is_none());
}

#[test]
fn a_save_naming_a_species_this_build_lacks_is_refused() {
    let mut save = blank_save(vec![blank_entity()]);
    save.monster_body = Some(Cow::Borrowed("no such species"));
    let err = load_bytes("species", &encode(SAVE_VERSION, &save))
        .err()
        .expect("an unknown species must not load as a plain nihil");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("no such species"));
}

/// Every saved field set to a value no empty default shares, the wide ones
/// past a one-byte varint, and each enum at its last variant: inserting a
/// variant anywhere before it moves its index and changes the bytes, while
/// appending one (which the module docs allow) does not. A new field is a
/// compile error here, which is the point.
fn full_entity<'a>() -> EntitySave<'a> {
    EntitySave {
        position: Some((700, 300)),
        renderable: Some(('@', 14)),
        player: true,
        hidden: true,
        invisible: true,
        consume: true,
        amulet: true,
        name: Some(Cow::Borrowed("name")),
        viewshed: Some((9, bits(&[3, 69]))),
        fighter: Some((300, 301, -2, 5, 6, 7, 8)),
        magic: Some(Magic {
            points: 3,
            max_points: 4,
        }),
        faction: Some(Faction::Spirits),
        backpack: Some(vec![1, 300]),
        score: Some(-5000),
        mob: Some(MovementType::Ambush),
        item: true,
        value: Some(300),
        potion: Some("potion of polymorph".to_string()),
        battery: Some(-3),
        wand: Some("wand of swapping".to_string()),
        ranged: Some(300),
        scroll: Some("scroll of atonement".to_string()),
        ring: Some(RingEffect::Polymorph),
        equipped: Some(Slot::Finger),
        power_die: Some(300),
        power_bonus: Some(-300),
        armor_die: Some(301),
        armor_bonus: Some(-301),
        throw_bonus: Some(302),
        max_hp_bonus: None,
        stack: Some(200),
        curse: true,
        megacurse: true,
        known_quality: true,
        vorpal: Some(Cow::Borrowed("bane")),
        effects: vec![
            SavedEffect {
                id: "first".into(),
                lifetime: SavedLifetime::Turns(300),
                count: 0,
            },
            SavedEffect {
                id: "second".into(),
                lifetime: SavedLifetime::NextAction,
                count: 0,
            },
        ],
        speed: Some(SpeedKind::Quick),
        trap: Some(("dart trap".to_string(), TrapReveal::Triggered, true)),
        pickup: Some("hero coin".to_string()),
        plated: 1,
        forged: 1,
        spellset: Some(vec!["Gate Down".to_string()]),
        equipped_by: Some(300),
        helper: true,
        mimic: true,
        ice_cube: true,
        aggravated: Some((700, 701)),
        alignment: Some(-100),
        deck: Some(vec![("XXIII GOLDEN WIND".to_string(), true)]),
        rune: Some(("rune of protection".to_string(), true)),
        content: Some(Cow::Borrowed("arrow")),
    }
}

fn bits(set: &[usize]) -> FixedBitSet {
    let mut bits = FixedBitSet::with_capacity(70);
    bits.extend(set.iter().copied());
    bits
}

fn full_save<'a>() -> SaveGame<'a> {
    let mut save = blank_save(vec![full_entity(), blank_entity()]);
    save.player_name = Cow::Borrowed("player");
    save.depth = 200;
    save.floor_changes = 70_000;
    save.rng_seed = 0x0123_4567_89ab_cdef;
    save.dark_tiles = bits(&[0, 64]);
    save.dug_tiles = vec![1, 70_000];
    save.cleared = true;
    save.spirits_hostile = true;
    save.monster_body = Some(Cow::Borrowed("dragon"));
    save.kills = vec![(Cow::Borrowed("rat"), 70_000)];
    save
}

fn split_header(bytes: &[u8]) -> (u16, &[u8]) {
    postcard::take_from_bytes::<u16>(bytes).expect("a version leads the file")
}

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/save.bin");

/// The bytes of [`full_save`] are checked in. A change to a saved struct,
/// or to a serialised dependency such as the RNG, changes them, and the
/// test then asks for two things in order: bump [`SAVE_VERSION`], then
/// regenerate the file. It will not regenerate under the old version, so a
/// format change cannot slip into a release as a patch.
#[test]
fn the_save_layout_only_changes_with_the_version() {
    let now = encode(SAVE_VERSION, &full_save());
    let (now_version, now_body) = split_header(&now);
    let golden = std::fs::read(GOLDEN).unwrap_or_default();
    let (golden_version, golden_body) = match golden.is_empty() {
        true => (0, &golden[..]),
        false => split_header(&golden),
    };
    if now_body != golden_body {
        assert_ne!(
            now_version, golden_version,
            "the saved layout changed, but SAVE_VERSION is still {now_version}. Bump it in \
             saveload.rs: old saves no longer load, so this ships as a minor release."
        );
    }
    if std::env::var_os("NIHILURK_REGEN_GOLDEN").is_some() {
        std::fs::create_dir_all(std::path::Path::new(GOLDEN).parent().unwrap()).unwrap();
        std::fs::write(GOLDEN, &now).unwrap();
        return;
    }
    assert!(
        now == golden,
        "tests/golden/save.bin is out of date. Regenerate it with \
         `NIHILURK_REGEN_GOLDEN=1 cargo test -p nihilurk-models --lib the_save_layout`"
    );
    decode(&golden).expect("the checked-in save still decodes");
}

fn load_one(tag: &str, entity: EntitySave) -> World {
    load_bytes(tag, &encode(SAVE_VERSION, &blank_save(vec![entity])))
        .expect("a well-formed save loads")
}

#[test]
fn a_wand_no_row_has_refuses_the_load() {
    let mut entity = blank_entity();
    entity.wand = Some("wand of nowhere".to_string());
    let bytes = encode(SAVE_VERSION, &blank_save(vec![entity]));
    let err = load_bytes("unknown-wand", &bytes)
        .err()
        .expect("the load is refused");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn a_potion_no_row_has_refuses_the_load() {
    let mut entity = blank_entity();
    entity.potion = Some("potion of nowhere".to_string());
    let bytes = encode(SAVE_VERSION, &blank_save(vec![entity]));
    let err = load_bytes("unknown-potion", &bytes)
        .err()
        .expect("the load is refused");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn a_scroll_no_row_has_refuses_the_load() {
    let mut entity = blank_entity();
    entity.scroll = Some("scroll of nowhere".to_string());
    let bytes = encode(SAVE_VERSION, &blank_save(vec![entity]));
    let err = load_bytes("unknown-scroll", &bytes)
        .err()
        .expect("the load is refused");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn a_trap_no_row_has_refuses_the_load() {
    let mut entity = blank_entity();
    entity.trap = Some(("trap of nowhere".to_string(), TrapReveal::Sight, false));
    let bytes = encode(SAVE_VERSION, &blank_save(vec![entity]));
    let err = load_bytes("unknown-trap", &bytes)
        .err()
        .expect("the load is refused");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn a_rune_no_row_has_refuses_the_load() {
    let mut entity = blank_entity();
    entity.rune = Some(("rune of nowhere".to_string(), true));
    let bytes = encode(SAVE_VERSION, &blank_save(vec![entity]));
    let err = load_bytes("unknown-rune", &bytes)
        .err()
        .expect("the load is refused");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn a_saved_content_id_wins_over_the_saved_name() {
    let mut entity = blank_entity();
    entity.name = Some(Cow::Borrowed("a name no row has"));
    entity.content = Some(Cow::Borrowed("arrow"));
    let mut w = load_one("id-wins", entity);
    let arrow = w.query_filtered::<Entity, With<Projectile>>().single(&w);
    assert_eq!(
        crate::helpers::item_label(&w, arrow),
        strings::content_name("arrow")
    );
}

#[test]
fn a_saved_name_alone_restores_nothing_from_the_catalog() {
    let mut entity = blank_entity();
    entity.name = Some(Cow::Borrowed("arrow"));
    let mut w = load_one("name-alone", entity);
    assert_eq!(w.query::<&Projectile>().iter(&w).count(), 0);
    let only = w.query::<Entity>().single(&w);
    assert_eq!(crate::helpers::item_label(&w, only), "arrow");
}

#[test]
fn an_in_range_backpack_index_still_loads() {
    let bytes = crafted_save_with_backpack_index(0);
    assert!(load_bytes("ok", &bytes).is_ok());
}

/// A save carrying a `player_name` with control/escape bytes -- a shared
/// save is untrusted the same way a crafted backpack index is, and this
/// field reaches a real terminal verbatim (`clear_data_prompt`, the HUD)
/// rather than an index bounds-check.
fn crafted_save_with_player_name(name: &str, cleared: bool) -> Vec<u8> {
    let mut save = blank_save(vec![blank_entity()]);
    save.player_name = Cow::Borrowed(name);
    save.cleared = cleared;
    encode(SAVE_VERSION, &save)
}

#[test]
fn a_loaded_player_name_has_its_control_bytes_stripped() {
    let evil = "Bae\u{1b}]0;pwned\u{7}";
    let bytes = crafted_save_with_player_name(evil, false);
    let world = load_bytes("ctrl-name", &bytes).expect("a bad name must not fail the load");
    let loaded = &world.resource::<PlayerName>().what;
    assert!(
        !loaded.chars().any(|c| c.is_control()),
        "loaded player name still has control bytes: {loaded:?}"
    );
    assert_eq!(loaded, "Bae]0;pwned");
}

#[test]
fn clear_data_strips_control_bytes_from_the_winners_name() {
    let evil = "Bae\u{1b}]0;pwned\u{7}";
    let bytes = crafted_save_with_player_name(evil, true);
    let path = std::env::temp_dir().join(format!(
        "nihilurk-saveload-unit-{}-clear-ctrl.sav",
        std::process::id()
    ));
    std::fs::write(&path, &bytes).unwrap();
    let clear = clear_data(path.to_str().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    let name = clear
        .expect("cleared=true must read back as clear data")
        .player_name;
    assert!(
        !name.chars().any(|c| c.is_control()),
        "clear-data player name still has control bytes: {name:?}"
    );
}

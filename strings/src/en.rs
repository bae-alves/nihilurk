//! English. The reference translation -- every other language starts as a
//! copy of this file (see `lib.rs`) and is judged against it.
//!
//! One function or const per player-facing sentence, grouped by the source
//! file that used to hold the literal inline. Names describe the message,
//! not the caller, since the same sentence sometimes has more than one call
//! site (e.g. several "you can't do that right now" guards used to duplicate
//! the same literal; they now duplicate a call to the same function instead).

/// A content-table row's display name.
///
/// A monster's, item's or trap's `name` field is *also* the id `-content`,
/// `-am <species>`, `NIHILURK_SPAWN` and save files match against
/// (`docs/reference/cli-and-env.md`), so it cannot simply be swapped for
/// translated text the way a log line can. English's id and display text are
/// the same string today, so this is the identity function; a translated
/// locale overrides it with its own id -> name table instead of re-exporting
/// this one, once someone actually writes that table.
pub fn content_name(id: &str) -> &str {
    id
}

// ---------------------------------------------------------------------------
// HUD status-line abbreviations (models/src/body.rs, engine/src/view.rs)
//
// Shared between the two: `view.rs` also matches a rendered label back
// against these to pick a colour, so both call sites reading from the same
// function is what keeps the label and the match in step with each other.
// ---------------------------------------------------------------------------

pub fn hp_abbr() -> &'static str {
    "HP"
}

pub fn magic_abbr() -> &'static str {
    "Ma"
}

pub fn power_abbr() -> &'static str {
    "Pow."
}

pub fn armor_abbr() -> &'static str {
    "Arm."
}

pub fn skill_abbr() -> &'static str {
    "Skl."
}

// ---------------------------------------------------------------------------
// models/src/body.rs
// ---------------------------------------------------------------------------

pub fn fear_the_lurk(step: i32, stat: &str) -> String {
    format!("FEAR THE LURK! (+{step} {stat})")
}

pub fn no_hands_lurk(item_name: &str) -> String {
    format!("Fur and fangs and four paws: no part of a lurk holds {item_name}.")
}

pub fn no_hands_polymorphed(item_name: &str) -> String {
    format!("This shape has no hands for {item_name}.")
}

pub fn no_hands_monster(species: &str, item_name: &str) -> String {
    format!("A {species} has no hands for {item_name}.")
}

// ---------------------------------------------------------------------------
// models/src/monsters.rs
// ---------------------------------------------------------------------------

pub fn wearing_gear(verb: &str, name: &str) -> String {
    format!("They are {verb} {name}.")
}

pub fn xeroc_disguise_falls() -> &'static str {
    "The disguise falls away - they were a xeroc all along!"
}

// ---------------------------------------------------------------------------
// models/src/items/wands.rs
// ---------------------------------------------------------------------------

pub fn dazzle_player_line() -> &'static str {
    "The flash leaves you reeling - you are dazzled!"
}

pub fn dazzle_mob_verb() -> &'static str {
    "is dazzled"
}

pub fn bolt_magic_missile() -> &'static str {
    "A brilliant cyan bolt leaps from the wand!"
}

pub fn bolt_lightning() -> &'static str {
    "A forking bolt of lightning cracks out!"
}

pub fn bolt_striking() -> &'static str {
    "An invisible fist hammers down the line!"
}

pub fn bolt_drain_life() -> &'static str {
    "A tendril of black light drinks the life from its path."
}

pub fn drain_life_gained(taken: i32) -> String {
    format!("You drain {taken} life.")
}

pub fn blast_fire() -> &'static str {
    "A roaring sphere of fire erupts!"
}

pub fn blast_cold() -> &'static str {
    "A blast of freezing air detonates!"
}

pub fn dig_crumbles() -> &'static str {
    "The rock crumbles away in a straight line."
}

pub fn wand_does_nothing() -> &'static str {
    "The wand does nothing. It was well named."
}

pub fn light_reveals(label: &str) -> String {
    format!("The light reveals {label}!")
}

pub fn light_floods_room() -> &'static str {
    "Warm light floods the room."
}

pub fn light_races_passage() -> &'static str {
    "Light races the length of the passage."
}

pub fn polymorph_fizzles() -> &'static str {
    "The bolt of change fizzles against nothing."
}

pub fn polymorph_self(article: &str, new_name: &str) -> String {
    format!("You take on the nature of {article} {new_name}!")
}

pub fn polymorph_drops_gear() -> &'static str {
    "Your gear slips off you."
}

// TODO: placeholder English; needs a human's pass.
pub fn form_holds_player() -> &'static str {
    "The bolt of change finds nothing in you to bend."
}

// TODO: placeholder English; needs a human's pass.
pub fn form_holds_mob(name: &str) -> String {
    format!("The bolt of change slides off the {name}.")
}

pub fn polymorph_same_looking(old_name: &str, new_name: &str) -> String {
    format!("The {old_name} twists and warps into a different-looking {new_name}!")
}

pub fn polymorph_different(old_name: &str, article: &str, new_name: &str) -> String {
    format!("The {old_name} twists and warps into {article} {new_name}!")
}

pub fn shapeshift_reveal(from_article: &str, from: &str, to_article: &str, to: &str) -> String {
    format!("It was never {from_article} {from}, but {to_article} {to}!")
}

pub fn faerie_reveal(from_article: &str, from: &str) -> String {
    format!("It was never {from_article} {from}, but a faerie shapeshifter!")
}

pub fn nothing_to_enchant() -> &'static str {
    "Nothing there to enchant."
}

pub fn teleport_pull_finds_nothing() -> &'static str {
    "The wand's pull finds nothing."
}

pub fn yanked_into_dark(name: &str) -> String {
    format!("The {name} is yanked away into the dark.")
}

pub fn dragged_to_your_side(name: &str) -> String {
    format!("The {name} is dragged to your side!")
}

pub fn bursts_in_transit(name: &str) -> String {
    format!(
        "The {name} is dragged into the space between and comes apart - it bursts in a spray of gore!"
    )
}

pub fn teleport_self_player() -> &'static str {
    "You teleport straight to yourself. What a trip."
}

pub fn teleport_self_mob(name: &str) -> String {
    format!("The {name} teleports directly to themselves.")
}

pub fn swap_finds_nothing() -> &'static str {
    "The wand's twist finds nothing to trade places with."
}

pub fn you_swap_places(name: &str) -> String {
    format!("You trade places with the {name}.")
}

/// `with` is `None` when the other side is you.
pub fn mob_swaps_places(name: &str, with: Option<&str>) -> String {
    match with {
        Some(other) => format!("The {name} trades places with the {other}."),
        None => format!("The {name} trades places with you!"),
    }
}

pub fn blast_shuffles_places() -> &'static str {
    "Everyone in the blast trades places!"
}

pub fn cancellation_strikes_stone() -> &'static str {
    "The grey ray strikes only stone."
}

pub fn cancellation_sputters(name: &str) -> String {
    format!("The {name}'s magic sputters and dies.")
}

pub fn cancellation_player_wave() -> &'static str {
    "A grey wave washes over you. Your pack goes quiet, your gear goes plain, and every curse on you simply lets go."
}

// ---------------------------------------------------------------------------
// models/src/traps.rs
// ---------------------------------------------------------------------------

pub fn bear_trap_thrash() -> &'static str {
    "As you try to free yourself, the trap flays your leg."
}

pub fn clamped_thrash() -> &'static str {
    "As you try to pull free, its jaws tear at your leg."
}

pub fn steps_on_trap(who: &str, article: &str, label: &str) -> String {
    format!("{who} steps on {article} {label}!")
}

pub fn trap_breaks(label: &str) -> String {
    format!("The {label} breaks!")
}

pub fn hero_coin_ultimate() -> &'static str {
    "The hero coin gives up everything it knows at once."
}

pub fn relic_takes_the_hit() -> &'static str {
    "The Element of Yoord takes the hit - and answers."
}

pub fn ultimate_trick_shot_shout() -> &'static str {
    "ULTIMATE TRICK SHOT!"
}

pub fn trick_shot_shout_self() -> &'static str {
    "WHY!"
}

pub fn trick_shot_shout_other() -> &'static str {
    "BAM!"
}

pub fn trick_shot_line(shout: &str) -> String {
    format!("{shout} Trick shot!")
}

pub fn drops_through_trapdoor(who: &str) -> String {
    format!("{who} drops through the trapdoor and is gone.")
}

pub fn trapdoor_grinds_shut() -> &'static str {
    "A trapdoor gapes - but there is only solid rock below. It grinds shut."
}

pub fn trapdoor_yawns_open() -> &'static str {
    "A trapdoor yawns open beneath you!"
}

pub fn bear_trap_snare() -> &'static str {
    "Steel jaws snap shut on your leg - you can't take a step, but your arms are free!"
}

pub fn sleep_gas_snare() -> &'static str {
    "Gas billows up around you. Your eyelids turn to lead..."
}

pub fn teleport_trap_whisked() -> &'static str {
    "The walls change! You are whisked to a different part of the dungeon."
}

pub fn arrow_whistles_past(who: &str) -> String {
    format!("An arrow whistles past {who} and clatters away.")
}

pub fn arrow_plinks(who: &str, damage: i32) -> String {
    format!("An arrow plinks into {who} for {damage} damage!")
}

pub fn dart_glances_off(who: &str) -> String {
    format!("A dart glances off {who}.")
}

pub fn dart_pricks(who: &str, damage: i32) -> String {
    format!("A poisoned dart pricks {who} for {damage} damage!")
}

pub fn dart_poison_resisted() -> &'static str {
    "The poison burns, but your strength holds firm."
}

pub fn dart_poison_took() -> &'static str {
    "The poison courses through you - you feel your strength ebb away."
}

// ---------------------------------------------------------------------------
// models/src/items/theft.rs
// ---------------------------------------------------------------------------

pub fn leprechaun_theft(attacker: &str, item: &str, target: &str) -> String {
    format!("The {attacker} snatches the {item} from {target} and cackles!")
}

pub fn nymph_theft(attacker: &str, item: &str, target: &str) -> String {
    format!("The {attacker} rips the {item} from {target} and vanishes in a puff of smoke!")
}

// ---------------------------------------------------------------------------
// models/src/items/rings.rs
// ---------------------------------------------------------------------------

pub const FANFARE: [&str; 4] = [
    "Magenta, cyan and gold pour off you all at once.",
    "The dungeon, briefly, is a ballroom.",
    "And you do it with style!",
    "Your score is doubled!",
];

pub fn adornment_spent(name: &str) -> String {
    format!("The {name} has nothing left to give.")
}

// ---------------------------------------------------------------------------
// models/src/hud.rs, models/src/score.rs
// ---------------------------------------------------------------------------

/// The score-flash word for a combo kill. Shared with [`with_pride`]/`"With
/// style."` by the same rule that gives a combo the ring of adornment's own
/// words: they are the same feat, so they share the same six letters too.
pub fn combo_word() -> &'static str {
    "COMBO!"
}

pub fn with_pride() -> &'static str {
    "With pride."
}

pub fn with_style() -> &'static str {
    "With style."
}

// ---------------------------------------------------------------------------
// models/src/catalog.rs, models/src/components.rs
// ---------------------------------------------------------------------------

pub fn wizard_now() -> &'static str {
    "You're a wizard now!"
}

pub fn wizard_no_more() -> &'static str {
    "You're no longer that magical."
}

pub fn welcome_new_run() -> &'static str {
    "Welcome to nihilurk! Good luck and have fun!"
}

pub fn welcome_back() -> &'static str {
    "Welcome back to nihilurk! Good luck and have fun!"
}

pub fn you_monster() -> &'static str {
    "You monster."
}

// ---------------------------------------------------------------------------
// models/src/effects.rs - what a worn body is born with (`-am`)
// ---------------------------------------------------------------------------

pub const fn feel_fireproof() -> &'static str {
    "You feel fireproof."
}

pub const fn feel_frostproof() -> &'static str {
    "You feel frostproof."
}

pub const fn feel_undead() -> &'static str {
    "You feel dead inside."
}

pub const fn feel_vorpal_target() -> &'static str {
    "You feel marked for beheading."
}

pub const fn feel_item_user() -> &'static str {
    "You feel handy."
}

pub const fn feel_rusts_armor() -> &'static str {
    "You feel corrosive."
}

pub const fn feel_regenerates() -> &'static str {
    "You feel your wounds knit on their own."
}

pub const fn feel_flies() -> &'static str {
    "You feel light as air."
}

pub const fn feel_batty() -> &'static str {
    "You feel batty."
}

pub const fn feel_binds() -> &'static str {
    "You feel clingy."
}

pub const fn feel_gorgon() -> &'static str {
    "You feel your gaze could turn flesh to stone."
}

pub const fn feel_vampiric() -> &'static str {
    "You feel thirsty for blood."
}

pub const fn feel_venomous() -> &'static str {
    "You feel venomous."
}

pub const fn feel_score_bounty() -> &'static str {
    "You feel worth a fortune."
}

pub const fn feel_splits() -> &'static str {
    "You feel like you could split in two."
}

pub const fn feel_green_blood() -> &'static str {
    "You feel green-blooded."
}

pub const fn feel_freezing() -> &'static str {
    "You feel numbingly cold."
}

pub const fn feel_steals_and_flees() -> &'static str {
    "You feel light-fingered."
}

pub const fn feel_steals_and_vanishes() -> &'static str {
    "You feel sneaky."
}

pub const fn feel_swims() -> &'static str {
    "You feel at home in the water."
}

pub const fn feel_always_tamed() -> &'static str {
    "You feel loyal."
}

pub const fn feel_always_helper() -> &'static str {
    "You feel helpful."
}

pub const fn feel_priority_helper() -> &'static str {
    "You feel like somebody's best friend."
}

pub const fn feel_shapeshift_on_kill() -> &'static str {
    "You feel restless in your own skin."
}

// TODO: placeholder English; needs a human's pass.
pub const fn feel_mirror_on_kill() -> &'static str {
    "You feel like whatever you see."
}

pub const fn feel_faerie_on_death() -> &'static str {
    "You feel a faerie stir inside you."
}

// ---------------------------------------------------------------------------
// engine/src/main.rs
//
// CLI output: prompts, `-help`, and error messages for a misused flag. In
// scope for the same reason a `GameLog` line is - a real person reads these -
// but note that `-content`'s own per-category listing (the actual content
// names, grouped) is deliberately NOT translated here: those names are the
// same ids `NIHILURK_SPAWN` and `-am` match against, not prose.
// ---------------------------------------------------------------------------

pub fn content_header(count: usize) -> String {
    format!("nihilurk content - {count} entries")
}

pub fn content_spawn_hint() -> &'static str {
    "Spawn any of them with: NIHILURK_SPAWN=\"<name>,<name>\" nihilurk"
}

pub fn content_group_header(group: &str, count: usize) -> String {
    format!("\n{group} ({count})")
}

pub fn leaderboard_header(count: usize) -> String {
    format!("nihilurk leaderboard - top {count}")
}

pub fn leaderboard_empty() -> &'static str {
    "No runs recorded yet."
}

pub fn leaderboard_entry(rank: usize, name: &str, outcome: &str, score: i64, when: &str) -> String {
    format!("{rank}. {name} - {outcome} - {score} ({when})")
}

pub fn help_text() -> &'static str {
    "\
nihilurk - terminal roguelike

USAGE
    nihilurk [NAME|SAVE] [OPTIONS]

OPTIONS
    -s SEED          use a reproducible u64 seed
    -c               centre the map on the player
    -ns              do not write a save file
    -nb              disable blood and corpse animation
    -nshake          disable screen shake
    -nobones         skip the bones mechanic (saving and loading a run's own)
    -endless         no Element of Yoord, no way up, no way to win: just floors
    -anim-rate N     set animation pacing multiplier (0.1..=5.0)
    -b BODY          play as nihil (default) or lurk
    -am SPECIES      play as a monster: any bestiary name, e.g. -am dragon
    -content         list names accepted by NIHILURK_SPAWN
    -scores          show the leaderboard and exit, without playing
    -h, -help, --help show this help and exit

POSITIONAL ARGUMENT (first argument only)
    NAME             start a new run with this player name
    SAVE             load an existing save, with or without .sav

ENVIRONMENT
    NIHILURK_SPAWN       comma-separated names to place near the player on every
                     generated floor; use -content to list valid names

EXAMPLES
    nihilurk
    nihilurk bae
    nihilurk -s 1234 -ns
    nihilurk -b lurk
    nihilurk bae -am dragon
    NIHILURK_SPAWN=\"dragon,ring of protection\" nihilurk

SEE ALSO
    man nihilurk          full command, environment, and spawn API reference
    docs/reference/cli-and-env.md
    docs/reference/spawn-api.md
"
}

pub fn no_such_pride_flag(name: &str, known: &str) -> String {
    format!("nihilurk: no flag called '{name}'. Try one of: {known}.")
}

pub fn conflicting_bodies(first_flag: &str, second: &str) -> String {
    format!("nihilurk: {first_flag} and {second} are the same choice. Pick one.")
}

pub fn no_such_body(name: &str) -> String {
    format!("nihilurk: no body called '{name}'. There is nihil, and there is lurk.")
}

pub fn no_such_monster(name: &str) -> String {
    format!("nihilurk: no monster called '{name}'. Try -content for the bestiary.")
}

pub fn stray_positional(stray: &str) -> String {
    format!("nihilurk: '{stray}' is not a flag, and a name has to come first: nihilurk {stray} ...")
}

pub fn clear_data_prompt(player_name: &str) -> String {
    format!(
        "{player_name} has ascended with the Element of Yoord and brought happiness back to the world. \
If you start another journey, the Element will also return to the Dungeon Lord. Do it? ([Y]es/[N]o)"
    )
}

pub fn world_keeps_its_light() -> &'static str {
    "The world keeps its light. Farewell."
}

pub fn body_conflicts_with_load(flag: &str, name: &str) -> String {
    format!("nihilurk: a save already knows what body it is in; drop {flag} {name} to load it.")
}

// TODO: placeholder English; needs a human's pass.
pub fn invalid_save() -> &'static str {
    "Invalid save file for this version, or it is corrupted."
}

pub fn clear_data_not_saved() -> &'static str {
    "Clear data not saved (-ns)."
}

pub fn clear_data_saved(save_name: &str) -> String {
    format!("Clear data saved to '{save_name}'.")
}

pub fn failed_to_save_clear_data(err: &str) -> String {
    format!("Failed to save clear data: {err}")
}

pub fn game_not_saved() -> &'static str {
    "Game not saved (-ns)."
}

pub fn game_saved(save_name: &str) -> String {
    format!("Game saved to '{save_name}'. Resume with: nihilurk {save_name}")
}

pub fn failed_to_save_game(err: &str) -> String {
    format!("Failed to save game: {err}")
}

// ---------------------------------------------------------------------------
// engine/src/view.rs
// ---------------------------------------------------------------------------

pub fn more_prompt() -> &'static str {
    "--MORE-- (Press Space)"
}

pub fn you_die() -> &'static str {
    "You die..."
}

pub fn lose_title() -> &'static str {
    "LOSE"
}

pub fn win_title() -> &'static str {
    "WIN"
}

pub fn score_line(score_text: &str) -> String {
    format!("SCORE {score_text}")
}

pub fn press_any_key_to_depart() -> &'static str {
    "Press any key to depart."
}

// ---------------------------------------------------------------------------
// models/src/pride.rs
// ---------------------------------------------------------------------------

pub fn pride_off_refusal() -> &'static str {
    "ERROR: You cannot ever take our pride."
}

// ---------------------------------------------------------------------------
// models/src/pack.rs
// ---------------------------------------------------------------------------

pub fn pack_title_browse() -> &'static str {
    " INVENTORY "
}
pub fn pack_title_use() -> &'static str {
    " USE WHAT? "
}
pub fn pack_title_throw() -> &'static str {
    " THROW WHAT? "
}
pub fn pack_title_drop() -> &'static str {
    " DROP WHAT? "
}
pub fn pack_title_equip() -> &'static str {
    " EQUIP WHAT? "
}
pub fn pack_title_quaff() -> &'static str {
    " QUAFF WHAT? "
}
pub fn pack_title_read() -> &'static str {
    " READ WHAT? "
}
pub fn pack_title_zap() -> &'static str {
    " ZAP WHAT? "
}
pub fn pack_title_wield() -> &'static str {
    " WIELD WHAT? "
}
pub fn pack_title_wear() -> &'static str {
    " WEAR WHAT? "
}
pub fn pack_title_put_on() -> &'static str {
    " PUT ON WHAT? "
}

pub fn pack_nothing_browse() -> &'static str {
    "You have no items."
}
pub fn pack_nothing_use() -> &'static str {
    "You have nothing to use."
}
pub fn pack_nothing_throw() -> &'static str {
    "You have nothing to throw."
}
pub fn pack_nothing_drop() -> &'static str {
    "You have nothing to drop."
}
pub fn pack_nothing_equip() -> &'static str {
    "You have nothing to equip."
}
pub fn pack_nothing_quaff() -> &'static str {
    "You have nothing to quaff."
}
pub fn pack_nothing_read() -> &'static str {
    "You have nothing to read."
}
pub fn pack_nothing_zap() -> &'static str {
    "You have nothing to zap."
}
pub fn pack_nothing_wield() -> &'static str {
    "You have nothing to wield."
}
pub fn pack_nothing_wear() -> &'static str {
    "You have nothing to wear."
}
pub fn pack_nothing_put_on() -> &'static str {
    "You have nothing to put on."
}

// ---------------------------------------------------------------------------
// models/src/magicmap.rs
// ---------------------------------------------------------------------------

pub fn magicmap_row_by_row() -> &'static str {
    "The dungeon's shape springs into your mind."
}
pub fn magicmap_spiral() -> &'static str {
    "The dungeon unwinds around you like a scroll."
}
pub fn magicmap_explode() -> &'static str {
    "Knowledge of the dungeon bursts outward from where you stand."
}

// ---------------------------------------------------------------------------
// models/src/effects.rs - the EFFECTS table's `ends`/`beware` text.
//
// `const fn`, not `fn`: `EFFECTS` is a `const` (see the table's own doc
// comment on why an id can never be reused as an index), so whatever fills
// its `ends`/`beware` fields has to be const-evaluable too.
// ---------------------------------------------------------------------------

pub const fn beware_aggravating_shriek() -> &'static str {
    "aggravating shriek"
}
pub const fn beware_corrosive_touch() -> &'static str {
    "corrosive touch"
}
pub const fn beware_regeneration() -> &'static str {
    "regeneration"
}
pub const fn beware_erratic_strikes() -> &'static str {
    "erratic strikes"
}
pub const fn beware_binding_bite() -> &'static str {
    "binding bite"
}
pub const fn beware_petrifying_gaze() -> &'static str {
    "petrifying gaze"
}
pub const fn beware_draining_touch() -> &'static str {
    "draining touch"
}
pub const fn beware_venomous_bite() -> &'static str {
    "venomous bite"
}
pub const fn beware_splitting_flesh() -> &'static str {
    "splitting flesh"
}
pub const fn beware_paralysing_touch() -> &'static str {
    "paralysing touch"
}
pub const fn beware_thieving_touch() -> &'static str {
    "thieving touch"
}
pub const fn beware_fireproof() -> &'static str {
    "fire doesn't harm them"
}
pub const fn beware_frostproof() -> &'static str {
    "cold doesn't harm them"
}
pub const fn beware_flying() -> &'static str {
    "flying"
}
pub const fn beware_phasing() -> &'static str {
    "walking through walls"
}
pub const fn beware_ranged_shots() -> &'static str {
    "ranged shots"
}
pub const fn beware_spellcasting() -> &'static str {
    "spellcasting"
}
pub const fn beware_confusing_touch() -> &'static str {
    "confusing touch"
}

pub const fn ends_asleep() -> &'static str {
    "You shake off the drowsiness and come to."
}
pub const fn ends_petrified() -> &'static str {
    "The stone sloughs off you and your flesh is your own again."
}
pub const fn ends_pinned() -> &'static str {
    "You wrench your leg free of the bear trap."
}
pub const fn ends_rooted() -> &'static str {
    "Whatever was holding you lets go."
}
pub const fn ends_clamped() -> &'static str {
    "The jaws locked around your leg finally go slack."
}
// TODO: placeholder English; needs a human's pass.
pub const fn ends_protected() -> &'static str {
    "The hush lifts. You can bleed again."
}

// ---------------------------------------------------------------------------
// models/src/conditions.rs
// ---------------------------------------------------------------------------

pub fn mob_verb_line(name: &str, verb: &str) -> String {
    format!("The {name} {verb}.")
}

pub fn blind_mob_verb() -> &'static str {
    "gropes about, blinded"
}

pub fn paralyzed_mob_verb() -> &'static str {
    "seizes up, paralysed"
}

pub fn blind_player_line() -> &'static str {
    "You can't see a thing!"
}

pub fn paralyse_player_line() -> &'static str {
    "Your limbs seize up. You can barely move!"
}

pub fn paralysis_lost_turn() -> &'static str {
    "Your body will not answer you."
}

// Const: these three rows sit in `AFFLICTIONS`, a `const` table (see
// `effects.rs`'s note on why `ends`/`beware` are `const fn` too).
pub const fn blind_cured_line() -> &'static str {
    "The darkness lifts from your eyes."
}
pub const fn blind_cured_noun() -> &'static str {
    "blindness"
}
pub const fn blind_lifted_adjective() -> &'static str {
    "blind"
}
pub const fn paralyzed_cured_line() -> &'static str {
    "Your limbs are your own again."
}
pub const fn paralyzed_cured_noun() -> &'static str {
    "paralysis"
}
pub const fn paralyzed_lifted_adjective() -> &'static str {
    "paralysed"
}
pub const fn confused_cured_line() -> &'static str {
    "Your head clears."
}
pub const fn confused_cured_noun() -> &'static str {
    "confusion"
}
pub const fn confused_lifted_adjective() -> &'static str {
    "confused"
}

pub const fn sluggish_cured_line() -> &'static str {
    "The lead goes out of your legs."
}
pub const fn sluggish_cured_noun() -> &'static str {
    "sluggishness"
}

pub const fn power_restored_line() -> &'static str {
    "Strength trickles back into your arm."
}
pub const fn power_restored_noun() -> &'static str {
    "weakness"
}

pub fn snaps_out_of(name: &str, noun: &str) -> String {
    format!("The {name} snaps out of {noun}.")
}

// `HOLD_ADJECTIVES`/`FLOOR_BOONS`/`OTHER_CONDITIONS`: also `const` tables.
pub const fn adjective_asleep() -> &'static str {
    "asleep"
}
pub const fn adjective_pinned() -> &'static str {
    "pinned"
}
pub const fn adjective_held() -> &'static str {
    "held"
}
pub const fn adjective_clamped() -> &'static str {
    "clamped"
}
pub const fn form_chimera() -> &'static str {
    "chimera"
}
pub const fn form_typhon() -> &'static str {
    "typhon"
}
pub const fn form_echidna() -> &'static str {
    "echidna"
}
pub fn system_shock_player() -> &'static str {
    "SYSTEM SHOCK! You come apart, and barely come back."
}
pub fn system_shock_mob(name: &str) -> String {
    format!("SYSTEM SHOCK! The {name} unravels!")
}
pub const fn adjective_polymorphed() -> &'static str {
    "polymorphed"
}
pub const fn adjective_warded() -> &'static str {
    "warded"
}
pub const fn adjective_coiled() -> &'static str {
    "coiled"
}
pub const fn adjective_stone() -> &'static str {
    "stone"
}
pub const fn adjective_stealthy() -> &'static str {
    "stealthy"
}
pub const fn adjective_sluggish() -> &'static str {
    "sluggish"
}

pub fn too_many_conditions() -> &'static str {
    "You already have too many conditions."
}

pub fn no_longer(adjective: &str) -> String {
    format!("You are no longer {adjective}.")
}

pub fn already_as_extreme_player(extreme: &str) -> String {
    format!("You are already as {extreme} as you can be.")
}

pub fn already_as_extreme_mob(name: &str, extreme: &str) -> String {
    format!("The {name} is already as {extreme} as they can be.")
}

pub fn extreme_quick() -> &'static str {
    "quick"
}
pub fn extreme_sluggish() -> &'static str {
    "sluggish"
}

pub fn haste_player_line() -> &'static str {
    "The world lurches into slow motion around you."
}
pub fn slow_player_line() -> &'static str {
    "Your limbs turn to lead."
}
pub fn haste_mob_line(name: &str) -> String {
    format!("The {name} blurs into sudden speed!")
}
// TODO: placeholder English; needs a human's pass.
pub fn haste_overflow_player_line() -> &'static str {
    "Your speed overflows, no longer hasted!"
}
// TODO: placeholder English; needs a human's pass.
pub fn haste_overflow_mob_line(name: &str) -> String {
    format!("The {name}'s speed overflows, no longer hasted!")
}
pub fn slow_mob_line(name: &str) -> String {
    format!("The {name} lurches into slow motion.")
}

// ---------------------------------------------------------------------------
// models/src/equipment.rs
// ---------------------------------------------------------------------------

pub fn donned_hand(name: &str) -> String {
    format!("You wield the {name}.")
}
pub fn donned_body_or_finger(name: &str) -> String {
    format!("You put on the {name}.")
}
pub fn doffed_hand(name: &str) -> String {
    format!("You unwield the {name}.")
}
pub fn doffed_body(name: &str) -> String {
    format!("You take off the {name}.")
}
pub fn doffed_finger(name: &str) -> String {
    format!("You remove the {name}.")
}
pub fn stuck_hand(name: &str) -> String {
    format!("You can't - the {name} is welded to your grip!")
}
pub fn stuck_body(name: &str) -> String {
    format!("You can't - the {name} clings to you and won't come off!")
}
pub fn stuck_finger(name: &str) -> String {
    format!("You can't - the {name} is fused to your finger!")
}
pub fn cursed_reveal_hand(name: &str) -> String {
    format!("The {name} welds itself to your grip! It is cursed!")
}
pub fn cursed_reveal_body(name: &str) -> String {
    format!("The {name} clings to your body! It is cursed!")
}
pub fn cursed_reveal_finger(name: &str) -> String {
    format!("The {name} welds to your finger! It is cursed!")
}
pub fn blocked_hand(name: &str) -> String {
    format!("You can't switch weapons! The {name} won't leave your hand.")
}
pub fn blocked_body(name: &str) -> String {
    format!("You can't change armour! The {name} won't come off.")
}
pub fn blocked_finger(name: &str) -> String {
    format!("You can't - the {name} won't leave your finger.")
}

// TODO: placeholder English; needs a human's pass.
pub fn curses_merge(recipient: &str, donor: &str, result: &str) -> String {
    format!("The curses fuse! The {recipient} and the {donor} become the {result}.")
}

// TODO: placeholder English; needs a human's pass.
pub fn curses_merge_break(recipient: &str, donor: &str) -> String {
    format!("The curses clash! The {recipient} and the {donor} shatter.")
}

pub fn armor_shrugs_off_corrosion() -> &'static str {
    "Your armour drinks the corrosion and shrugs it off."
}

pub fn armor_corrodes(name: &str) -> String {
    format!("Your {name} corrodes! It is weaker.")
}

// TODO: placeholder English; needs a human's pass.
pub fn armor_shatters(target_name: &str) -> String {
    format!("The {target_name}'s armour shatters!")
}

// TODO: placeholder English; needs a human's pass.
pub fn cursed_armor_splinters() -> &'static str {
    "Splinters of evil magic fly out of it!"
}

// TODO: placeholder English; needs a human's pass.
pub fn splinter_hits(target_name: &str, damage: i32) -> String {
    format!("A splinter pierces the {target_name} for {damage} damage.")
}

// ---------------------------------------------------------------------------
// models/src/abilities.rs
// ---------------------------------------------------------------------------

// Const: these three sit in `ABILITIES`, a `const` table.
pub const fn flavour_aggravates() -> &'static str {
    "You yip! The whole floor turns your way."
}
pub const fn flavour_regenerates() -> &'static str {
    "The ring on your finger is warm."
}
pub const fn flavour_polymorphitis() -> &'static str {
    "Something on your finger will not sit still."
}

pub const fn flavour_teleportitis() -> &'static str {
    "Something on your finger is pleased with itself."
}

pub fn heavy_stagger_player() -> &'static str {
    "The blow staggers you and you can't gather yourself to answer it!"
}
pub fn heavy_stagger_mob(name: &str) -> String {
    format!("The {name} reels from the blow, staggered!")
}

pub fn chaos_recoil() -> &'static str {
    "The edge of chaos bites you!"
}

// TODO: placeholder English; needs a human's pass.
pub fn spike_prick_player() -> &'static str {
    "Spikes prick you as you strike!"
}
// TODO: placeholder English; needs a human's pass.
pub fn spike_prick_mob(name: &str) -> String {
    format!("The {name} is pricked by spikes for striking!")
}

pub fn venom_resisted_player() -> &'static str {
    "The venom burns, but your strength holds firm."
}
pub fn venom_resisted_mob(name: &str) -> String {
    format!("The venom burns, but the {name}'s strength holds firm.")
}
pub fn venom_took_player() -> &'static str {
    "Venom! Your strength ebbs away."
}
pub fn venom_took_mob(name: &str) -> String {
    format!("Venom courses through the {name} - its strength ebbs away.")
}

pub fn vampiric_drain_player() -> &'static str {
    "The vampiric kiss drains your life!"
}
pub fn vampiric_drain_mob(name: &str) -> String {
    format!("The vampiric kiss drains the {name}'s life!")
}

pub fn bind_victim_player(name: &str) -> String {
    format!(
        "The {name} clamps their jaws around your leg - you can't take a step, but your arms are free!"
    )
}
pub fn bind_victim_mob(attacker_name: &str, target_name: &str) -> String {
    format!("The {attacker_name} clamps their jaws around the {target_name}!")
}

pub fn medusa_gaze_line() -> &'static str {
    "Your eyes meet the medusa's and your flesh turns to cold stone!"
}

// ---------------------------------------------------------------------------
// models/src/visibility.rs
// ---------------------------------------------------------------------------

pub fn spotted_line(phrase: &str, worn: &str) -> String {
    format!("You spotted {phrase}{worn}.")
}

pub fn trap_spotted(article: &str, label: &str) -> String {
    format!("You spot {article} {label}.")
}

// ---------------------------------------------------------------------------
// models/src/helpers.rs
// ---------------------------------------------------------------------------

pub fn ward_turns_aside(name: &str) -> String {
    format!("The {name}'s ward turns the magic aside.")
}

// TODO: placeholder English; needs a human's pass.
pub fn protection_turns_aside(name: &str) -> String {
    format!("The hush swallows the harm meant for {name}.")
}

pub fn unharmed_by(name: &str, element_noun: &str) -> String {
    format!("The {name} is unharmed by the {element_noun}.")
}

// TODO: placeholder English; needs a human's pass.
pub fn armor_takes_the_blow(name: &str) -> String {
    format!("Your {name} is splintered and cracked, and takes the blow meant for you!")
}

pub fn badly_wounded() -> &'static str {
    "You are badly wounded!"
}

// ---------------------------------------------------------------------------
// models/src/components.rs - Element::noun()
// ---------------------------------------------------------------------------

pub fn element_fire_noun() -> &'static str {
    "flames"
}
pub fn element_cold_noun() -> &'static str {
    "cold"
}
pub fn element_drain_noun() -> &'static str {
    "evil magic"
}

// ---------------------------------------------------------------------------
// models/src/saveload.rs
// ---------------------------------------------------------------------------

pub fn retired_enchantment_singular() -> &'static str {
    "One enchantment in this save is unknown to this build, and is gone."
}

pub fn retired_enchantment_plural(n: usize) -> String {
    format!("{n} enchantments in this save are unknown to this build, and are gone.")
}

// ---------------------------------------------------------------------------
// models/src/map/levels.rs
// ---------------------------------------------------------------------------

pub fn element_seeks_the_sun() -> &'static str {
    "The Element of Yoord seeks the sun; it will not let you descend."
}

pub fn cannot_go_down() -> &'static str {
    "You cannot go down from here."
}

pub fn dungeon_lord_prevents_up() -> &'static str {
    "The Dungeon Lord's power prevents you from going upstairs."
}

pub fn cannot_go_up() -> &'static str {
    "You cannot go up from here."
}

pub fn climb_last_stair() -> &'static str {
    "You climb the last stair into the final sky, the Element of Yoord blazing in your hands."
}

pub fn portal_down(depth: u8) -> String {
    format!("The Dungeon Lord opens a portal beneath your feet! You fall downward. (Depth {depth})")
}

pub fn portal_up(depth: u8) -> String {
    format!(
        "The Element of Yoord flares and rips a portal above your head! You rise upward. (Depth {depth})"
    )
}

pub fn trapdoor_arrival(depth: u8) -> String {
    format!("You crash down onto the floor below in a shower of dust. (Depth {depth})")
}

pub fn potion_arrival(depth: u8) -> String {
    format!("The stone above you thins to nothing and you drift up through it. (Depth {depth})")
}

pub fn lost_in_the_fall(name: &str) -> String {
    format!("Your {name} tumbles from your pack and is lost in the fall!")
}

pub fn descend_stairs(depth: u8) -> String {
    format!("You descend the stairs. (Depth {depth})")
}

pub fn climb_stairs(depth: u8) -> String {
    format!("You climb the stairs. (Depth {depth})")
}

pub fn bones_ghost_arrives(name: &str) -> String {
    format!("A chill runs through you. {name} remembers what you did, and is furious.")
}

pub fn bones_ghost_arrives_self() -> String {
    "A chill runs through you. Someone who was once you remembers what you did, and is furious."
        .to_string()
}

pub fn element_wont_let_you_land() -> &'static str {
    "The Element of Yoord strains toward the sun - but the last stair you must climb yourself."
}

pub fn portal_no_deeper_floor() -> &'static str {
    "The Dungeon Lord claws at the floor, but there is nowhere deeper to cast you."
}

// ---------------------------------------------------------------------------
// models/src/map.rs
// ---------------------------------------------------------------------------

pub fn dragon_hoard_enter() -> &'static str {
    "You see gleaming gold, but the smell of sulfur assaults you. Here be dragons!"
}

pub fn treasure_hive_enter() -> &'static str {
    "The bees. Not the bees."
}

pub fn red_room_enter() -> &'static str {
    "Treasure gleams, but you can only pick one."
}

// TODO: placeholder line; needs real wording.
// TODO: placeholder English; needs a human's pass.
pub fn monster_zoo_enter() -> &'static str {
    "Every tile in here is looking at you."
}

// TODO: placeholder line; needs real wording.
// TODO: placeholder English; needs a human's pass.
pub fn dark_room_enter() -> &'static str {
    "It is very dark in here."
}

pub fn labyrinth_arrival() -> &'static str {
    "Welcome to a world of darkness."
}

pub fn vault_arrival() -> &'static str {
    "A treasure vault the Dungeon Lord guards jealously!"
}

pub fn bee_world_arrival() -> &'static str {
    "Welcome to the bee world."
}

/// One of these follows [`bee_world_arrival`], picked at random.
pub fn bee_world_quotes() -> [&'static str; 7] {
    [
        "\"Ya like jazz?\"",
        "\"Yellow, black, yellow, black. Oh! Black and yellow!\"",
        "\"According to all known laws of aviation, there is no way an apis should be able to fly.\"",
        "\"Thinking bee! Thinking bee!\"",
        "\"And I have no pants.\"",
        "\"I'm talking to a human! And the human is talking to me!\"",
        "\"Ooh, a little breakfast to go!\"",
    ]
}

// ---------------------------------------------------------------------------
// models/src/items/pickups.rs
// ---------------------------------------------------------------------------

pub fn hidden_item_found() -> &'static str {
    "Hey! There's something here!"
}

pub fn red_room_claimed() -> &'static str {
    "The rest crumbles to dust the moment your hand closes."
}

pub fn element_bursts_thief(thief: &str) -> String {
    format!(
        "The {thief}'s hand closes on the Element of Yoord, and the {thief} bursts apart in a shower of gore!"
    )
}

pub fn element_surfaces() -> &'static str {
    "The Element of Yoord will not drown. It leaps from the water into your hands, and everything else you carried burns away in a flash."
}

pub fn sinks_with_a_splash(what: &str) -> String {
    format!("Splash! The depths take {what}.")
}

pub fn take_element_of_yoord() -> &'static str {
    "You take the Element of Yoord. \"The element of Yoord seeks the sun.\""
}

pub fn pick_up(taken: &str) -> String {
    format!("You pick up {taken}.")
}

pub fn pick_up_pickup(name: &str, line: &str) -> String {
    format!("You pick up the {name}. {line}")
}

pub fn coin_gives_itself_up(name: &str, line: &str) -> String {
    format!("The {name} gives itself up to you. {line}")
}

pub fn coin_ledger() -> &'static str {
    "It goes straight into the ledger."
}

pub fn heal_line(healed: i32) -> String {
    format!("Warmth spreads through you. ({healed} HP)")
}

pub fn refill_magic_line(gained: u8) -> String {
    format!("Your head cools. ({gained} Ma)")
}

pub fn cleanse_one() -> &'static str {
    "The taste of it clears one thing."
}

pub fn cleanse_many(n: i32) -> String {
    format!("The taste of it clears {n} things.")
}

pub fn learn_spell_full() -> &'static str {
    "Something ancient stirs in your mind and finds nowhere to sit."
}

pub fn learn_spell_all_known() -> &'static str {
    "Something ancient stirs in your mind and finds nothing new to teach."
}

pub fn learn_spell_line(name: &str) -> String {
    format!("Something ancient and violent settles into your mind. You have learned {name}!")
}

pub fn restore_strength_line(given: i32) -> String {
    format!("Your strength returns! ({given} Pow.)")
}

pub fn promise_platinum_offer() -> &'static str {
    "It does not tarnish. Neither, for now, will you. (PLAT)"
}
pub fn promise_forge_offer() -> &'static str {
    "It is still warm. Something is being made. (FORG)"
}
pub fn promise_platinum_broken() -> &'static str {
    "The platinum dulls. So much for perfection."
}
pub fn promise_forge_broken() -> &'static str {
    "The forge goes cold."
}

pub fn pay_platinum_power() -> &'static str {
    "Untarnished. The platinum goes into your arm. (Pow. +1)"
}
pub fn pay_platinum_armor() -> &'static str {
    "Untarnished. The platinum goes into your hide. (Arm. +1)"
}
pub fn pay_forge_collects() -> &'static str {
    "The forge collects. By its magic, something of yours shall be finished properly."
}
pub fn pay_forge_nothing_worth() -> &'static str {
    "...but you are carrying nothing worth finishing."
}

// ---------------------------------------------------------------------------
// models/src/items/potions.rs
// ---------------------------------------------------------------------------

pub fn potion_healing_player() -> &'static str {
    "You feel refreshed as your wounds mend!"
}
pub fn potion_healing_mob() -> &'static str {
    "glows eerily, wounds closing"
}
pub fn potion_extra_healing_player() -> &'static str {
    "You have never felt better than this!"
}

pub fn potion_confusion_player() -> &'static str {
    "The world is spinning! you are confused!"
}
pub fn potion_confusion_mob() -> &'static str {
    "reels, eyes swimming"
}

pub fn potion_gain_strength_player() -> &'static str {
    "You feel stronger. What bulging muscles!"
}
pub fn potion_gain_strength_mob() -> &'static str {
    "swells with muscle"
}

pub fn potion_gain_magic_player() -> &'static str {
    "Your head clears and then some - the power is all there."
}
pub fn potion_gain_magic_mob() -> &'static str {
    "hums with borrowed power"
}

pub fn potion_poison_player() -> &'static str {
    "You feel very sick now - the strength drains out of you."
}
pub fn potion_poison_mob() -> &'static str {
    "retches, their limbs going slack"
}

pub fn potion_restore_strength_noop_player() -> &'static str {
    "You feel warm all over."
}
pub fn potion_restore_strength_noop_mob() -> &'static str {
    "shivers"
}
pub fn potion_restore_strength_player() -> &'static str {
    "Your old strength comes surging back into your body."
}
pub fn potion_restore_strength_mob() -> &'static str {
    "straightens, their strength returning"
}

pub fn potion_see_invisible_player() -> &'static str {
    "Your eyes sting, and the air fills with things that were never not there."
}
pub fn potion_see_invisible_mob() -> &'static str {
    "eyes gleam, tracking something unseen"
}

pub fn detect_monsters_none() -> &'static str {
    "You listen hard, and hear nothing at all moving on this floor."
}
pub fn detect_monsters_some() -> &'static str {
    "You feel the floor's inhabitants shifting in the dark."
}

pub fn detect_magic_none() -> &'static str {
    "You reach for the hum of magic, and this floor holds none."
}
pub fn detect_magic_some() -> &'static str {
    "Magic hums up through the floor, and you know where every piece of it lies."
}

pub fn distant_laughter() -> &'static str {
    "You hear distant laughter."
}

pub fn raise_level_win() -> &'static str {
    "The potion hauls you up through stone and root and out into the open sky. You are IN RAPTURE."
}

pub fn potion_fruit_juice() -> &'static str {
    "Cold, sweet and thick. Yummy!"
}
pub fn potion_water() -> &'static str {
    "It is water. Just water."
}
pub fn potion_flavour_mob() -> &'static str {
    "smacks their lips"
}
pub fn potion_adjustment_player() -> &'static str {
    "A wave of calm settles over you. Whatever pull you felt is gone."
}

// ---------------------------------------------------------------------------
// models/src/items/throwing.rs
// ---------------------------------------------------------------------------

pub fn thrown_wand_confetti(seen_name: &str) -> String {
    format!(
        "The {seen_name} bursts in a shower of colourful confetti. That's it. That's the whole spell."
    )
}

pub fn thrown_wand_shatters(seen_name: &str, charges: i32) -> String {
    format!("The {seen_name} shatters, and {charges} charges' worth of magic gets out at once!")
}

pub fn very_clever() -> &'static str {
    "Very clever."
}

pub fn you_fire(phrase: &str) -> String {
    format!("You fire {phrase}.")
}
pub fn you_throw(seen_name: &str) -> String {
    format!("You throw the {seen_name}.")
}
pub fn mob_fires(thrower: &str, phrase: &str) -> String {
    format!("The {thrower} fires {phrase}.")
}
pub fn mob_throws(thrower: &str, seen_name: &str) -> String {
    format!("The {thrower} throws the {seen_name}.")
}
pub fn doorway_goes_inert() -> &'static str {
    "The doorway cracks and goes grey and inert."
}

pub fn scroll_read_aloud(who: &str, seen_name: &str) -> String {
    format!("The {who} unrolls the {seen_name} and reads it aloud!")
}

pub fn wand_clatters_unspent(seen_name: &str) -> String {
    format!("The {seen_name} clatters to the floor, its magic still bottled up.")
}

// TODO: placeholder English; needs a human's pass.
pub fn boomerang_returns(seen_name: &str) -> String {
    format!("The {seen_name} returns to you.")
}

pub fn picked_up_thrown_verb_hand() -> &'static str {
    "snatches it up and wields it"
}
pub fn picked_up_thrown_verb_body() -> &'static str {
    "pulls it on"
}
pub fn picked_up_thrown_verb_other() -> &'static str {
    "slips it on"
}
// TODO: placeholder English; needs a human's pass.
pub fn armor_binds_thrown(victim_name: &str, armor: &str) -> String {
    format!("The {victim_name} is somehow bound by the {armor}! But they can still attack!")
}

pub fn picks_up_thrown(victim_name: &str, verb: &str) -> String {
    format!("The {victim_name} {verb}!")
}

pub fn monster_shot_wild(shooter_name: &str, noun: &str, target_label: &str) -> String {
    format!("The {shooter_name} looses a wild {noun} - it goes nowhere near {target_label}.")
}

pub fn monster_shot_hit(
    shooter_name: &str,
    article: &str,
    noun: &str,
    target_label: &str,
    damage: i32,
) -> String {
    format!("The {shooter_name} looses {article} {noun} at {target_label} for {damage} damage!")
}

pub fn potion_shatters_floor(seen_name: &str) -> String {
    format!("The {seen_name} shatters on the floor.")
}

pub fn throw_bounces_off(seen_name: &str, hit_name: &str) -> String {
    format!("The {seen_name} bounces off the {hit_name}.")
}

pub fn throw_glances_off(seen_name: &str, hit_name: &str) -> String {
    format!("The {seen_name} glances off the {hit_name}.")
}

pub fn throw_hits(seen_name: &str, hit_name: &str, damage: i32) -> String {
    format!("The {seen_name} hits the {hit_name} for {damage} damage.")
}

// ---------------------------------------------------------------------------
// models/src/items/spells.rs
// ---------------------------------------------------------------------------

pub fn no_magic_for_that() -> &'static str {
    "You don't have the magic for that."
}

pub fn you_cast(name: &str) -> String {
    format!("You cast {name}!")
}

pub fn sting_misses() -> &'static str {
    "The dart of venom finds nothing to bite."
}
pub fn sting_glances(name: &str) -> String {
    format!("The sting glances off the {name}.")
}
pub fn sting_hits(name: &str, damage: i32) -> String {
    format!("A venomous sting pricks the {name} for {damage} damage!")
}

pub fn thunderbolt_misses() -> &'static str {
    "Thunder cracks over empty stone."
}
pub fn thunderbolt_hits_you(damage: i32) -> String {
    format!("A bolt of thunder slams into you for {damage} damage!")
}
pub fn thunderbolt_hits(name: &str, damage: i32) -> String {
    format!("A bolt of thunder slams into the {name} for {damage} damage!")
}

pub fn cure_self_nothing_to_cure() -> &'static str {
    "There's nothing wrong with you to cure."
}

pub fn bide_coil() -> &'static str {
    "You coil, gathering strength for the blow to come."
}

pub fn breathe_fire_player() -> &'static str {
    "A gout of fl-FIREBALL!"
}
pub fn breathe_fire_mob(name: &str) -> String {
    format!("A gout of flame erupts from the {name}!")
}

pub fn force_lance_cast() -> &'static str {
    "An invisible lance hammers down the line!"
}
pub fn force_lance_hits(name: &str, damage: i32) -> String {
    format!("The lance forces the {name} for {damage} damage!")
}

pub fn setup_planted() -> &'static str {
    "You plant arrow traps at your flanks, springs cocked in plain sight."
}
pub fn setup_no_room() -> &'static str {
    "There's no room at your flanks for a trap."
}

pub fn lux_cast() -> &'static str {
    "You hurl a shard of pure light!"
}

pub fn circle_of_death_nothing() -> &'static str {
    "Ashen light gathers around you and finds nothing at all to feed on."
}
pub fn circle_of_death_cast() -> &'static str {
    "Ashen light rises off the floor. Unnecessary flames roar through the room, and \
         everything they touch turns grey."
}
pub fn circle_of_death_drain(drained: i32) -> String {
    format!("You drain {drained} life from the circle.")
}

pub fn magic_ward_cast() -> &'static str {
    "You ward yourself. Only your own magic can touch you now."
}

pub fn heal_self_line(healed: i32) -> String {
    format!("Warmth floods through you, and your wounds close. (+{healed} HP)")
}
pub fn heal_self_full() -> &'static str {
    "You are already at full strength."
}

pub fn meteor_strike_cast() -> &'static str {
    "You hurl fire at the sky. It does not come back down where you'd expect."
}
pub fn meteor_screams_down() -> &'static str {
    "A meteor screams down!"
}
pub fn sky_tears_open_again() -> &'static str {
    "The sky tears open again!"
}

pub fn frost_nova_cast() -> &'static str {
    "A CYAN STAR erupts around you - Ice! Glitter! And entirely too much of both."
}

pub fn haste_self_gathers() -> &'static str {
    "Power gathers. Power gathers more. It's time to go further beyond."
}
pub fn haste_self_the_fast() -> &'static str {
    "You are not quick. You are not swift. You are THE FAST!"
}

// ---------------------------------------------------------------------------
// models/src/items/scrolls.rs
// ---------------------------------------------------------------------------

pub fn already_recognise_everything() -> &'static str {
    "You already recognise everything in your pack."
}
pub fn identify_everything() -> &'static str {
    "The scroll identifies everything in your pack!"
}

pub fn remove_curse_freed() -> &'static str {
    "You feel as though somebody is watching over you. Your cursed gear crumbles away."
}
pub fn remove_curse_nothing() -> &'static str {
    "You feel as though somebody is watching over you."
}

pub fn scare_monster_some() -> &'static str {
    "The parchment flares with the pathos of fear!"
}
pub fn scare_monster_none() -> &'static str {
    "The parchment radiates a menacing aura, but nothing is here to feel it."
}

pub fn blank_paper() -> &'static str {
    "The scroll is blank. Someone got the last laugh."
}

pub fn amnesia_poof() -> &'static str {
    "1... 2... Poof!"
}
pub fn amnesia_nothing_to_forget() -> &'static str {
    "There was nothing there to forget."
}
pub fn amnesia_forgotten(name: &str) -> String {
    format!("You've forgotten how to use {name}!")
}
pub fn amnesia_dungeon_slips_away() -> &'static str {
    "The dungeon around you slips away like a half-remembered dream."
}

pub fn teleport_scroll_blonk() -> &'static str {
    "BLONK! You are whisked away!"
}

pub fn aggravate_scroll_shriek() -> &'static str {
    "A shrill shriek rips through the dungeon. Everything on this floor heard it and knows your address."
}

pub fn create_monster_nowhere() -> &'static str {
    "The air curdles - then settles. Whatever was coming thought better of it."
}
pub fn create_monster_line(article: &str, name: &str) -> String {
    format!("The air curdles into {article} {name}, created from nothing!")
}

pub fn charm_fizzles() -> &'static str {
    "The wand sparkles, but there is nothing here to charm."
}
pub fn charm_self() -> &'static str {
    "You feel the most adorable you've ever felt."
}
pub fn charm_target_line(name: &str) -> String {
    format!("The {name} gazes at you with adoring eyes!")
}
pub fn charm_room_some() -> &'static str {
    "Every eye in the room turns to you, adoring."
}
pub fn charm_room_none() -> &'static str {
    "The scroll glows with charm, but nothing here to catch it."
}

pub fn vorpalize_fizzles() -> &'static str {
    "The scroll gutters out, failing to brand a weapon."
}
pub fn vorpalize_crumbles(wname: &str) -> String {
    format!("The {wname} screams in pain and crumbles to dust.")
}
pub fn vorpalize_branded(wname: &str, bane: &str) -> String {
    format!("The {wname} sings with a razor light, an omen of death to any {bane}.")
}

pub fn enchant_sparks(name: &str) -> String {
    format!("The {name} throws off a shower of orange sparks!")
}
pub fn enchant_curse_burns(name: &str) -> String {
    format!("The curse on the {name} burns away with them.")
}
pub fn enchant_missing_armor() -> &'static str {
    "The sparks flare over bare skin and die. You are wearing no armour."
}
pub fn enchant_missing_weapon() -> &'static str {
    "The sparks flare over an empty hand and die."
}

pub fn confusing_touch_fresh_player() -> &'static str {
    "Your hands begin to glow with a violet light! Prepare to be confusing!"
}
pub fn confusing_touch_fresh_mob() -> &'static str {
    "flexes their claws, and a violet light crawls over them"
}
pub fn confusing_touch_deeper_player() -> &'static str {
    "The violet light on your hands deepens to no real effect."
}
pub fn confusing_touch_deeper_mob() -> &'static str {
    "shakes out their glowing claws"
}
pub fn confusing_touch_discharge_player() -> &'static str {
    "The violet light bursts against you - the room tilts!"
}
pub fn confusing_touch_discharge_mob() -> &'static str {
    "staggers as the violet light bursts over it"
}

pub fn hold_monster_none() -> &'static str {
    "The words land like iron - on nothing at all."
}
pub fn hold_monster_some() -> &'static str {
    "The words land like iron. Every creature in sight is rooted where it stands."
}

pub fn sleep_scroll_none() -> &'static str {
    "A wave of drowsiness rolls out over an empty room."
}
pub fn sleep_scroll_some() -> &'static str {
    "A wave of drowsiness rolls out, and everything in sight goes down with it."
}
pub fn sleep_backfire_player() -> &'static str {
    "The words slur and thicken in your own mouth. The floor comes up to meet you..."
}
pub fn sleep_backfire_mob() -> &'static str {
    "reads itself into a heap on the floor"
}

pub fn food_detection_not_player() -> &'static str {
    "The words mean nothing to it."
}
pub fn food_detection_none() -> &'static str {
    "You cast about for anything plain and useful, and this floor is bare."
}
pub fn food_detection_some() -> &'static str {
    "The floor gives up its odds and ends: you know where every plain thing lies."
}

// ---------------------------------------------------------------------------
// engine/src/update.rs
// ---------------------------------------------------------------------------

pub fn stumble_foolishly() -> &'static str {
    "You stumble foolishly."
}

pub fn pack_full() -> &'static str {
    "Your pack is full."
}

pub fn strain_against_rooted() -> &'static str {
    "You strain against whatever is holding you, and go nowhere."
}

pub fn too_confused_right_now() -> &'static str {
    "You are too confused for that right now."
}

pub fn too_injured_now() -> &'static str {
    "You are too injured for that now."
}

pub fn nothing_to_fight() -> &'static str {
    "There is nothing to fight."
}

pub fn cant_reach_it() -> &'static str {
    "You can't reach it from here."
}

pub fn out_of_ammo() -> &'static str {
    "You're out of ammo."
}

pub fn out_of_range() -> &'static str {
    "Out of range."
}

pub fn no_clear_shot() -> &'static str {
    "No clear shot."
}

pub fn well_played() -> &'static str {
    "Well played."
}

pub fn great_idea_but_no() -> &'static str {
    "Great idea! But no."
}

pub fn you_see(phrase: &str, worn: &str) -> String {
    format!("You see {phrase}{worn}.")
}

pub fn beware(phrases: &[&str]) -> String {
    format!("Beware: {}.", phrases.join("; "))
}

pub fn you_drop(name: &str) -> String {
    format!("You drop the {name}.")
}

pub fn not_while_monster_in_sight() -> &'static str {
    "Not while a creature is in sight."
}

// TODO: placeholder English; needs a human's pass.
pub fn charge() -> &'static str {
    "CHARGE!"
}

pub fn cant_run_that_way() -> &'static str {
    "You can't run that way."
}

pub fn no_spell_there() -> &'static str {
    "You don't have a spell there."
}

pub fn no_spells() -> &'static str {
    "You have no spells."
}

pub fn toggle_on() -> &'static str {
    "ON"
}
pub fn toggle_off() -> &'static str {
    "OFF"
}
pub fn auto_pickup_state(state: &str) -> String {
    format!("Pick-up on auto-explore {state}.")
}

pub fn not_wielding_launcher() -> &'static str {
    "You aren't wielding a launcher."
}

pub fn no_ammo_to_fire(noun: &str) -> String {
    format!("You have no {noun} to fire.")
}

pub fn not_wielding_reach_weapon() -> &'static str {
    "You aren't wielding a reach weapon."
}

pub fn nothing_left_to_explore() -> &'static str {
    "There is nothing left to explore."
}

pub fn dir_up() -> &'static str {
    "up"
}
pub fn dir_down() -> &'static str {
    "down"
}

pub fn cannot_go_direction(dir: &str) -> String {
    format!("You cannot go {dir} from here.")
}

pub fn cant_find_path_to_stairs(dir: &str) -> String {
    format!("You can't find a path to the {dir}-stairs.")
}

pub fn travel_interrupted() -> &'static str {
    "Travel interrupted."
}
pub fn auto_explore_interrupted() -> &'static str {
    "Auto-explore interrupted."
}

pub fn arrive_at_staircase() -> &'static str {
    "You arrive at the staircase."
}
pub fn you_stop() -> &'static str {
    "You stop."
}

pub fn monster_nearby() -> &'static str {
    "There is a monster nearby."
}

pub fn cant_find_path_there() -> &'static str {
    "You can't find a path there."
}
pub fn explored_everywhere() -> &'static str {
    "You have explored everywhere you can."
}

pub fn already_there() -> &'static str {
    "You are already there."
}

// ---------------------------------------------------------------------------
// models/src/items.rs
// ---------------------------------------------------------------------------

pub fn cant_use_right_now(name: &str) -> String {
    format!("You can't use {name} right now.")
}

pub fn you_zap(seen_name: &str) -> String {
    format!("You zap the {seen_name}.")
}

pub fn wand_crumbles(seen_name: &str) -> String {
    format!("The {seen_name} crumbles to dust!")
}
pub fn you_drink(seen_name: &str) -> String {
    format!("You drink the {seen_name}.")
}
pub fn you_read(seen_name: &str) -> String {
    format!("You read the {seen_name}.")
}
pub fn ring_shivers_apart(seen_name: &str) -> String {
    format!("The {seen_name} shivers apart into a thousand glittering motes.")
}
pub fn item_turns_to_dust() -> &'static str {
    "The item turns to dust!"
}

// Runes. The effects of Chaos and Ice speak through the spells they cast.

// TODO: placeholder English; needs a human's pass.
pub fn you_invoke(seen_name: &str) -> String {
    format!("You invoke the {seen_name}.")
}
// TODO: placeholder English; needs a human's pass.
pub const fn rune_inert_suffix() -> &'static str {
    "inert"
}
// TODO: placeholder English; needs a human's pass.
pub const fn rune_is_inert() -> &'static str {
    "The rune is dark. It wakes when you take the stairs."
}
// TODO: placeholder English; needs a human's pass.
pub const fn rune_blank_nothing() -> &'static str {
    "The blank rune does nothing, and means it."
}
// TODO: placeholder English; needs a human's pass.
pub const fn rune_recharged() -> &'static str {
    "Your wands drink in the rune's light."
}
// TODO: placeholder English; needs a human's pass.
pub const fn rune_recharge_nothing() -> &'static str {
    "The rune hums. You carry no wand to charge."
}
// TODO: placeholder English; needs a human's pass.
pub const fn rune_justice_marks() -> &'static str {
    "The rune burns a verdict into everything that sees you. When they fall, they burst."
}
// TODO: placeholder English; needs a human's pass.
pub const fn rune_justice_nobody() -> &'static str {
    "The rune finds nobody to judge."
}
// TODO: placeholder English; needs a human's pass.
pub fn rune_protection_cast(turns: u32) -> String {
    format!("A hush falls over you. Nothing can hurt you for {turns} turns.")
}

// ---------------------------------------------------------------------------
// models/src/combat.rs
// ---------------------------------------------------------------------------

pub fn killer_unknown() -> &'static str {
    "Killer unknown"
}

pub fn mob_dies(name: &str) -> String {
    format!("The {name} dies.")
}

// ---------------------------------------------------------------------------
// Helpers (models/src/companion.rs)
// ---------------------------------------------------------------------------

/// Use, from the pack, on a treat or a piece of ammunition.
pub fn for_throwing(name: &str) -> String {
    format!("The {name} is for throwing.")
}

pub fn becomes_helper(name: &str) -> String {
    format!("The {name} is now your boon companion!")
}

pub fn refuses_treat(name: &str, treat: &str) -> String {
    format!("The {name} gobbles the {treat} and stays hostile.")
}

pub fn old_helper_explodes(name: &str) -> String {
    format!("The {name} EXPLODES!")
}

pub fn so_much_for_loyalty() -> &'static str {
    "So much for loyalty these days."
}

pub fn helper_dies(name: &str) -> String {
    format!("Your boon companion, the {name}, is gone!")
}

pub fn helper_left_behind(name: &str) -> String {
    format!("The {name} could not follow you.")
}

pub fn gear_clatters_to_floor(name: &str) -> String {
    format!("The {name} clatters to the floor.")
}

pub fn lunge_hit(target_name: &str, damage: i32) -> String {
    format!(
        "You lunge, blade flashing past every guard, and skewer the {target_name} for {damage} damage!"
    )
}

/// [`lunge_hit`], for the lurk: a wolf closing the last stride of a run and
/// finishing with its teeth rather than a blade.
pub fn lunge_hit_lurk(target_name: &str, damage: i32) -> String {
    format!("You lunge, jaws snapping shut, and tear into the {target_name} for {damage} damage!")
}

pub fn you_have_slain(target_name: &str) -> String {
    format!("You have slain the {target_name}!")
}

pub fn strike_at_nothing() -> &'static str {
    "You strike at nothing but air."
}

pub fn slain_by(attacker_name: &str) -> String {
    format!("Slain by the {attacker_name}")
}

pub fn excellent_hit(target_name: &str, damage: i32) -> String {
    format!("You score an excellent hit on the {target_name} for {damage} damage!")
}
pub fn glancing_blow(target_name: &str) -> String {
    format!("You deal a glancing blow to the {target_name}.")
}
pub fn plain_hit(target_name: &str, damage: i32) -> String {
    format!("You hit the {target_name} for {damage} damage.")
}
pub fn garrote_kill(target_name: &str) -> String {
    format!("You choke the life out of the helpless {target_name}! Atrocious!")
}
pub fn vorpal_kill(target_name: &str) -> String {
    format!("Snicker-snack! The blade shears clean through the {target_name}!")
}

/// Bones-ghost-only variants of the five lines above plus `you_have_slain`,
/// for the one case those can't express in every language at once: the
/// target is a bones ghost sharing the *player's own* name - see
/// `crate::combat::report_player_hit`. Kept as dedicated English-only
/// strings rather than threading a "yourself" label through the shared
/// functions, which would have broken `pt`/`es`'s own grammar (gendered
/// articles, contractions) for every ordinary target.
pub fn excellent_hit_self(damage: i32) -> String {
    format!("You score an excellent hit on yourself for {damage} damage!")
}
pub fn glancing_blow_self() -> &'static str {
    "You deal a glancing blow to yourself."
}
pub fn plain_hit_self(damage: i32) -> String {
    format!("You hit yourself for {damage} damage.")
}
pub fn garrote_kill_self() -> &'static str {
    "You choke the life out of your helpless self! Atrocious!"
}
pub fn vorpal_kill_self() -> &'static str {
    "Snicker-snack! The blade shears clean through yourself!"
}
pub fn you_have_slain_self() -> &'static str {
    "You have slain yourself!"
}

/// The reverse direction: a bones ghost sharing the player's own name,
/// attacking the real player. `mob_hits`/`mob_misses`/`mob_strikes_you_down`
/// are written for a third-person subject ("The orc hits you"), which breaks
/// the moment the subject is also "you" - see
/// `crate::combat::report_ghost_self_hit`.
pub fn ghost_self_misses() -> &'static str {
    "You miss yourself."
}
pub fn ghost_self_hits(damage: i32) -> String {
    format!("You hit yourself for {damage} damage.")
}
pub fn ghost_self_strikes_you_down() -> &'static str {
    "You strike yourself down..."
}

pub fn mob_misses(atk: &str, target_label: &str) -> String {
    format!("{atk} misses {target_label}.")
}
pub fn mob_hits(atk: &str, target_label: &str, damage: i32) -> String {
    format!("{atk} hits {target_label} for {damage} damage.")
}
pub fn mob_strikes_you_down(atk: &str) -> String {
    format!("{atk} strikes you down...")
}
pub fn mob_kills(atk: &str, target_name: &str) -> String {
    format!("{atk} kills the {target_name}!")
}

pub fn blown_up_by(article: &str, label: &str) -> String {
    format!("Blown up by {article} {label}")
}

// ---------------------------------------------------------------------------
// engine/src/view.rs - HUD badges, prompts, menu titles
// ---------------------------------------------------------------------------

pub fn badge_stone() -> &'static str {
    "STONE"
}
pub fn badge_asleep() -> &'static str {
    "ASLEEP"
}
pub fn badge_held() -> &'static str {
    "HELD"
}
pub fn badge_ascending() -> &'static str {
    "ASCENDING"
}
pub fn badge_traveling() -> &'static str {
    "TRAVELING"
}
pub fn badge_exploring() -> &'static str {
    "EXPLORING"
}
pub fn badge_travel_query() -> &'static str {
    "TRAVEL?"
}

pub fn depth_label() -> &'static str {
    "DEPTH"
}

pub fn time_label() -> &'static str {
    "TIME"
}

pub fn travel_cursor_prompt() -> &'static str {
    "Move where?"
}
pub fn travel_cursor_hint() -> &'static str {
    "[hjkl/arrows move · Enter travel · Esc/x cancel]"
}

pub fn quit_question() -> &'static str {
    "Really quit?"
}
pub fn quit_answers() -> &'static str {
    "[y] yes    [n] no"
}
// TODO: placeholder English; needs a human's pass.
/// The key [`quit_answers`] labels yes.
pub fn quit_yes_key() -> char {
    'y'
}
// TODO: placeholder English; needs a human's pass.
pub fn time_stopped_not_saved() -> [&'static str; 2] {
    [
        "Time is stopped, so the game did not save",
        "since you activated THE WORLD.",
    ]
}
// TODO: placeholder English; needs a human's pass.
pub fn quit_really_sure() -> &'static str {
    "REALLY sure?"
}

pub fn onboarding_keys() -> [&'static str; 3] {
    ["[hjklyubn] MOVE", "[o/TAB] EXPLORE/FIGHT", "[i] ITEMS"]
}

pub fn pack_slots(used: usize, total: usize) -> String {
    format!("PACK: ({used}/{total})")
}

pub fn help_title() -> &'static str {
    " KEYS "
}

pub fn help_rows() -> [&'static str; 13] {
    [
        "hjklyubn    move or attack (arrows and numpad too)",
        "Shift+dir   run",
        "o           explore",
        "Tab         fight the weakest foe in sight",
        "O           travel to a spot",
        "> <         take the stairs",
        "i           items",
        "a t d       use, throw, drop",
        "e w W P     equip, wield, wear, put on",
        "q r z       quaff, read, zap",
        "f v         fire, reach attack",
        "Z ;         spells, look",
        "A x Q       auto-pickup, close a menu, quit",
    ]
}

pub fn help_close() -> &'static str {
    "[any key] close"
}

pub fn spells_menu_title() -> &'static str {
    " SPELLS "
}

pub fn offer_menu_title() -> &'static str {
    " CHOOSE ONE "
}

// ---------------------------------------------------------------------------
// models/src/pack.rs - the Use/Throw/Drop action modal
// ---------------------------------------------------------------------------

pub fn action_use() -> &'static str {
    " Use    "
}
pub fn action_throw() -> &'static str {
    " Throw  "
}
pub fn action_drop() -> &'static str {
    " Drop   "
}

// ---------------------------------------------------------------------------
// models/src/items/throwing.rs
// ---------------------------------------------------------------------------

pub fn element_wont_leave_hand() -> &'static str {
    "The Element of Yoord will not leave your hand."
}

// ---------------------------------------------------------------------------
// models/src/monsters.rs - the xeroc's disguise, a cosmetic-only category word
// ---------------------------------------------------------------------------

pub const fn mimic_look_scroll() -> &'static str {
    "scroll"
}
pub const fn mimic_look_potion() -> &'static str {
    "potion"
}
pub const fn mimic_look_wand() -> &'static str {
    "wand"
}
pub const fn mimic_look_gold_coin() -> &'static str {
    "gold coin"
}
pub const fn mimic_look_ring() -> &'static str {
    "ring"
}
pub const fn mimic_look_suit_of_armor() -> &'static str {
    "suit of armor"
}
pub const fn mimic_look_weapon() -> &'static str {
    "weapon"
}

// ---------------------------------------------------------------------------
// models/src/equipment.rs
// ---------------------------------------------------------------------------

pub fn worn_tag(names: &str) -> String {
    format!(" (w. {names})")
}

// ---------------------------------------------------------------------------
// Shared "the X" / "The X" / pronoun fragments (models/src/combat.rs,
// models/src/items/theft.rs, models/src/traps.rs)
// ---------------------------------------------------------------------------

pub fn atonement() -> &'static str {
    "The spirits forgive you."
}

pub fn challenge_the_balance() -> &'static str {
    "You challenge the balance!"
}

pub fn spirit_does_nothing(name: &str) -> String {
    format!("The {name} takes no notice of you.")
}

pub fn barter_satisfied() -> &'static str {
    "The spirit poofs, satisfied."
}

pub fn barter_your_side() -> &'static str {
    " YOU "
}

pub fn barter_their_side() -> &'static str {
    " THEM "
}

pub fn barter_confirm_row() -> &'static str {
    "[ Trade ]"
}

pub fn spirit_refuses(name: &str) -> String {
    format!("The {name} grunts, unimpressed.")
}

pub fn pink_demon_submits() -> &'static str {
    "The demon whimpers and pledges themselves to you."
}

pub fn pink_demon_vanishes() -> &'static str {
    "The demon sneers and vanishes."
}

pub fn offer_too_dear() -> &'static str {
    "You don't have enough to pay for that."
}

/// A priced offer row's tag: `stat` is [`hp_abbr`] or [`magic_abbr`].
pub fn offer_price(amount: i32, stat: &str) -> String {
    format!("(-{amount} max {stat})")
}

pub fn angel_tests_your_faith() -> &'static str {
    "Tests your faith!"
}

pub fn the(name: &str) -> String {
    format!("the {name}")
}
pub fn capital_the(name: &str) -> String {
    format!("The {name}")
}
pub fn pronoun_you() -> &'static str {
    "you"
}
pub fn pronoun_something() -> &'static str {
    "Something"
}

pub fn adjective_sees_unseen() -> &'static str {
    "able to see the unseen"
}

pub fn equipped_suffix() -> &'static str {
    " (E)"
}

// ---------------------------------------------------------------------------
// Decks of cards (models/src/items/decks.rs)
// ---------------------------------------------------------------------------

pub fn card_drawn(name: &str, reversed: bool) -> String {
    match reversed {
        false => format!("You draw {name}."),
        true => format!("You draw {name}, reversed."),
    }
}

pub fn card_chain_spent() -> &'static str {
    "The cards run out of tricks."
}

pub fn card_dulls(name: &str) -> String {
    format!("The {name} dulls with a grey flicker.")
}

pub fn card_bala() -> &'static str {
    "The scales tip your way. And the deal is battle!"
}

pub fn card_bole() -> &'static str {
    "Your skin goes hard as bark."
}

pub fn card_crit() -> &'static str {
    "The Chariot rides with you. Every blow lands true."
}

pub fn card_oof() -> &'static str {
    "The Chariot rides over you. Every blow will find you."
}

pub fn card_fool() -> &'static str {
    "Nothing happens. The Fool grins."
}

pub fn card_world() -> &'static str {
    "THE WORLD! Time stops."
}

pub const fn ends_time_stopped() -> &'static str {
    "Time flows again."
}

pub fn card_wands_full() -> &'static str {
    "Your wands hum, full to the brim."
}

pub fn card_wands_empty() -> &'static str {
    "Your wands go cold and quiet."
}

pub fn card_no_ring() -> &'static str {
    "The Princess looks for a ring and finds none."
}

pub fn card_no_stair() -> &'static str {
    "The card points nowhere."
}

pub fn card_to_stair(up: bool) -> &'static str {
    match up {
        true => "A small hand leads you to the stair up.",
        false => "A bony hand leads you to the stair down.",
    }
}

pub fn card_seen(name: &str, reversed: bool) -> String {
    match reversed {
        false => name.to_string(),
        true => format!("{name} (reversed)"),
    }
}

pub fn card_eyes(names: &[String]) -> String {
    format!("The deck holds, top first: {}.", names.join(", "))
}

pub fn card_eyes_nothing() -> &'static str {
    "There is no deck left to see."
}

pub fn card_eyes_never_lie() -> &'static str {
    "They really, really never lie."
}

pub fn card_excuse_nobody() -> &'static str {
    "The excuse falls on no one."
}

pub fn card_jester_nothing() -> &'static str {
    "The Jester shrugs. There is nothing left to play."
}

pub fn card_golden_wind() -> &'static str {
    "A golden wind blows. The floor comes alive, on your side."
}

pub fn card_golden_wind_nothing() -> &'static str {
    "A golden wind blows over a bare floor."
}

pub fn card_hand(hand: &str, points: i32) -> String {
    format!("{hand} ({points} points)")
}

pub fn hand_anti_flush() -> &'static str {
    "Anti-Flush"
}

pub fn hand_pair() -> &'static str {
    "A Pair"
}

pub fn hand_two_pair() -> &'static str {
    "Two Pair"
}

pub fn hand_three_of_a_kind() -> &'static str {
    "Three of a Kind"
}

pub fn hand_full_house() -> &'static str {
    "Full House"
}

pub fn hand_four_of_a_kind() -> &'static str {
    "Four of a Kind"
}

pub fn hand_five_flush() -> &'static str {
    "Five Flush!"
}

// TODO: placeholder English; needs a human's pass.
pub fn card_five_flush_gear() -> &'static str {
    "Everything you wear burns white: +5, and no curse holds."
}

pub fn throw_hangs_in_the_air(name: &str) -> String {
    format!("The {name} leaves your hand and stops in the air.")
}

// TODO: placeholder English; needs a human's pass.
pub fn ice_cube_name() -> &'static str {
    "ice cube"
}

// TODO: placeholder English; needs a human's pass.
pub fn mob_freezes_solid(name: &str) -> String {
    format!("The {name} freezes solid.")
}

// TODO: placeholder English; needs a human's pass.
pub fn ice_cube_slams(name: &str) -> String {
    format!("The ice cube slams into the {name}.")
}

// TODO: placeholder English; needs a human's pass.
pub fn ice_cube_shatters() -> &'static str {
    "The ice cube shatters!"
}

// TODO: placeholder English; needs a human's pass.
pub fn killed_by_ice_cube() -> &'static str {
    "Killed by an ice cube"
}

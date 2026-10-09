//! Turns a keypress into a turn: the whole input state machine, and the three
//! things that can drive it besides a human at the keyboard.
//!
//! [`process_input_and_update`] blocks for one key and hands it to
//! [`dispatch_key`], which routes through the `--MORE--` gate and whichever
//! modal (pack, spells, targeting, quit prompt) is open before falling
//! through to [`handle_movement_input`]. [`auto_explore_step`],
//! [`travel_cursor_step`] and [`fast_move_run`] are the other three ways a
//! turn gets taken, each called by the main loop in place of a blocking read
//! while its own flag is set.

use std::time::Duration;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, poll, read};

use crate::constants::timing::{INCAPACITATED_PAUSE_MS, TRAVEL_BLINK_MS};
use models::*;
use models::{GameState, components::GameLog};

/// The direction of a Shift + movement-key press, for NetHack-style running.
/// Accepts the shifted vi keys (`H J K L Y U B N`), Shift + arrow keys, and
/// Shift + a numpad direction (a numpad digit never changes character under
/// Shift the way a letter does, so it only reads as a run when the modifier
/// comes through on the key event itself). `None` for anything else.
///
/// The shifted WASD cluster used to run too. It doesn't any more: `w`, `a`, `s`
/// and `d` are commands now (see [`handle_movement_input`]), and a movement
/// scheme whose *shifted* half still steered would be a trap laid for the exact
/// player who hasn't noticed the change yet.
fn run_direction(code: KeyCode, mods: KeyModifiers) -> Option<(i16, i16)> {
    match code {
        KeyCode::Char('K') => Some((0, -1)),
        KeyCode::Char('J') => Some((0, 1)),
        KeyCode::Char('H') => Some((-1, 0)),
        KeyCode::Char('L') => Some((1, 0)),
        KeyCode::Char('Y') => Some((-1, -1)),
        KeyCode::Char('U') => Some((1, -1)),
        KeyCode::Char('B') => Some((-1, 1)),
        KeyCode::Char('N') => Some((1, 1)),
        KeyCode::Up if mods.contains(KeyModifiers::SHIFT) => Some((0, -1)),
        KeyCode::Down if mods.contains(KeyModifiers::SHIFT) => Some((0, 1)),
        KeyCode::Left if mods.contains(KeyModifiers::SHIFT) => Some((-1, 0)),
        KeyCode::Right if mods.contains(KeyModifiers::SHIFT) => Some((1, 0)),
        KeyCode::Char('8') if mods.contains(KeyModifiers::SHIFT) => Some((0, -1)),
        KeyCode::Char('2') if mods.contains(KeyModifiers::SHIFT) => Some((0, 1)),
        KeyCode::Char('4') if mods.contains(KeyModifiers::SHIFT) => Some((-1, 0)),
        KeyCode::Char('6') if mods.contains(KeyModifiers::SHIFT) => Some((1, 0)),
        KeyCode::Char('7') if mods.contains(KeyModifiers::SHIFT) => Some((-1, -1)),
        KeyCode::Char('9') if mods.contains(KeyModifiers::SHIFT) => Some((1, -1)),
        KeyCode::Char('1') if mods.contains(KeyModifiers::SHIFT) => Some((-1, 1)),
        KeyCode::Char('3') if mods.contains(KeyModifiers::SHIFT) => Some((1, 1)),
        _ => None,
    }
}

/// Whether the player currently carries the [`Confused`] condition.
fn player_confused(world: &mut World) -> bool {
    world
        .query_filtered::<(), (With<Player>, With<Confused>)>()
        .iter(world)
        .next()
        .is_some()
}

/// The player entity, if one exists.
fn player_entity_opt(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
}

/// The player entity. Panics if there is none — by the time input is handled
/// there always is one.
fn player_entity(world: &mut World) -> Entity {
    player_entity_opt(world).expect("player entity exists during input handling")
}

/// One Tab press: close on — or strike — the weakest foe in sight. Each of the
/// refusals bows out early with its own line; only the last path spends a turn.
/// Returns whether the turn was consumed.
///
/// A launcher in hand changes the whole shape of the press: see
/// [`ranged_auto_fight`]. It never falls back to the walk-and-strike path
/// below — a bow means Tab either shoots or refuses, on the theory that a
/// player who drew a bow did not mean to go melee it.
fn auto_fight_turn(world: &mut World) -> bool {
    if player_confused(world) {
        world
            .resource_mut::<GameLog>()
            .add(strings::too_confused_right_now());
        return false;
    }
    if player_too_injured(world) {
        world
            .resource_mut::<GameLog>()
            .add(strings::too_injured_now());
        return false;
    }
    let player = player_entity(world);
    if wielded_launcher(world, player).is_some() || wielded_returner(world, player).is_some() {
        return ranged_auto_fight(world, player);
    }
    let Some(target) = auto_fight_target(world) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::nothing_to_fight());
        return false;
    };
    match fight_step(world, target) {
        Some((dx, dy)) => {
            world.resource_mut::<GameLog>().unread.clear();
            models::queue_step(world, dx, dy)
        }
        None => {
            world
                .resource_mut::<GameLog>()
                .add(strings::cant_reach_it());
            false
        }
    }
}

/// Tab, with a launcher wielded: fire the first matching projectile at the
/// auto-fight target instead of walking toward it; with a [`Returns`] weapon
/// wielded, throw the weapon itself. Refuses — no turn spent — when there's
/// nothing to shoot at, nothing left to shoot with, or the shot to the target
/// isn't clear; it never falls back to closing the distance by hand.
fn ranged_auto_fight(world: &mut World, player: Entity) -> bool {
    let Some(target) = auto_fight_target(world) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::nothing_to_fight());
        return false;
    };
    let Some(item) = wielded_returner(world, player).or_else(|| first_matching_ammo(world, player))
    else {
        world.resource_mut::<GameLog>().add(strings::out_of_ammo());
        return false;
    };
    if let Some(refusal) = throw_refusal(world, player, item) {
        world.resource_mut::<GameLog>().add(refusal);
        return false;
    }
    let target_pos = *world.get::<Position>(target).unwrap();
    let player_pos = *world.get::<Position>(player).unwrap();
    if chebyshev(player_pos, target_pos) > throw_reach(world, player, item) {
        world.resource_mut::<GameLog>().add(strings::out_of_range());
        return false;
    }
    if !has_clear_shot(world, target) {
        world
            .resource_mut::<GameLog>()
            .add(strings::no_clear_shot());
        return false;
    }

    let Some((slot, item)) = world.get_mut::<Backpack>(player).and_then(|mut bp| {
        let pos = bp.items.iter().position(|&e| e == item)?;
        Some((pos, bp.items.remove(pos)))
    }) else {
        return false;
    };
    let missile = draw_one(world, player, item, Some(slot));
    world.resource_mut::<GameLog>().unread.clear();
    world
        .resource_mut::<ThrowQueue>()
        .throws
        .push(WantsToThrow {
            thrower: player,
            item: missile,
            target: target_pos,
            slot_idx: Some(slot),
        });
    true
}

/// Handles one tick of player input: forfeits the turn if the player is
/// incapacitated, otherwise blocks on a key and applies it. Returns whether a
/// turn was spent, so the caller knows to let the monsters act.
pub fn process_input_and_update(world: &mut World) -> std::io::Result<bool> {
    let more_pending = {
        let width = world.resource::<CommandBar>().log_width();
        let log = world.resource::<GameLog>();
        log_view(&log.unread, width).2
    };
    if !more_pending && player_incapacitated(world) {
        return Ok(true);
    }

    if !more_pending && !modal_open(world) && paralysis_forfeits_turn(world) {
        std::thread::sleep(Duration::from_millis(INCAPACITATED_PAUSE_MS));
        return Ok(true);
    }

    let Event::Key(key) = read()? else {
        return Ok(false);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }
    dispatch_key(world, key)
}

/// Saves the run to `path` the moment time stops or starts again, and only
/// then: `stopped` is whether it was stopped last time this was asked. No save
/// is written while THE WORLD holds, so quitting inside it loses those turns
/// (see [`answer_quit_prompt`]). A failed save says so in the log.
pub(crate) fn save_on_time_edge(world: &mut World, stopped: &mut bool, path: &std::path::Path) {
    let now = models::time_stopped(world);
    if now == *stopped {
        return;
    }
    *stopped = now;
    if let Err(e) = models::save_game(world, path) {
        world
            .resource_mut::<GameLog>()
            .add(strings::failed_to_save_game(&e.to_string()));
    }
}

/// Everything [`process_input_and_update`] does once it is holding a key:
/// Ctrl+C, the rule that no other Ctrl key does anything (Ctrl+Alt is how
/// Windows reports AltGr, so it passes), the `--MORE--` gate, the universal
/// escape hatch, and the four input contexts.
///
/// Split out from the read so the whole modal stack can be exercised without a
/// terminal — `read()` blocks, and a state machine that can only be tested by
/// a person pressing keys is a state machine nobody tests. Everything above
/// this line needs a real keyboard; nothing below it does.
pub(crate) fn dispatch_key(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        world.resource_mut::<GameState>().is_running = false;
        return Ok(false);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) {
        return Ok(false);
    }

    let width = world.resource::<CommandBar>().log_width();
    let more = {
        let log = world.resource::<GameLog>();
        log_view(&log.unread, width).2
    };
    let spirit_menu = world.resource::<OfferMenu>().open || world.resource::<BarterMenu>().open;
    if more && !spirit_menu {
        if key.code == KeyCode::Char(' ') || key.code == KeyCode::Enter {
            acknowledge(&mut world.resource_mut::<GameLog>().unread, width);
        }
        return Ok(false);
    }

    let escape_hatch = matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X'));
    if escape_hatch && close_all_modals(world) {
        return Ok(false);
    }

    if world.resource::<HelpMenu>().open {
        world.resource_mut::<HelpMenu>().open = false;
        return Ok(false);
    }

    if world.resource::<QuitPrompt>().open {
        return Ok(answer_quit_prompt(world, key));
    }
    if world.resource::<TargetingState>().active {
        return handle_targeting_input(world, key);
    }
    if world.resource::<PackIsOpen>().open {
        return handle_inventory_input(world, key);
    }
    if world.resource::<SpellsMenu>().open {
        return handle_spells_input(world, key);
    }
    if world.resource::<OfferMenu>().open {
        return handle_offer_input(world, key);
    }
    if world.resource::<BarterMenu>().open {
        return handle_barter_input(world, key);
    }
    handle_movement_input(world, key)
}

/// Whether any menu, prompt or aiming reticle has the keyboard. A paralysed
/// player's lost turn is not rolled over one: browsing a menu spends no time.
fn modal_open(world: &World) -> bool {
    world.resource::<TargetingState>().active
        || world.resource::<PackIsOpen>().open
        || world.resource::<SpellsMenu>().open
        || world.resource::<OfferMenu>().open
        || world.resource::<BarterMenu>().open
        || world.resource::<QuitPrompt>().open
        || world.resource::<HelpMenu>().open
}

/// Slams every modal shut and returns to plain movement. Returns whether
/// anything was actually open (so a bare `x` on the map doesn't eat the
/// keypress movement would otherwise want).
fn close_all_modals(world: &mut World) -> bool {
    let mut closed = false;
    let mut ts = world.resource_mut::<TargetingState>();
    if ts.active {
        ts.active = false;
        ts.item = None;
        ts.throwing = false;
        ts.spell_effect = None;
        ts.looking = false;
        ts.reach_attack = false;
        closed = true;
    }
    let mut pack = world.resource_mut::<PackIsOpen>();
    if pack.open {
        pack.close();
        closed = true;
    }
    let mut menu = world.resource_mut::<SpellsMenu>();
    if menu.open {
        menu.open = false;
        closed = true;
    }
    let mut offer = world.resource_mut::<OfferMenu>();
    if offer.open {
        offer.open = false;
        offer.options.clear();
        closed = true;
    }
    if world.resource::<BarterMenu>().open {
        cancel_barter(world);
        closed = true;
    }
    let mut quit = world.resource_mut::<QuitPrompt>();
    if quit.open {
        quit.open = false;
        quit.warned = false;
        closed = true;
    }
    let mut help = world.resource_mut::<HelpMenu>();
    if help.open {
        help.open = false;
        closed = true;
    }
    closed
}

/// A keypress while "Really quit?" is up: `y` (or the language's own yes key,
/// [`strings::quit_yes_key`]) ends the run, `n` or `Esc` goes
/// back to the dungeon, and anything else is ignored rather than guessed at.
/// Never spends a turn. (`x` and `X` also cancel, via `close_all_modals`
/// upstream — the prompt is a modal like any other.)
///
/// With time stopped the first `y` only warns: nothing has been saved since
/// THE WORLD began ([`save_on_time_edge`]), and quitting now loses those
/// turns. A second `y` quits.
fn answer_quit_prompt(world: &mut World, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char(c) if [strings::quit_yes_key(), 'y'].contains(&c.to_ascii_lowercase()) => {
            let warn = !world.resource::<QuitPrompt>().warned && models::time_stopped(world);
            let mut quit = world.resource_mut::<QuitPrompt>();
            quit.warned = warn;
            quit.open = warn;
            world.resource_mut::<GameState>().is_running = warn;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            let mut quit = world.resource_mut::<QuitPrompt>();
            quit.open = false;
            quit.warned = false;
        }
        _ => {}
    }
    false
}

/// A keypress while the aiming reticle is up: move it, jump it to the next
/// interesting thing in view, fire it, or cancel.
fn handle_targeting_input(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    let mut cancel = false;
    let mut confirm = false;
    let mut cycle = false;
    let mut dx = 0i16;
    let mut dy = 0i16;
    match key.code {
        KeyCode::Esc => cancel = true,
        KeyCode::Enter | KeyCode::Char(' ') => confirm = true,
        KeyCode::Tab => cycle = true,
        KeyCode::Char('k') | KeyCode::Up | KeyCode::Char('8') => dy = -1,
        KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('2') => dy = 1,
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('4') => dx = -1,
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('6') => dx = 1,
        KeyCode::Char('y') | KeyCode::Char('7') => (dx, dy) = (-1, -1),
        KeyCode::Char('u') | KeyCode::Char('9') => (dx, dy) = (1, -1),
        KeyCode::Char('b') | KeyCode::Char('1') => (dx, dy) = (-1, 1),
        KeyCode::Char('n') | KeyCode::Char('3') => (dx, dy) = (1, 1),
        _ => {}
    }

    if cancel {
        let mut ts = world.resource_mut::<TargetingState>();
        ts.active = false;
        ts.item = None;
        ts.throwing = false;
        ts.spell_effect = None;
        ts.looking = false;
        ts.reach_attack = false;
        return Ok(false);
    }
    if cycle {
        return Ok(cycle_target(world));
    }
    if dx != 0 || dy != 0 {
        return Ok(spell_target_cursor(world, dx, dy));
    }
    if confirm {
        return fire_at_target(world);
    }
    Ok(false)
}

/// `Tab`, while aiming: snap the reticle to the next monster or item in the
/// player's viewshed and within the item's own reach — the same targets a
/// manual nudge could reach, just without the walk there. Wraps around, and
/// does nothing when nothing qualifies.
fn cycle_target(world: &mut World) -> bool {
    let (item, spell_effect, looking, throwing, reach_attack, cursor_x, cursor_y) = {
        let ts = world.resource::<TargetingState>();
        (
            ts.item,
            ts.spell_effect,
            ts.looking,
            ts.throwing,
            ts.reach_attack,
            ts.cursor_x,
            ts.cursor_y,
        )
    };
    let player = player_entity(world);
    let player_pos = *world.get::<Position>(player).unwrap();
    let visible = world.get::<Viewshed>(player).unwrap().visible_tiles.clone();
    let max_range = aim_range(
        world,
        player,
        item,
        spell_effect,
        looking,
        throwing,
        reach_attack,
    );

    let mut candidates: Vec<(u16, u16)> = {
        let mut q =
            world.query_filtered::<(&Position, Option<&Mob>, Option<&Item>), Without<Hidden>>();
        q.iter(world)
            .filter(|(pos, mob, item)| {
                (mob.is_some() || item.is_some())
                    && (pos.x, pos.y) != (player_pos.x, player_pos.y)
                    && visible.contains(&(pos.x, pos.y))
                    && chebyshev(player_pos, **pos) <= max_range
            })
            .map(|(pos, ..)| (pos.x, pos.y))
            .collect()
    };
    if candidates.is_empty() {
        return false;
    }
    candidates.sort_unstable_by_key(|&(x, y)| (y, x));
    candidates.dedup();

    let current = (cursor_x as u16, cursor_y as u16);
    let next = candidates
        .iter()
        .position(|&c| c == current)
        .map_or(0, |i| (i + 1) % candidates.len());
    let (nx, ny) = candidates[next];
    {
        let mut ts = world.resource_mut::<TargetingState>();
        ts.cursor_x = nx as i16;
        ts.cursor_y = ny as i16;
    }
    announce_look(world)
}

/// A directional key while aiming: nudge the reticle one tile, but only onto a
/// tile that is both in view and inside the reticle's reach.
fn spell_target_cursor(world: &mut World, dx: i16, dy: i16) -> bool {
    let (item, spell_effect, looking, cursor_x, cursor_y, throwing, reach_attack) = {
        let ts = world.resource::<TargetingState>();
        (
            ts.item,
            ts.spell_effect,
            ts.looking,
            ts.cursor_x,
            ts.cursor_y,
            ts.throwing,
            ts.reach_attack,
        )
    };
    let new_x = cursor_x.saturating_add(dx);
    let new_y = cursor_y.saturating_add(dy);

    let player = player_entity(world);
    let player_pos = *world.get::<Position>(player).unwrap();
    let visible = world.get::<Viewshed>(player).unwrap().visible_tiles.clone();
    let max_range = aim_range(
        world,
        player,
        item,
        spell_effect,
        looking,
        throwing,
        reach_attack,
    );

    let distance = (new_x - player_pos.x as i16)
        .abs()
        .max((new_y - player_pos.y as i16).abs());
    let in_view = visible.contains(&(new_x as u16, new_y as u16));
    if distance > max_range as i16 || !in_view {
        return false;
    }
    {
        let mut ts = world.resource_mut::<TargetingState>();
        ts.cursor_x = new_x;
        ts.cursor_y = new_y;
    }
    announce_look(world)
}

/// While looking, reads out whatever the reticle now sits on — called after
/// every cursor move (arrow keys, `Tab`) as well as the moment `L` opens it,
/// so `l` then `Tab Tab Tab` reads off everything in view with no `Enter`
/// needed in between. A no-op for every other reticle purpose; those only
/// ever announce on confirm ([`fire_at_target`]).
fn announce_look(world: &mut World) -> bool {
    let (looking, cx, cy) = {
        let ts = world.resource::<TargetingState>();
        (ts.looking, ts.cursor_x, ts.cursor_y)
    };
    if !looking {
        return false;
    }
    let target = Position {
        x: cx as u16,
        y: cy as u16,
    };
    for line in describe_target(world, target) {
        world.resource_mut::<GameLog>().add(line);
    }
    meet_its_eyes(world, target)
}

/// Looking is not free when the thing you are looking at looks back.
///
/// A medusa petrifies whoever turns their attention on it, and `L` is turning
/// your attention on it — the reticle is the player's eyes, and a reticle that
/// could rest on a gorgon in perfect safety would be a way to scout the floor
/// that nothing else in the game offers. So look mode fires
/// [`models::Moment::OnTargeted`] exactly as a swing or a zap does; it is the
/// fifth path through that moment and it costs one line here.
///
/// Reports whether the turn went with it. When it did, the reticle closes:
/// you are not still browsing the room.
fn meet_its_eyes(world: &mut World, target: Position) -> bool {
    let Some(seen) = models::mob_at(world, target) else {
        return false;
    };
    if !models::answers_being_looked_at(world, seen) {
        return false;
    }
    let player = player_entity(world);
    if !models::fire_on_targeted(world, player, seen) {
        return false;
    }
    world.resource_mut::<GameLog>().add(strings::well_played());
    let mut ts = world.resource_mut::<TargetingState>();
    ts.active = false;
    ts.looking = false;
    true
}

/// How far the reticle reaches: a look can range over the whole viewshed (the
/// `in_view` check does the real work of bounding it), an active spell reaches
/// as far as its own [`SpellDef::range`](models::SpellDef::range), a throw goes
/// an arm's length — or a launcher's, when what's aimed is ammunition for the
/// bow in hand — a zapped item as far as its own [`Ranged`], and anything with
/// none of those, a bare 8.
fn aim_range(
    world: &World,
    thrower: Entity,
    item: Option<Entity>,
    spell_effect: Option<SpellEffect>,
    looking: bool,
    throwing: bool,
    reach_attack: bool,
) -> i32 {
    if looking {
        return (MAP_WIDTH as i32).max(MAP_HEIGHT as i32);
    }
    if let Some(effect) = spell_effect {
        return models::SpellDef::of(effect).range;
    }
    if throwing {
        return item.map_or(THROW_RANGE, |i| throw_reach(world, thrower, i));
    }
    if reach_attack {
        return item.and_then(|i| world.get::<Reach>(i)).map_or(1, |r| r.0);
    }
    item.and_then(|i| world.get::<Ranged>(i))
        .map_or(8, |r| r.range)
}

/// Enter/Space while aiming: a look just reads the tile and closes (no turn,
/// and it's the one reticle purpose allowed on the player's own tile); a spell
/// queues itself directly, nothing to pull from a pack; anything else pulls
/// the item from the pack and hands it to the throw or use queue. A shot at
/// the player's own tile is refused for every purpose but looking.
fn fire_at_target(world: &mut World) -> std::io::Result<bool> {
    let (tx, ty, item_entity, spell_effect, looking, throwing, reach_attack) = {
        let mut ts = world.resource_mut::<TargetingState>();
        ts.active = false;
        let grabbed = (
            ts.cursor_x,
            ts.cursor_y,
            ts.item,
            ts.spell_effect,
            ts.looking,
            ts.throwing,
            ts.reach_attack,
        );
        ts.item = None;
        ts.spell_effect = None;
        ts.looking = false;
        ts.throwing = false;
        ts.reach_attack = false;
        grabbed
    };
    let player = player_entity(world);
    let target = Position {
        x: tx as u16,
        y: ty as u16,
    };

    if looking {
        return Ok(false);
    }

    let at_self = world
        .get::<Position>(player)
        .is_some_and(|p| p.x == target.x && p.y == target.y);
    if at_self {
        world
            .resource_mut::<GameLog>()
            .add(strings::great_idea_but_no());
        return Ok(false);
    }

    if reach_attack {
        let Some(weapon) = item_entity else {
            return Ok(false);
        };
        models::queue_reach_attack(world, weapon, target);
        return Ok(true);
    }

    if let Some(effect) = spell_effect {
        world.resource_mut::<SpellQueue>().spells.push(WantsToCast {
            user: player,
            effect,
            target,
        });
        return Ok(true);
    }

    let Some((slot, item)) = world.get_mut::<Backpack>(player).and_then(|mut bp| {
        let pos = bp.items.iter().position(|&e| e == item_entity.unwrap())?;
        Some((pos, bp.items.remove(pos)))
    }) else {
        return Ok(false);
    };

    if throwing {
        let missile = models::draw_one(world, player, item, Some(slot));
        world
            .resource_mut::<ThrowQueue>()
            .throws
            .push(WantsToThrow {
                thrower: player,
                item: missile,
                target,
                slot_idx: Some(slot),
            });
        return Ok(true);
    }
    world.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: player,
        item,
        target: Some(target),
        slot_idx: Some(slot),
    });
    Ok(true)
}

/// `L`: open the aiming reticle in look mode — the one reticle purpose that
/// queues nothing and spends no turn, confirmed by [`fire_at_target`].
fn begin_look(world: &mut World) -> std::io::Result<bool> {
    let player = player_entity(world);
    open_reticle_for(world, player, None, None, true, false, false);
    Ok(announce_look(world))
}

/// What `Look` reads off the aimed tile: whichever monster or item is
/// standing there (a monster wins over something lying under it), or —
/// failing that — an already-revealed trap, or nothing at all, which logs
/// nothing: the reticle already shows bare floor. A [`Hidden`]
/// thing is passed over exactly as it is for every other purpose in the game:
/// looking is not a way to cheat a search.
///
/// Whatever it is wearing rides along in the same line — `"You see a centaur
/// (w. a short bow)."` — exactly as it does when the thing is first spotted
/// ([`models::worn_tag`]).
///
/// A monster gets a line of its own after the sighting: every notable move,
/// immunity or on-hit trick it carries, `"Beware: flying; spellcasting."`. Every creature in
/// the dungeon is a they, whatever it is — a dungeon has no business guessing
/// at what lives in it, and "its" for the ones without the wits to use items
/// was a distinction the player never asked for.
fn describe_target(world: &mut World, target: Position) -> Vec<String> {
    let visible: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Position)>();
        q.iter(world)
            .filter(|(_, p)| **p == target)
            .map(|(e, _)| e)
            .filter(|&e| world.get::<Hidden>(e).is_none())
            .collect()
    };
    let pick = visible
        .iter()
        .find(|&&e| world.get::<Mob>(e).is_some() || world.get::<Player>(e).is_some())
        .or_else(|| visible.iter().find(|&&e| world.get::<Item>(e).is_some()))
        .or_else(|| visible.iter().find(|&&e| world.get::<Trap>(e).is_some()))
        .copied();
    let Some(seen) = pick else {
        return Vec::new();
    };

    let mut lines = vec![strings::you_see(
        &models::with_article(world, seen),
        &models::worn_tag(world, seen),
    )];
    if world.get::<Mob>(seen).is_some() {
        let dangers = models::dangers_of(world, seen);
        if !dangers.is_empty() {
            lines.push(strings::beware(&dangers));
        }
    }
    lines
}

/// A keypress while the pack is open: routed to the action modal (Use / Throw /
/// Drop) when one is up, otherwise to the item list underneath it.
fn handle_inventory_input(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    let (selected, action_mode, action_selected) = {
        let pack = world.resource::<PackIsOpen>();
        (pack.selected, pack.action_mode, pack.action_selected)
    };
    if let Some(item_idx) = action_mode {
        return Ok(run_action_modal(world, key, item_idx, action_selected));
    }
    Ok(navigate_pack(world, key, selected))
}

/// The Use / Throw / Drop modal. Returns whether the keypress spent a turn —
/// true only when it committed to a plain (non-aimed) Use or a successful Drop.
fn run_action_modal(
    world: &mut World,
    key: KeyEvent,
    item_idx: usize,
    action_selected: usize,
) -> bool {
    const ACTION_COUNT: usize = ItemAction::MENU.len();
    let mut close = false;
    let mut confirm = false;
    let mut sel = action_selected;
    match key.code {
        KeyCode::Esc => close = true,
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('8') => {
            sel = (sel + ACTION_COUNT - 1) % ACTION_COUNT;
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('2') => {
            sel = (sel + 1) % ACTION_COUNT;
        }
        KeyCode::Enter | KeyCode::Char(' ') => confirm = true,
        _ => {}
    }

    {
        let mut pack = world.resource_mut::<PackIsOpen>();
        pack.action_selected = sel;
        if close || confirm {
            pack.close();
        }
    }

    if !confirm {
        return false;
    }
    let Some(player) = player_entity_opt(world) else {
        return true;
    };
    let action = ItemAction::at(sel);
    commit_item_action(world, player, item_idx, action)
}

/// Pulls the chosen item out of the pack and carries out `action` on it: queue
/// a use, open the aiming reticle, or drop it. Returns whether a turn was spent
/// (a plain Use or a completed Drop; aiming and refusals do not).
fn commit_item_action(
    world: &mut World,
    player: Entity,
    item_idx: usize,
    action: ItemAction,
) -> bool {
    match action {
        ItemAction::Use => {
            let Some(item) = take_pack_item(world, player, item_idx) else {
                return true;
            };
            use_or_aim(world, player, item, item_idx)
        }
        ItemAction::Throw => {
            let Some(item) = take_pack_item(world, player, item_idx) else {
                return true;
            };
            aim_throw(world, player, item, item_idx);
            false
        }
        ItemAction::Drop => {
            let Some(item) = world
                .get::<Backpack>(player)
                .and_then(|pack| pack.items.get(item_idx).copied())
            else {
                return true;
            };
            models::queue_drop(world, player, item)
        }
    }
}

/// Use: a plain item goes straight to the use queue (turn spent); a ranged one
/// goes back in the pack and opens the aiming reticle instead (no turn); a
/// thing that is only ever thrown goes back in the pack and says so (no turn).
fn use_or_aim(world: &mut World, player: Entity, item: Entity, item_idx: usize) -> bool {
    if let Some(refusal) =
        use_refusal(world, item).or_else(|| models::toggle_refusal(world, player, item))
    {
        return_to_pack(world, player, item, item_idx);
        world.resource_mut::<GameLog>().add(refusal);
        return false;
    }
    let needs_reticle = world.get::<Ranged>(item).is_some()
        && world
            .get::<Wand>(item)
            .map_or(true, |w| w.effect.needs_target());
    if !needs_reticle {
        world.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user: player,
            item,
            target: None,
            slot_idx: Some(item_idx),
        });
        return true;
    }
    return_to_pack(world, player, item, item_idx);
    open_reticle(world, player, item, false);
    false
}

/// Throw: the item waits in the pack and the aiming reticle opens — unless
/// something (the Element, cursed worn gear) refuses to leave the hand.
fn aim_throw(world: &mut World, player: Entity, item: Entity, item_idx: usize) {
    return_to_pack(world, player, item, item_idx);
    if let Some(refusal) = throw_refusal(world, player, item) {
        world.resource_mut::<GameLog>().add(refusal);
        return;
    }
    open_reticle(world, player, item, true);
}

/// Removes pack row `idx` and hands back the item, or `None` if the row is out
/// of range (the pack shifted since the menu opened).
fn take_pack_item(world: &mut World, player: Entity, idx: usize) -> Option<Entity> {
    let mut backpack = world.get_mut::<Backpack>(player)?;
    (idx < backpack.items.len()).then(|| backpack.items.remove(idx))
}

/// Puts `item` back at pack row `idx`.
fn return_to_pack(world: &mut World, player: Entity, item: Entity, idx: usize) {
    if let Some(mut backpack) = world.get_mut::<Backpack>(player) {
        backpack.items.insert(idx, item);
    }
}

/// Arms the aiming reticle on `item`, centred on the player.
fn open_reticle(world: &mut World, player: Entity, item: Entity, throwing: bool) {
    open_reticle_for(world, player, Some(item), None, false, throwing, false);
}

/// Arms the aiming reticle on whichever one of an item, a spell or a plain look
/// the caller wants — exactly one of `item` / `spell_effect` / `looking` is
/// ever set, and [`fire_at_target`] is what reads the combination back apart.
fn open_reticle_for(
    world: &mut World,
    player: Entity,
    item: Option<Entity>,
    spell_effect: Option<SpellEffect>,
    looking: bool,
    throwing: bool,
    reach_attack: bool,
) {
    let pos = *world.get::<Position>(player).unwrap();
    let snap = (!looking)
        .then(|| {
            let range = aim_range(
                world,
                player,
                item,
                spell_effect,
                looking,
                throwing,
                reach_attack,
            );
            nearest_mob(world, player, range)
        })
        .flatten();
    let (cx, cy) = snap.unwrap_or((pos.x, pos.y));
    let mut ts = world.resource_mut::<TargetingState>();
    ts.active = true;
    ts.item = item;
    ts.spell_effect = spell_effect;
    ts.looking = looking;
    ts.throwing = throwing;
    ts.reach_attack = reach_attack;
    ts.cursor_x = cx as i16;
    ts.cursor_y = cy as i16;
}

/// The closest visible monster inside `max_range`, ties broken by reading
/// order so the same floor always opens the reticle on the same tile. What
/// [`open_reticle_for`] starts on; `Tab` ([`cycle_target`]) walks the rest.
/// A [`Hidden`] thing is passed over, exactly as it is everywhere else — a
/// reticle that snapped to a disguised mimic would be a way to spot one for
/// free. So is the player's [`Helper`], which `Tab` can still reach.
fn nearest_mob(world: &mut World, player: Entity, max_range: i32) -> Option<(u16, u16)> {
    let player_pos = *world.get::<Position>(player)?;
    let visible = world.get::<Viewshed>(player)?.visible_tiles.clone();
    let mut q = world.query_filtered::<&Position, (
        With<Mob>,
        Without<Hidden>,
        Without<Helper>,
        Without<models::IceCube>,
    )>();
    q.iter(world)
        .filter(|p| {
            (p.x, p.y) != (player_pos.x, player_pos.y)
                && visible.contains(&(p.x, p.y))
                && chebyshev(player_pos, **p) <= max_range
        })
        .min_by_key(|p| (chebyshev(player_pos, **p), p.y, p.x))
        .map(|p| (p.x, p.y))
}

/// The row `dir` steps around the ring from `from`. `rows` are backpack
/// indices and `from` is normally one of them; if it isn't (the pack shifted
/// under the cursor) the highlight lands on the first row rather than nowhere.
fn step_row(rows: &[usize], from: usize, dir: isize) -> usize {
    let fallback = rows.first().copied().unwrap_or(from);
    let Some(at) = rows.iter().position(|&r| r == from) else {
        return fallback;
    };
    let next = (at as isize + dir).rem_euclid(rows.len() as isize) as usize;
    rows[next]
}

/// A keypress in the item list: move the highlight, close the pack, or act on
/// the highlighted row.
///
/// Which rows exist at all is [`PackMode`]'s business (`pack_rows`), and so is
/// what "act" means: browsing opens the Use / Throw / Drop modal, every other
/// mode carries its own verb and commits on the spot. Returns whether a turn
/// was spent — only a committed Use or Drop can.
fn navigate_pack(world: &mut World, key: KeyEvent, current_selected: usize) -> bool {
    let mode = world.resource::<PackIsOpen>().mode;
    let rows = pack_rows(world, mode);
    let mut new_selected = current_selected;
    let mut close = false;
    let mut trigger = None;
    match key.code {
        KeyCode::Esc => close = true,
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('8') => {
            new_selected = step_row(&rows, new_selected, -1);
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('2') => {
            new_selected = step_row(&rows, new_selected, 1);
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            trigger = rows.contains(&new_selected).then_some(new_selected);
        }
        KeyCode::Char(c) if c.is_ascii_lowercase() => {
            let idx = (c as u32 - 'a' as u32) as usize;
            trigger = rows.contains(&idx).then_some(idx);
            new_selected = trigger.unwrap_or(new_selected);
        }
        _ => {}
    }

    {
        let mut pack = world.resource_mut::<PackIsOpen>();
        pack.selected = new_selected;
        if close {
            pack.close();
        }
    }

    let Some(idx) = trigger else {
        return false;
    };
    let Some(action) = mode.action() else {
        let mut pack = world.resource_mut::<PackIsOpen>();
        pack.action_mode = Some(idx);
        pack.action_selected = 0;
        return false;
    };
    world.resource_mut::<PackIsOpen>().close();
    let Some(player) = player_entity_opt(world) else {
        return false;
    };
    commit_item_action(world, player, idx, action)
}

/// The four `Alt`+letter shortcuts into spell slots one to four. They are
/// checked before the bare letters, so `Alt`+`q` casts and `q` still quaffs;
/// any other `Alt`-held key does what it would do bare.
const SPELL_KEYS: [char; 4] = ['q', 'w', 'e', 'r'];

/// A keypress while walking the map: a run, a command, or a single step.
///
/// Movement is vi keys, arrows and the numpad. WASD used to be a fourth scheme
/// and is not one any more: those four letters are commands (`a` use, `d` drop,
/// `w` wield, plus `W` wear), which is the whole reason they had to stop being
/// directions. Nothing else changed about how you move.
///
/// Quitting is `Q` or `X`, both of which raise the "Really quit?" prompt, or
/// Ctrl+C, which doesn't. `Esc` does *not* quit: it is the key a player mashes
/// to get out of a menu, and the last thing that should do from the map is end
/// the run.
fn handle_movement_input(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    if let Some((rdx, rdy)) = run_direction(key.code, key.modifiers) {
        return Ok(start_run(world, rdx, rdy));
    }

    let step = match key.code {
        KeyCode::Char('Q') | KeyCode::Char('X') => {
            world.resource_mut::<QuitPrompt>().open = true;
            return Ok(false);
        }
        KeyCode::Char(c)
            if key.modifiers.contains(KeyModifiers::ALT)
                && SPELL_KEYS.contains(&c.to_ascii_lowercase()) =>
        {
            let slot = SPELL_KEYS
                .iter()
                .position(|&k| k == c.to_ascii_lowercase())
                .unwrap();
            return fire_spell(world, slot);
        }
        KeyCode::Char('Z') => return begin_spells_menu(world),
        KeyCode::F(1) => {
            world.resource_mut::<HelpMenu>().open = true;
            return Ok(false);
        }
        KeyCode::F(2) => {
            let mut bar = world.resource_mut::<CommandBar>();
            bar.hidden = !bar.hidden;
            return Ok(false);
        }
        KeyCode::Char(';') => return begin_look(world),
        KeyCode::Char('i') => return open_pack(world, PackMode::Browse),
        KeyCode::Char('a') => return open_pack(world, PackMode::Use),
        KeyCode::Char('t') => return open_pack(world, PackMode::Throw),
        KeyCode::Char('d') => return open_pack(world, PackMode::Drop),
        KeyCode::Char('e') => return open_pack(world, PackMode::Equip),
        KeyCode::Char('q') => return open_pack(world, PackMode::Quaff),
        KeyCode::Char('r') => return open_pack(world, PackMode::Read),
        KeyCode::Char('z') => return open_pack(world, PackMode::Zap),
        KeyCode::Char('w') => return open_pack(world, PackMode::Wield),
        KeyCode::Char('W') => return open_pack(world, PackMode::Wear),
        KeyCode::Char('P') => return open_pack(world, PackMode::PutOn),
        KeyCode::Char('o') => return begin_auto_explore(world),
        KeyCode::Char('A') => {
            toggle_auto_pickup(world);
            return Ok(false);
        }
        KeyCode::Char('f') => return begin_fire(world),
        KeyCode::Char('v') => return begin_reach_attack(world),
        KeyCode::Char('T') => return Ok(models::queue_willed_teleport(world)),
        KeyCode::Char('O') => {
            open_travel_cursor(world);
            return Ok(false);
        }
        KeyCode::Tab => return Ok(auto_fight_turn(world)),
        KeyCode::Char('>') | KeyCode::Char('.') => return Ok(travel_or_use_stairs(world, true)),
        KeyCode::Char('<') | KeyCode::Char(',') => return Ok(travel_or_use_stairs(world, false)),
        KeyCode::Char('k') | KeyCode::Up | KeyCode::Char('8') => Some((0, -1)),
        KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('2') => Some((0, 1)),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('4') => Some((-1, 0)),
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('6') => Some((1, 0)),
        KeyCode::Char('y') | KeyCode::Char('7') => Some((-1, -1)),
        KeyCode::Char('u') | KeyCode::Char('9') => Some((1, -1)),
        KeyCode::Char('b') | KeyCode::Char('1') => Some((-1, 1)),
        KeyCode::Char('n') | KeyCode::Char('3') => Some((1, 1)),
        _ => None,
    };

    let Some((dx, dy)) = step else {
        return Ok(false);
    };
    world.resource_mut::<GameLog>().unread.clear();
    Ok(models::queue_step(world, dx, dy))
}

/// Shift + direction: kick off a NetHack-style run, charge a creature in view,
/// or say why neither can happen. Returns whether a turn was spent, which only
/// a charge does.
fn start_run(world: &mut World, rdx: i16, rdy: i16) -> bool {
    if player_confused(world) {
        world
            .resource_mut::<GameLog>()
            .add(strings::too_confused_right_now());
        return false;
    }
    match fast_move_plan(world, rdx, rdy) {
        FastMovePlan::MonsterInSight => {
            world
                .resource_mut::<GameLog>()
                .add(strings::not_while_monster_in_sight());
        }
        FastMovePlan::ThickOfIt => {
            world
                .resource_mut::<GameLog>()
                .add(strings::cannot_charge_in_the_thick_of_it());
        }
        FastMovePlan::Charge(target) => {
            world.resource_mut::<GameLog>().unread.clear();
            return models::queue_charge(world, target);
        }
        FastMovePlan::Blocked => {
            world
                .resource_mut::<GameLog>()
                .add(strings::cant_run_that_way());
        }
        FastMovePlan::Straight => {
            world.resource_mut::<GameLog>().unread.clear();
            world.resource_mut::<FastMove>().start(rdx, rdy, None);
        }
        FastMovePlan::Travel(tile) => {
            world.resource_mut::<GameLog>().unread.clear();
            world.resource_mut::<FastMove>().start(rdx, rdy, Some(tile));
        }
    }
    false
}

/// Any of the eleven pack keys — `i` `a` `t` `d` `e` `q` `r` `z` `w` `W` `P`:
/// open the pack in `mode` with the cursor on its first row, or — when that
/// mode has no rows to show — say so and stay on the map. Never spends a turn;
/// the row that gets picked might.
fn open_pack(world: &mut World, mode: PackMode) -> std::io::Result<bool> {
    let rows = pack_rows(world, mode);
    let Some(&first) = rows.first() else {
        world.resource_mut::<GameLog>().add(mode.nothing_line());
        world.resource_mut::<PackIsOpen>().close();
        return Ok(false);
    };
    world.resource_mut::<PackIsOpen>().open_at(mode, first);
    Ok(false)
}

/// Fires (opens the aiming reticle for) spell slot `slot` of the player's
/// [`Spellset`] — `Alt`+`Q`/`W`/`E`/`R` on the map, or a row picked from the
/// `Z` menu. Refuses, no turn spent, if the slot is empty or the pool can't
/// cover it; the check here is a courtesy so the reticle never opens on a
/// spell that can only fizzle — [`models::spell_system`] checks again before it
/// actually spends the cost.
fn fire_spell(world: &mut World, slot: usize) -> std::io::Result<bool> {
    let player = player_entity(world);
    let Some(effect) = world
        .get::<Spellset>(player)
        .and_then(|m| m.slots.get(slot).copied())
    else {
        world
            .resource_mut::<GameLog>()
            .add(strings::no_spell_there());
        return Ok(false);
    };
    let cost = models::spell_cost(world, player, effect);
    let affordable = world.get::<Magic>(player).is_some_and(|m| m.points >= cost);
    if !affordable {
        world
            .resource_mut::<GameLog>()
            .add(strings::no_magic_for_that());
        return Ok(false);
    }
    if !effect.needs_target() {
        let target = world
            .get::<Position>(player)
            .copied()
            .unwrap_or(Position { x: 0, y: 0 });
        world.resource_mut::<SpellQueue>().spells.push(WantsToCast {
            user: player,
            effect,
            target,
        });
        return Ok(true);
    }
    open_reticle_for(world, player, None, Some(effect), false, false, false);
    Ok(false)
}

/// `Z`: open the spells list, cursor on the first slot. Says so and stays on
/// the map rather than opening an empty menu.
fn begin_spells_menu(world: &mut World) -> std::io::Result<bool> {
    let player = player_entity(world);
    let has_any = world
        .get::<Spellset>(player)
        .is_some_and(|m| !m.slots.is_empty());
    if !has_any {
        world.resource_mut::<GameLog>().add(strings::no_spells());
        return Ok(false);
    }
    let mut menu = world.resource_mut::<SpellsMenu>();
    menu.open = true;
    menu.selected = 0;
    Ok(false)
}

/// A keypress while the `Z` spells menu is up: navigate, jump straight to a
/// slot by its row letter, confirm, or cancel. Never spends a turn itself —
/// only the reticle [`fire_spell`] opens can do that.
///
/// Rows are lettered `a`-`d`, like the pack's, and for the same reason: the
/// digits are already the numpad's directions here, and a key that navigates
/// and selects at once is a key that does the wrong one of the two.
fn handle_spells_input(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    let player = player_entity(world);
    let row_count = world.get::<Spellset>(player).map_or(0, |m| m.slots.len());
    let slot_count = row_count.max(1);
    let selected = world.resource::<SpellsMenu>().selected;

    let mut close = false;
    let mut fire = None;
    match key.code {
        KeyCode::Esc => close = true,
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('8') => {
            world.resource_mut::<SpellsMenu>().selected = (selected + slot_count - 1) % slot_count;
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('2') => {
            world.resource_mut::<SpellsMenu>().selected = (selected + 1) % slot_count;
        }
        KeyCode::Enter | KeyCode::Char(' ') => fire = Some(selected),
        KeyCode::Char(c) if c.is_ascii_lowercase() => {
            let idx = c as usize - 'a' as usize;
            if idx < row_count {
                fire = Some(idx);
            }
        }
        _ => {}
    }

    if close {
        world.resource_mut::<SpellsMenu>().open = false;
        return Ok(false);
    }
    let Some(idx) = fire else {
        return Ok(false);
    };
    world.resource_mut::<SpellsMenu>().open = false;
    fire_spell(world, idx)
}

/// A keypress while a spirit's "choose one of three" offer menu is up:
/// navigate, jump straight to a row by its letter, confirm, or cancel. A
/// spirit's melee already spent the turn that opened this menu, so
/// confirming never spends another — [`models::spirits::confirm_offer`]
/// only applies whichever option was picked.
fn handle_offer_input(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    let row_count = world.resource::<OfferMenu>().options.len();
    let selected = world.resource::<OfferMenu>().selected;

    let mut close = false;
    let mut pick = None;
    match key.code {
        KeyCode::Esc => close = true,
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('8') => {
            world.resource_mut::<OfferMenu>().selected = (selected + row_count - 1) % row_count;
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('2') => {
            world.resource_mut::<OfferMenu>().selected = (selected + 1) % row_count;
        }
        KeyCode::Enter | KeyCode::Char(' ') => pick = Some(selected),
        KeyCode::Char(c) if c.is_ascii_lowercase() => {
            let idx = c as usize - 'a' as usize;
            if idx < row_count {
                pick = Some(idx);
            }
        }
        _ => {}
    }

    if close {
        let mut menu = world.resource_mut::<OfferMenu>();
        menu.open = false;
        menu.options.clear();
        return Ok(false);
    }
    let Some(idx) = pick else {
        return Ok(false);
    };
    let choice = world.resource::<OfferMenu>().options[idx];
    let player = player_entity(world);
    confirm_offer(world, player, choice);
    let mut menu = world.resource_mut::<OfferMenu>();
    if !menu.open {
        menu.options.clear();
    }
    Ok(false)
}

/// A keypress while the barter menu is up: left/right switches columns,
/// up/down moves the cursor within one, Enter/Space toggles the row under it
/// in or out of the trade — except on the player column's trailing "confirm
/// the trade" row, which instead closes the deal via
/// [`models::spirits::confirm_barter`]. Esc backs out with nothing moved.
/// Never spends a turn: the melee that opened this already did.
fn handle_barter_input(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    let (column, cursor, player_rows, demon_rows) = {
        let menu = world.resource::<BarterMenu>();
        (
            menu.column,
            menu.cursor,
            menu.player_side.len(),
            menu.demon_visible().len(),
        )
    };
    let row_count = match column {
        BarterColumn::Player => player_rows + 1,
        BarterColumn::Demon => demon_rows.max(1),
    };

    let mut close = false;
    let mut confirm = false;
    let mut toggle = false;
    match key.code {
        KeyCode::Esc => close = true,
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('8') => {
            world.resource_mut::<BarterMenu>().cursor = (cursor + row_count - 1) % row_count;
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('2') => {
            world.resource_mut::<BarterMenu>().cursor = (cursor + 1) % row_count;
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('4') => {
            let mut menu = world.resource_mut::<BarterMenu>();
            menu.column = BarterColumn::Player;
            menu.cursor = 0;
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('6') => {
            if demon_rows > 0 {
                let mut menu = world.resource_mut::<BarterMenu>();
                menu.column = BarterColumn::Demon;
                menu.cursor = 0;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            confirm = column == BarterColumn::Player && cursor == player_rows;
            toggle = !confirm;
        }
        _ => {}
    }

    if close {
        cancel_barter(world);
        return Ok(false);
    }
    if toggle {
        toggle_barter_selection(world);
    }
    if confirm {
        let player = player_entity(world);
        confirm_barter(world, player);
    }
    Ok(false)
}

/// `A`: flip whether auto-explore detours to pick things up, and say which way
/// it landed. A preference, not an action — no turn is spent.
fn toggle_auto_pickup(world: &mut World) {
    let enabled = {
        let mut pickup = world.resource_mut::<AutoPickup>();
        pickup.enabled = !pickup.enabled;
        pickup.enabled
    };
    let state = if enabled {
        strings::toggle_on()
    } else {
        strings::toggle_off()
    };
    world
        .resource_mut::<GameLog>()
        .add(strings::auto_pickup_state(state));
}

/// `f`: fire the wielded launcher's first matching projectile — a shortcut for
/// the pack's `i` → item → Throw path when what's in hand is a bow or
/// crossbow. Opens the aiming reticle exactly as that path does, pre-loaded
/// with the first arrow (or quarrel) in the pack; Enter/Space looses it via
/// the ordinary [`fire_at_target`]. A [`Returns`] weapon in hand is thrown
/// instead ([`aim_returner`]).
fn begin_fire(world: &mut World) -> std::io::Result<bool> {
    let player = player_entity(world);
    if let Some(weapon) = wielded_returner(world, player) {
        return aim_returner(world, player, weapon);
    }
    if wielded_launcher(world, player).is_none() {
        world
            .resource_mut::<GameLog>()
            .add(strings::not_wielding_launcher());
        return Ok(false);
    }
    let Some(item) = first_matching_ammo(world, player) else {
        let noun = ammo_noun(world, player);
        world
            .resource_mut::<GameLog>()
            .add(strings::no_ammo_to_fire(&noun));
        return Ok(false);
    };
    if let Some(refusal) = throw_refusal(world, player, item) {
        world.resource_mut::<GameLog>().add(refusal);
        return Ok(false);
    }
    open_reticle(world, player, item, true);
    Ok(false)
}

/// `f` or `v` with a [`Returns`] weapon (a boomerang, a moon blade) in hand:
/// the reticle the pack's Throw opens, loaded with the weapon itself. Refuses,
/// no turn spent, when it can't leave the hand ([`throw_refusal`]).
fn aim_returner(world: &mut World, player: Entity, weapon: Entity) -> std::io::Result<bool> {
    if let Some(refusal) = throw_refusal(world, player, weapon) {
        world.resource_mut::<GameLog>().add(refusal);
        return Ok(false);
    }
    open_reticle(world, player, weapon, true);
    Ok(false)
}

/// `v`: a reach weapon's own strike — a bardiche, a whip. Opens the aiming
/// reticle out to the wielded weapon's own [`Reach`], pre-loaded with the
/// weapon itself so [`fire_at_target`] knows to resolve a strike in place
/// rather than a throw or a use. A [`Returns`] weapon in hand is thrown
/// instead ([`aim_returner`]).
fn begin_reach_attack(world: &mut World) -> std::io::Result<bool> {
    let player = player_entity(world);
    if let Some(weapon) = wielded_returner(world, player) {
        return aim_returner(world, player, weapon);
    }
    let Some(weapon) = wielded_reach_weapon(world, player) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::not_wielding_reach_weapon());
        return Ok(false);
    };
    open_reticle_for(world, player, Some(weapon), None, false, false, true);
    Ok(false)
}

/// `o`: arm auto-explore, unless confused, a monster is in view, or the map is
/// already fully known.
fn begin_auto_explore(world: &mut World) -> std::io::Result<bool> {
    if player_confused(world) {
        world
            .resource_mut::<GameLog>()
            .add(strings::too_confused_right_now());
        return Ok(false);
    }
    if monster_in_sight(world) {
        world
            .resource_mut::<GameLog>()
            .add(strings::not_while_monster_in_sight());
        return Ok(false);
    }
    if explore_step(world).is_none() {
        world
            .resource_mut::<GameLog>()
            .add(strings::nothing_left_to_explore());
        return Ok(false);
    }
    world.resource_mut::<GameLog>().unread.clear();
    world.resource_mut::<AutoExplore>().start(None);
    Ok(false)
}

/// `O`: drop the blinking travel cursor on the player's own tile to steer it.
fn open_travel_cursor(world: &mut World) {
    let pos = {
        let mut q = world.query_filtered::<&Position, With<Player>>();
        q.iter(world).next().copied()
    };
    let Some(pos) = pos else {
        return;
    };
    world.resource_mut::<GameLog>().unread.clear();
    world.resource_mut::<TravelCursor>().open(pos.x, pos.y);
}

/// Handles `>` / `<`. Standing on the matching staircase, it uses it. Otherwise,
/// if that staircase has already been discovered, the coast is clear and a route
/// to it exists, it starts an auto-walk there; if not, it just prints the usual
/// "you cannot go that way" line. Never consumes a turn itself.
fn travel_or_use_stairs(world: &mut World, going_down: bool) -> bool {
    let dir = if going_down {
        strings::dir_down()
    } else {
        strings::dir_up()
    };
    let want_tile = if going_down {
        TileType::Downstairs
    } else {
        TileType::Upstairs
    };

    let ppos = {
        let mut q = world.query_filtered::<&Position, With<Player>>();
        q.iter(world).next().copied()
    };
    let Some(ppos) = ppos else { return false };

    if world.resource::<Map>().tile(ppos.x, ppos.y) == want_tile {
        world.resource_mut::<GameLog>().unread.clear();
        return models::queue_stairs(world, going_down);
    }

    // Otherwise, offer to walk there — but only if we know where it is.
    let known_target = stair_location(world.resource::<Map>(), going_down).filter(|&(tx, ty)| {
        let mut q = world.query_filtered::<&Viewshed, With<Player>>();
        q.iter(world)
            .next()
            .is_some_and(|v| v.revealed_tiles.contains(tile_index(tx, ty)))
    });
    let Some(target) = known_target else {
        world
            .resource_mut::<GameLog>()
            .add(strings::cannot_go_direction(dir));
        return false;
    };

    if monster_in_sight(world) {
        world
            .resource_mut::<GameLog>()
            .add(strings::not_while_monster_in_sight());
        return false;
    }
    if travel_step(world, target).is_none() {
        world
            .resource_mut::<GameLog>()
            .add(strings::cant_find_path_to_stairs(dir));
        return false;
    }

    world.resource_mut::<GameLog>().unread.clear();
    world.resource_mut::<AutoExplore>().start(Some(target));
    false
}

/// One tick of an auto-walk, called by the main loop in place of
/// [`process_input_and_update`] while [`AutoExplore::active`] is set. Returns
/// whether a turn was consumed.
///
/// The walk is halted — the flag cleared — the instant anything worth the
/// player's attention happens: a key is pressed, a message was logged on the
/// previous turn (a monster spotted, a hit taken, an item picked up), a monster
/// is in plain sight, or there is nowhere left to go.
pub fn auto_explore_step(world: &mut World) -> std::io::Result<bool> {
    let travelling = world.resource::<AutoExplore>().target.is_some();

    if poll(Duration::from_millis(0))? {
        let _ = read()?;
        world.resource_mut::<AutoExplore>().stop();
        world.resource_mut::<GameLog>().add(if travelling {
            strings::travel_interrupted()
        } else {
            strings::auto_explore_interrupted()
        });
        return Ok(false);
    }

    if let Some(target) = world.resource::<AutoExplore>().target {
        let arrived = {
            let mut q = world.query_filtered::<&Position, With<Player>>();
            q.iter(world).next().is_some_and(|p| (p.x, p.y) == target)
        };
        if arrived {
            world.resource_mut::<AutoExplore>().stop();
            let msg = match world.resource::<Map>().tile(target.0, target.1) {
                TileType::Upstairs | TileType::Downstairs => strings::arrive_at_staircase(),
                _ => strings::you_stop(),
            };
            world.resource_mut::<GameLog>().add(msg);
            return Ok(false);
        }
    }

    if !world.resource::<GameLog>().unread.is_empty() {
        world.resource_mut::<AutoExplore>().stop();
        return Ok(false);
    }

    if monster_in_sight(world) {
        world.resource_mut::<AutoExplore>().stop();
        world
            .resource_mut::<GameLog>()
            .add(strings::monster_nearby());
        return Ok(false);
    }

    {
        let mut auto = world.resource_mut::<AutoExplore>();
        auto.steps += 1;
        if auto.steps > AUTO_EXPLORE_STEP_CAP {
            auto.stop();
            return Ok(false);
        }
    }

    let next = match world.resource::<AutoExplore>().target {
        Some(target) => travel_step(world, target),
        None => explore_step(world),
    };
    let Some((dx, dy)) = next else {
        world.resource_mut::<AutoExplore>().stop();
        world.resource_mut::<GameLog>().add(if travelling {
            strings::cant_find_path_there()
        } else {
            strings::explored_everywhere()
        });
        return Ok(false);
    };

    let moved = models::queue_step(world, dx, dy);
    if !moved {
        world.resource_mut::<AutoExplore>().stop();
    }
    Ok(moved)
}

/// One tick of the `O` travel cursor, called by the main loop in place of
/// [`process_input_and_update`] while [`TravelCursor::active`] is set.
///
/// Blocks up to one blink interval for a keypress. Arrow / vi / numpad keys walk
/// the cursor over revealed ground — sliding along a single axis when the
/// diagonal tile is still unseen — Enter or Space commits the destination to an
/// auto-travel, and Esc or `O` cancels. A bare timeout just flips the
/// highlight's blink phase and repaints.
pub fn travel_cursor_step(world: &mut World) -> std::io::Result<()> {
    if !poll(Duration::from_millis(TRAVEL_BLINK_MS))? {
        let mut tc = world.resource_mut::<TravelCursor>();
        tc.blink_on = !tc.blink_on;
        return Ok(());
    }

    let Event::Key(key) = read()? else {
        return Ok(());
    };
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }

    let (mut dx, mut dy) = (0i32, 0i32);
    match key.code {
        KeyCode::Esc | KeyCode::Char('O') | KeyCode::Char('x') | KeyCode::Char('X') => {
            world.resource_mut::<TravelCursor>().close();
            return Ok(());
        }
        KeyCode::Enter | KeyCode::Char(' ') => return confirm_travel_cursor(world),
        KeyCode::Char('k') | KeyCode::Up | KeyCode::Char('8') => dy = -1,
        KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('2') => dy = 1,
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('4') => dx = -1,
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('6') => dx = 1,
        KeyCode::Char('y') | KeyCode::Char('7') => {
            dx = -1;
            dy = -1;
        }
        KeyCode::Char('u') | KeyCode::Char('9') => {
            dx = 1;
            dy = -1;
        }
        KeyCode::Char('b') | KeyCode::Char('1') => {
            dx = -1;
            dy = 1;
        }
        KeyCode::Char('n') | KeyCode::Char('3') => {
            dx = 1;
            dy = 1;
        }
        _ => return Ok(()),
    }

    let (cx, cy) = {
        let tc = world.resource::<TravelCursor>();
        (tc.x as i32, tc.y as i32)
    };
    for (nx, ny) in [(cx + dx, cy + dy), (cx + dx, cy), (cx, cy + dy)] {
        if nx < 0 || ny < 0 || (nx == cx && ny == cy) {
            continue;
        }
        let (nx, ny) = (nx as u16, ny as u16);
        if tile_is_revealed(world, nx, ny) {
            let mut tc = world.resource_mut::<TravelCursor>();
            tc.x = nx;
            tc.y = ny;
            tc.blink_on = true;
            break;
        }
    }
    Ok(())
}

/// Commit the travel cursor's tile: with the coast clear, start an auto-travel
/// to it — or, if it's a wall or unreachable, to the nearest walkable tile the
/// player can reach. Always closes the cursor; never consumes a turn itself.
fn confirm_travel_cursor(world: &mut World) -> std::io::Result<()> {
    let (tx, ty) = {
        let tc = world.resource::<TravelCursor>();
        (tc.x, tc.y)
    };
    world.resource_mut::<TravelCursor>().close();

    if monster_in_sight(world) {
        world
            .resource_mut::<GameLog>()
            .add(strings::not_while_monster_in_sight());
        return Ok(());
    }

    let Some(goal) = nearest_reachable(world, (tx, ty)) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::cant_find_path_there());
        return Ok(());
    };

    let here = {
        let mut q = world.query_filtered::<&Position, With<Player>>();
        q.iter(world).next().map(|p| (p.x, p.y))
    };
    if here == Some(goal) {
        world
            .resource_mut::<GameLog>()
            .add(strings::already_there());
        return Ok(());
    }

    world.resource_mut::<GameLog>().unread.clear();
    world.resource_mut::<AutoExplore>().start(Some(goal));
    Ok(())
}

/// Run an entire NetHack-style fast move to completion, then hand control back.
/// Called by the main loop in place of [`process_input_and_update`] while
/// [`FastMove::active`] is set.
///
/// The screen is deliberately left untouched until this returns — the whole run
/// reads as a single jump. Every step still advances the world by a full turn
/// (`schedule.run`), so running costs exactly as many turns as walking.
///
/// The run halts the instant anything wants the player's attention: a key is
/// pressed, a creature is (or comes) in view, a message is logged (something
/// spotted, an item picked up, a hit taken), the beeline reaches its target, a
/// straight run meets a door / staircase / corridor branch or a wall, or the
/// step cap trips.
pub fn fast_move_run(world: &mut World, schedule: &mut Schedule) -> std::io::Result<()> {
    loop {
        if poll(Duration::from_millis(0))? {
            let _ = read()?;
            break;
        }

        {
            let mut fm = world.resource_mut::<FastMove>();
            fm.steps += 1;
            if fm.steps > FAST_MOVE_STEP_CAP {
                break;
            }
        }

        if monster_in_sight(world) {
            break;
        }

        let next = match world.resource::<FastMove>().target {
            Some(target) => travel_step(world, target),
            None => straight_step(world),
        };
        let Some((dx, dy)) = next else { break };

        if !models::queue_step(world, dx, dy) {
            break;
        }

        schedule.run(world);

        if world.resource::<Ending>().player_dead {
            break;
        }
        if monster_in_sight(world) || !world.resource::<GameLog>().unread.is_empty() {
            break;
        }

        let target = world.resource::<FastMove>().target;
        if fast_move_done(world, target) {
            break;
        }
    }

    world.resource_mut::<FastMove>().stop();
    Ok(())
}

/// Whether the fast move should stop before its next step: a beeline that has
/// reached its target, or a straight run that has met a junction.
fn fast_move_done(world: &mut World, target: Option<(u16, u16)>) -> bool {
    let Some(target) = target else {
        return straight_stop_here(world);
    };
    let mut q = world.query_filtered::<&Position, With<Player>>();
    q.iter(world).next().map(|p| (p.x, p.y)) == Some(target)
}

#[cfg(test)]
mod tests {
    /// Plans a step, then lets the schedule's first step apply it, the way
    /// a real turn does.
    fn step(w: &mut World, dx: i16, dy: i16) -> bool {
        w.init_resource::<PlayerActionQueue>();
        let spent = models::queue_step(w, dx, dy);
        models::player_action_system(w);
        spent
    }

    use super::*;
    use crossterm::event::KeyEvent;

    fn test_world(seed: u64) -> World {
        let mut w = World::new();
        w.insert_resource(GameRng(models::ChaCha12Rng::seed_from_u64(seed)));
        w.insert_resource(RngSeed(seed));
        w.init_resource::<GameLog>();
        w.insert_resource(PlayerName {
            what: "TESTER".into(),
        });
        initialize_world(&mut w);
        w.init_resource::<PlayerActionQueue>();
        w
    }

    /// The resources the modal stack reads, on top of a built world. `main.rs`
    /// inserts these at startup; a test world has to do it too.
    fn modal_world(seed: u64) -> World {
        let mut w = test_world(seed);
        w.init_resource::<PackIsOpen>();
        w.init_resource::<QuitPrompt>();
        w.init_resource::<HelpMenu>();
        w.init_resource::<CommandBar>();
        w.init_resource::<AutoExplore>();
        w.init_resource::<AutoPickup>();
        w.init_resource::<FastMove>();
        w.init_resource::<TravelCursor>();
        w.init_resource::<UseQueue>();
        w.init_resource::<ThrowQueue>();
        w.init_resource::<AttackQueue>();
        w.init_resource::<SpellQueue>();
        w.init_resource::<SpellsMenu>();
        w.init_resource::<OfferMenu>();
        w.init_resource::<BarterMenu>();
        w.insert_resource(TargetingState {
            active: false,
            item: None,
            throwing: false,
            spell_effect: None,
            looking: false,
            reach_attack: false,
            cursor_x: 0,
            cursor_y: 0,
        });
        w
    }

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    /// Queue enough unread messages that `log_view` has to raise `--MORE--`.
    fn flood_the_log(w: &mut World) -> usize {
        let mut log = w.resource_mut::<GameLog>();
        log.unread.clear();
        for i in 0..12 {
            log.unread.push(LogEntry::plain(format!(
                "Message number {i} is a fairly long one."
            )));
        }
        let unread = w.resource::<GameLog>().unread.len();
        assert!(
            log_view(&w.resource::<GameLog>().unread, LOG_WIDTH).2,
            "the fixture did not actually raise a --MORE-- prompt"
        );
        unread
    }

    fn player_pos(w: &mut World) -> Position {
        let mut q = w.query_filtered::<&Position, With<Player>>();
        *q.iter(w).next().unwrap()
    }

    /// Deep water is a wall to feet that cannot swim and floor to feet that
    /// can — the player's own step included, whatever body they are wearing.
    #[test]
    fn only_a_swimming_player_steps_into_deep_water() {
        for swims in [false, true] {
            let mut w = test_world(7);
            let player = player_entity(&mut w);
            let here = player_pos(&mut w);
            let east = Position {
                x: here.x + 1,
                y: here.y,
            };
            w.resource_mut::<Map>().tiles[tile_index(east.x, east.y)] = TileType::Water;
            if swims {
                lend(&mut w, player, Grant::of::<Swims>(), Lifetime::Permanent);
            }
            step(&mut w, 1, 0);
            assert_eq!(
                player_pos(&mut w) == east,
                swims,
                "swims = {swims}: the step into the water went the wrong way"
            );
        }
    }

    /// Tab-fire was the one aiming path that never checked its reach: the
    /// cursor UI clamps every other one. The floor's own monsters are cleared
    /// out first, because `auto_fight_target` picks the lowest-HP foe in sight
    /// and any of them would outrank a planted dummy — leaving the dummy the
    /// provable target rather than a seed's good luck.
    #[test]
    fn tab_with_a_launcher_refuses_targets_outside_throw_range() {
        let mut w = test_world(2100);
        let player = player_entity(&mut w);
        let bow = spawn_launcher(&mut w, "short bow", Position { x: 0, y: 0 });
        let arrow = spawn_ammo(&mut w, "arrow", Position { x: 0, y: 0 });
        {
            let mut backpack = w.get_mut::<Backpack>(player).unwrap();
            backpack.items.push(arrow);
        }
        toggle_equipped(&mut w, player, bow);

        let floor_mobs: Vec<Entity> = w
            .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
            .iter(&w)
            .collect();
        for mob in floor_mobs {
            w.despawn(mob);
        }

        let start = player_pos(&mut w);
        let dummy = w
            .spawn((
                Name {
                    what: "dummy".into(),
                },
                Mob {
                    movement_type: MovementType::Static,
                },
                Position {
                    x: (start.x as i32 + LAUNCHER_RANGE + 1) as u16,
                    y: start.y,
                },
                Fighter {
                    hp: 5,
                    max_hp: 5,
                    armor: 0,
                    power: 1,
                    max_power: 1,
                    armor_bonus: 0,
                    power_bonus: 0,
                },
                Faction::Monster,
                Blood,
            ))
            .id();
        assert_eq!(
            models::autofight::auto_fight_target(&mut w),
            Some(dummy),
            "the fixture is not aiming at the dummy it planted"
        );

        w.resource_mut::<GameLog>().unread.clear();
        let turn = ranged_auto_fight(&mut w, player);
        assert!(!turn, "out-of-range auto-fire still spent a turn");
        assert!(
            w.resource::<GameLog>()
                .unread
                .iter()
                .any(|line| line == "Out of range."),
            "auto-fire did not reject an out-of-range target"
        );
    }

    /// A wielded boomerang, with nothing else on the floor to fight.
    fn wield_a_boomerang(w: &mut World) -> (Entity, Entity) {
        let player = player_entity(w);
        let boomerang = spawn_weapon(w, "boomerang", Position { x: 0, y: 0 });
        w.entity_mut(boomerang).remove::<Position>();
        w.get_mut::<Backpack>(player).unwrap().items.push(boomerang);
        toggle_equipped(w, player, boomerang);
        let floor_mobs: Vec<Entity> = w
            .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
            .iter(w)
            .collect();
        for mob in floor_mobs {
            w.despawn(mob);
        }
        (player, boomerang)
    }

    #[test]
    fn f_and_v_aim_a_wielded_returning_weapon_as_a_throw() {
        for begin in [begin_fire, begin_reach_attack] {
            let mut w = modal_world(2101);
            let (_, boomerang) = wield_a_boomerang(&mut w);
            begin(&mut w).unwrap();
            let t = w.resource::<TargetingState>();
            assert!(t.active, "no reticle opened");
            assert_eq!(t.item, Some(boomerang));
            assert!(t.throwing && !t.reach_attack, "aimed as a throw");
        }
    }

    #[test]
    fn tab_throws_a_wielded_returning_weapon_at_the_target() {
        let mut w = modal_world(2102);
        let (_, boomerang) = wield_a_boomerang(&mut w);
        let start = player_pos(&mut w);
        for dx in 1..=2 {
            w.resource_mut::<Map>().tiles[tile_index(start.x + dx, start.y)] = TileType::Room;
        }
        let at = Position {
            x: start.x + 2,
            y: start.y,
        };
        w.spawn((
            Name {
                what: "dummy".into(),
            },
            Mob {
                movement_type: MovementType::Static,
            },
            at,
            Fighter {
                hp: 5,
                max_hp: 5,
                armor: 0,
                power: 1,
                max_power: 1,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
            Blood,
        ));

        assert!(auto_fight_turn(&mut w), "the throw spends the turn");
        let throws = &w.resource::<ThrowQueue>().throws;
        assert_eq!(throws.len(), 1, "Tab threw rather than walked");
        assert_eq!(throws[0].item, boomerang);
        assert_eq!(throws[0].target, at);
    }

    #[test]
    fn numpad_digits_move_the_same_as_their_vi_key_equivalents() {
        for (digit, letter) in [
            ('8', 'k'),
            ('2', 'j'),
            ('4', 'h'),
            ('6', 'l'),
            ('7', 'y'),
            ('9', 'u'),
            ('1', 'b'),
            ('3', 'n'),
        ] {
            let mut w = test_world(2112);
            handle_movement_input(
                &mut w,
                KeyEvent::new(KeyCode::Char(digit), KeyModifiers::NONE),
            )
            .unwrap();
            models::player_action_system(&mut w);
            let after_digit = player_pos(&mut w);

            let mut w2 = test_world(2112);
            handle_movement_input(
                &mut w2,
                KeyEvent::new(KeyCode::Char(letter), KeyModifiers::NONE),
            )
            .unwrap();
            models::player_action_system(&mut w2);
            let after_letter = player_pos(&mut w2);

            assert_eq!(
                (after_digit.x, after_digit.y),
                (after_letter.x, after_letter.y),
                "numpad {digit} should move exactly like {letter}"
            );
        }
    }

    #[test]
    fn shift_numpad_direction_reads_as_a_run_direction_like_shift_arrows() {
        assert_eq!(
            run_direction(KeyCode::Char('8'), KeyModifiers::SHIFT),
            run_direction(KeyCode::Up, KeyModifiers::SHIFT)
        );
        assert_eq!(
            run_direction(KeyCode::Char('7'), KeyModifiers::SHIFT),
            Some((-1, -1))
        );
        assert_eq!(run_direction(KeyCode::Char('8'), KeyModifiers::NONE), None);
    }

    // -----------------------------------------------------------------------
    // The --MORE-- gate
    // -----------------------------------------------------------------------

    #[test]
    fn a_more_prompt_accepts_the_acknowledgement_and_swallows_everything_else() {
        for key in ['j', 'i', 'Q', 'a', '>'] {
            let mut w = modal_world(3);
            let queued = flood_the_log(&mut w);
            let turn = dispatch_key(&mut w, press(key)).unwrap();
            assert!(!turn, "'{key}' spent a turn through a --MORE-- prompt");
            assert_eq!(
                w.resource::<GameLog>().unread.len(),
                queued,
                "'{key}' dropped a message the player had not acknowledged"
            );
        }
    }

    #[test]
    fn acknowledging_drops_exactly_the_messages_that_were_on_screen() {
        for ack in [KeyCode::Char(' '), KeyCode::Enter] {
            let mut w = modal_world(4);
            let queued = flood_the_log(&mut w);
            let shown = log_view(&w.resource::<GameLog>().unread, LOG_WIDTH).1;
            assert!(shown > 0 && shown < queued, "the fixture proves nothing");

            let turn = dispatch_key(&mut w, KeyEvent::new(ack, KeyModifiers::NONE)).unwrap();
            assert!(!turn, "acknowledging a prompt is not a turn");
            assert_eq!(w.resource::<GameLog>().unread.len(), queued - shown);
        }
    }

    #[test]
    fn the_gate_outranks_the_escape_hatch() {
        let mut w = modal_world(5);
        w.resource_mut::<PackIsOpen>().open_at(PackMode::Browse, 0);
        let queued = flood_the_log(&mut w);

        dispatch_key(&mut w, press('x')).unwrap();
        assert!(
            w.resource::<PackIsOpen>().open,
            "the pack closed behind the prompt"
        );
        assert_eq!(w.resource::<GameLog>().unread.len(), queued);
    }

    #[test]
    fn an_open_barter_takes_the_keys_ahead_of_a_pending_more() {
        let mut w = modal_world(10);
        let player = player_entity(&mut w);
        let item = w.get::<Backpack>(player).unwrap().items[0];
        {
            let mut menu = w.resource_mut::<BarterMenu>();
            menu.open = true;
            menu.player_side = vec![Tradeable::Item(item)];
        }
        let queued = flood_the_log(&mut w);

        dispatch_key(&mut w, press(' ')).unwrap();
        assert_eq!(
            w.resource::<BarterMenu>().player_selected,
            vec![Tradeable::Item(item)],
            "Space went to the --MORE-- prompt, not the barter"
        );
        assert_eq!(
            w.resource::<GameLog>().unread.len(),
            queued,
            "the barter's Space also acknowledged the prompt"
        );
    }

    #[test]
    fn an_open_offer_takes_the_keys_ahead_of_a_pending_more() {
        let mut w = modal_world(11);
        {
            let mut menu = w.resource_mut::<OfferMenu>();
            menu.open = true;
            menu.options = vec![
                OfferOption::Spell(SpellEffect::DragonBreath),
                OfferOption::Spell(SpellEffect::DragonBreath),
            ];
        }
        let queued = flood_the_log(&mut w);

        dispatch_key(&mut w, press('j')).unwrap();
        assert_eq!(
            w.resource::<OfferMenu>().selected,
            1,
            "the cursor key went to the --MORE-- prompt, not the offer"
        );
        assert_eq!(w.resource::<GameLog>().unread.len(), queued);
    }

    // -----------------------------------------------------------------------
    // The x / X escape hatch
    // -----------------------------------------------------------------------

    #[test]
    fn x_and_shift_x_close_whichever_modal_is_open() {
        for key in ['x', 'X'] {
            let mut w = modal_world(6);
            w.resource_mut::<PackIsOpen>().open_at(PackMode::Browse, 0);
            assert!(
                !dispatch_key(&mut w, press(key)).unwrap(),
                "escaping is not a turn"
            );
            assert!(
                !w.resource::<PackIsOpen>().open,
                "'{key}' left the pack open"
            );

            let mut w = modal_world(6);
            w.resource_mut::<TargetingState>().active = true;
            dispatch_key(&mut w, press(key)).unwrap();
            assert!(
                !w.resource::<TargetingState>().active,
                "'{key}' left the reticle up"
            );

            let mut w = modal_world(6);
            w.resource_mut::<QuitPrompt>().open = true;
            dispatch_key(&mut w, press(key)).unwrap();
            assert!(
                !w.resource::<QuitPrompt>().open,
                "'{key}' left the prompt up"
            );
            assert!(
                w.resource::<GameState>().is_running,
                "'{key}' answered the prompt instead of closing it"
            );
        }
    }

    fn stop_time(w: &mut World) {
        let player = player_entity(w);
        lend(w, player, Grant::of::<TimeStopped>(), Lifetime::Floor);
    }

    #[test]
    fn quitting_with_time_stopped_takes_a_second_yes() {
        let mut w = modal_world(10);
        stop_time(&mut w);
        dispatch_key(&mut w, press('Q')).unwrap();
        dispatch_key(&mut w, press('y')).unwrap();
        assert!(w.resource::<GameState>().is_running, "one yes quit");
        assert!(w.resource::<QuitPrompt>().warned, "no warning shown");
        dispatch_key(&mut w, press('y')).unwrap();
        assert!(!w.resource::<GameState>().is_running);
    }

    #[test]
    fn a_no_to_the_time_stopped_warning_starts_the_ask_over() {
        let mut w = modal_world(11);
        stop_time(&mut w);
        for key in ['Q', 'y', 'n', 'Q', 'y'] {
            dispatch_key(&mut w, press(key)).unwrap();
        }
        assert!(w.resource::<GameState>().is_running);
        assert!(w.resource::<QuitPrompt>().warned);
    }

    #[test]
    fn quitting_with_time_running_takes_one_yes() {
        let mut w = modal_world(12);
        dispatch_key(&mut w, press('Q')).unwrap();
        dispatch_key(&mut w, press('y')).unwrap();
        assert!(!w.resource::<GameState>().is_running);
    }

    #[test]
    fn the_run_saves_as_time_stops_and_as_it_starts_again_and_never_between() {
        let mut w = modal_world(13);
        let path = std::env::temp_dir().join(format!("nihilurk-wrld-{}.sav", std::process::id()));
        let mut stopped = false;
        let saved = |w: &mut World, stopped: &mut bool| {
            let _ = std::fs::remove_file(&path);
            save_on_time_edge(w, stopped, &path);
            std::fs::remove_file(&path).is_ok()
        };

        assert!(!saved(&mut w, &mut stopped), "time running: no save");
        stop_time(&mut w);
        assert!(saved(&mut w, &mut stopped), "THE WORLD drawn: save");
        assert!(!saved(&mut w, &mut stopped), "time stopped: no save");
        let player = player_entity(&mut w);
        revoke(&mut w, player, Grant::of::<TimeStopped>());
        assert!(saved(&mut w, &mut stopped), "THE WORLD over: save");
        assert!(!saved(&mut w, &mut stopped));
    }

    #[test]
    fn shift_x_escapes_a_modal_rather_than_asking_about_quitting() {
        let mut w = modal_world(7);
        w.resource_mut::<PackIsOpen>().open_at(PackMode::Browse, 0);
        dispatch_key(&mut w, press('X')).unwrap();
        assert!(!w.resource::<PackIsOpen>().open);
        assert!(
            !w.resource::<QuitPrompt>().open,
            "X raised the quit prompt with the pack open"
        );
    }

    #[test]
    fn shift_x_falls_through_to_the_quit_prompt_with_nothing_open() {
        let mut w = modal_world(8);
        dispatch_key(&mut w, press('X')).unwrap();
        assert!(
            w.resource::<QuitPrompt>().open,
            "X on the bare map did nothing"
        );
    }

    #[test]
    fn lowercase_x_on_the_bare_map_is_not_swallowed() {
        let mut w = modal_world(9);
        assert!(
            !close_all_modals(&mut w),
            "nothing was open, so nothing closed"
        );
        let turn = dispatch_key(&mut w, press('x')).unwrap();
        assert!(!turn);
    }

    #[test]
    fn closing_everything_at_once_leaves_no_modal_behind() {
        let mut w = modal_world(10);
        w.resource_mut::<PackIsOpen>().open_at(PackMode::Browse, 0);
        w.resource_mut::<TargetingState>().active = true;
        w.resource_mut::<QuitPrompt>().open = true;

        assert!(close_all_modals(&mut w));
        assert!(!w.resource::<PackIsOpen>().open);
        assert!(!w.resource::<TargetingState>().active);
        assert!(w.resource::<TargetingState>().item.is_none());
        assert!(!w.resource::<QuitPrompt>().open);
    }

    // -----------------------------------------------------------------------
    // The quit prompt
    // -----------------------------------------------------------------------

    #[test]
    fn the_quit_prompt_answers_yes_and_no_and_guesses_at_nothing() {
        for yes in ['y', 'Y'] {
            let mut w = modal_world(11);
            w.resource_mut::<QuitPrompt>().open = true;
            dispatch_key(&mut w, press(yes)).unwrap();
            assert!(
                !w.resource::<GameState>().is_running,
                "'{yes}' did not quit"
            );
        }
        for no in ['n', 'N'] {
            let mut w = modal_world(11);
            w.resource_mut::<QuitPrompt>().open = true;
            dispatch_key(&mut w, press(no)).unwrap();
            assert!(
                !w.resource::<QuitPrompt>().open,
                "'{no}' left the prompt up"
            );
            assert!(w.resource::<GameState>().is_running);
        }
        for other in ['j', 'q', 'i', ' '] {
            let mut w = modal_world(11);
            w.resource_mut::<QuitPrompt>().open = true;
            dispatch_key(&mut w, press(other)).unwrap();
            assert!(
                w.resource::<QuitPrompt>().open,
                "'{other}' answered a question it was not asked"
            );
            assert!(w.resource::<GameState>().is_running);
        }
    }

    #[test]
    fn the_quit_prompt_outranks_every_other_context() {
        let mut w = modal_world(12);
        let before = *w.query_filtered::<&Position, With<Player>>().single(&w);
        w.resource_mut::<QuitPrompt>().open = true;
        dispatch_key(&mut w, press('j')).unwrap();
        let after = *w.query_filtered::<&Position, With<Player>>().single(&w);
        assert_eq!((before.x, before.y), (after.x, after.y), "the player moved");
        assert!(w.resource::<QuitPrompt>().open);
    }

    #[test]
    fn escape_closes_the_quit_prompt_but_never_raises_one() {
        let mut w = modal_world(13);
        w.resource_mut::<QuitPrompt>().open = true;
        dispatch_key(&mut w, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
        assert!(!w.resource::<QuitPrompt>().open);

        let mut w = modal_world(13);
        dispatch_key(&mut w, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
        assert!(
            !w.resource::<QuitPrompt>().open,
            "Esc raised the quit prompt"
        );
        assert!(w.resource::<GameState>().is_running);
    }

    // -----------------------------------------------------------------------
    // Keys that used to shadow each other
    // -----------------------------------------------------------------------

    /// A player with `slots` spells and the magic to pay for all of them.
    fn spells_world(seed: u64, slots: &[SpellEffect]) -> World {
        let mut w = modal_world(seed);
        let player = player_entity(&mut w);
        w.entity_mut(player).insert(Spellset {
            slots: slots.to_vec(),
        });
        w.entity_mut(player).insert(Magic {
            points: 250,
            max_points: 250,
        });
        w
    }

    /// The four moves the shortcut tests fire, all of them aimed ones, so
    /// firing a slot is visible as the reticle it opens.
    const FOUR_SPELLS: [SpellEffect; 4] = [
        SpellEffect::DragonBreath,
        SpellEffect::Sting,
        SpellEffect::Thunderbolt,
        SpellEffect::ForceLance,
    ];

    #[test]
    fn semicolon_looks_and_shift_l_still_runs() {
        let mut w = modal_world(21);
        dispatch_key(&mut w, press(';')).unwrap();
        let ts = w.resource::<TargetingState>();
        assert!(ts.active && ts.looking, "`;` did not open look mode");

        let mut w = modal_world(21);
        dispatch_key(&mut w, press('L')).unwrap();
        assert!(
            !w.resource::<TargetingState>().looking,
            "`L` opened look mode instead of running east"
        );
    }

    #[test]
    fn the_moves_menu_picks_a_slot_by_letter() {
        let mut w = spells_world(22, &FOUR_SPELLS);
        dispatch_key(&mut w, press('Z')).unwrap();
        assert!(w.resource::<SpellsMenu>().open);

        dispatch_key(&mut w, press('c')).unwrap();
        assert!(!w.resource::<SpellsMenu>().open, "the menu stayed open");
        assert_eq!(
            w.resource::<TargetingState>().spell_effect,
            Some(FOUR_SPELLS[2]),
            "`c` did not fire the third slot"
        );
    }

    #[test]
    fn a_letter_past_the_last_slot_does_nothing_in_the_moves_menu() {
        let mut w = spells_world(23, &FOUR_SPELLS[..2]);
        dispatch_key(&mut w, press('Z')).unwrap();
        dispatch_key(&mut w, press('d')).unwrap();
        assert!(
            w.resource::<SpellsMenu>().open,
            "`d` closed a menu that has no fourth row"
        );
        assert!(w.resource::<TargetingState>().spell_effect.is_none());
    }

    #[test]
    fn digits_navigate_the_moves_menu_and_never_fire_a_slot() {
        let mut w = spells_world(24, &FOUR_SPELLS);
        dispatch_key(&mut w, press('Z')).unwrap();
        dispatch_key(&mut w, press('2')).unwrap();
        assert!(w.resource::<SpellsMenu>().open, "`2` closed the menu");
        assert!(
            w.resource::<TargetingState>().spell_effect.is_none(),
            "`2` fired a spell"
        );
        assert_eq!(
            w.resource::<SpellsMenu>().selected,
            1,
            "`2` did not step the highlight down"
        );
    }

    #[test]
    fn alt_q_w_e_r_fire_spell_slots_one_to_four() {
        for (slot, letter) in ['q', 'w', 'e', 'r'].into_iter().enumerate() {
            let mut w = spells_world(27, &FOUR_SPELLS);
            dispatch_key(
                &mut w,
                KeyEvent::new(KeyCode::Char(letter), KeyModifiers::ALT),
            )
            .unwrap();
            assert_eq!(
                w.resource::<TargetingState>().spell_effect,
                Some(FOUR_SPELLS[slot]),
                "Alt+{letter} did not aim slot {slot}"
            );
            assert!(
                !w.resource::<PackIsOpen>().open,
                "Alt+{letter} also opened a pack"
            );
        }
    }

    #[test]
    fn alt_on_an_empty_slot_says_so_and_does_not_fall_through_to_the_bare_letter() {
        let mut w = spells_world(27, &FOUR_SPELLS[..1]);
        dispatch_key(&mut w, KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT)).unwrap();
        assert!(w.resource::<TargetingState>().spell_effect.is_none());
        assert!(
            !w.resource::<PackIsOpen>().open,
            "Alt+w fell through to wield"
        );
    }

    #[test]
    fn alt_with_any_other_letter_still_does_what_the_bare_letter_does() {
        let mut w = spells_world(27, &FOUR_SPELLS);
        dispatch_key(&mut w, KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT)).unwrap();
        assert!(
            w.resource::<PackIsOpen>().open,
            "Alt+a did not fall through to use"
        );
    }

    #[test]
    fn ctrl_held_letters_do_nothing_but_ctrl_c_quits() {
        for letter in ['z', 'j', 'l', 'q', 'a', 'o'] {
            let mut w = spells_world(29, &FOUR_SPELLS);
            let player = player_entity(&mut w);
            let before = w.get::<Position>(player).copied();
            let key = KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL);
            let spent = dispatch_key(&mut w, key).unwrap();
            assert!(!spent, "Ctrl+{letter} spent a turn");
            assert!(
                !w.resource::<PackIsOpen>().open,
                "Ctrl+{letter} opened a pack"
            );
            assert!(w.resource::<GameState>().is_running, "Ctrl+{letter} quit");
            assert_eq!(
                w.get::<Position>(player).copied(),
                before,
                "Ctrl+{letter} moved"
            );
        }
    }

    #[test]
    fn ctrl_c_quits_from_every_context_including_a_pending_more() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

        let mut w = spells_world(28, &FOUR_SPELLS);
        dispatch_key(&mut w, press('Z')).unwrap();
        dispatch_key(&mut w, ctrl_c).unwrap();
        assert!(
            !w.resource::<GameState>().is_running,
            "the spells menu swallowed Ctrl+C"
        );
        assert!(
            w.resource::<TargetingState>().spell_effect.is_none(),
            "Ctrl+C fired the spell in row `c`"
        );

        let mut w = modal_world(28);
        dispatch_key(&mut w, press('i')).unwrap();
        dispatch_key(&mut w, ctrl_c).unwrap();
        assert!(
            !w.resource::<GameState>().is_running,
            "the pack swallowed Ctrl+C"
        );

        let mut w = modal_world(28);
        flood_the_log(&mut w);
        dispatch_key(&mut w, ctrl_c).unwrap();
        assert!(
            !w.resource::<GameState>().is_running,
            "the --MORE-- gate swallowed Ctrl+C"
        );

        let mut w = modal_world(28);
        dispatch_key(&mut w, ctrl_c).unwrap();
        assert!(!w.resource::<GameState>().is_running);
    }

    /// F1 raises the key list and never spends a turn.
    #[test]
    fn f1_opens_the_help_and_spends_no_turn() {
        let mut w = modal_world(40);
        let spent = dispatch_key(&mut w, KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)).unwrap();
        assert!(!spent, "F1 spent a turn");
        assert!(w.resource::<HelpMenu>().open, "F1 left the help shut");
    }

    /// F2 hides the command bar, a second F2 brings it back; neither costs a turn.
    #[test]
    fn f2_toggles_the_command_bar_and_spends_no_turn() {
        let mut w = modal_world(40);
        let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE);
        for hidden in [true, false] {
            let spent = dispatch_key(&mut w, f2).unwrap();
            assert!(!spent, "F2 spent a turn");
            assert_eq!(w.resource::<CommandBar>().hidden, hidden);
        }
    }

    /// With the help up, the next key only closes it: `j` must not also walk
    /// south, and `x` (the escape hatch) shuts it like any other modal.
    #[test]
    fn a_key_closes_the_help_and_does_nothing_else() {
        for key in ['j', 'x', 'Q'] {
            let mut w = modal_world(41);
            let before = player_pos(&mut w);
            w.resource_mut::<HelpMenu>().open = true;
            let spent = dispatch_key(&mut w, press(key)).unwrap();
            assert!(!spent, "'{key}' spent a turn behind the help");
            assert!(!w.resource::<HelpMenu>().open, "'{key}' left the help up");
            assert_eq!(player_pos(&mut w), before, "'{key}' moved the player");
            assert!(
                !w.resource::<QuitPrompt>().open,
                "'{key}' reached the map under the help"
            );
        }
    }

    // -----------------------------------------------------------------------
    // The turn order
    // -----------------------------------------------------------------------

    /// A zap resolves against the dungeon the player aimed at, not the one the
    /// monsters have already walked through.
    ///
    /// `item_system` used to run *after* `ai`, so a wand that reads one exact
    /// tile — teleport, polymorph, haste, slow, cancellation — found that tile
    /// empty whenever the target took a step first, and reported that there
    /// was nothing there. The reticle had been on the monster the whole time.
    #[test]
    fn a_zap_lands_on_the_tile_the_player_aimed_at_not_the_one_the_target_left() {
        let mut w = modal_world(3);
        let player = player_entity(&mut w);
        let here = *w.get::<Position>(player).unwrap();

        let row: Vec<u16> = {
            let map = w.resource::<Map>();
            (1..=4)
                .map(|d| here.x + d)
                .take_while(|&x| !map.blocks(x, here.y))
                .collect()
        };
        assert!(row.len() >= 3, "need open floor for the walk");
        let mob = w
            .spawn((
                Name { what: "orc".into() },
                Mob {
                    movement_type: MovementType::Chase,
                },
                Position {
                    x: row[2],
                    y: here.y,
                },
                Fighter {
                    hp: 5,
                    max_hp: 5,
                    armor: 0,
                    power: 1,
                    max_power: 1,
                    armor_bonus: 0,
                    power_bonus: 0,
                },
                Faction::Monster,
                Blood,
                Speed::new(SpeedKind::Normal),
            ))
            .id();
        w.get_mut::<Viewshed>(player).unwrap().visible_tiles = row
            .iter()
            .map(|&x| (x, here.y))
            .chain([(here.x, here.y)])
            .collect();

        let aimed_at = *w.get::<Position>(mob).unwrap();
        let wand = models::spawn_wand(&mut w, WandEffect::SlowMonster, Position { x: 0, y: 0 });
        w.entity_mut(wand).remove::<Position>();
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user: player,
            item: wand,
            target: Some(aimed_at),
            slot_idx: None,
        });

        crate::turn_schedule().run(&mut w);

        assert_eq!(
            w.get::<Speed>(mob).unwrap().kind,
            SpeedKind::Slow,
            "the zap found the target on the tile it was aimed at"
        );
    }

    /// `visibility_system` queues `Hidden` for every mob out of sight. The
    /// trapdoor is a `&mut World` system that despawns the whole floor, so if it
    /// runs after `visibility_system` and before that system's commands apply,
    /// they land on dead entities and bevy panics (B0003). Run the real turn
    /// over many floors: the executor is free to pick either order unless the
    /// schedule pins one.
    #[test]
    fn a_trapdoor_fall_never_races_the_visibility_commands() {
        for seed in 0..60 {
            let mut w = modal_world(seed);
            let player = player_entity(&mut w);
            let here = *w.get::<Position>(player).unwrap();
            w.spawn(models::TrapBundle::trapdoor(here));
            models::mark_moved(&mut w, player);
            w.get_mut::<Viewshed>(player).unwrap().dirty = true;

            crate::turn_schedule().run(&mut w);

            assert_eq!(w.resource::<Depth>().what, 2, "seed {seed}: fell one floor");
        }
    }

    /// The reticle opens on the nearest thing in view, so the common case —
    /// one monster and one wand — is `z`, pick, `Enter`.
    #[test]
    fn the_reticle_opens_on_the_closest_visible_monster() {
        let mut w = modal_world(5);
        let player = player_entity(&mut w);
        let here = *w.get::<Position>(player).unwrap();
        let row: Vec<u16> = {
            let map = w.resource::<Map>();
            (1..=4)
                .map(|d| here.x + d)
                .take_while(|&x| !map.blocks(x, here.y))
                .collect()
        };
        assert!(row.len() >= 3, "need open floor beside the player");

        let near = Position {
            x: row[1],
            y: here.y,
        };
        let far = Position {
            x: row[2],
            y: here.y,
        };
        for at in [far, near] {
            w.spawn((
                Name { what: "orc".into() },
                Mob {
                    movement_type: MovementType::Static,
                },
                at,
                Faction::Monster,
            ));
        }
        w.get_mut::<Viewshed>(player).unwrap().visible_tiles =
            row.iter().map(|&x| (x, here.y)).collect();

        let wand = models::spawn_wand(&mut w, WandEffect::SlowMonster, Position { x: 0, y: 0 });
        w.entity_mut(wand).remove::<Position>();
        open_reticle(&mut w, player, wand, false);

        let ts = w.resource::<TargetingState>();
        assert_eq!(
            (ts.cursor_x, ts.cursor_y),
            (near.x as i16, near.y as i16),
            "the closer of the two, not the player's own tile"
        );
    }

    /// Looking at bare floor says nothing: the reticle already shows there is
    /// nothing there, and every cursor step would otherwise log it again.
    #[test]
    fn looking_at_nothing_logs_nothing() {
        let mut w = modal_world(5);
        let player = player_entity(&mut w);
        let here = *w.get::<Position>(player).unwrap();
        let empty = Position {
            x: here.x + 1,
            y: here.y,
        };
        assert!(describe_target(&mut w, empty).is_empty());
    }

    /// A creature's dangers share one line after the sighting, so a dragon
    /// costs two log lines however much it carries.
    #[test]
    fn dangers_share_one_line() {
        let mut w = modal_world(5);
        let player = player_entity(&mut w);
        let here = *w.get::<Position>(player).unwrap();
        let beside = Position {
            x: here.x + 1,
            y: here.y,
        };
        let mob = w
            .spawn((
                Name {
                    what: "kestrel".into(),
                },
                Mob {
                    movement_type: MovementType::Static,
                },
                beside,
                Faction::Monster,
            ))
            .id();
        for g in [
            models::Grant::of::<models::Flies>(),
            models::Grant::of::<models::FireImmune>(),
        ] {
            models::lend(&mut w, mob, g, models::Lifetime::Permanent);
        }

        let lines = describe_target(&mut w, beside);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(
            lines[1],
            strings::beware(&["fire doesn't harm them", "flying"])
        );
    }

    /// Look mode is the one reticle that still opens on you: its cursor is
    /// your attention, and snapping it onto a medusa would petrify you for
    /// pressing `L`.
    #[test]
    fn look_mode_still_opens_on_your_own_tile() {
        let mut w = modal_world(5);
        let player = player_entity(&mut w);
        let here = *w.get::<Position>(player).unwrap();
        let beside = Position {
            x: here.x + 1,
            y: here.y,
        };
        w.spawn((
            Name {
                what: "medusa".into(),
            },
            Mob {
                movement_type: MovementType::Static,
            },
            beside,
            Faction::Monster,
        ));
        w.get_mut::<Viewshed>(player).unwrap().visible_tiles = vec![(beside.x, beside.y)];

        open_reticle_for(&mut w, player, None, None, true, false, false);

        let ts = w.resource::<TargetingState>();
        assert_eq!((ts.cursor_x, ts.cursor_y), (here.x as i16, here.y as i16));
    }

    /// A throw comes down on the floor as it was when the player let go, not
    /// as the monsters have since rearranged it.
    ///
    /// Aimed at the empty tile *in front of* an approaching orc: with
    /// `throw_system` after `ai`, the orc had already stepped onto that tile
    /// by the time the dagger arrived and caught one it was never thrown at.
    #[test]
    fn a_throw_lands_where_the_floor_was_when_the_player_let_go() {
        let mut w = modal_world(5);
        let player = player_entity(&mut w);
        let here = *w.get::<Position>(player).unwrap();
        let row: Vec<u16> = {
            let map = w.resource::<Map>();
            (1..=4)
                .map(|d| here.x + d)
                .take_while(|&x| !map.blocks(x, here.y))
                .collect()
        };
        assert!(row.len() >= 3, "need open floor beside the player");

        let mob = w
            .spawn((
                Name { what: "orc".into() },
                Mob {
                    movement_type: MovementType::Chase,
                },
                Position {
                    x: row[2],
                    y: here.y,
                },
                Fighter {
                    hp: 20,
                    max_hp: 20,
                    armor: 0,
                    power: 1,
                    max_power: 1,
                    armor_bonus: 0,
                    power_bonus: 0,
                },
                Faction::Monster,
                Blood,
                Speed::new(SpeedKind::Normal),
            ))
            .id();
        w.get_mut::<Viewshed>(player).unwrap().visible_tiles = row
            .iter()
            .map(|&x| (x, here.y))
            .chain([(here.x, here.y)])
            .collect();

        let empty_tile = Position {
            x: row[1],
            y: here.y,
        };
        let dagger = models::spawn_weapon(&mut w, "dagger", Position { x: 0, y: 0 });
        w.entity_mut(dagger).remove::<Position>();
        w.resource_mut::<ThrowQueue>().throws.push(WantsToThrow {
            thrower: player,
            item: dagger,
            target: empty_tile,
            slot_idx: None,
        });

        crate::turn_schedule().run(&mut w);

        assert_eq!(
            w.get::<Position>(mob).unwrap().x,
            empty_tile.x,
            "the orc did walk onto the aimed tile this turn"
        );
        assert_eq!(
            w.get::<Fighter>(mob).unwrap().hp,
            20,
            "but the dagger was already on the floor by then"
        );
    }

    /// A fresh world with the floor's own monsters swept off, and a Helper
    /// planted one step east of the player.
    fn world_with_a_helper(seed: u64) -> (World, Entity, Entity) {
        let mut w = modal_world(seed);
        let strays: Vec<Entity> = w
            .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
            .iter(&w)
            .collect();
        for e in strays {
            w.despawn(e);
        }
        let player = player_entity(&mut w);
        let here = player_pos(&mut w);
        let east = Position {
            x: here.x + 1,
            y: here.y,
        };
        assert!(w.resource::<Map>().walkable(east.x, east.y, false));
        let pal = w
            .spawn((
                Name { what: "rat".into() },
                Mob {
                    movement_type: MovementType::Chase,
                },
                east,
                Fighter {
                    hp: 5,
                    max_hp: 5,
                    armor: 0,
                    power: 1,
                    max_power: 1,
                    armor_bonus: 0,
                    power_bonus: 0,
                },
                Faction::Monster,
                Blood,
            ))
            .id();
        models::recruit(&mut w, pal);
        (w, player, pal)
    }

    #[test]
    fn walking_into_your_helper_swaps_places_with_it() {
        let (mut w, player, pal) = world_with_a_helper(3);
        let here = player_pos(&mut w);
        let there = *w.get::<Position>(pal).unwrap();

        assert!(
            step(&mut w, 1, 0),
            "the swap is a step, and a step is a turn"
        );

        assert_eq!(*w.get::<Position>(player).unwrap(), there);
        assert_eq!(*w.get::<Position>(pal).unwrap(), here);
        assert_eq!(w.get::<Fighter>(pal).unwrap().hp, 5, "not a swing at it");
    }

    #[test]
    fn you_cannot_swap_into_a_wall_your_helper_is_standing_in() {
        let (mut w, player, pal) = world_with_a_helper(3);
        let here = player_pos(&mut w);
        let there = *w.get::<Position>(pal).unwrap();
        w.resource_mut::<Map>().tiles[models::tile_index(there.x, there.y)] = TileType::Wall;

        step(&mut w, 1, 0);

        assert_eq!(*w.get::<Position>(player).unwrap(), here);
        assert_eq!(*w.get::<Position>(pal).unwrap(), there);
    }

    #[test]
    fn using_an_item_with_no_use_says_so_and_costs_nothing() {
        let mut w = modal_world(3);
        let player = player_entity(&mut w);
        let element = models::spawn_element_of_yoord(&mut w, Position { x: 0, y: 0 });
        w.entity_mut(element).remove::<Position>();
        w.get_mut::<Backpack>(player)
            .unwrap()
            .items
            .insert(0, element);

        let turn = commit_item_action(&mut w, player, 0, ItemAction::Use);

        assert!(!turn, "no turn spent on a refusal");
        assert_eq!(w.get::<Backpack>(player).unwrap().items[0], element);
        assert!(w.resource::<UseQueue>().uses.is_empty());
        assert!(
            w.resource::<GameLog>()
                .history
                .iter()
                .any(|l| l.contains("right now"))
        );
    }

    #[test]
    fn using_a_snack_says_it_is_for_throwing_and_costs_nothing() {
        let mut w = modal_world(3);
        let player = player_entity(&mut w);
        let snack = models::spawn_named(&mut w, "snack", Position { x: 0, y: 0 }).unwrap();
        w.entity_mut(snack).remove::<Position>();
        w.get_mut::<Backpack>(player)
            .unwrap()
            .items
            .insert(0, snack);

        let turn = commit_item_action(&mut w, player, 0, ItemAction::Use);

        assert!(!turn, "no turn spent on a refusal");
        assert_eq!(w.get::<Backpack>(player).unwrap().items[0], snack);
        assert!(w.resource::<UseQueue>().uses.is_empty());
        assert!(
            w.resource::<GameLog>()
                .history
                .iter()
                .any(|l| l.contains("for throwing"))
        );
    }

    #[test]
    fn using_an_inert_rune_says_so_and_costs_nothing() {
        let mut w = modal_world(3);
        let player = player_entity(&mut w);
        let rune = models::spawn_rune(
            &mut w,
            models::RuneEffect::Displacement,
            Position { x: 0, y: 0 },
        );
        w.entity_mut(rune).remove::<Position>();
        w.get_mut::<models::Rune>(rune).unwrap().charged = false;
        w.get_mut::<Backpack>(player).unwrap().items.insert(0, rune);

        let turn = commit_item_action(&mut w, player, 0, ItemAction::Use);

        assert!(!turn, "no turn spent on a dark rune");
        assert_eq!(w.get::<Backpack>(player).unwrap().items[0], rune);
        assert!(w.resource::<UseQueue>().uses.is_empty());
        assert!(
            w.resource::<GameLog>()
                .history
                .iter()
                .any(|l| l.contains("rune is dark"))
        );
    }

    #[test]
    fn taking_off_cursed_gear_says_it_is_stuck_and_costs_nothing() {
        let mut w = modal_world(3);
        let player = player_entity(&mut w);
        let ring = models::spawn_ring(
            &mut w,
            models::RingEffect::Protection,
            Position { x: 0, y: 0 },
        );
        w.entity_mut(ring)
            .remove::<Position>()
            .insert(models::Curse);
        w.get_mut::<Backpack>(player).unwrap().items.insert(0, ring);
        models::toggle_equipped(&mut w, player, ring);
        w.resource_mut::<GameLog>().history.clear();

        let turn = commit_item_action(&mut w, player, 0, ItemAction::Use);

        assert!(!turn, "no turn spent on welded gear");
        assert!(w.resource::<UseQueue>().uses.is_empty());
        assert_eq!(w.get::<Backpack>(player).unwrap().items[0], ring);
        assert!(!w.resource::<GameLog>().history.is_empty());
    }

    #[test]
    fn modal_open_sees_every_menu() {
        let mut w = modal_world(1);
        assert!(!modal_open(&w));
        w.resource_mut::<PackIsOpen>().open = true;
        assert!(modal_open(&w));
        w.resource_mut::<PackIsOpen>().open = false;
        w.resource_mut::<SpellsMenu>().open = true;
        assert!(modal_open(&w));
        w.resource_mut::<SpellsMenu>().open = false;
        w.resource_mut::<HelpMenu>().open = true;
        assert!(modal_open(&w));
        w.resource_mut::<HelpMenu>().open = false;
        w.resource_mut::<TargetingState>().active = true;
        assert!(modal_open(&w));
    }
}

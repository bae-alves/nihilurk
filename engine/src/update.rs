use std::time::Duration;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, poll, read};
use models::*;
use models::{GameState, components::GameLog};
use rand::Rng;

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

/// How long a turn lost to paralysis holds the screen, so the monsters' free
/// move is something the player watches happen rather than finds already done.
/// The same pacing `main.rs` gives a player asleep in gas.
const PARALYSIS_PAUSE_MS: u64 = 90;

/// The eight steps a confused stumble can send you.
const STUMBLE_DIRS: [(i16, i16); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
];

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

/// Confusion tax: half of every intended step goes off in a random direction
/// instead. Returns the step to actually attempt and whether it was hijacked (a
/// hijacked lurch into a wall still burns the turn).
fn maybe_stumble(world: &mut World, dx: i16, dy: i16) -> (i16, i16, bool) {
    if !player_confused(world) {
        return (dx, dy, false);
    }
    let mut rng = world.resource_mut::<models::GameRng>();
    if !rng.0.gen_bool(0.5) {
        return (dx, dy, false);
    }
    let (sx, sy) = STUMBLE_DIRS[rng.0.gen_range(0..STUMBLE_DIRS.len())];
    world
        .resource_mut::<GameLog>()
        .add("You stumble foolishly.");
    (sx, sy, true)
}

/// The one path every step and every melee attack goes through — the arrow
/// keys, auto-explore, fast-move and auto-fight all end up here.
///
/// The checks below run in a fixed order and each can end the attempt; only
/// reaching the move itself (or a confused lurch into a wall) spends the turn.
/// `docs/reference/input-and-turn-loop.md` walks that order, so keep the two in
/// step if you add a check.
fn move_player(world: &mut World, dx: i16, dy: i16) -> bool {
    let (dx, dy, stumbled) = maybe_stumble(world, dx, dy);

    let mut player_data = None;
    {
        let mut query = world.query_filtered::<(Entity, &Position), With<Player>>();
        if let Some((entity, pos)) = query.iter(world).next() {
            player_data = Some((
                entity,
                pos.x,
                pos.y,
                pos.x.saturating_add_signed(dx),
                pos.y.saturating_add_signed(dy),
            ));
        }
    }
    let Some((player_entity, old_x, old_y, new_x, new_y)) = player_data else {
        return false;
    };

    if world.resource::<Map>().blocks(new_x, new_y) {
        // A deliberate wall-bump is free; a confused lurch into it is not.
        return stumbled;
    }

    // A diagonal step only connects tiles of the same kind — no cutting across
    // a doorway or squeezing between a room and a corridor.
    if !world
        .resource::<Map>()
        .diagonal_step_ok(old_x, old_y, new_x, new_y)
    {
        return stumbled; // Can't cut this corner
    }

    // Walking into a creature is how you hit it; there is no attack key.
    let mut target_mob_entity = None;
    {
        let mut query = world.query_filtered::<(Entity, &Position), With<Mob>>();
        for (entity, pos) in query.iter(world) {
            if pos.x == new_x && pos.y == new_y {
                target_mob_entity = Some(entity);
                break;
            }
        }
    }

    if let Some(target_entity) = target_mob_entity {
        player_attack(world, player_entity, target_entity);
        return true; // Attacking consumes a turn
    }

    // Something has your leg. A swing at an adjacent foe (above) still
    // lands either way, but the step you were about to take does not: against a
    // bear trap it becomes a bloody lurch at the jaws — one wasted turn, a
    // scratch of damage, a lot of blood — and against a scroll's hold it is
    // simply a turn spent straining at nothing. Nothing a monster can read
    // holds the player today; this is here so it stays true if one ever does.
    match player_snare(world) {
        Some(SnareKind::Bear) => {
            bear_trap_thrash(world, player_entity);
            return true;
        }
        Some(SnareKind::Hold) => {
            world
                .resource_mut::<GameLog>()
                .add("You strain against whatever is holding you, and go nowhere.");
            return true;
        }
        _ => {}
    }

    // The path is clear: take the step.
    if let Some(mut pos) = world.get_mut::<Position>(player_entity) {
        pos.x = new_x;
        pos.y = new_y;
    }
    if let Some(mut viewshed) = world.get_mut::<Viewshed>(player_entity) {
        viewshed.dirty = true;
    }
    // Tag the move so `trap_system` checks the new tile for a trap.
    world.entity_mut(player_entity).insert(EntityMoved);

    // Arriving on an item picks it up. There is no `,` key: roog has nine pack
    // slots and a floor full of coins that are spent where they lie, so walking
    // over a thing is decision enough.
    let mut item_entity_to_pickup = None;
    {
        let mut query = world.query_filtered::<(Entity, &Position), With<Item>>();
        for (entity, pos) in query.iter(world) {
            if pos.x == new_x && pos.y == new_y {
                item_entity_to_pickup = Some(entity);
                break;
            }
        }
    }
    if let Some(item_entity) = item_entity_to_pickup {
        // `models::pick_up` owns everything from here: the stash reveal, a
        // coin spent where it lies, the score a treasure is worth, and the pack.
        // `None` back means the item is still on the floor — either the pack is
        // full, or it is a pickup that would have done nothing yet.
        let stowable = world.get::<Pickup>(item_entity).is_none();
        match models::pick_up(world, player_entity, item_entity) {
            Some(msg) => world.resource_mut::<GameLog>().add(msg),
            // Anything that needs a pack slot and did not get one says so; a
            // coin left where it lies says nothing, because a coin you cannot
            // use yet being still there is not news.
            None if stowable => world.resource_mut::<GameLog>().add("Your pack is full."),
            None => {}
        }
    }

    true // Successfully moved, consuming a turn
}

fn player_attack(world: &mut World, attacker_entity: Entity, target_entity: Entity) {
    // Same opposed-roll resolution the monsters use, including the player's
    // chip-damage floor and excellent-hit chance.
    resolve_attack(world, attacker_entity, target_entity);
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
            .add("You are too confused for that right now.");
        return false;
    }
    if player_too_injured(world) {
        world
            .resource_mut::<GameLog>()
            .add("You are too injured for that now.");
        return false;
    }
    let player = player_entity(world);
    if wielded_launcher(world, player).is_some() {
        return ranged_auto_fight(world, player);
    }
    let Some(target) = auto_fight_target(world) else {
        world
            .resource_mut::<GameLog>()
            .add("There is nothing to fight.");
        return false;
    };
    match fight_step(world, target) {
        Some((dx, dy)) => {
            world.resource_mut::<GameLog>().unread.clear();
            move_player(world, dx, dy)
        }
        None => {
            world
                .resource_mut::<GameLog>()
                .add("You can't reach it from here.");
            false
        }
    }
}

/// Tab, with a launcher wielded: fire the first matching projectile at the
/// auto-fight target instead of walking toward it. Refuses — no turn spent —
/// when there's nothing to shoot at, nothing left to shoot with, or the shot
/// to the target isn't clear; it never falls back to closing the distance by
/// hand.
fn ranged_auto_fight(world: &mut World, player: Entity) -> bool {
    let Some(target) = auto_fight_target(world) else {
        world
            .resource_mut::<GameLog>()
            .add("There is nothing to fight.");
        return false;
    };
    let Some(item) = first_matching_ammo(world, player) else {
        world.resource_mut::<GameLog>().add("You're out of ammo.");
        return false;
    };
    if !has_clear_shot(world, target) {
        world.resource_mut::<GameLog>().add("No clear shot.");
        return false;
    }

    let Some((slot, item)) = world.get_mut::<Backpack>(player).and_then(|mut bp| {
        let pos = bp.items.iter().position(|&e| e == item)?;
        Some((pos, bp.items.remove(pos)))
    }) else {
        return false;
    };
    let target_pos = *world.get::<Position>(target).unwrap();
    let missile = draw_one(world, player, item, Some(slot));
    world.resource_mut::<GameLog>().unread.clear();
    world
        .resource_mut::<ThrowQueue>()
        .throws
        .push(WantsToThrow {
            thrower: player,
            item: missile,
            target: target_pos,
        });
    true
}

pub fn process_input_and_update(world: &mut World) -> std::io::Result<bool> {
    // A player asleep in sleeping gas forfeits the turn outright — no key is
    // read — as long as there's no pending --MORE-- prompt to clear first.
    // `snare_system` ages the snare down as the turn resolves. A bear trap does
    // *not* forfeit the turn: it only blocks movement (see `move_player`), so
    // input is still read and the player can swing or thrash.
    let more_pending = {
        let log = world.resource::<GameLog>();
        log_view(&log.unread).2
    };
    if !more_pending && player_incapacitated(world) {
        return Ok(true);
    }

    // Paralysis eats a share of the turns its slowing still leaves you. The coin
    // is flipped here, once, before a key is read — so a lost turn is a turn the
    // monsters get and the player doesn't, rather than a swallowed keystroke —
    // and the pause is what makes it read as time passing instead of a freeze.
    if !more_pending && paralysis_forfeits_turn(world) {
        std::thread::sleep(Duration::from_millis(PARALYSIS_PAUSE_MS));
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

/// Everything [`process_input_and_update`] does once it is holding a key: the
/// `--MORE--` gate, the universal escape hatch, and the four input contexts.
///
/// Split out from the read so the whole modal stack can be exercised without a
/// terminal — `read()` blocks, and a state machine that can only be tested by
/// a person pressing keys is a state machine nobody tests. Everything above
/// this line needs a real keyboard; nothing below it does.
pub(crate) fn dispatch_key(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    // While a --MORE-- prompt is up, the only input accepted is the
    // acknowledgement: it drops the messages already shown and lets the rest
    // flow up on the next frame.
    let (_lines, shown, more) = {
        let log = world.resource::<GameLog>();
        log_view(&log.unread)
    };
    if more {
        if key.code == KeyCode::Char(' ') || key.code == KeyCode::Enter {
            world.resource_mut::<GameLog>().unread.drain(0..shown);
        }
        return Ok(false);
    }

    // 'x' is the universal escape hatch: from any modal (aiming, the pack, its
    // action menu, the quit prompt) it drops straight back to plain movement,
    // no turn spent. 'X' does the same, which is the whole reason it is checked
    // here rather than alongside 'Q' in `handle_movement_input`: it is one
    // shift away from the escape hatch, so with something open it must escape
    // rather than ask about ending the run. Only with nothing open does it fall
    // through to the quit prompt.
    let escape_hatch = matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X'));
    if escape_hatch && close_all_modals(world) {
        return Ok(false);
    }

    // Four input contexts, each with its own handler: answering "Really quit?",
    // aiming a wand or a throw, navigating the pack, or walking the map. The
    // quit prompt outranks the rest — it is only ever raised from the map, and
    // while it is up the only question on the table is that one.
    if world.resource::<QuitPrompt>().open {
        return Ok(answer_quit_prompt(world, key));
    }
    if world.resource::<TargetingState>().active {
        return handle_targeting_input(world, key);
    }
    if world.resource::<PackIsOpen>().open {
        return handle_inventory_input(world, key);
    }
    handle_movement_input(world, key)
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
        closed = true;
    }
    let mut pack = world.resource_mut::<PackIsOpen>();
    if pack.open {
        pack.close();
        closed = true;
    }
    let mut quit = world.resource_mut::<QuitPrompt>();
    if quit.open {
        quit.open = false;
        closed = true;
    }
    closed
}

/// A keypress while "Really quit?" is up: `y` ends the run, `n` or `Esc` goes
/// back to the dungeon, and anything else is ignored rather than guessed at.
/// Never spends a turn. (`x` and `X` also cancel, via `close_all_modals`
/// upstream — the prompt is a modal like any other.)
fn answer_quit_prompt(world: &mut World, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            world.resource_mut::<QuitPrompt>().open = false;
            world.resource_mut::<GameState>().is_running = false;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            world.resource_mut::<QuitPrompt>().open = false;
        }
        _ => {}
    }
    false
}

/// A keypress while the aiming reticle is up: move it, fire it, or cancel.
fn handle_targeting_input(world: &mut World, key: KeyEvent) -> std::io::Result<bool> {
    let mut cancel = false;
    let mut confirm = false;
    let mut dx = 0i16;
    let mut dy = 0i16;
    match key.code {
        KeyCode::Esc => cancel = true,
        KeyCode::Enter | KeyCode::Char(' ') => confirm = true,
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
        return Ok(false); // cancelled aiming, no turn consumed
    }
    if dx != 0 || dy != 0 {
        move_target_cursor(world, dx, dy);
        return Ok(false);
    }
    if confirm {
        return fire_at_target(world);
    }
    Ok(false) // any other key: ignored while aiming
}

/// A directional key while aiming: nudge the reticle one tile, but only onto a
/// tile that is both in view and inside the item's reach.
fn move_target_cursor(world: &mut World, dx: i16, dy: i16) {
    let (item, cursor_x, cursor_y, throwing) = {
        let ts = world.resource::<TargetingState>();
        (ts.item, ts.cursor_x, ts.cursor_y, ts.throwing)
    };
    let new_x = cursor_x.saturating_add(dx);
    let new_y = cursor_y.saturating_add(dy);

    let player = player_entity(world);
    let player_pos = *world.get::<Position>(player).unwrap();
    let visible = world.get::<Viewshed>(player).unwrap().visible_tiles.clone();
    let max_range = aim_range(world, item, throwing);

    let distance = (new_x - player_pos.x as i16)
        .abs()
        .max((new_y - player_pos.y as i16).abs());
    let in_view = visible.contains(&(new_x as u16, new_y as u16));
    if distance > max_range as i16 || !in_view {
        return;
    }
    let mut ts = world.resource_mut::<TargetingState>();
    ts.cursor_x = new_x;
    ts.cursor_y = new_y;
}

/// How far the aimed item reaches: an arm's length for a throw, the item's own
/// `Ranged` for a zap, a bare 8 for anything without one.
fn aim_range(world: &World, item: Option<Entity>, throwing: bool) -> i32 {
    if throwing {
        return THROW_RANGE;
    }
    item.and_then(|i| world.get::<Ranged>(i))
        .map_or(8, |r| r.range)
}

/// Enter/Space while aiming: pull the item from the pack and hand it to the
/// throw or use queue. A shot at the player's own tile is refused.
fn fire_at_target(world: &mut World) -> std::io::Result<bool> {
    let (tx, ty, item_entity, throwing) = {
        let mut ts = world.resource_mut::<TargetingState>();
        ts.active = false;
        let grabbed = (ts.cursor_x, ts.cursor_y, ts.item.unwrap(), ts.throwing);
        ts.item = None;
        ts.throwing = false;
        grabbed
    };
    let player = player_entity(world);
    let target = Position {
        x: tx as u16,
        y: ty as u16,
    };

    let at_self = world
        .get::<Position>(player)
        .is_some_and(|p| p.x == target.x && p.y == target.y);
    if at_self {
        world.resource_mut::<GameLog>().add("Great idea! But no.");
        return Ok(false);
    }

    let Some((slot, item)) = world.get_mut::<Backpack>(player).and_then(|mut bp| {
        let pos = bp.items.iter().position(|&e| e == item_entity)?;
        Some((pos, bp.items.remove(pos)))
    }) else {
        return Ok(false);
    };

    if throwing {
        // The item is gone from the pack; where it lands is `throw_system`'s
        // business. (A quiver is the exception — `draw_one` splits one arrow off
        // and puts the rest back in the slot.)
        let missile = models::draw_one(world, player, item, Some(slot));
        world
            .resource_mut::<ThrowQueue>()
            .throws
            .push(WantsToThrow {
                thrower: player,
                item: missile,
                target,
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
    let Some(item) = take_pack_item(world, player, item_idx) else {
        return true;
    };
    match action {
        ItemAction::Use => use_or_aim(world, player, item, item_idx),
        ItemAction::Throw => {
            aim_throw(world, player, item, item_idx);
            false
        }
        ItemAction::Drop => drop_from_pack(world, player, item, item_idx),
    }
}

/// Use: a plain item goes straight to the use queue (turn spent); a ranged one
/// goes back in the pack and opens the aiming reticle instead (no turn).
fn use_or_aim(world: &mut World, player: Entity, item: Entity, item_idx: usize) -> bool {
    // The wand of light is ranged but self-targeted, so it skips the reticle.
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

/// Drop: cursed gear won't come off and so can't be put down (it goes back in
/// the pack); anything else is un-equipped on the way to the floor. Returns
/// whether the drop actually happened.
fn drop_from_pack(world: &mut World, player: Entity, item: Entity, item_idx: usize) -> bool {
    if let Some(refusal) = drop_refusal(world, player, item) {
        return_to_pack(world, player, item, item_idx);
        world.resource_mut::<GameLog>().add(refusal);
        return false;
    }
    let Some(pos) = world.get::<Position>(player).cloned() else {
        return true;
    };
    force_unequip(world, item);
    sync_equipment_effects(world, player);
    world.entity_mut(item).insert(pos);
    let name = models::display_name(world, item);
    world
        .resource_mut::<GameLog>()
        .add(format!("You drop the {name}."));
    true
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
    let pos = *world.get::<Position>(player).unwrap();
    let mut ts = world.resource_mut::<TargetingState>();
    ts.active = true;
    ts.item = Some(item);
    ts.throwing = throwing;
    ts.cursor_x = pos.x as i16;
    ts.cursor_y = pos.y as i16;
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
    // Browsing asks which verb; the branch below is every mode that already
    // knows, so it closes the menu and does the thing.
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
    // Shift + direction: NetHack-style running. Either zoom in a straight line,
    // or make a beeline for the nearest feature (stairs > door > item) roughly
    // that way. Refused with a creature in view; the main loop drives the run to
    // completion and only then repaints.
    if let Some((rdx, rdy)) = run_direction(key.code, key.modifiers) {
        start_run(world, rdx, rdy);
        return Ok(false);
    }

    let step = match key.code {
        // Ctrl+C is the shell's own kill and takes no answer. `Q` and `X` ask.
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            world.resource_mut::<GameState>().is_running = false;
            None
        }
        // `X` only reaches this far with nothing open — see the escape hatch in
        // `process_input_and_update`.
        KeyCode::Char('Q') | KeyCode::Char('X') => {
            world.resource_mut::<QuitPrompt>().open = true;
            return Ok(false);
        }
        // The pack, one key per verb. `i` is the one that asks afterwards.
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
        // The one undocumented key in the game: with a ring of teleportation on
        // and magic to spend, it jumps you. Without either it does nothing and
        // says nothing — see `models::willed_teleport`.
        KeyCode::Char('T') => return Ok(models::willed_teleport(world)),
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
    Ok(move_player(world, dx, dy))
}

/// Shift + direction: kick off a NetHack-style run, or say why it can't start.
fn start_run(world: &mut World, rdx: i16, rdy: i16) {
    if player_confused(world) {
        world
            .resource_mut::<GameLog>()
            .add("You are too confused for that right now.");
        return;
    }
    match fast_move_plan(world, rdx, rdy) {
        FastMovePlan::MonsterInSight => {
            world
                .resource_mut::<GameLog>()
                .add("Not while a creature is in sight.");
        }
        FastMovePlan::Blocked => {
            world
                .resource_mut::<GameLog>()
                .add("You can't run that way.");
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

/// `A`: flip whether auto-explore detours to pick things up, and say which way
/// it landed. A preference, not an action — no turn is spent.
fn toggle_auto_pickup(world: &mut World) {
    let enabled = {
        let mut pickup = world.resource_mut::<AutoPickup>();
        pickup.enabled = !pickup.enabled;
        pickup.enabled
    };
    let state = if enabled { "ON" } else { "OFF" };
    world
        .resource_mut::<GameLog>()
        .add(format!("Pick-up on auto-explore {state}."));
}

/// `f`: fire the wielded launcher's first matching projectile — a shortcut for
/// the pack's `i` → item → Throw path when what's in hand is a bow or
/// crossbow. Opens the aiming reticle exactly as that path does, pre-loaded
/// with the first arrow (or quarrel) in the pack; Enter/Space looses it via
/// the ordinary [`fire_at_target`].
fn begin_fire(world: &mut World) -> std::io::Result<bool> {
    let player = player_entity(world);
    if wielded_launcher(world, player).is_none() {
        world
            .resource_mut::<GameLog>()
            .add("You aren't wielding a launcher.");
        return Ok(false);
    }
    let Some(item) = first_matching_ammo(world, player) else {
        let noun = ammo_noun(world, player);
        world
            .resource_mut::<GameLog>()
            .add(format!("You have no {noun} to fire."));
        return Ok(false);
    };
    if let Some(refusal) = throw_refusal(world, player, item) {
        world.resource_mut::<GameLog>().add(refusal);
        return Ok(false);
    }
    open_reticle(world, player, item, true);
    Ok(false)
}

/// `o`: arm auto-explore, unless confused, a monster is in view, or the map is
/// already fully known.
fn begin_auto_explore(world: &mut World) -> std::io::Result<bool> {
    if player_confused(world) {
        world
            .resource_mut::<GameLog>()
            .add("You are too confused for that right now.");
        return Ok(false);
    }
    if monster_in_sight(world) {
        world
            .resource_mut::<GameLog>()
            .add("Not while a creature is in sight.");
        return Ok(false);
    }
    if explore_step(world).is_none() {
        world
            .resource_mut::<GameLog>()
            .add("There is nothing left to explore.");
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
    let dir = if going_down { "down" } else { "up" };
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

    // Already on the right staircase: use it now.
    if world.resource::<Map>().tile(ppos.x, ppos.y) == want_tile {
        world.resource_mut::<GameLog>().unread.clear();
        return change_level(world, going_down);
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
            .add(format!("You cannot go {dir} from here."));
        return false;
    };

    if monster_in_sight(world) {
        world
            .resource_mut::<GameLog>()
            .add("Not while a creature is in sight.");
        return false;
    }
    if travel_step(world, target).is_none() {
        world
            .resource_mut::<GameLog>()
            .add(format!("You can't find a path to the {dir}-stairs."));
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

    // Any pending keypress cancels the walk. Swallow it so it doesn't also act
    // as a move on the next frame.
    if poll(Duration::from_millis(0))? {
        let _ = read()?;
        world.resource_mut::<AutoExplore>().stop();
        world.resource_mut::<GameLog>().add(if travelling {
            "Travel interrupted."
        } else {
            "Auto-explore interrupted."
        });
        return Ok(false);
    }

    // Travelling and arrived: stop cleanly on the staircase.
    if let Some(target) = world.resource::<AutoExplore>().target {
        let arrived = {
            let mut q = world.query_filtered::<&Position, With<Player>>();
            q.iter(world).next().is_some_and(|p| (p.x, p.y) == target)
        };
        if arrived {
            world.resource_mut::<AutoExplore>().stop();
            let msg = match world.resource::<Map>().tile(target.0, target.1) {
                TileType::Upstairs | TileType::Downstairs => "You arrive at the staircase.",
                _ => "You stop.",
            };
            world.resource_mut::<GameLog>().add(msg);
            return Ok(false);
        }
    }

    // Something was logged last turn: stop and let the player read it.
    if !world.resource::<GameLog>().unread.is_empty() {
        world.resource_mut::<AutoExplore>().stop();
        return Ok(false);
    }

    // A monster came into view (or was already there when we started).
    if monster_in_sight(world) {
        world.resource_mut::<AutoExplore>().stop();
        world
            .resource_mut::<GameLog>()
            .add("There is a monster nearby.");
        return Ok(false);
    }

    // Runaway guard.
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
            "You can't find a path there."
        } else {
            "You have explored everywhere you can."
        });
        return Ok(false);
    };

    let moved = move_player(world, dx, dy);
    if !moved {
        // The pathfinder only ever steps onto open ground, so this shouldn't
        // happen — but if it does, don't spin.
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
    if !poll(Duration::from_millis(400))? {
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
        // `x` / `X` close this the way they close every other modal. The cursor
        // runs its own loop outside `process_input_and_update`, so the universal
        // escape hatch there never sees these keys — this arm is that hatch.
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
    // Prefer the full move; fall back to a one-axis slide so the cursor can
    // still hug a wall or room edge when the diagonal tile is unseen.
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
            .add("Not while a creature is in sight.");
        return Ok(());
    }

    // Route to the picked tile, or — when it is a wall or somewhere unreachable
    // — to the nearest walkable tile the player can actually get to.
    let Some(goal) = nearest_reachable(world, (tx, ty)) else {
        world
            .resource_mut::<GameLog>()
            .add("You can't find a path there.");
        return Ok(());
    };

    let here = {
        let mut q = world.query_filtered::<&Position, With<Player>>();
        q.iter(world).next().map(|p| (p.x, p.y))
    };
    if here == Some(goal) {
        world
            .resource_mut::<GameLog>()
            .add("You are already there.");
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
        // A keypress aborts the run. Swallow it so it isn't also read as a move.
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

        // Never start a step with a creature in view.
        if monster_in_sight(world) {
            break;
        }

        let next = match world.resource::<FastMove>().target {
            Some(target) => travel_step(world, target),
            None => straight_step(world),
        };
        let Some((dx, dy)) = next else { break };

        if !move_player(world, dx, dy) {
            break;
        }

        // One turn passes: monsters act, visibility is recomputed.
        schedule.run(world);

        if world.resource::<Ending>().player_dead {
            break;
        }
        // Something entered view, or a message needs reading.
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
        w
    }

    /// The resources the modal stack reads, on top of a built world. `main.rs`
    /// inserts these at startup; a test world has to do it too.
    fn modal_world(seed: u64) -> World {
        let mut w = test_world(seed);
        w.init_resource::<PackIsOpen>();
        w.init_resource::<QuitPrompt>();
        w.init_resource::<AutoExplore>();
        w.init_resource::<AutoPickup>();
        w.init_resource::<FastMove>();
        w.init_resource::<TravelCursor>();
        w.init_resource::<UseQueue>();
        w.init_resource::<ThrowQueue>();
        w.init_resource::<AttackQueue>();
        w.insert_resource(TargetingState {
            active: false,
            item: None,
            throwing: false,
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
            log.unread
                .push(format!("Message number {i} is a fairly long one."));
        }
        let unread = w.resource::<GameLog>().unread.len();
        assert!(
            log_view(&w.resource::<GameLog>().unread).2,
            "the fixture did not actually raise a --MORE-- prompt"
        );
        unread
    }

    fn player_pos(w: &mut World) -> Position {
        let mut q = w.query_filtered::<&Position, With<Player>>();
        *q.iter(w).next().unwrap()
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
            let after_digit = player_pos(&mut w);

            let mut w2 = test_world(2112);
            handle_movement_input(
                &mut w2,
                KeyEvent::new(KeyCode::Char(letter), KeyModifiers::NONE),
            )
            .unwrap();
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
        // Without Shift, a numpad digit is a plain step, not a run.
        assert_eq!(run_direction(KeyCode::Char('8'), KeyModifiers::NONE), None);
    }

    // -----------------------------------------------------------------------
    // The --MORE-- gate
    // -----------------------------------------------------------------------

    #[test]
    fn a_more_prompt_accepts_the_acknowledgement_and_swallows_everything_else() {
        // The gate's whole job: while messages are waiting, no key does
        // anything except the one that says "I have read them".
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
        // Not all of them, and not one: the ones the panel actually showed.
        // Dropping more loses messages the player never saw.
        for ack in [KeyCode::Char(' '), KeyCode::Enter] {
            let mut w = modal_world(4);
            let queued = flood_the_log(&mut w);
            let shown = log_view(&w.resource::<GameLog>().unread).1;
            assert!(shown > 0 && shown < queued, "the fixture proves nothing");

            let turn = dispatch_key(&mut w, KeyEvent::new(ack, KeyModifiers::NONE)).unwrap();
            assert!(!turn, "acknowledging a prompt is not a turn");
            assert_eq!(w.resource::<GameLog>().unread.len(), queued - shown);
        }
    }

    #[test]
    fn the_gate_outranks_the_escape_hatch() {
        // `x` closes modals everywhere else. Behind a --MORE-- prompt it must
        // not, or a player mashing it loses the line telling them why they are
        // about to die.
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

    // -----------------------------------------------------------------------
    // The x / X escape hatch
    // -----------------------------------------------------------------------

    #[test]
    fn x_and_shift_x_close_whichever_modal_is_open() {
        for key in ['x', 'X'] {
            // The pack.
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

            // The aiming reticle.
            let mut w = modal_world(6);
            w.resource_mut::<TargetingState>().active = true;
            dispatch_key(&mut w, press(key)).unwrap();
            assert!(
                !w.resource::<TargetingState>().active,
                "'{key}' left the reticle up"
            );

            // And the quit prompt, which is a modal like any other.
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

    #[test]
    fn shift_x_escapes_a_modal_rather_than_asking_about_quitting() {
        // The whole reason `X` is checked before `handle_movement_input`: it is
        // one shift away from the escape hatch, so with something open it must
        // escape. Asking "Really quit?" because a player overshot `x` is the
        // bug this ordering exists to prevent.
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
        // ...and only then. `close_all_modals` reporting "nothing was open" is
        // what lets the key through.
        let mut w = modal_world(8);
        dispatch_key(&mut w, press('X')).unwrap();
        assert!(
            w.resource::<QuitPrompt>().open,
            "X on the bare map did nothing"
        );
    }

    #[test]
    fn lowercase_x_on_the_bare_map_is_not_swallowed() {
        // `close_all_modals` returns whether anything was actually shut, so a
        // bare `x` falls through to the movement handler rather than being
        // eaten. It is not a movement key, so nothing happens — but the
        // distinction is what keeps the hatch from stealing keypresses.
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
        // Every other key is ignored rather than guessed at — the prompt stays
        // up, and the run stays alive.
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
        // It is checked first because while it is up, that question is the only
        // one on the table. A movement key must not walk out from under it.
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
        // Esc is the key a player mashes to get out of a menu; from the map it
        // must do nothing at all, and certainly not end the run.
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
}

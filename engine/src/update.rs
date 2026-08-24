use bevy_ecs::prelude::*;
use crossterm::event::{read, Event, KeyCode, KeyEventKind};
use models::*;
use models::{GameState, components::GameLog}; 

fn move_player(world: &mut World, dx: i16, dy: i16) -> bool {
    // 1. Get player entity and calculate target position
    let mut player_data = None;
    {
        let mut query = world.query_filtered::<(Entity, &Position), With<Player>>();
        if let Some((entity, pos)) = query.iter(world).next() {
            player_data = Some((
                entity,
                pos.x.saturating_add_signed(dx),
                pos.y.saturating_add_signed(dy),
            ));
        }
    }
    let Some((player_entity, new_x, new_y)) = player_data else {
        return false;
    };
    
    // 2. Check if a Wall exists at the target coordinates
    let is_wall = world
        .query_filtered::<&Position, With<Wall>>()
        .iter(world)
        .any(|pos| pos.x == new_x && pos.y == new_y);

    if is_wall {
        return false; // Bumped into a wall, turn is NOT consumed
    }

    // 3. Check if a Mob exists at the target coordinates to attack
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

    // 4. Move player if the path is completely clear
    if let Some(mut pos) = world.get_mut::<Position>(player_entity) {
        pos.x = new_x;
        pos.y = new_y;
    }
    if let Some(mut viewshed) = world.get_mut::<Viewshed>(player_entity) {
        viewshed.dirty = true;
    }

    //5. Query world to see if there is an item at the new position and pick it up if so
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
        if let Some(mut backpack) = world.get_mut::<Backpack>(player_entity) {
            backpack.items.push(item_entity);
            world.entity_mut(item_entity).remove::<Position>();
            let mut log = world.resource_mut::<GameLog>();
            log.add("You pick up an item!".to_string());
        }
    }

    true // Successfully moved, consuming a turn
}

fn player_attack(world: &mut World, attacker_entity: Entity, target_entity: Entity) {
    let attacker_power = world
        .get::<Fighter>(attacker_entity)
        .map(|f| f.power)
        .unwrap_or(1);

    let target_armor = world
        .get::<Fighter>(target_entity)
        .map(|f| f.armor)
        .unwrap_or(0);

    let damage = (attacker_power - target_armor).max(0);
    let mut log = world.resource_mut::<GameLog>();
    if damage > 0 {
        log.add(format!("You smack the monster for {} damage!", damage));
    } else {
        log.add("You swing wildly and miss!".to_string());
    }
    drop(log); // Drop borrow before despawning below
    if let Some(mut target_fighter) = world.get_mut::<Fighter>(target_entity) {
        target_fighter.hp -= damage;
        if target_fighter.hp <= 0 {
            world.despawn(target_entity);
            let mut log = world.resource_mut::<GameLog>();
            log.add("The monster is dead!".to_string());
        }
    }
}

pub fn process_input_and_update(world: &mut World) -> std::io::Result<bool> {
    let event = read()?;
    let mut turn_taken = false;

    if let Event::Key(key) = event {
        if key.kind != KeyEventKind::Press { return Ok(false); }
        
        // 1. Handle logs first
        {
            let mut log = world.resource_mut::<GameLog>();
            if log.unread.len() > 3 {
                if key.code == KeyCode::Char(' ') || key.code == KeyCode::Enter {
                    let to_remove = log.unread.len().min(3);
                    log.unread.drain(0..to_remove);
                }
                return Ok(false); 
            }
        }

        // ==========================================
        // BRANCH TARGETING: We are aiming a wand
        // ==========================================
        let is_targeting = world.resource::<TargetingState>().active;

        if is_targeting {
            let mut cancel = false;
            let mut confirm = false;
            let mut dx = 0;
            let mut dy = 0;

            match key.code {
                KeyCode::Esc => cancel = true,
                KeyCode::Enter | KeyCode::Char(' ') => confirm = true,
                KeyCode::Char('w') | KeyCode::Char('k') | KeyCode::Up => dy = -1,
                KeyCode::Char('s') | KeyCode::Char('j') | KeyCode::Down => dy = 1,
                KeyCode::Char('a') | KeyCode::Char('h') | KeyCode::Left => dx = -1,
                KeyCode::Char('d') | KeyCode::Char('l') | KeyCode::Right => dx = 1,
                KeyCode::Char('y') => { dx = -1; dy = -1; }
                KeyCode::Char('u') => { dx = 1; dy = -1; }
                KeyCode::Char('b') => { dx = -1; dy = 1; }
                KeyCode::Char('n') => { dx = 1; dy = 1; }
                _ => {}
            }

            let mut target_state = world.resource_mut::<TargetingState>();

            if cancel {
                target_state.active = false;
                target_state.item = None;
                return Ok(false); // Cancelled aiming, no turn consumed
            }

            if dx != 0 || dy != 0 {
                // 1. Grab what we need from TargetingState and drop it instantly
                let (cursor_item, cursor_x, cursor_y) = {
                    let target_state = world.resource::<TargetingState>();
                    (target_state.item, target_state.cursor_x, target_state.cursor_y)
                }; // target_state borrow is dead and gone here

                let new_x = cursor_x.saturating_add(dx);
                let new_y = cursor_y.saturating_add(dy);
                
                // 2. Now world is completely free to query the player safely
                let (player_pos, max_range, visible_tiles) = {
                    let player_entity = world.query_filtered::<Entity, With<Player>>().iter(world).next().unwrap();
                    let pos = world.get::<Position>(player_entity).unwrap();
                    let p_pos = (pos.x, pos.y);
                    
                    let mut range = 8; // Fallback range
                    if let Some(item_entity) = cursor_item {
                        if let Some(ranged) = world.get::<Ranged>(item_entity) {
                            range = ranged.range;
                        }
                    }

                    let viewshed = world.get::<Viewshed>(player_entity).unwrap();
                    let visible = viewshed.visible_tiles.clone();

                    (p_pos, range, visible)
                }; 

                // 3. Range and Viewshed checks
                let dist_x = (new_x - player_pos.0 as i16).abs();
                let dist_y = (new_y - player_pos.1 as i16).abs();
                let distance = std::cmp::max(dist_x, dist_y);

                let target_coord = (new_x as u16, new_y as u16);
                let is_visible = visible_tiles.contains(&target_coord);

                if distance <= max_range as i16 && is_visible {
                    let mut target_state = world.resource_mut::<TargetingState>();
                    target_state.cursor_x = new_x;
                    target_state.cursor_y = new_y;
                }
                
                return Ok(false); 
            }

            if confirm {
                target_state.active = false;
                let tx = target_state.cursor_x;
                let ty = target_state.cursor_y;
                let item_entity = target_state.item.unwrap();
                target_state.item = None;
                drop(target_state);

                let player_entity = world.query_filtered::<Entity, With<Player>>().iter(world).next().unwrap();
                
                let mut extracted_item = None;
                let mut original_idx = None;
                if let Some(mut backpack) = world.get_mut::<Backpack>(player_entity) {
                    if let Some(pos) = backpack.items.iter().position(|&e| e == item_entity) {
                        original_idx = Some(pos);
                        extracted_item = Some(backpack.items.remove(pos));
                    }
                }

                if let Some(item) = extracted_item {
                    let mut use_queue = world.resource_mut::<UseQueue>();
                    use_queue.uses.push(WantsToUse { 
                        user: player_entity, 
                        item,
                        target: Some(Position { x: tx as u16, y: ty as u16 }),
                        slot_idx: original_idx,
                    });
                    return Ok(true);
                }

                return Ok(false);
            }

            return Ok(false); // Ignore all other keys while targeting
        }

        // ==========================================
        // BRANCH INVENTORY: Navigating Pack / Modal
        // ==========================================
        let (is_open, current_selected, action_mode, action_selected) = {
            let pack_state = world.resource::<PackIsOpen>();
            (pack_state.open, pack_state.selected, pack_state.action_mode, pack_state.action_selected)
        };

        if is_open {
            let player_entity = world.query_filtered::<Entity, With<Player>>().iter(world).next();
            let item_count = player_entity
                .and_then(|entity| world.get::<Backpack>(entity))
                .map_or(0, |bp| bp.items.len());
                
            // Sub-Branch: Action Modal (Use/Drop)
            if let Some(action_item_idx) = action_mode {
                let mut close_inventory = false;
                let mut confirm_action = false;
                let mut new_action_sel = action_selected;
                
                match key.code {
                    KeyCode::Esc | KeyCode::Char('i') => close_inventory = true,
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => { 
                        new_action_sel = if new_action_sel == 0 { 1 } else { 0 }; 
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => { 
                        new_action_sel = if new_action_sel == 1 { 0 } else { 1 }; 
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => confirm_action = true,
                    _ => {}
                }
                
                let mut pack_state = world.resource_mut::<PackIsOpen>();
                pack_state.action_selected = new_action_sel;
                
                if close_inventory {
                    pack_state.open = false;
                    pack_state.action_mode = None; 
                } else if confirm_action {
                    pack_state.open = false;
                    pack_state.action_mode = None;
                    turn_taken = true;
                }
                
                drop(pack_state);
                
                if confirm_action {
                    if let Some(player) = player_entity {
                        // 1. Remove from backpack temporarily
                        let item_entity = if let Some(mut backpack) = world.get_mut::<Backpack>(player) {
                            if action_item_idx < backpack.items.len() {
                                Some(backpack.items.remove(action_item_idx))
                            } else { None }
                        } else { None };
                        
                        // 2. Route to correct action
                        if let Some(item) = item_entity {
                            if new_action_sel == 0 { // USE
                                let is_ranged = world.get::<Ranged>(item).is_some();

                                if is_ranged {
                                    // Put it right back exactly where it was!
                                    if let Some(mut backpack) = world.get_mut::<Backpack>(player) {
                                        backpack.items.insert(action_item_idx, item);
                                    }

                                    let player_pos = world.get::<Position>(player).unwrap().clone();
                                    let mut target_state = world.resource_mut::<TargetingState>();
                                    target_state.active = true;
                                    target_state.item = Some(item);
                                    target_state.cursor_x = player_pos.x as i16;
                                    target_state.cursor_y = player_pos.y as i16;
                                    
                                    turn_taken = false; // Override: aiming takes no time!
                                } else {
                                    let mut use_queue = world.resource_mut::<UseQueue>();
                                    use_queue.uses.push(WantsToUse { user: player, item:item, target: None, slot_idx: Some(action_item_idx)});
                                }
                            } else { // DROP
                                let player_pos = world.get::<Position>(player).cloned();
                                if let Some(pos) = player_pos {
                                    world.entity_mut(item).insert(pos);
                                    let mut log = world.resource_mut::<GameLog>();
                                    log.add("You dropped an item.".to_string());
                                }
                            }
                        }
                    }
                }
                return Ok(turn_taken);
            } 
            // Sub-Branch: Navigating the main list
            else {
                let mut new_selected = current_selected;
                let mut close_inventory = false;
                let mut trigger_action_menu = None;

                match key.code {
                    KeyCode::Esc | KeyCode::Char('i') => close_inventory = true,
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => {
                        new_selected = if new_selected == 0 {
                            item_count.saturating_sub(1)
                        } else {
                            new_selected - 1
                        };
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => {
                        new_selected = if new_selected + 1 >= item_count {
                            0
                        } else {
                            new_selected + 1
                        };
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => trigger_action_menu = Some(new_selected),
                    KeyCode::Char(c) if c.is_ascii_lowercase() => {
                        let idx = (c as u32 - 'a' as u32) as usize;
                        if idx < item_count { trigger_action_menu = Some(idx); }
                    }
                    _ => {}
                }

                let mut pack_state = world.resource_mut::<PackIsOpen>();
                pack_state.selected = new_selected;

                if close_inventory {
                    pack_state.open = false;
                }

                if let Some(idx) = trigger_action_menu {
                    pack_state.action_mode = Some(idx);
                    pack_state.action_selected = 0; 
                }
                
                return Ok(false); 
            }
        }

        // ==========================================
        // BRANCH NORMAL: Walking around the map
        // ==========================================
        let mut dx = 0;
        let mut dy = 0;
        let mut action_attempted = false;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                world.resource_mut::<GameState>().is_running = false;
            }
            KeyCode::Char('i') => {
                let player_entity = world.query_filtered::<Entity, With<Player>>().iter(world).next();
                let is_empty = player_entity
                    .and_then(|entity| world.get::<Backpack>(entity))
                    .map_or(true, |bp| bp.items.is_empty());

                if is_empty {
                    let mut log = world.resource_mut::<GameLog>();
                    log.add("You have no items.");
                    world.resource_mut::<PackIsOpen>().open = false;
                } else {
                    let mut pack_state = world.resource_mut::<PackIsOpen>();
                    pack_state.open = true;
                    pack_state.selected = 0; 
                }
                return Ok(false);
            }
            KeyCode::Char('w') | KeyCode::Char('k') | KeyCode::Up => { dy = -1; action_attempted = true; }
            KeyCode::Char('s') | KeyCode::Char('j') | KeyCode::Down => { dy = 1; action_attempted = true; }
            KeyCode::Char('a') | KeyCode::Char('h') | KeyCode::Left => { dx = -1; action_attempted = true; }
            KeyCode::Char('d') | KeyCode::Char('l') | KeyCode::Right => { dx = 1; action_attempted = true; }
            KeyCode::Char('y') => { dx = -1; dy = -1; action_attempted = true; }
            KeyCode::Char('u') => { dx = 1; dy = -1; action_attempted = true; }
            KeyCode::Char('b') => { dx = -1; dy = 1; action_attempted = true; }
            KeyCode::Char('n') => { dx = 1; dy = 1; action_attempted = true; }
            _ => {} 
        }

        if action_attempted {
            world.resource_mut::<GameLog>().unread.clear();
            turn_taken = move_player(world, dx, dy);
        }
    }

    Ok(turn_taken)
}
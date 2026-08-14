///Helper Functions for each movement type. These are used by the `monster_ai_system` to determine how each mob should move.
pub fn static_move(
    mob_pos: &Position,
    player_pos: &Position,
    map_width: u16,
    map_height: u16,
    tiles: &[TileType],
) -> Option<(u16, u16)> {
    // Static mobs do not move, so we return None to indicate no movement.
    None
}

//Chase is a simple greedy algorithm that moves the mob one step closer to the player, avoiding walls.
//It does not use pathfinding, so it may get stuck if there are obstacles in the way.
pub fn chase_move(
    mob_pos: &Position,
    player_pos: &Position,
    map_width: u16,
    map_height: u16,
    tiles: &[TileType],
) -> Option<(u16, u16)> {
    let dx = player_pos.x as i16 - mob_pos.x as i16;
    let dy = player_pos.y as i16 - mob_pos.y as i16;

    let step_x = if dx > 0 { 1 } else if dx < 0 { -1 } else { 0 };
    let step_y = if dy > 0 { 1 } else if dy < 0 { -1 } else { 0 };

    let new_x = (mob_pos.x as i16 + step_x) as u16;
    let new_y = (mob_pos.y as i16 + step_y) as u16;

    // Check bounds and tile type
    if new_x < map_width && new_y < map_height {
        let idx = (new_y * map_width + new_x) as usize;
        if tiles[idx] != TileType::Wall {
            return Some((new_x, new_y));
        }
    }
    None
}

//Flee is a simple greedy algorithm that moves the mob one step away from the player, avoiding walls.
//It does not use pathfinding, so it may get stuck if there are obstacles in the way.
pub fn flee_move(
    mob_pos: &Position,
    player_pos: &Position,
    map_width: u16,
    map_height: u16,
    tiles: &[TileType],
) -> Option<(u16, u16)> {
    let dx = player_pos.x as i16 - mob_pos.x as i16;
    let dy = player_pos.y as i16 - mob_pos.y as i16;

    // Inverted: step away from the player instead of toward them
    let step_x = if dx > 0 { -1 } else if dx < 0 { 1 } else { 0 };
    let step_y = if dy > 0 { -1 } else if dy < 0 { 1 } else { 0 };

    let new_x = (mob_pos.x as i16 + step_x) as u16;
    let new_y = (mob_pos.y as i16 + step_y) as u16;

    // Check bounds and tile type
    if new_x < map_width && new_y < map_height {
        let idx = (new_y * map_width + new_x) as usize;
        if tiles[idx] != TileType::Wall {
            return Some((new_x, new_y));
        }
    }
    None
}

//Confused is a random movement algorithm that moves the mob in a random direction, avoiding walls.
//It's associated with a counter that decrements each turn, and when it reaches zero, the mob returns to its normal movement type.
pub fn confused_move(
    mob_pos: &Position,
    map_width: u16,
    map_height: u16,
    tiles: &[TileType],
) -> Option<(u16, u16)> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let directions = [
        (0, -1),  // Up
        (0, 1),   // Down
        (-1, 0),  // Left
        (1, 0),   // Right
        (-1, -1), // Up-Left
        (1, -1),  // Up-Right
        (-1, 1),  // Down-Left
        (1, 1),   // Down-Right
    ];
    let (step_x, step_y) = directions[rng.gen_range(0..directions.len())];
    let new_x = (mob_pos.x as i16 + step_x) as u16;
    let new_y = (mob_pos.y as i16 + step_y) as u16;

    // Check bounds and tile type
    if new_x < map_width && new_y < map_height {
        let idx = (new_y * map_width + new_x) as usize;
        if tiles[idx] != TileType::Wall {
            return Some((new_x, new_y));
        }
    }
    None
}

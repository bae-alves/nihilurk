use bevy_ecs::prelude::*;
use crossterm::style::Color;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

#[derive(Component)]
pub struct Renderable {
    pub glyph: char,
    pub color: Color,
}

#[derive(Resource)]
pub struct GameState {
    pub is_running: bool,
}

#[derive(PartialEq, Copy, Clone)]
enum TileType {
    Wall, Floor
}

impl GameState {
    pub fn new() -> Self {
        Self { is_running: true }
    }
}


//Helper function to create a test map layout
pub fn create_test_map(world: &mut World) {
    let map_width = 80;
    let map_height = 22;

    // 1. Initialize empty floor grid
    let mut tiles = vec![TileType::Floor; (map_width * map_height) as usize];

    // 2. Set border walls
    for y in 0..map_height {
        for x in 0..map_width {
            if x == 0 || x == map_width - 1 || y == 0 || y == map_height - 1 {
                let idx = (y * map_width + x) as usize;
                tiles[idx] = TileType::Wall;
            }
        }
    }

    // 3. Add random walls into the grid
    for _ in 0..400 {
        let x = 1 + (getrandom::u32().unwrap() as u16 % (map_width - 1));
        let y = 1 + (getrandom::u32().unwrap() as u16 % (map_height - 1));
        let idx = (y * map_width + x) as usize;
        tiles[idx] = TileType::Wall;
    }

    // 4. Spawn exactly ONE entity per coordinate
    for y in 0..map_height {
        for x in 0..map_width {
            let idx = (y * map_width + x) as usize;
            match tiles[idx] {
                TileType::Floor => {
                    world.spawn((
                        Position { x, y },
                        Renderable { glyph: '.', color: Color::White },
                    ));
                }
                TileType::Wall => {
                    world.spawn((
                        Position { x, y },
                        Renderable { glyph: '#', color: Color::Green },
                        Wall,
                    ));
                }
            }
        }
    }
}

pub fn initialize_world(world: &mut World) {
    world.insert_resource(GameState::new());
    create_test_map(world);
    world.spawn((
        Player,
        Position { x: 10, y: 10 },
        Renderable {
            glyph: '@',
            color: Color::Yellow,
        },
    ));
}
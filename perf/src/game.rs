//! The load roog actually produces, as opposed to the load the reel produces.
//!
//! The reel exists to find the cliff: 800 motes a frame, thirty times a second,
//! two to three orders of magnitude past anything the game asks for. That is
//! the right question on a desktop, where the answer is "nowhere near it", and
//! the wrong one on a Raspberry Pi Zero, where the answer is "immediately" and
//! tells you nothing about whether roog is playable there.
//!
//! So this module builds the other workload: a real floor, drawn the way
//! `engine/src/view.rs` draws it, animated by the batches the real
//! `models::Particles` constructors queue when you hit something, zap a wand,
//! or kill a monster. One batch per turn, played out frame by frame until the
//! last mote dies, then the next turn -- which is exactly the loop
//! `view.rs::play_particles` runs.
//!
//! `compat/` gates on this one. The reel run is kept as the ceiling.
//!
//! # What is real here and what is a stand-in
//!
//! Real: the floor (`models::initialize_world`, so the same generator, the same
//! rooms, the same monsters and floor loot a seed produces), the tile glyphs
//! and colours ([`models::tile_appearance`]), which monsters are visible
//! ([`models::visibility_system`]), the blast radii
//! ([`models::constants::wands`]), and every particle batch -- these are calls
//! to the same constructors the game calls, not imitations of them.
//!
//! A stand-in: the text in the status and log rows. The game's HUD is built by
//! `engine`, which this crate cannot reach; what is painted here is the same
//! number of cells in the same rows, which is all those rows cost. Nothing else
//! is approximated.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use models::constants::wands::{BLAST_RADIUS, GRENADE_RADIUS};
use models::{
    BlastPalette, GameLog, GameRng, Hidden, MAP_HEIGHT, MAP_WIDTH, Map, Particles, Player,
    PlayerName, Position, Renderable, RngSeed, SeedableRng, TileType, Viewshed, initialize_world,
    on_map, tile_appearance, visibility_system,
};
use models::{ChaCha12Rng, Fighter};

use crate::frames::{Frame, Ink, Reel};

/// Map rows the log occupies, in the map-relative coordinates the rig's ink
/// uses. `engine/src/view.rs` paints the map at `y + 1` on an 80x25 grid, so
/// map row 22 and 23 land on grid rows 23 and 24 -- the log. The status line
/// sits on grid row 0, which map-relative coordinates cannot address; it is 80
/// of the grid's 2000 cells and the one part of the screen this load leaves
/// out.
const LOG_ROWS: [u16; 2] = [MAP_HEIGHT, MAP_HEIGHT + 1];

/// The seed the floor is built from when none is given. Fixed, because the
/// whole point is that two runs of the compat matrix on two machines measure
/// the same dungeon: a floor with more rooms in it is a floor with more cells
/// to paint.
pub const DEFAULT_SEED: u64 = 0x0000_1F00_D00D_2600;

/// One turn's worth of animation, as a name for the report.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    /// A landed melee blow: one spark, and the blood it throws.
    Hit,
    /// A wand bolt racing to its target, and the flourish where it lands.
    Zap,
    /// A fire wand's blast disc, and the smoke that follows it.
    Blast,
    /// A thrown dagger in flight.
    Throw,
    /// A kill: the corpse flung clear, bone shrapnel after it.
    Kill,
    /// A thrown wand bursting: the widest animation in the game.
    Grenade,
}

impl Beat {
    /// The cycle, in the order a turn loop walks it. Weighted the way a real
    /// run is: you land far more blows than you throw grenades, so `Hit`
    /// appears more than once and `Grenade` appears once.
    pub const CYCLE: [Beat; 10] = [
        Beat::Hit,
        Beat::Hit,
        Beat::Zap,
        Beat::Hit,
        Beat::Throw,
        Beat::Hit,
        Beat::Blast,
        Beat::Hit,
        Beat::Kill,
        Beat::Grenade,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Beat::Hit => "hit",
            Beat::Zap => "zap",
            Beat::Blast => "blast",
            Beat::Throw => "throw",
            Beat::Kill => "kill",
            Beat::Grenade => "grenade",
        }
    }
}

/// A real floor, plus the turn loop that animates it.
///
/// `Clone` so `--workload all` can measure the three workloads against a floor
/// that is not merely equivalent but identical, the way the reel run shares one
/// parse. Cloning is a `Vec<Ink>` of about a thousand cells; regenerating from
/// the seed would give the same dungeon, but three map generations inside one
/// report is three chances for something to differ.
#[derive(Clone)]
pub struct Turns {
    /// The floor as the game paints it, resolved once. Static for the length of
    /// an animation, which is not an approximation: the turn has already
    /// resolved by the time `play_particles` runs, so nothing on the map moves
    /// between one animation frame and the next. Only the motes do.
    floor: Frame,
    player: (u16, u16),
    target: (u16, u16),
    /// Which beat of [`Beat::CYCLE`] the next turn plays.
    beat: usize,
    /// Turns animated so far, and motes queued over all of them.
    pub turns: u64,
    pub spawned: u64,
}

impl Turns {
    /// Build a floor and the turn loop over it.
    ///
    /// The world is thrown away once the floor is drawn: what the rig needs is
    /// the picture and two coordinates, and keeping a live `World` around would
    /// put ECS iteration inside a measurement that is supposed to be about the
    /// particle layer and the redraw.
    pub fn new(seed: u64) -> Self {
        let mut world = World::new();
        world.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
        world.insert_resource(RngSeed(seed));
        world.init_resource::<GameLog>();
        world.insert_resource(PlayerName {
            what: "COMPAT".into(),
        });
        initialize_world(&mut world);

        // The real visibility pass, so the monsters this floor draws are the
        // ones the game would draw: out-of-view mobs come back `Hidden` and are
        // skipped below, exactly as in `view.rs`.
        let mut schedule = Schedule::default();
        schedule.add_systems(visibility_system);
        schedule.run(&mut world);

        let player = player_position(&mut world);
        let target = nearest_mob(&mut world, player)
            .unwrap_or((player.0.saturating_add(6).min(MAP_WIDTH - 1), player.1));

        Self {
            floor: Frame {
                ink: paint_floor(&mut world),
            },
            player,
            target,
            beat: 0,
            turns: 0,
            spawned: 0,
        }
    }

    /// The floor, as a one-frame reel.
    ///
    /// A reel of one frame is the honest shape for this load rather than a
    /// trick: the base layer genuinely does not change while a batch plays out,
    /// so `Scene` painting frame 0 every time is what the game does. It also
    /// means the redraw workloads measure what an animation frame really costs
    /// -- a full repaint of the map, a diff over all 2000 cells, and escape
    /// sequences for the handful of cells the motes actually changed.
    pub fn reel(&self) -> Reel {
        Reel {
            frames: vec![Frame {
                ink: self.floor.ink.clone(),
            }],
        }
    }

    /// Cells the floor paints per frame. Reported so a run says how big the
    /// picture being repainted was.
    pub fn floor_cells(&self) -> usize {
        self.floor.ink.len()
    }

    /// Queue the next turn's batch, if the last one has finished playing.
    ///
    /// Returns the motes queued, or 0 on a frame that is still playing one out.
    /// The `any_alive` gate is the whole model: `view.rs::play_particles`
    /// queues a batch when a turn resolves and then renders frames until the
    /// last mote dies, so a frame either continues an animation or starts one,
    /// and the game never has two turns' batches in the air at once.
    pub fn step(&mut self, fx: &mut Particles) -> u64 {
        if fx.any_alive() {
            return 0;
        }
        let beat = Beat::CYCLE[self.beat % Beat::CYCLE.len()];
        self.beat = self.beat.wrapping_add(1);
        self.turns += 1;

        let before = fx.live.len();
        self.queue(fx, beat);
        let queued = (fx.live.len() - before) as u64;
        self.spawned += queued;
        queued
    }

    /// The batches themselves -- every one a call into `models::Particles`.
    fn queue(&self, fx: &mut Particles, beat: Beat) {
        let (px, py) = self.player;
        let (tx, ty) = self.target;
        match beat {
            Beat::Hit => {
                fx.hit_spark(tx, ty);
                let spray = self.line_from(self.target, 3);
                let travel = fx.blood_streak(&spray);
                if let Some(&(bx, by)) = spray.last() {
                    fx.blood_hit(bx, by, travel);
                }
            }
            Beat::Zap => {
                let path = self.path_to_target();
                let travel = fx.beam(&path, Color::Cyan);
                fx.impact_sparks(tx, ty, Color::Cyan, travel);
            }
            Beat::Blast => {
                let cells = disc(self.target, BLAST_RADIUS);
                fx.explosion(&cells, BlastPalette::Fire);
                fx.smoke_burst(&cells);
            }
            Beat::Throw => {
                let path = self.path_to_target();
                fx.hurl(&path, ')', Color::Grey);
            }
            Beat::Kill => {
                let flight = self.line_from(self.target, 4);
                let travel = fx.death_fling(&flight, Color::Red);
                if let Some(&(bx, by)) = flight.last() {
                    fx.blood_hit(bx, by, travel);
                }
                for shard in 0..3 {
                    fx.bone_shard(&self.line_from(self.target, 2 + shard), '·');
                }
            }
            Beat::Grenade => {
                let cells = disc(self.target, GRENADE_RADIUS);
                fx.explosion(&cells, BlastPalette::Fire);
                fx.smoke_burst(&cells);
                fx.secondary_burst(tx, ty, GRENADE_RADIUS, BlastPalette::Fire);
                let _ = (px, py);
            }
        }
    }

    /// The traced line from the player to the target.
    ///
    /// `models::helpers::get_line` is the real Bresenham and is crate-private,
    /// so this walks the straight run between the two instead. The difference
    /// is which cells light up, never how many: every cell on a path is one
    /// `push` of one `Particle` regardless of where it sits, so the cost this
    /// measures is the cost the game pays.
    fn path_to_target(&self) -> Vec<(u16, u16)> {
        let (px, py) = (self.player.0 as i32, self.player.1 as i32);
        let (tx, ty) = (self.target.0 as i32, self.target.1 as i32);
        let steps = (tx - px).abs().max((ty - py).abs()).max(1);
        (1..=steps)
            .filter_map(|i| {
                let x = px + (tx - px) * i / steps;
                let y = py + (ty - py) * i / steps;
                on_map(x, y)
            })
            .collect()
    }

    /// A short line radiating from `origin` -- a blood spray, a flung corpse, a
    /// bone shard. Direction is away from the player, which is where a blow
    /// throws things.
    fn line_from(&self, origin: (u16, u16), len: u16) -> Vec<(u16, u16)> {
        let dx = (origin.0 as i32 - self.player.0 as i32).signum().max(-1);
        let dy = (origin.1 as i32 - self.player.1 as i32).signum();
        let (dx, dy) = if dx == 0 && dy == 0 { (1, 0) } else { (dx, dy) };
        (1..=len as i32)
            .filter_map(|i| on_map(origin.0 as i32 + dx * i, origin.1 as i32 + dy * i))
            .collect()
    }
}

/// Every tile within `radius` of a centre, tagged with its distance.
///
/// The same disc `models::items::wands` builds before calling
/// `Particles::explosion`, minus the line-of-sight check -- which removes cells
/// and so can only make a blast cheaper. Measuring the unblocked disc is
/// measuring the blast at its widest, which is the one worth knowing.
fn disc(center: (u16, u16), radius: f32) -> Vec<(u16, u16, f32)> {
    let r = radius as i32;
    let mut cells = Vec::new();
    for dy in -r..=r {
        for dx in -r..=r {
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist > radius {
                continue;
            }
            if let Some((x, y)) = on_map(center.0 as i32 + dx, center.1 as i32 + dy) {
                cells.push((x, y, dist));
            }
        }
    }
    cells
}

/// The floor as cells to paint, mirroring the terrain and actor passes of
/// `engine/src/view.rs::render`.
///
/// One deliberate difference: `view.rs` skips tiles the player has not explored
/// yet, and this paints the lot. A fully-explored floor is the most expensive
/// map the game ever draws, and a hardware floor test wants the expensive one
/// -- otherwise the answer depends on how far into the level the imaginary
/// player had walked.
fn paint_floor(world: &mut World) -> Vec<Ink> {
    let map = world.resource::<Map>().clone();
    let visible: Vec<(u16, u16)> = world
        .query_filtered::<&Viewshed, With<Player>>()
        .iter(world)
        .next()
        .map(|v| v.visible_tiles.clone())
        .unwrap_or_default();

    let mut ink = Vec::with_capacity(MAP_WIDTH as usize * MAP_HEIGHT as usize);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let tile = map.tile(x, y);
            // Rogue only draws the walls that frame a room; corridor walls stay
            // dark so passages read as tunnels. Same test as `view.rs`.
            if tile == TileType::Wall && !map.is_room_wall(x, y) {
                continue;
            }
            let (glyph, lit) = tile_appearance(tile);
            let color = if visible.contains(&(x, y)) {
                lit
            } else {
                Color::DarkGrey
            };
            ink.push(Ink { x, y, glyph, color });
        }
    }

    // Actors and floor loot, over the terrain, in the order `view.rs` paints
    // them. `Without<Hidden>` is what keeps an out-of-view monster off the
    // screen here exactly as it does in the game.
    let actors: Vec<Ink> = world
        .query_filtered::<(&Position, &Renderable), Without<Hidden>>()
        .iter(world)
        .map(|(pos, r)| Ink {
            x: pos.x,
            y: pos.y,
            glyph: r.glyph,
            color: r.color,
        })
        .collect();
    ink.extend(actors);

    // The log rows. See `LOG_ROWS`: the cells are real, the words are not.
    for (row, text) in LOG_ROWS.iter().zip([
        "You hit the kobold. The kobold dies!",
        "You feel a surge of power.",
    ]) {
        for (i, glyph) in text.chars().enumerate() {
            ink.push(Ink {
                x: i as u16,
                y: *row,
                glyph,
                color: Color::Grey,
            });
        }
    }
    ink
}

fn player_position(world: &mut World) -> (u16, u16) {
    world
        .query_filtered::<&Position, With<Player>>()
        .iter(world)
        .next()
        .map(|p| (p.x, p.y))
        .unwrap_or((MAP_WIDTH / 2, MAP_HEIGHT / 2))
}

/// The monster nearest the player -- who a blow, a bolt or a blast is aimed at.
/// Chebyshev distance, because that is the metric a grid with diagonals moves
/// on.
fn nearest_mob(world: &mut World, from: (u16, u16)) -> Option<(u16, u16)> {
    world
        .query_filtered::<&Position, (With<Fighter>, Without<Player>)>()
        .iter(world)
        .map(|p| (p.x, p.y))
        .min_by_key(|&(x, y)| {
            let dx = (x as i32 - from.0 as i32).abs();
            let dy = (y as i32 - from.1 as i32).abs();
            dx.max(dy)
        })
}

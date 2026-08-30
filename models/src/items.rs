use bevy_ecs::{entity::Entity, prelude::Bundle, world::World};
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;
use crate::{components::*, map::{GameRng, Map}, particles::Particles, helpers::{apply_damage, get_entities_at_position, get_line}};
use crate::identify::Identified;

#[derive(Bundle)]
pub struct ItemBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub value: Value,
    pub item: Item,
}

impl ItemBundle {
    pub fn gold_coin(position: Position) -> Self {
        Self {
            name: Name { what: String::from("gold coin") },
            glyph: Renderable { glyph: '$', color: Color::Yellow },
            position,
            value: Value { amount: 1000 },
            item: Item,
        }
    }

    pub fn silver_coin(position: Position) -> Self {
        Self {
            name: Name { what: String::from("silver coin") },
            glyph: Renderable { glyph: '$', color: Color::Grey },
            position,
            value: Value { amount: 100 },
            item: Item,
        }
    }
}

/// The Element of Yoord: the relic each run retrieves from the deepest floor,
/// spawned in place of that floor's down-stair. Carrying it flips the staircase
/// rules (see [`crate::map::change_level`]) so the player can climb back out.
#[derive(Bundle)]
pub struct AmuletBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub value: Value,
    pub item: Item,
    pub amulet: Amulet,
}

impl AmuletBundle {
    pub fn element_of_yoord(position: Position) -> Self {
        Self {
            name: Name { what: String::from("The Element of Yoord") },
            glyph: Renderable { glyph: '&', color: Color::Yellow },
            position,
            value: Value { amount: 25000 },
            item: Item,
            amulet: Amulet,
        }
    }
}
#[derive(Bundle)]
pub struct PotionBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub potion: Potion,
    pub consume: Consume,
}

impl PotionBundle {
    /// Potions draw as `!` and are quaffed once, then gone.
    fn new(name: &str, color: Color, effect: PotionEffect, position: Position) -> Self {
        Self {
            name: Name { what: name.to_string() },
            glyph: Renderable { glyph: '!', color },
            position,
            item: Item,
            potion: Potion { effect },
            consume: Consume,
        }
    }

    pub fn confusion(position: Position) -> Self {
        Self::new("potion of confusion", Color::Magenta, PotionEffect::Confusion, position)
    }
    pub fn paralysis(position: Position) -> Self {
        Self::new("potion of paralysis", Color::DarkGrey, PotionEffect::Paralysis, position)
    }
    pub fn poison(position: Position) -> Self {
        Self::new("potion of poison", Color::Green, PotionEffect::Poison, position)
    }
    pub fn gain_strength(position: Position) -> Self {
        Self::new("potion of gain strength", Color::Red, PotionEffect::GainStrength, position)
    }
    pub fn see_invisible(position: Position) -> Self {
        Self::new("potion of see invisible", Color::Cyan, PotionEffect::SeeInvisible, position)
    }
    pub fn healing(position: Position) -> Self {
        Self::new("potion of healing", Color::Red, PotionEffect::Healing, position)
    }
    pub fn monster_detection(position: Position) -> Self {
        Self::new("potion of monster detection", Color::Yellow, PotionEffect::MonsterDetection, position)
    }
    pub fn magic_detection(position: Position) -> Self {
        Self::new("potion of magic detection", Color::Yellow, PotionEffect::MagicDetection, position)
    }
    pub fn raise_level(position: Position) -> Self {
        Self::new("potion of raise level", Color::White, PotionEffect::RaiseLevel, position)
    }
    pub fn extra_healing(position: Position) -> Self {
        Self::new("potion of extra healing", Color::Red, PotionEffect::ExtraHealing, position)
    }
    pub fn haste_self(position: Position) -> Self {
        Self::new("potion of haste self", Color::DarkYellow, PotionEffect::Haste, position)
    }
    pub fn restore_strength(position: Position) -> Self {
        Self::new("potion of restore strength", Color::Red, PotionEffect::RestoreStrength, position)
    }
    pub fn blindness(position: Position) -> Self {
        Self::new("potion of blindness", Color::DarkGrey, PotionEffect::Blindness, position)
    }
    pub fn thirst_quenching(position: Position) -> Self {
        Self::new("potion of thirst quenching", Color::Blue, PotionEffect::Water, position)
    }
}

fn apply_potion_effect(world: &mut World, user: Entity, effect: PotionEffect) {
    match effect {
        PotionEffect::Healing => {
            if let Some(mut fighter) = world.get_mut::<Fighter>(user) {
                fighter.hp = std::cmp::min(fighter.hp + 10, fighter.max_hp);
                world.resource_mut::<GameLog>().add("Healing!".to_string());
            }
        }
        _ => { /* handle other potion effects */ }
    }
}

#[derive(Bundle)]
pub struct WandBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub wand: Wand,
    pub ranged: Ranged,
    pub battery: Battery,
}

impl WandBundle {
    /// Wands/staves draw as `/`. `range` feeds the targeting reticle and
    /// `charges` the battery.
    fn new(name: &str, color: Color, effect: WandEffect, range: i32, charges: i8, position: Position) -> Self {
        Self {
            name: Name { what: name.to_string() },
            glyph: Renderable { glyph: '/', color },
            position,
            item: Item,
            wand: Wand { effect },
            ranged: Ranged { range },
            battery: Battery { charges },
        }
    }

    pub fn light(position: Position) -> Self {
        Self::new("wand of light", Color::Yellow, WandEffect::Light, 8, 5, position)
    }
    pub fn striking(position: Position) -> Self {
        Self::new("wand of striking", Color::White, WandEffect::Striking, 6, 4, position)
    }
    pub fn lightning(position: Position) -> Self {
        Self::new("wand of lightning", Color::Cyan, WandEffect::Lightning, 8, 3, position)
    }
    pub fn fire(position: Position) -> Self {
        Self::new("wand of fire", Color::Red, WandEffect::Fire, 8, 3, position)
    }
    pub fn cold(position: Position) -> Self {
        Self::new("wand of cold", Color::Blue, WandEffect::Cold, 8, 3, position)
    }
    pub fn polymorph(position: Position) -> Self {
        Self::new("wand of polymorph", Color::Magenta, WandEffect::Polymorph, 6, 5, position)
    }
    pub fn magic_missile(position: Position) -> Self {
        Self::new("wand of magic missile", Color::Cyan, WandEffect::MagicMissile, 6, 5, position)
    }
    pub fn haste_monster(position: Position) -> Self {
        Self::new("wand of haste monster", Color::DarkYellow, WandEffect::HasteMonster, 6, 4, position)
    }
    pub fn slow_monster(position: Position) -> Self {
        Self::new("wand of slow monster", Color::DarkCyan, WandEffect::SlowMonster, 6, 5, position)
    }
    pub fn drain_life(position: Position) -> Self {
        Self::new("wand of drain life", Color::DarkRed, WandEffect::DrainLife, 6, 3, position)
    }
    pub fn nothing(position: Position) -> Self {
        Self::new("wand of nothing", Color::DarkGrey, WandEffect::Nothing, 6, 3, position)
    }
    pub fn teleport_away(position: Position) -> Self {
        Self::new("wand of teleport away", Color::Green, WandEffect::TeleportAway, 8, 4, position)
    }
    pub fn teleport_to(position: Position) -> Self {
        Self::new("wand of teleport to", Color::Green, WandEffect::TeleportTo, 8, 4, position)
    }
    pub fn cancellation(position: Position) -> Self {
        Self::new("wand of cancellation", Color::DarkMagenta, WandEffect::Cancellation, 6, 5, position)
    }
}

fn apply_wand_effect(world: &mut World, user: Entity, target: Option<Position>, effect: WandEffect) {
    let target_pos = match target {
        Some(pos) => pos,
        None => return, // Safety catch: Wands require targets!
    };

    // Pega a posição do usuário para cálculos de trajetória e distância
    let user_pos = match world.get::<Position>(user) {
        Some(pos) => *pos,
        None => return,
    };

    // Bolt wands: travel a straight line to the target, damaging everything on
    // the way. `None` means this effect isn't a damaging bolt. The colour is the
    // one the animated beam streaks in.
    let bolt: Option<(i32, &str, Color)> = match effect {
        WandEffect::MagicMissile => Some((10, "A brilliant cyan bolt leaps from the wand!", Color::Cyan)),
        WandEffect::Lightning => Some((20, "A forking bolt of lightning cracks out!", Color::Yellow)),
        WandEffect::Striking => Some((14, "An invisible fist hammers down the line!", Color::White)),
        WandEffect::DrainLife => Some((12, "A tendril of black light drinks the life from its path.", Color::DarkMagenta)),
        _ => None,
    };

    match effect {
        _ if bolt.is_some() => {
            let (damage, msg, color) = bolt.unwrap();
            world.resource_mut::<GameLog>().add(msg.to_string());
            let map = world.resource::<Map>().clone();
            let line_points = get_line(user_pos, target_pos);
            let mut beam_cells: Vec<(u16, u16)> = Vec::new();
            for pos in line_points {
                if map.blocks(pos.x, pos.y) {
                    break;
                }
                if !(pos.x == user_pos.x && pos.y == user_pos.y) {
                    beam_cells.push((pos.x, pos.y));
                }
                let entities_at_pos = get_entities_at_position(world, pos);
                for entity in entities_at_pos {
                    if entity != user {
                        apply_damage(world, entity, damage);
                    }
                }
            }
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.beam(&beam_cells, color);
            }
        }
        WandEffect::Fire | WandEffect::Cold => {
            let is_fire = effect == WandEffect::Fire;
            let (msg, damage) = if is_fire {
                ("A roaring sphere of fire erupts!", 25)
            } else {
                ("A blast of freezing air detonates!", 18)
            };
            world.resource_mut::<GameLog>().add(msg.to_string());

            // Radius of the blast disc, in tiles.
            let radius: f32 = 3.0;
            let map = world.resource::<Map>().clone();
            let cx = target_pos.x as i32;
            let cy = target_pos.y as i32;
            let r = radius.ceil() as i32;

            // Every tile within the disc that the blast centre has line of sight
            // to (walls stop the flames), tagged with its distance from centre
            // so the animation can ripple outward.
            let mut blast_cells: Vec<(u16, u16, f32)> = Vec::new();
            for dy in -r..=r {
                for dx in -r..=r {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist > radius {
                        continue;
                    }
                    let Some((tx, ty)) = crate::particles::on_map(cx + dx, cy + dy) else {
                        continue;
                    };
                    let ray = get_line(target_pos, Position { x: tx, y: ty });
                    let blocked = ray
                        .iter()
                        .any(|p| map.blocks(p.x, p.y) && !(p.x == tx && p.y == ty));
                    if !blocked {
                        blast_cells.push((tx, ty, dist));
                    }
                }
            }

            // Damage every fighter standing in a blast cell.
            let cell_set: std::collections::HashSet<(u16, u16)> =
                blast_cells.iter().map(|&(x, y, _)| (x, y)).collect();
            let mut affected_entities = Vec::new();
            let mut query = world.query::<(Entity, &Position)>();
            for (entity, pos) in query.iter(world) {
                if cell_set.contains(&(pos.x, pos.y)) {
                    affected_entities.push(entity);
                }
            }
            for entity in affected_entities {
                apply_damage(world, entity, damage);
            }

            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.explosion(&blast_cells, is_fire);
            }
        }
        // Utility wands (polymorph, haste/slow, teleport, cancellation, light,
        // nothing): flavour only for now.
        _ => {
            let msg = match effect {
                WandEffect::Light => "The wand sheds a warm, steady glow.",
                WandEffect::Polymorph => "Reality shudders around your target.",
                WandEffect::HasteMonster => "Your target blurs with sudden speed. Nice going.",
                WandEffect::SlowMonster => "Your target lurches into slow motion.",
                WandEffect::TeleportAway => "Your target is yanked elsewhere.",
                WandEffect::TeleportTo => "Your target is dragged to your feet.",
                WandEffect::Cancellation => "Your target's magic sputters and dies.",
                WandEffect::Nothing => "The wand does nothing. It was well named.",
                _ => "The wand discharges with a faint hiss.",
            };
            world.resource_mut::<GameLog>().add(msg.to_string());
        }
    }
}

#[derive(Bundle)]
pub struct WeaponsBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub wield: Wield,
}

impl WeaponsBundle {
    /// Shared constructor. `power_increase` bumps the wielder's `power` die size
    /// via [`Wield::pow_increase`]. Weapons all draw as `)` in the classic Rogue
    /// style, tinted by material.
    fn new(name: &str, color: Color, power_increase: i8, position: Position) -> Self {
        Self {
            name: Name { what: name.to_string() },
            glyph: Renderable { glyph: ')', color },
            position,
            item: Item,
            wield: Wield { wielder: None, pow_increase: power_increase, pow_bonus: 0 },
        }
    }

    /// Dagger — WC 1, power increase 4.
    pub fn dagger(position: Position) -> Self {
        Self::new("dagger", Color::Grey, 4, position)
    }

    /// Mace — WC 2, power increase 6.
    pub fn mace(position: Position) -> Self {
        Self::new("mace", Color::DarkGrey, 6, position)
    }

    /// Long Sword — WC 3, power increase 8.
    pub fn long_sword(position: Position) -> Self {
        Self::new("long sword", Color::White, 8, position)
    }

    /// Two-Handed Sword — WC 4, power increase 10.
    pub fn two_handed_sword(position: Position) -> Self {
        Self::new("two-handed sword", Color::Cyan, 10, position)
    }
}

#[derive(Bundle)]
pub struct ArmorBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub wear: Wear,
}

impl ArmorBundle {
    /// Shared constructor. `armor_increase` bumps the wearer's `armor` die size
    /// via [`Wear::arm_increase`]. Armor draws as `]` in the classic Rogue style.
    fn new(name: &str, color: Color, armor_increase: i8, position: Position) -> Self {
        Self {
            name: Name { what: name.to_string() },
            glyph: Renderable { glyph: ']', color },
            position,
            item: Item,
            wear: Wear { wearer: None, arm_increase: armor_increase, arm_bonus: 0 },
        }
    }

    /// Leather armor — armor increase 2.
    pub fn leather_armor(position: Position) -> Self {
        Self::new("leather armor", Color::DarkYellow, 2, position)
    }

    /// Ring mail — armor increase 3.
    pub fn ring_mail(position: Position) -> Self {
        Self::new("ring mail", Color::Grey, 3, position)
    }

    /// Studded leather armor — armor increase 4.
    pub fn studded_leather_armor(position: Position) -> Self {
        Self::new("studded leather armor", Color::DarkYellow, 4, position)
    }

    /// Scale mail — armor increase 5.
    pub fn scale_mail(position: Position) -> Self {
        Self::new("scale mail", Color::Grey, 5, position)
    }

    /// Chain mail — armor increase 6.
    pub fn chain_mail(position: Position) -> Self {
        Self::new("chain mail", Color::Grey, 6, position)
    }

    /// Splint mail — armor increase 7.
    pub fn splint_mail(position: Position) -> Self {
        Self::new("splint mail", Color::White, 7, position)
    }

    /// Banded mail — armor increase 8.
    pub fn banded_mail(position: Position) -> Self {
        Self::new("banded mail", Color::White, 8, position)
    }

    /// Plate mail — armor increase 9.
    pub fn plate_mail(position: Position) -> Self {
        Self::new("plate mail", Color::Cyan, 9, position)
    }
}

#[derive(Bundle)]
pub struct ScrollBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub scroll: Scroll,
    pub consume: Consume,
}

impl ScrollBundle {
    /// Scrolls draw as `?` and are read once, then crumble.
    fn new(name: &str, effect: ScrollEffect, position: Position) -> Self {
        Self {
            name: Name { what: name.to_string() },
            glyph: Renderable { glyph: '?', color: Color::White },
            position,
            item: Item,
            scroll: Scroll { effect },
            consume: Consume,
        }
    }

    pub fn monster_confusion(position: Position) -> Self {
        Self::new("scroll of monster confusion", ScrollEffect::MonsterConfusion, position)
    }
    pub fn magic_mapping(position: Position) -> Self {
        Self::new("scroll of magic mapping", ScrollEffect::MagicMapping, position)
    }
    pub fn hold_monster(position: Position) -> Self {
        Self::new("scroll of hold monster", ScrollEffect::HoldMonster, position)
    }
    pub fn sleep(position: Position) -> Self {
        Self::new("scroll of sleep", ScrollEffect::Sleep, position)
    }
    pub fn enchant_armor(position: Position) -> Self {
        Self::new("scroll of enchant armor", ScrollEffect::EnchantArmor, position)
    }
    pub fn identify(position: Position) -> Self {
        Self::new("scroll of identify", ScrollEffect::Identify, position)
    }
    pub fn scare_monster(position: Position) -> Self {
        Self::new("scroll of scare monster", ScrollEffect::ScareMonster, position)
    }
    pub fn food_detection(position: Position) -> Self {
        Self::new("scroll of food detection", ScrollEffect::FoodDetection, position)
    }
    pub fn teleportation(position: Position) -> Self {
        Self::new("scroll of teleportation", ScrollEffect::Teleportation, position)
    }
    pub fn enchant_weapon(position: Position) -> Self {
        Self::new("scroll of enchant weapon", ScrollEffect::EnchantWeapon, position)
    }
    pub fn create_monster(position: Position) -> Self {
        Self::new("scroll of create monster", ScrollEffect::CreateMonster, position)
    }
    pub fn remove_curse(position: Position) -> Self {
        Self::new("scroll of remove curse", ScrollEffect::RemoveCurse, position)
    }
    pub fn aggravate_monsters(position: Position) -> Self {
        Self::new("scroll of aggravate monsters", ScrollEffect::AggravateMonsters, position)
    }
    pub fn blank_paper(position: Position) -> Self {
        Self::new("scroll of blank paper", ScrollEffect::BlankPaper, position)
    }
    pub fn vorpalize_weapon(position: Position) -> Self {
        Self::new("scroll of vorpalize weapon", ScrollEffect::VorpalizeWeapon, position)
    }
}

#[derive(Bundle)]
pub struct RingBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub item: Item,
    pub puton: PutOn,
}

impl RingBundle {
    /// Rings draw as `=` and are worn, not consumed.
    pub fn new(effect: RingEffect, position: Position) -> Self {
        let name = match effect {
            RingEffect::Protection => "ring of protection",
            RingEffect::AddStrength => "ring of add strength",
            RingEffect::SustainStrength => "ring of sustain strength",
            RingEffect::Searching => "ring of searching",
            RingEffect::SeeInvisible => "ring of see invisible",
            RingEffect::Adornment => "ring of adornment",
            RingEffect::AggravateMonster => "ring of aggravate monster",
            RingEffect::Dexterity => "ring of dexterity",
            RingEffect::IncreaseDamage => "ring of increase damage",
            RingEffect::Regeneration => "ring of regeneration",
            RingEffect::SlowDigestion => "ring of slow digestion",
            RingEffect::Teleportation => "ring of teleportation",
            RingEffect::Stealth => "ring of stealth",
            RingEffect::MaintainArmor => "ring of maintain armor",
        };
        Self {
            name: Name { what: name.to_string() },
            glyph: Renderable { glyph: '=', color: Color::Yellow },
            position,
            item: Item,
            puton: PutOn { bearer: None, effect },
        }
    }
}

/// The quality every weapon, armour and ring drop rolls when it spawns.
///
/// | Quality     | Odds | Bonus (equal-probability integer) |
/// |-------------|------|-----------------------------------|
/// | Normal      | 25%  | +0                                |
/// | Exceptional | 10%  | +1 .. +3                          |
/// | Cursed      | 65%  | -6 .. +4 (yes, a cursed item can roll positive) |
///
/// Weapons and armour apply the bonus as a flat modifier on the opposed combat
/// roll (`pow_bonus` / `arm_bonus`), never to the die size. Rings carry no
/// numeric bonus — they are simply cursed or not.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quality {
    Normal,
    Exceptional,
    Cursed,
}

impl Quality {
    fn roll(rng: &mut ChaCha12Rng) -> Self {
        match rng.gen_range(0..100) {
            0..=24 => Quality::Normal,
            25..=34 => Quality::Exceptional,
            _ => Quality::Cursed,
        }
    }
}

/// Rolls quality for a freshly spawned weapon, armour or ring and stamps the
/// result onto the entity: the flat bonus on its [`Wield`]/[`Wear`] component,
/// and a [`Curse`] tag if it came up cursed. Rings get only the tag.
pub fn enchant_equipment(world: &mut World, rng: &mut ChaCha12Rng, item: Entity) {
    let quality = Quality::roll(rng);
    let bonus: i8 = match quality {
        Quality::Normal => 0,
        Quality::Exceptional => rng.gen_range(1..=3),
        Quality::Cursed => rng.gen_range(-6..=4),
    };

    let mut entity = world.entity_mut(item);
    if let Some(mut wield) = entity.get_mut::<Wield>() {
        wield.pow_bonus = bonus;
    }
    if let Some(mut wear) = entity.get_mut::<Wear>() {
        wear.arm_bonus = bonus;
    }
    if quality == Quality::Cursed {
        entity.insert(Curse);
    }
}

/// Strips the [`Curse`] tag from every item in `user`'s pack (a scroll of remove
/// curse). Returns how many items were freed.
pub(crate) fn lift_curses(world: &mut World, user: Entity) -> usize {
    let cursed: Vec<Entity> = world
        .get::<Backpack>(user)
        .map(|bp| bp.items.iter().copied().filter(|&e| world.get::<Curse>(e).is_some()).collect())
        .unwrap_or_default();
    for &e in &cursed {
        world.entity_mut(e).remove::<Curse>();
    }
    cursed.len()
}

/// An item's display name, or a vague fallback.
pub(crate) fn item_label(world: &World, item: Entity) -> String {
    world.get::<Name>(item).map(|n| n.what.clone()).unwrap_or_else(|| "item".to_string())
}

/// Toggles `item` as `user`'s wielded weapon. Equipping first unequips whatever
/// else `user` had wielded — only one weapon at a time.
fn toggle_wield(world: &mut World, user: Entity, item: Entity) {
    let name = item_label(world, item);
    if world.get::<Wield>(item).and_then(|w| w.wielder) == Some(user) {
        if world.get::<Curse>(item).is_some() {
            world.resource_mut::<GameLog>().add(format!("You can't — the {name} is welded to your grip!"));
            return;
        }
        if let Some(mut w) = world.get_mut::<Wield>(item) {
            w.wielder = None;
        }
        world.resource_mut::<GameLog>().add(format!("You stop wielding the {name}."));
        return;
    }

    let others: Vec<Entity> = world
        .get::<Backpack>(user)
        .map(|bp| {
            bp.items
                .iter()
                .copied()
                .filter(|&e| e != item && world.get::<Wield>(e).is_some_and(|w| w.wielder == Some(user)))
                .collect()
        })
        .unwrap_or_default();
    if let Some(&stuck) = others.iter().find(|&&e| world.get::<Curse>(e).is_some()) {
        let stuck_name = item_label(world, stuck);
        world.resource_mut::<GameLog>().add(format!("You can't switch weapons — the {stuck_name} won't leave your hand."));
        return;
    }
    for e in others {
        if let Some(mut w) = world.get_mut::<Wield>(e) {
            w.wielder = None;
        }
    }
    if let Some(mut w) = world.get_mut::<Wield>(item) {
        w.wielder = Some(user);
    }
    world.resource_mut::<GameLog>().add(format!("You wield the {name}."));
}

/// Toggles `item` as `user`'s worn armour. Equipping first removes whatever else
/// `user` had worn — only one suit at a time.
fn toggle_wear(world: &mut World, user: Entity, item: Entity) {
    let name = item_label(world, item);
    if world.get::<Wear>(item).and_then(|w| w.wearer) == Some(user) {
        if world.get::<Curse>(item).is_some() {
            world.resource_mut::<GameLog>().add(format!("You can't — the {name} clings to you and won't come off!"));
            return;
        }
        if let Some(mut w) = world.get_mut::<Wear>(item) {
            w.wearer = None;
        }
        world.resource_mut::<GameLog>().add(format!("You take off the {name}."));
        return;
    }

    let others: Vec<Entity> = world
        .get::<Backpack>(user)
        .map(|bp| {
            bp.items
                .iter()
                .copied()
                .filter(|&e| e != item && world.get::<Wear>(e).is_some_and(|w| w.wearer == Some(user)))
                .collect()
        })
        .unwrap_or_default();
    if let Some(&stuck) = others.iter().find(|&&e| world.get::<Curse>(e).is_some()) {
        let stuck_name = item_label(world, stuck);
        world.resource_mut::<GameLog>().add(format!("You can't change armour — the {stuck_name} won't come off."));
        return;
    }
    for e in others {
        if let Some(mut w) = world.get_mut::<Wear>(e) {
            w.wearer = None;
        }
    }
    if let Some(mut w) = world.get_mut::<Wear>(item) {
        w.wearer = Some(user);
    }
    world.resource_mut::<GameLog>().add(format!("You put on the {name}."));
}

/// Toggles `item` as `user`'s worn ring. Mirrors [`toggle_wear`]; only one
/// ring at a time for now. Putting one on is a ring's only "use", so it's
/// also where ring identification is triggered.
fn toggle_puton(world: &mut World, user: Entity, item: Entity) {
    let name = crate::identify::display_name(world, item);
    if world.get::<PutOn>(item).and_then(|p| p.bearer) == Some(user) {
        if world.get::<Curse>(item).is_some() {
            world.resource_mut::<GameLog>().add(format!("You can't — the {name} is fused to your finger!"));
            return;
        }
        if let Some(mut p) = world.get_mut::<PutOn>(item) {
            p.bearer = None;
        }
        world.resource_mut::<GameLog>().add(format!("You remove the {name}."));
        return;
    }

    let others: Vec<Entity> = world
        .get::<Backpack>(user)
        .map(|bp| {
            bp.items
                .iter()
                .copied()
                .filter(|&e| e != item && world.get::<PutOn>(e).is_some_and(|p| p.bearer == Some(user)))
                .collect()
        })
        .unwrap_or_default();
    if let Some(&stuck) = others.iter().find(|&&e| world.get::<Curse>(e).is_some()) {
        let stuck_name = crate::identify::display_name(world, stuck);
        world.resource_mut::<GameLog>().add(format!("You can't — the {stuck_name} won't leave your finger."));
        return;
    }
    for e in others {
        if let Some(mut p) = world.get_mut::<PutOn>(e) {
            p.bearer = None;
        }
    }
    if let Some(mut p) = world.get_mut::<PutOn>(item) {
        p.bearer = Some(user);
    }
    world.resource_mut::<GameLog>().add(format!("You put on the {name}."));

    if let Some(effect) = world.get::<PutOn>(item).map(|p| p.effect) {
        let true_name = item_label(world, item);
        let newly_identified = world.resource_mut::<Identified>().rings.insert(effect);
        if newly_identified {
            world.resource_mut::<GameLog>().add(format!("That was {} {true_name}!", crate::identify::article_for(&true_name)));
        }
    }
}

/// Picks a uniformly random item in `user`'s backpack whose true type isn't
/// identified yet and identifies it directly. Used by
/// [`ScrollEffect::Identify`], which has no interactive item picker (yet).
fn identify_random_unknown_item(world: &mut World, user: Entity) {
    let candidates: Vec<Entity> = world.get::<Backpack>(user).map(|bp| bp.items.clone()).unwrap_or_default();

    let is_unidentified = |world: &World, e: Entity| -> bool {
        let identified = world.resource::<Identified>();
        world.get::<Potion>(e).is_some_and(|p| !identified.potions.contains(&p.effect))
            || world.get::<Scroll>(e).is_some_and(|s| !identified.scrolls.contains(&s.effect))
            || world.get::<Wand>(e).is_some_and(|w| !identified.wands.contains(&w.effect))
            || world.get::<PutOn>(e).is_some_and(|p| !identified.rings.contains(&p.effect))
    };

    let unknown: Vec<Entity> = candidates.into_iter().filter(|&e| is_unidentified(world, e)).collect();
    if unknown.is_empty() {
        world.resource_mut::<GameLog>().add("You already recognise everything in your pack.".to_string());
        return;
    }
    let target = {
        let mut rng = world.resource_mut::<GameRng>();
        unknown[rng.0.gen_range(0..unknown.len())]
    };

    let name = item_label(world, target);
    let potion_effect = world.get::<Potion>(target).map(|p| p.effect);
    let scroll_effect = world.get::<Scroll>(target).map(|s| s.effect);
    let wand_effect = world.get::<Wand>(target).map(|w| w.effect);
    let ring_effect = world.get::<PutOn>(target).map(|p| p.effect);

    let mut identified = world.resource_mut::<Identified>();
    if let Some(effect) = potion_effect {
        identified.potions.insert(effect);
    }
    if let Some(effect) = scroll_effect {
        identified.scrolls.insert(effect);
    }
    if let Some(effect) = wand_effect {
        identified.wands.insert(effect);
    }
    if let Some(effect) = ring_effect {
        identified.rings.insert(effect);
    }
    drop(identified);

    world.resource_mut::<GameLog>().add(format!("The scroll identifies your {name}!"));
}

fn apply_scroll_effect(world: &mut World, user: Entity, effect: ScrollEffect) {
    if effect == ScrollEffect::Identify {
        identify_random_unknown_item(world, user);
        return;
    }
    if effect == ScrollEffect::RemoveCurse {
        let freed = lift_curses(world, user);
        let msg = if freed > 0 {
            "You feel as though somebody is watching over you. Your gear loosens its grip."
        } else {
            "You feel as though somebody is watching over you."
        };
        world.resource_mut::<GameLog>().add(msg.to_string());
        return;
    }
    let msg = match effect {
        ScrollEffect::MagicMapping => "The dungeon's shape springs into your mind.",
        ScrollEffect::Teleportation => "You blink to somewhere else.",
        ScrollEffect::AggravateMonsters => "A shrill note rings out. Everything heard that.",
        ScrollEffect::CreateMonster => "The air curdles into something with teeth.",
        ScrollEffect::ScareMonster => "The parchment radiates a menacing aura.",
        ScrollEffect::BlankPaper => "The scroll is blank. Someone got the last laugh.",
        ScrollEffect::VorpalizeWeapon => "Your weapon hums with a keen new edge.",
        _ => "You read the scroll, but nothing obvious happens.",
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
}

pub fn item_system(world: &mut World) {
    let mut use_queue = world.resource_mut::<UseQueue>();
    let uses = std::mem::take(&mut use_queue.uses);
    drop(use_queue);

    for item_use in uses {
        // What the player sees right now (appearance if unidentified, true
        // name otherwise) and the item's true name, captured before any
        // despawn below could make `item_use.item` unqueryable.
        let seen_name = crate::identify::display_name(world, item_use.item);
        let true_name = item_label(world, item_use.item);

        // We store the "work to be done" here
        let mut potion_effect: Option<PotionEffect> = None;
        let mut wand_effect: Option<WandEffect> = None;
        let mut scroll_effect: Option<ScrollEffect> = None;
        let mut is_wield = false;
        let mut is_wear = false;
        let mut is_puton = false;
        let mut destroy_item = false;
        let mut return_to_inventory = false;

        {
            let mut item_entity = world.entity_mut(item_use.item);
            
            // Check for Potion
            if let Some(p) = item_entity.get::<Potion>() {
                potion_effect = Some(p.effect);
            }

            // Check for Wand
            if let Some(w) = item_entity.get::<Wand>() {
                wand_effect = Some(w.effect);
            }

            // Check for Scroll
            if let Some(s) = item_entity.get::<Scroll>() {
                scroll_effect = Some(s.effect);
            }

            // Equipment: using it toggles the equipped state (handled below).
            if item_entity.get::<Wield>().is_some() {
                is_wield = true;
            }
            if item_entity.get::<Wear>().is_some() {
                is_wear = true;
            }
            if item_entity.get::<PutOn>().is_some() {
                is_puton = true;
            }

            // Handle Wands / Battery logic
            if let Some(mut battery) = item_entity.get_mut::<Battery>() {
                battery.charges -= 1;
                if battery.charges <= 0 {
                    destroy_item = true;
                } else {
                    // Item survives! We need to put it back in the user's bag.
                    return_to_inventory = true;
                }
            }

            // Handle basic consumables
            if let Some(_consume) = item_entity.get::<Consume>() {
                destroy_item = true;
            }
        } // Drop the entity_mut borrow so we can freely use the world again

        // 0. Equipment toggles — these items always go back in the pack.
        if is_wield {
            toggle_wield(world, item_use.user, item_use.item);
            return_to_inventory = true;
        }
        if is_wear {
            toggle_wear(world, item_use.user, item_use.item);
            return_to_inventory = true;
        }
        if is_puton {
            toggle_puton(world, item_use.user, item_use.item);
            return_to_inventory = true;
        }

        // Anything the game doesn't know how to "use" is handed straight back
        // rather than vanishing into limbo.
        if !destroy_item
            && !return_to_inventory
            && potion_effect.is_none()
            && wand_effect.is_none()
            && scroll_effect.is_none()
        {
            let name = crate::identify::with_the(&item_label(world, item_use.item));
            world.resource_mut::<GameLog>().add(format!("You can't use {name} right now."));
            return_to_inventory = true;
        }

        // 1. Manage the item's physical existence
        if return_to_inventory {
            if let Some(mut backpack) = world.get_mut::<Backpack>(item_use.user) {
                if let Some(idx) = item_use.slot_idx {
                    // Put it back in its exact slot (clamp if inventory shifted somehow)
                    let insert_pos = std::cmp::min(idx, backpack.items.len());
                    backpack.items.insert(insert_pos, item_use.item);
                } else {
                    backpack.items.push(item_use.item); // Fallback
                }
            }
        }

        if destroy_item {
            let is_wand = world.get::<Wand>(item_use.item).is_some();
            let is_potion = world.get::<Potion>(item_use.item).is_some();
            let is_scroll = world.get::<Scroll>(item_use.item).is_some();

            let mut log = world.resource_mut::<GameLog>();
            if is_wand {
                log.add(format!("The {seen_name} crumbles to dust!"));
            } else if is_potion {
                log.add(format!("You drink the {seen_name}."));
            } else if is_scroll {
                log.add(format!("You read the {seen_name}."));
            } else {
                log.add("The item turns to dust!".to_string());
            }

            world.entity_mut(item_use.item).despawn();
        } else if wand_effect.is_some() {
            // Wands survive a zap (until their battery runs dry, handled
            // above), so the "you use it" beat lives here instead.
            world.resource_mut::<GameLog>().add(format!("You zap the {seen_name}."));
        }

        // 2. Dispatch to specialized functions. Using a potion, scroll or
        // wand always identifies its true type — every roguelike's
        // use-to-identify convention (rings identify on wear instead, inside
        // `toggle_puton`).
        if let Some(eff) = potion_effect {
            apply_potion_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().potions.insert(eff) {
                world.resource_mut::<GameLog>().add(format!("That was {} {true_name}!", crate::identify::article_for(&true_name)));
            }
        }

        if let Some(eff) = wand_effect {
            apply_wand_effect(world, item_use.user, item_use.target, eff);
            if world.resource_mut::<Identified>().wands.insert(eff) {
                world.resource_mut::<GameLog>().add(format!("That was {} {true_name}!", crate::identify::article_for(&true_name)));
            }
        }

        if let Some(eff) = scroll_effect {
            apply_scroll_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().scrolls.insert(eff) {
                world.resource_mut::<GameLog>().add(format!("That was {} {true_name}!", crate::identify::article_for(&true_name)));
            }
        }
    }
}
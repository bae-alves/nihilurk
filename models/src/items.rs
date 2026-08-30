use bevy_ecs::{entity::Entity, prelude::Bundle, world::World};
use crossterm::style::Color;
use crate::{components::*, map::Map, particles::Particles, helpers::{apply_damage, get_entities_at_position, get_line}};

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
            name: Name { what: String::from("Gold coin") },
            glyph: Renderable { glyph: '$', color: Color::Yellow },
            position,
            value: Value { amount: 1000 },
            item: Item,
        }
    }

    pub fn silver_coin(position: Position) -> Self {
        Self {
            name: Name { what: String::from("Silver coin") },
            glyph: Renderable { glyph: '$', color: Color::Grey },
            position,
            value: Value { amount: 100 },
            item: Item,
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
        Self::new("Potion of Confusion", Color::Magenta, PotionEffect::Confusion, position)
    }
    pub fn paralysis(position: Position) -> Self {
        Self::new("Potion of Paralysis", Color::DarkGrey, PotionEffect::Paralysis, position)
    }
    pub fn poison(position: Position) -> Self {
        Self::new("Potion of Poison", Color::Green, PotionEffect::Poison, position)
    }
    pub fn gain_strength(position: Position) -> Self {
        Self::new("Potion of Gain Strength", Color::Red, PotionEffect::GainStrength, position)
    }
    pub fn see_invisible(position: Position) -> Self {
        Self::new("Potion of See Invisible", Color::Cyan, PotionEffect::SeeInvisible, position)
    }
    pub fn healing(position: Position) -> Self {
        Self::new("Potion of Healing", Color::Red, PotionEffect::Healing, position)
    }
    pub fn monster_detection(position: Position) -> Self {
        Self::new("Potion of Monster Detection", Color::Yellow, PotionEffect::MonsterDetection, position)
    }
    pub fn magic_detection(position: Position) -> Self {
        Self::new("Potion of Magic Detection", Color::Yellow, PotionEffect::MagicDetection, position)
    }
    pub fn raise_level(position: Position) -> Self {
        Self::new("Potion of Raise Level", Color::White, PotionEffect::RaiseLevel, position)
    }
    pub fn extra_healing(position: Position) -> Self {
        Self::new("Potion of Extra Healing", Color::Red, PotionEffect::ExtraHealing, position)
    }
    pub fn haste_self(position: Position) -> Self {
        Self::new("Potion of Haste Self", Color::DarkYellow, PotionEffect::Haste, position)
    }
    pub fn restore_strength(position: Position) -> Self {
        Self::new("Potion of Restore Strength", Color::Red, PotionEffect::RestoreStrength, position)
    }
    pub fn blindness(position: Position) -> Self {
        Self::new("Potion of Blindness", Color::DarkGrey, PotionEffect::Blindness, position)
    }
    pub fn thirst_quenching(position: Position) -> Self {
        Self::new("Potion of Thirst Quenching", Color::Blue, PotionEffect::Water, position)
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
        Self::new("Wand of Light", Color::Yellow, WandEffect::Light, 8, 5, position)
    }
    pub fn striking(position: Position) -> Self {
        Self::new("Wand of Striking", Color::White, WandEffect::Striking, 6, 4, position)
    }
    pub fn lightning(position: Position) -> Self {
        Self::new("Wand of Lightning", Color::Cyan, WandEffect::Lightning, 8, 3, position)
    }
    pub fn fire(position: Position) -> Self {
        Self::new("Wand of Fire", Color::Red, WandEffect::Fire, 8, 3, position)
    }
    pub fn cold(position: Position) -> Self {
        Self::new("Wand of Cold", Color::Blue, WandEffect::Cold, 8, 3, position)
    }
    pub fn polymorph(position: Position) -> Self {
        Self::new("Wand of Polymorph", Color::Magenta, WandEffect::Polymorph, 6, 5, position)
    }
    pub fn magic_missile(position: Position) -> Self {
        Self::new("Wand of Magic Missile", Color::Cyan, WandEffect::MagicMissile, 6, 5, position)
    }
    pub fn haste_monster(position: Position) -> Self {
        Self::new("Wand of Haste Monster", Color::DarkYellow, WandEffect::HasteMonster, 6, 4, position)
    }
    pub fn slow_monster(position: Position) -> Self {
        Self::new("Wand of Slow Monster", Color::DarkCyan, WandEffect::SlowMonster, 6, 5, position)
    }
    pub fn drain_life(position: Position) -> Self {
        Self::new("Wand of Drain Life", Color::DarkRed, WandEffect::DrainLife, 6, 3, position)
    }
    pub fn nothing(position: Position) -> Self {
        Self::new("Wand of Nothing", Color::DarkGrey, WandEffect::Nothing, 6, 3, position)
    }
    pub fn teleport_away(position: Position) -> Self {
        Self::new("Wand of Teleport Away", Color::Green, WandEffect::TeleportAway, 8, 4, position)
    }
    pub fn teleport_to(position: Position) -> Self {
        Self::new("Wand of Teleport To", Color::Green, WandEffect::TeleportTo, 8, 4, position)
    }
    pub fn cancellation(position: Position) -> Self {
        Self::new("Wand of Cancellation", Color::DarkMagenta, WandEffect::Cancellation, 6, 5, position)
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
        Self::new("Dagger", Color::Grey, 4, position)
    }

    /// Mace — WC 2, power increase 6.
    pub fn mace(position: Position) -> Self {
        Self::new("Mace", Color::DarkGrey, 6, position)
    }

    /// Long Sword — WC 3, power increase 8.
    pub fn long_sword(position: Position) -> Self {
        Self::new("Long Sword", Color::White, 8, position)
    }

    /// Two-Handed Sword — WC 4, power increase 10.
    pub fn two_handed_sword(position: Position) -> Self {
        Self::new("Two-Handed Sword", Color::Cyan, 10, position)
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
        Self::new("Leather Armor", Color::DarkYellow, 2, position)
    }

    /// Ring mail — armor increase 3.
    pub fn ring_mail(position: Position) -> Self {
        Self::new("Ring Mail", Color::Grey, 3, position)
    }

    /// Studded leather armor — armor increase 4.
    pub fn studded_leather_armor(position: Position) -> Self {
        Self::new("Studded Leather Armor", Color::DarkYellow, 4, position)
    }

    /// Scale mail — armor increase 5.
    pub fn scale_mail(position: Position) -> Self {
        Self::new("Scale Mail", Color::Grey, 5, position)
    }

    /// Chain mail — armor increase 6.
    pub fn chain_mail(position: Position) -> Self {
        Self::new("Chain Mail", Color::Grey, 6, position)
    }

    /// Splint mail — armor increase 7.
    pub fn splint_mail(position: Position) -> Self {
        Self::new("Splint Mail", Color::White, 7, position)
    }

    /// Banded mail — armor increase 8.
    pub fn banded_mail(position: Position) -> Self {
        Self::new("Banded Mail", Color::White, 8, position)
    }

    /// Plate mail — armor increase 9.
    pub fn plate_mail(position: Position) -> Self {
        Self::new("Plate Mail", Color::Cyan, 9, position)
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
        Self::new("Scroll of Monster Confusion", ScrollEffect::MonsterConfusion, position)
    }
    pub fn magic_mapping(position: Position) -> Self {
        Self::new("Scroll of Magic Mapping", ScrollEffect::MagicMapping, position)
    }
    pub fn hold_monster(position: Position) -> Self {
        Self::new("Scroll of Hold Monster", ScrollEffect::HoldMonster, position)
    }
    pub fn sleep(position: Position) -> Self {
        Self::new("Scroll of Sleep", ScrollEffect::Sleep, position)
    }
    pub fn enchant_armor(position: Position) -> Self {
        Self::new("Scroll of Enchant Armor", ScrollEffect::EnchantArmor, position)
    }
    pub fn identify(position: Position) -> Self {
        Self::new("Scroll of Identify", ScrollEffect::Identify, position)
    }
    pub fn scare_monster(position: Position) -> Self {
        Self::new("Scroll of Scare Monster", ScrollEffect::ScareMonster, position)
    }
    pub fn food_detection(position: Position) -> Self {
        Self::new("Scroll of Food Detection", ScrollEffect::FoodDetection, position)
    }
    pub fn teleportation(position: Position) -> Self {
        Self::new("Scroll of Teleportation", ScrollEffect::Teleportation, position)
    }
    pub fn enchant_weapon(position: Position) -> Self {
        Self::new("Scroll of Enchant Weapon", ScrollEffect::EnchantWeapon, position)
    }
    pub fn create_monster(position: Position) -> Self {
        Self::new("Scroll of Create Monster", ScrollEffect::CreateMonster, position)
    }
    pub fn remove_curse(position: Position) -> Self {
        Self::new("Scroll of Remove Curse", ScrollEffect::RemoveCurse, position)
    }
    pub fn aggravate_monsters(position: Position) -> Self {
        Self::new("Scroll of Aggravate Monsters", ScrollEffect::AggravateMonsters, position)
    }
    pub fn blank_paper(position: Position) -> Self {
        Self::new("Scroll of Blank Paper", ScrollEffect::BlankPaper, position)
    }
    pub fn vorpalize_weapon(position: Position) -> Self {
        Self::new("Scroll of Vorpalize Weapon", ScrollEffect::VorpalizeWeapon, position)
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
            RingEffect::Protection => "Ring of Protection",
            RingEffect::AddStrength => "Ring of Add Strength",
            RingEffect::SustainStrength => "Ring of Sustain Strength",
            RingEffect::Searching => "Ring of Searching",
            RingEffect::SeeInvisible => "Ring of See Invisible",
            RingEffect::Adornment => "Ring of Adornment",
            RingEffect::AggravateMonster => "Ring of Aggravate Monster",
            RingEffect::Dexterity => "Ring of Dexterity",
            RingEffect::IncreaseDamage => "Ring of Increase Damage",
            RingEffect::Regeneration => "Ring of Regeneration",
            RingEffect::SlowDigestion => "Ring of Slow Digestion",
            RingEffect::Teleportation => "Ring of Teleportation",
            RingEffect::Stealth => "Ring of Stealth",
            RingEffect::MaintainArmor => "Ring of Maintain Armor",
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

/// An item's display name, or a vague fallback.
fn item_label(world: &World, item: Entity) -> String {
    world.get::<Name>(item).map(|n| n.what.clone()).unwrap_or_else(|| "item".to_string())
}

/// Toggles `item` as `user`'s wielded weapon. Equipping first unequips whatever
/// else `user` had wielded — only one weapon at a time.
fn toggle_wield(world: &mut World, user: Entity, item: Entity) {
    let name = item_label(world, item);
    if world.get::<Wield>(item).and_then(|w| w.wielder) == Some(user) {
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

fn apply_scroll_effect(world: &mut World, _user: Entity, effect: ScrollEffect) {
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
        // We store the "work to be done" here
        let mut potion_effect: Option<PotionEffect> = None;
        let mut wand_effect: Option<WandEffect> = None;
        let mut scroll_effect: Option<ScrollEffect> = None;
        let mut is_wield = false;
        let mut is_wear = false;
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

        // Anything the game doesn't know how to "use" (e.g. an unworn ring) is
        // handed straight back rather than vanishing into limbo.
        if !destroy_item
            && !return_to_inventory
            && potion_effect.is_none()
            && wand_effect.is_none()
            && scroll_effect.is_none()
        {
            let name = item_label(world, item_use.item);
            world.resource_mut::<GameLog>().add(format!("You can't use the {name} right now."));
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
                log.add("The wand crumbles to dust!".to_string());
            } else if is_potion {
                log.add("You drink the potion.".to_string());
            } else if is_scroll {
                log.add("The scroll crumbles to dust!".to_string());
            } else {
                log.add("The item turns to dust!".to_string());
            }

            world.entity_mut(item_use.item).despawn();
        }

        // 2. Dispatch to specialized functions
        if let Some(eff) = potion_effect {
            apply_potion_effect(world, item_use.user, eff);
        }

        if let Some(eff) = wand_effect {
            apply_wand_effect(world, item_use.user, item_use.target, eff);
        }

        if let Some(eff) = scroll_effect {
            apply_scroll_effect(world, item_use.user, eff);
        }
    }
}
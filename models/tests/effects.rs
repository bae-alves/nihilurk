//! The effect vocabulary itself: that a creature's innate magic and a piece of
//! gear's lent magic really are the same components, read by the same code.
//!
//! These tests exist to pin the decoupling down. If someone reintroduces a
//! `match ring_effect { ... }` in a subsystem, the "gear grants what a monster
//! is born with" tests below are what should start failing.

#[path = "common/monster.rs"]
mod monster;

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

/// A grant list exactly as a catalog row would spell it.
const FIRE_RESISTANCE: &[Grant] = &[Grant::of::<FireImmune>()];
const UNDEAD: &[Grant] = &[Grant::of::<Undead>()];

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Builds exactly what a new [`RingDef`] row would put in the world — a ring
/// item carrying modifier components and a grant list — without adding one to
/// the shipped catalog. If this needs more than components to work, the design
/// has leaked.
fn custom_ring(
    w: &mut World,
    name: &str,
    grants: &'static [Grant],
    modifiers: impl Bundle,
) -> Entity {
    w.spawn((
        Name { what: name.into() },
        Item,
        Ring {
            effect: RingEffect::Adornment,
        },
        Equipped::loose(Slot::Finger),
        Grants(grants),
        modifiers,
    ))
    .id()
}

/// Put `item` on the player the way the pack screen does.
fn wear(w: &mut World, p: Entity, item: Entity) {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(item);
    toggle_equipped(w, p, item);
}

#[test]
fn a_ring_can_grant_what_a_monster_is_born_with() {
    let mut w = test_world(1);
    let p = player(&mut w);

    // Innate immunity is a component, nothing more.
    let monster = monster::monster(&mut w, "test monster", Position { x: 10, y: 10 });
    grant_all(&mut w, monster, FIRE_RESISTANCE);
    assert!(w.get::<FireImmune>(monster).is_some());
    assert!(w.get::<FireImmune>(p).is_none());

    // A "ring of fire resistance" is one catalog row: the same component, lent.
    let ring = custom_ring(&mut w, "ring of fire resistance", FIRE_RESISTANCE, ());
    wear(&mut w, p, ring);

    assert!(
        w.get::<FireImmune>(p).is_some(),
        "the wearer answers the same query the dragon does"
    );
}

#[test]
fn taking_the_ring_off_takes_the_effect_with_it() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let ring = custom_ring(&mut w, "ring of fire resistance", FIRE_RESISTANCE, ());

    wear(&mut w, p, ring);
    assert!(w.get::<FireImmune>(p).is_some());

    toggle_equipped(&mut w, p, ring);
    assert!(
        w.get::<FireImmune>(p).is_none(),
        "lent magic goes back with the ring"
    );
}

#[test]
fn a_removed_ring_never_strips_innate_magic() {
    let mut w = test_world(3);
    let monster = monster::monster(&mut w, "test monster", Position { x: 10, y: 10 });
    w.entity_mut(monster).insert(Backpack { items: Vec::new() });
    grant_all(&mut w, monster, FIRE_RESISTANCE);

    // Hand the dragon a ring of the immunity it already has, then take it away.
    let ring = custom_ring(&mut w, "ring of fire resistance", FIRE_RESISTANCE, ());
    wear(&mut w, monster, ring);
    toggle_equipped(&mut w, monster, ring);

    assert!(
        w.get::<FireImmune>(monster).is_some(),
        "innate magic remains after the ring is removed"
    );
}

#[test]
fn a_rings_armor_bonus_folds_in_exactly_like_armour() {
    let mut w = test_world(4);
    let p = player(&mut w);

    // The player starts in +1 ring mail, so measure the ring against that base.
    let base = equipped_total::<ArmorBonus>(&w, p);

    let plus_three = custom_ring(&mut w, "ring of protection", &[], ArmorBonus(3));
    assert_eq!(equipped_total::<ArmorBonus>(&w, p), base);

    wear(&mut w, p, plus_three);
    assert_eq!(
        equipped_total::<ArmorBonus>(&w, p),
        base + 3,
        "combat and the HUD both read this one number"
    );

    // And a suit of armour lands in the very same fold. Wearing it swaps out the
    // starting ring mail, so its +1 replaces the base rather than adding to it.
    let mail = spawn_armor(&mut w, "plate mail", Position { x: 0, y: 0 });
    w.entity_mut(mail).insert(ArmorBonus(1));
    wear(&mut w, p, mail);
    assert_eq!(equipped_total::<ArmorBonus>(&w, p), 4);
    assert_eq!(
        equipped_total::<ArmorDie>(&w, p),
        9,
        "plate mail's own class"
    );
}

#[test]
fn cancellation_strips_every_effect_in_the_registry() {
    let mut w = test_world(5);
    let monster = monster::monster(&mut w, "test monster", Position { x: 10, y: 10 });
    grant_all(&mut w, monster, UNDEAD);
    assert!(w.get::<Undead>(monster).is_some());

    revoke_all(&mut w, monster);

    for effect in EFFECTS {
        assert!(
            !effect.grant.probe(&w, monster),
            "cancellation walks the whole registry"
        );
    }
}

#[test]
fn every_ring_in_the_catalog_has_a_row() {
    // Potions, scrolls, wands and rings are always shown by their true name —
    // there's no second appearance list to drift out of step with the catalog
    // any more, so all that's left to check is the catalog itself.
    let _w = test_world(6);
    for def in RINGS {
        assert_eq!(RingDef::of(def.effect).name, def.name);
    }
}

// ---------------------------------------------------------------------------
// The one rule the compiler cannot check
// ---------------------------------------------------------------------------
//
// An effect is two things that have to agree: the marker component, and the
// row in the bearer's `Effects` ledger that says who lent it and for how long.
// `lend` and `revoke` are the only two functions that keep them in step.
// Attach a marker with a bare `insert` and it has no row — so the save file
// never sees it, and `revoke` can never take it away. Detach one with a bare
// `remove` and the row outlives it — so `hold` still believes the creature is
// held, and the next trap to close on it does nothing.
//
// Both of those shipped. Nothing complained: each is one ordinary line that
// compiles, reads fine in review, and goes wrong only in a save file or six
// turns later. This is the thing that complains.
//
// It reads the game's source as a syntax tree rather than as lines, because
// the interesting ways to break the rule are not one line long. An effect
// handed to `spawn` inside a bundle, a component renamed on the way in
// (`use effects::Asleep as Sleepy`), a call `rustfmt` wrapped over four lines
// — a string search sees none of those, and a guard that passes for the wrong
// reason is worse than no guard.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use syn::visit::Visit;

/// Every crate whose code may touch an effect. The engine has none today, and
/// the cheapest way to keep it that way is to look.
const APP_CRATES: [&str; 2] = ["models", "engine"];

/// The macros that *declare* the effect vocabulary. Every effect name appears
/// inside them by definition, so their bodies are the one place a name is a
/// declaration rather than a use.
const DECLARING: [&str; 2] = ["effects", "modifiers"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("models/ has a parent")
        .to_path_buf()
}

/// Every component named in the `effects!` table, read out of the table
/// itself. A hand-rolled read rather than a second list kept in this file: a
/// list that has to be updated by hand is how an effect quietly stops being
/// covered, which is the whole failure this test exists to catch.
fn registered_effects(src: &str) -> Vec<String> {
    let body = src
        .split_once("effects! {")
        .expect("the effects! table is in effects.rs")
        .1
        .split_once("\n}")
        .expect("the table is closed")
        .0;
    body.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .filter_map(|line| line.split_once("=>"))
        .filter_map(|(_, rhs)| rhs.trim().split(['[', ' ', ',']).next())
        .filter(|ty| !ty.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("src/ is readable").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// What one effect is called *in one file*: its own name, plus whatever a
/// `use ... as ...` renamed it to there. Collected first so the walk below can
/// recognise `Sleepy` as `Asleep` without having to resolve anything.
#[derive(Default)]
struct LocalNames(HashMap<String, String>);

impl LocalNames {
    fn effect_for(&self, ident: &str) -> Option<&str> {
        self.0.get(ident).map(String::as_str)
    }
}

/// Walks a file's `use` trees, recording every local name that means an effect.
struct NameCollector<'a> {
    effects: &'a HashSet<String>,
    names: LocalNames,
}

impl<'ast> Visit<'ast> for NameCollector<'_> {
    fn visit_use_tree(&mut self, tree: &'ast syn::UseTree) {
        if let syn::UseTree::Rename(rename) = tree {
            let original = rename.ident.to_string();
            if self.effects.contains(&original) {
                self.names.0.insert(rename.rename.to_string(), original);
            }
        }
        syn::visit::visit_use_tree(self, tree);
    }
}

/// Whether the token at `i` is reached by `::` from a capitalised identifier —
/// `MovementType::Confused`, and not a bare `Confused`.
fn qualified_by_a_type(trees: &[proc_macro2::TokenTree], i: usize) -> bool {
    let colon = |n: usize| matches!(trees.get(n), Some(proc_macro2::TokenTree::Punct(p)) if p.as_char() == ':');
    if i < 3 || !colon(i - 1) || !colon(i - 2) {
        return false;
    }
    matches!(
        trees.get(i - 3),
        Some(proc_macro2::TokenTree::Ident(id)) if id.to_string().starts_with(char::is_uppercase)
    )
}

/// One place an effect was named outside the ledger.
struct Offence {
    line: usize,
    effect: String,
    how: &'static str,
}

const NAMED: &str = "named as a value — the only reason to build a marker is to \
                     attach it, and attaching goes through `lend`";
const REMOVED: &str = "detached with a bare `remove` — its ledger row outlives it";
const IN_MACRO: &str = "named inside a macro, where the ledger cannot be checked";

/// Walks a file looking for any mention of an effect that the ledger would not
/// have made.
///
/// The rule is deliberately blunt: an effect marker may not be named **as a
/// value** outside the module that declares it. Enumerating the methods that
/// attach one (`insert`, `spawn`, and whatever bevy adds next) only ever
/// catches the attaches somebody thought of — a helper taking `impl Bundle`
/// hides the `insert` behind a type parameter, and then neither end looks like
/// an attach. Naming the marker, though, is unavoidable: a value has to be
/// built somewhere before it can be hidden anywhere. Type position stays
/// legal, because that is how the effect is *read* — `world.get::<Asleep>(e)`,
/// `With<Asleep>`, `Grant::of::<Asleep>()`.
struct LedgerAudit<'a> {
    effects: &'a HashSet<String>,
    names: &'a LocalNames,
    found: Vec<Offence>,
}

impl LedgerAudit<'_> {
    /// What this local name means, if it means an effect: the effect itself,
    /// or whatever a `use ... as ...` renamed it to in this file.
    fn resolve(&self, ident: &str) -> Option<String> {
        if self.effects.contains(ident) {
            return Some(ident.to_string());
        }
        self.names.effect_for(ident).map(str::to_string)
    }

    /// The effect `path` names, and the line it sits on.
    ///
    /// Rust's own naming convention does the disambiguating, because several
    /// effects share a name with an enum variant — `MovementType::Confused`
    /// is not the `Confused` component, and `SpellEffect::MagicWard` is not the
    /// `MagicWard` one. A qualifier that is `snake_case` is a module, so the
    /// type after it is the real one; a qualifier that is `CamelCase` is a
    /// type, so what follows is its variant or its associated item and none of
    /// this test's business.
    ///
    /// So: the effect must be the *first* capitalised segment. `Asleep` and
    /// `Asleep::default()` are both attaches — `default` is the escape hatch
    /// every marker has, since `Grant::of` requires it — and anything else
    /// hung off the marker is one too, because a unit struct has nothing else
    /// worth naming. `SpellEffect::MagicWard` is a row in a catalog, and its
    /// first capitalised segment is `SpellEffect`, which is no effect of ours.
    /// A turbofish is not a segment at all, which is what keeps
    /// `Grant::of::<Asleep>()` legal.
    fn effect_in(&self, path: &syn::Path) -> Option<(String, usize)> {
        let segments: Vec<&syn::PathSegment> = path.segments.iter().collect();
        let first_type = segments
            .iter()
            .position(|seg| seg.ident.to_string().starts_with(char::is_uppercase))?;
        let seg = segments[first_type];
        self.resolve(&seg.ident.to_string())
            .map(|effect| (effect, seg.ident.span().start().line))
    }

    fn flag(&mut self, found: Option<(String, usize)>, how: &'static str) {
        if let Some((effect, line)) = found {
            self.found.push(Offence { line, effect, how });
        }
    }

    /// A macro body is tokens, not syntax: there is no way to tell a type from
    /// a value in one, so any effect named inside a macro is reported. No
    /// macro in the game mentions an effect today, and the two that declare
    /// them are exempt below, so this costs nothing and shuts the last door.
    fn scan_tokens(&mut self, tokens: proc_macro2::TokenStream) {
        let trees: Vec<proc_macro2::TokenTree> = tokens.into_iter().collect();
        for (i, tree) in trees.iter().enumerate() {
            match tree {
                proc_macro2::TokenTree::Group(group) => self.scan_tokens(group.stream()),
                proc_macro2::TokenTree::Ident(ident) => {
                    // The same convention `effect_in` leans on, spelled out in
                    // tokens: an identifier reached through `::` from a
                    // capitalised one is a variant or an associated item.
                    // `matches!(m.movement_type, MovementType::Confused)` is
                    // the shape this has to let through.
                    if qualified_by_a_type(&trees, i) {
                        continue;
                    }
                    let found = self
                        .resolve(&ident.to_string())
                        .map(|effect| (effect, ident.span().start().line));
                    self.flag(found, IN_MACRO);
                }
                _ => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for LedgerAudit<'_> {
    fn visit_expr_path(&mut self, expr: &'ast syn::ExprPath) {
        let found = self.effect_in(&expr.path);
        self.flag(found, NAMED);
        syn::visit::visit_expr_path(self, expr);
    }

    fn visit_expr_struct(&mut self, expr: &'ast syn::ExprStruct) {
        let found = self.effect_in(&expr.path);
        self.flag(found, NAMED);
        syn::visit::visit_expr_struct(self, expr);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "remove" {
            if let Some(turbofish) = call.turbofish.as_ref() {
                for arg in &turbofish.args {
                    if let syn::GenericArgument::Type(syn::Type::Path(p)) = arg {
                        let found = self.effect_in(&p.path);
                        self.flag(found, REMOVED);
                    }
                }
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        if !DECLARING.contains(&name.as_str()) {
            self.scan_tokens(mac.tokens.clone());
        }
        syn::visit::visit_macro(self, mac);
    }
}

#[test]
fn no_effect_is_attached_or_detached_behind_the_ledgers_back() {
    let root = workspace_root();
    let effects_rs = std::fs::read_to_string(root.join("models/src/effects.rs"))
        .expect("effects.rs is readable");
    let types = registered_effects(&effects_rs);
    // The parse is worth exactly as much as its agreement with the table it
    // claims to have read. A scanner that silently found nothing would pass
    // every check below, so it is held against `EFFECTS` — the compiled form
    // of the same rows — rather than against a number somebody chose.
    assert_eq!(
        types.len(),
        EFFECTS.len(),
        "the effects! table did not parse into one name per row — this test \n\
         would be scanning for {} of {} effects.\n\
         parsed: {types:?}",
        types.len(),
        EFFECTS.len()
    );
    let effects: HashSet<String> = types.into_iter().collect();

    let mut files = Vec::new();
    for krate in APP_CRATES {
        rust_files(&root.join(krate).join("src"), &mut files);
    }
    assert!(
        files.len() > 20,
        "only {} source files found — the walk is not reaching the game",
        files.len()
    );

    let mut offences: Vec<String> = Vec::new();
    for file in files {
        let shown = file
            .strip_prefix(&root)
            .unwrap_or(&file)
            .display()
            .to_string();
        let body = std::fs::read_to_string(&file).expect("a source file is readable");
        let parsed = syn::parse_file(&body)
            .unwrap_or_else(|e| panic!("{shown} is valid Rust, but did not parse: {e}"));

        // `effects.rs` defines attaching and detaching, in terms of a type
        // parameter rather than any named effect — there is nothing there for
        // this to match, so it needs no exemption and gets none.
        let mut collector = NameCollector {
            effects: &effects,
            names: LocalNames::default(),
        };
        collector.visit_file(&parsed);

        let mut audit = LedgerAudit {
            effects: &effects,
            names: &collector.names,
            found: Vec::new(),
        };
        audit.visit_file(&parsed);

        for offence in audit.found {
            offences.push(format!(
                "{shown}:{}: {} {}",
                offence.line, offence.effect, offence.how
            ));
        }
    }
    offences.sort();

    assert!(
        offences.is_empty(),
        "an effect marker is attached or detached without the ledger:\n  {}\n\n\
         Attach with `effects::lend(world, e, Grant::of::<T>(), lifetime)` and take \
         it back with `effects::revoke(world, e, Grant::of::<T>())`. A bare insert \
         has no ledger row, so it is missing from the save and cannot be revoked; \
         a bare remove leaves its row behind, still counting down.",
        offences.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// The three-condition ceiling
// ---------------------------------------------------------------------------

/// How many transient conditions the ledger counts on one creature.
fn conditions_held(w: &World, e: Entity) -> usize {
    w.get::<Effects>(e)
        .map(|l| l.0.iter().filter(|h| h.is_condition()).count())
        .unwrap_or(0)
}

#[test]
fn a_fourth_condition_sheds_the_oldest() {
    let mut w = test_world(7);
    let p = player(&mut w);

    lend(&mut w, p, Grant::of::<Blind>(), Lifetime::Floor);
    lend(&mut w, p, Grant::of::<Confused>(), Lifetime::Floor);
    lend(&mut w, p, Grant::of::<MagicWard>(), Lifetime::Floor);
    assert_eq!(conditions_held(&w, p), 3);
    assert!(w.get::<Blind>(p).is_some());

    lend(&mut w, p, Grant::of::<Paralyzed>(), Lifetime::Floor);
    assert_eq!(conditions_held(&w, p), 3, "the ceiling is three");
    assert!(
        w.get::<Blind>(p).is_none(),
        "the oldest is the one that goes"
    );
    assert!(w.get::<Confused>(p).is_some());
    assert!(w.get::<MagicWard>(p).is_some());
    assert!(w.get::<Paralyzed>(p).is_some());
    // Blindness leaving has to put the viewshed back, the same as a cure does.
    assert!(w.get::<Viewshed>(p).is_some_and(|v| v.dirty));
    // And the player has to be told, in the words a staircase uses.
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|e| e.contains("no longer blind")),
        "a shed condition is never silent: {:?}",
        w.resource::<GameLog>().history
    );
}

#[test]
fn worn_and_innate_magic_does_not_count_against_the_ceiling() {
    let mut w = test_world(7);
    let p = player(&mut w);

    let ring = custom_ring(&mut w, "ring of test fire", FIRE_RESISTANCE, ());
    wear(&mut w, p, ring);

    lend(&mut w, p, Grant::of::<Blind>(), Lifetime::Floor);
    lend(&mut w, p, Grant::of::<Confused>(), Lifetime::Floor);
    lend(&mut w, p, Grant::of::<MagicWard>(), Lifetime::Floor);

    assert_eq!(conditions_held(&w, p), 3);
    assert!(w.get::<Blind>(p).is_some(), "a ring is not a condition");
    assert!(w.get::<FireImmune>(p).is_some());
}

#[test]
fn a_temporary_boon_is_not_a_condition() {
    let mut w = test_world(7);
    let p = player(&mut w);

    lend(&mut w, p, Grant::of::<Blind>(), Lifetime::Floor);
    lend(&mut w, p, Grant::of::<Confused>(), Lifetime::Floor);
    lend(&mut w, p, Grant::of::<MagicWard>(), Lifetime::Floor);
    // A potion of see invisible: lent for the floor like the three above, and
    // nothing the player is afflicted or blessed with in the badge sense.
    grant_for_floor(&mut w, p, Grant::of::<SeesInvisible>());
    // The mark a potion of magic detection leaves, same lifetime again.
    lend(&mut w, p, Grant::of::<Detected>(), Lifetime::Floor);

    assert_eq!(conditions_held(&w, p), 3);
    assert!(w.get::<Blind>(p).is_some(), "the oldest condition stands");
    assert!(w.get::<SeesInvisible>(p).is_some());
    assert!(w.get::<Detected>(p).is_some());
}

//! Spanish. Preliminary pass -- see `lib.rs`'s module doc comment; still
//! wants review by a native speaker (see `beta_notice` in `lib.rs`).
//!
//! Style notes for anyone touching this file:
//! - No literal "tú"/"tu(s)"/"usted": the player is addressed by verb
//!   conjugation alone (Spanish drops the subject pronoun), and a possessive
//!   is replaced with a definite article ("la armadura", not "tu armadura")
//!   since context already makes clear whose it is.
//! - Clitic pronouns (te/se/lo/la...) lean enclitic (attached after an
//!   imperative, infinitive or gerund) rather than proclitic, wherever the
//!   sentence allows it; where the only natural phrasing needs a clitic
//!   before an indicative verb, a pronoun-free verb (quedar, perder, ganar,
//!   cobrar, notar...) is used instead. Third-person reflexives describing a
//!   monster or the scene ("se transforma", "se pone") aren't affected by
//!   this -- the rule is about how the player is addressed, not grammar in
//!   general.
//! - `content_name` (monster/item/trap ids) and `engine/src/main.rs`'s CLI
//!   output (usage/help text, flag errors, save/load prompts) are out of
//!   scope: only in-game text is localized here, so those stay re-exported
//!   from `en`.
//! - A few upstream functions (`models/src/identify.rs`, `models/src/
//!   monsters.rs`) hand this crate an English-only "a"/"an" article or a
//!   literal "wielding"/"wearing" verb. That's staying as-is. Functions below
//!   that take an `article` parameter ignore it and hardcode a plausible
//!   Spanish article instead.

#[allow(unused_imports)]
pub use super::en::content_name;

// engine/src/main.rs's CLI output (usage/help text, flag errors, save/load
// prompts) stays in English -- only in-game text is localized here.
#[allow(unused_imports)]
pub use super::en::{
    body_conflicts_with_load, clear_data_not_saved, clear_data_prompt, clear_data_saved,
    conflicting_bodies, content_group_header, content_header, content_spawn_hint,
    failed_to_save_clear_data, failed_to_save_game, game_not_saved, game_saved, help_text,
    no_such_body, no_such_monster, no_such_pride_flag, stray_positional, world_keeps_its_light,
};

/// Content ids stay untranslated (see `content_name` above), so one can
/// coincidentally already be a real, gendered Spanish word -- today just
/// "medusa" (spelled the same in English), which is feminine. "arrow"/
/// "arrows" are here for a different reason: unlike other content ids, ammo
/// nouns *do* get translated (see `ammo_word`), to "flecha(s)", which is
/// feminine. "quarrel(s)" needs no entry -- "virote" is already masculine,
/// matching the default. Everything else defaults masculine. Add an id here
/// if a future one collides the same way.
fn is_feminine_name(name: &str) -> bool {
    matches!(name, "medusa" | "arrow" | "arrows")
}

/// The definite article ("el"/"la") that reads correctly directly before
/// `name`, lowercase for mid-sentence use.
fn article(name: &str) -> &'static str {
    if is_feminine_name(name) { "la" } else { "el" }
}

/// [`article`], capitalized for sentence-initial use.
fn cap_article(name: &str) -> &'static str {
    if is_feminine_name(name) { "La" } else { "El" }
}

/// The indefinite article ("un"/"una") that reads correctly directly before
/// `name`.
fn indef_article(name: &str) -> &'static str {
    if is_feminine_name(name) { "una" } else { "un" }
}

/// [`indef_article`], capitalized for sentence-initial use.
fn cap_indef_article(name: &str) -> &'static str {
    if is_feminine_name(name) { "Una" } else { "Un" }
}

/// "de" + [`article`]: contracted "del" before masculine, uncontracted
/// "de la" before feminine -- Spanish only contracts with "el".
fn de_contraction(name: &str) -> &'static str {
    if is_feminine_name(name) {
        "de la"
    } else {
        "del"
    }
}

/// "a" + [`article`]: contracted "al" before masculine, uncontracted "a la"
/// before feminine.
fn a_contraction(name: &str) -> &'static str {
    if is_feminine_name(name) { "a la" } else { "al" }
}

/// "helpless", agreeing with `name`'s gender: "indefenso"/"indefensa".
fn helpless(name: &str) -> &'static str {
    if is_feminine_name(name) {
        "indefensa"
    } else {
        "indefenso"
    }
}

/// "wild" (of a shot gone astray), agreeing with `name`'s gender:
/// "descontrolado"/"descontrolada".
fn wild(name: &str) -> &'static str {
    if is_feminine_name(name) {
        "descontrolada"
    } else {
        "descontrolado"
    }
}

// ---------------------------------------------------------------------------
// HUD status-line abbreviations (models/src/body.rs, engine/src/view.rs)
// ---------------------------------------------------------------------------

pub fn hp_abbr() -> &'static str {
    "PV"
}

pub fn magic_abbr() -> &'static str {
    "Ma"
}

pub fn power_abbr() -> &'static str {
    "Pod."
}

pub fn armor_abbr() -> &'static str {
    "Arm."
}

pub fn skill_abbr() -> &'static str {
    "Hab."
}

// ---------------------------------------------------------------------------
// models/src/body.rs
// ---------------------------------------------------------------------------

pub fn fear_the_lurk(step: i32, stat: &str) -> String {
    format!("¡TEME AL LURK! (+{step} {stat})")
}

pub fn no_hands_lurk(item_name: &str) -> String {
    format!("Pelaje, colmillos y cuatro patas: ningún lurk tiene con qué sostener {item_name}.")
}

pub fn no_hands_monster(species: &str, item_name: &str) -> String {
    format!(
        "{} {species} no tiene manos para {item_name}.",
        cap_indef_article(species)
    )
}

// ---------------------------------------------------------------------------
// models/src/monsters.rs
// ---------------------------------------------------------------------------

pub fn wearing_gear(verb: &str, name: &str) -> String {
    let verb_es = match verb {
        "wielding" => "empuñando",
        "wearing" => "llevando puesto",
        other => other,
    };
    format!("Lleva {verb_es} {name}.")
}

pub fn xeroc_disguise_falls() -> &'static str {
    "¡El disfraz cae — era un xeroc todo este tiempo!"
}

// ---------------------------------------------------------------------------
// models/src/items/wands.rs
// ---------------------------------------------------------------------------

pub fn dazzle_player_line() -> &'static str {
    "El destello deja el mundo dando tumbos — ¡quedas deslumbrado!"
}

pub fn dazzle_mob_verb() -> &'static str {
    "queda deslumbrado"
}

pub fn bolt_magic_missile() -> &'static str {
    "¡Un rayo cian brillante salta de la varita!"
}

pub fn bolt_lightning() -> &'static str {
    "¡Un rayo bifurcado de relámpago restalla en el aire!"
}

pub fn bolt_striking() -> &'static str {
    "¡Un puño invisible golpea a lo largo de la línea!"
}

pub fn bolt_drain_life() -> &'static str {
    "Un zarcillo de luz negra bebe la vida de su camino."
}

pub fn drain_life_gained(taken: i32) -> String {
    format!("Drenas {taken} de vida.")
}

pub fn blast_fire() -> &'static str {
    "¡Una esfera rugiente de fuego estalla!"
}

pub fn blast_cold() -> &'static str {
    "¡Una ráfaga de aire helado detona!"
}

pub fn wand_does_nothing() -> &'static str {
    "La varita no hace nada. Bien merecido tiene el nombre."
}

pub fn light_reveals(label: &str) -> String {
    format!("¡La luz revela {label}!")
}

pub fn light_floods_room() -> &'static str {
    "Una luz cálida inunda la sala."
}

pub fn light_races_passage() -> &'static str {
    "La luz recorre todo el pasillo."
}

pub fn polymorph_fizzles() -> &'static str {
    "El rayo de cambio se apaga contra la nada."
}

pub fn polymorph_self_player() -> &'static str {
    "Una sensación de ser alguien distinto lo recorre todo."
}

pub fn polymorph_same_looking(old_name: &str, new_name: &str) -> String {
    format!(
        "¡{} {old_name} se retuerce y muta en {} {new_name} de aspecto distinto!",
        cap_article(old_name),
        indef_article(new_name)
    )
}

pub fn polymorph_different(old_name: &str, _article: &str, new_name: &str) -> String {
    format!(
        "¡{} {old_name} se retuerce y muta en {} {new_name}!",
        cap_article(old_name),
        indef_article(new_name)
    )
}

pub fn nothing_to_enchant() -> &'static str {
    "No hay nada ahí para encantar."
}

pub fn teleport_pull_finds_nothing() -> &'static str {
    "El tirón de la varita no encuentra nada."
}

pub fn yanked_into_dark(name: &str) -> String {
    format!("{} {name} es arrastrado a la oscuridad.", cap_article(name))
}

pub fn dragged_to_your_side(name: &str) -> String {
    format!(
        "¡{} {name} es arrastrado hasta el lado del jugador!",
        cap_article(name)
    )
}

pub fn bursts_in_transit(name: &str) -> String {
    format!(
        "{} {name} es arrastrado al espacio intermedio y se deshace en el camino — ¡estalla en una lluvia de vísceras!",
        cap_article(name)
    )
}

pub fn teleport_self_player() -> &'static str {
    "El teletransporte apunta exactamente al mismo lugar. Menudo viaje."
}

pub fn teleport_self_mob(name: &str) -> String {
    format!(
        "{} {name} se teletransporta directo a sí mismo.",
        cap_article(name)
    )
}

pub fn cancellation_strikes_stone() -> &'static str {
    "El rayo gris solo golpea piedra."
}

pub fn cancellation_sputters(name: &str) -> String {
    format!(
        "La magia {} {name} chisporrotea y se apaga.",
        de_contraction(name)
    )
}

pub fn cancellation_player_wave() -> &'static str {
    "Una ola gris lo cubre todo. El inventario enmudece, el equipo pierde todo brillo, y cada maldición simplemente se suelta."
}

// ---------------------------------------------------------------------------
// models/src/traps.rs
// ---------------------------------------------------------------------------

pub fn bear_trap_thrash() -> &'static str {
    "Al forcejear por soltarse, la trampa desgarra la pierna atrapada."
}

pub fn steps_on_trap(who: &str, _article: &str, label: &str) -> String {
    format!("¡{who} pisa una {label}!")
}

pub fn trap_breaks(label: &str) -> String {
    format!("¡La {label} se rompe!")
}

pub fn hero_coin_ultimate() -> &'static str {
    "La moneda del héroe entrega todo lo que sabe de una vez."
}

pub fn relic_takes_the_hit() -> &'static str {
    "El Elemento de Yoord recibe el golpe — y responde."
}

pub fn ultimate_trick_shot_shout() -> &'static str {
    "¡TIRO MAESTRO DEFINITIVO!"
}

pub fn trick_shot_shout_self() -> &'static str {
    "¡POR QUÉ!"
}

pub fn trick_shot_shout_other() -> &'static str {
    "¡PUM!"
}

pub fn trick_shot_line(shout: &str) -> String {
    format!("{shout} ¡Tiro con efecto!")
}

pub fn drops_through_trapdoor(who: &str) -> String {
    format!("{who} cae por la trampilla y desaparece.")
}

pub fn trapdoor_grinds_shut() -> &'static str {
    "Una trampilla se abre — pero solo hay roca sólida abajo. Chirría al cerrarse de nuevo."
}

pub fn trapdoor_yawns_open() -> &'static str {
    "¡Una trampilla se abre de par en par!"
}

pub fn bear_trap_snare() -> &'static str {
    "¡Unas fauces de acero se cierran de golpe — la pierna queda atrapada, pero los brazos están libres!"
}

pub fn sleep_gas_snare() -> &'static str {
    "Un gas se eleva alrededor. Los párpados se vuelven de plomo..."
}

pub fn teleport_trap_whisked() -> &'static str {
    "¡Las paredes cambian! Un salto lleva a otra parte de la mazmorra."
}

pub fn arrow_whistles_past(who: &str) -> String {
    format!("Una flecha silba junto a {who} y cae repicando por el suelo.")
}

pub fn arrow_plinks(who: &str, damage: i32) -> String {
    format!("¡Una flecha se clava en {who} y causa {damage} de daño!")
}

pub fn dart_glances_off(who: &str) -> String {
    format!("Un dardo roza a {who} sin hacer daño.")
}

pub fn dart_pricks(who: &str, damage: i32) -> String {
    format!("¡Un dardo envenenado pincha a {who} y causa {damage} de daño!")
}

pub fn dart_poison_resisted() -> &'static str {
    "El veneno arde, pero la fuerza no cede."
}

pub fn dart_poison_took() -> &'static str {
    "El veneno recorre el cuerpo entero — la fuerza se escurre poco a poco."
}

// ---------------------------------------------------------------------------
// models/src/items/theft.rs
// ---------------------------------------------------------------------------

pub fn leprechaun_theft(attacker: &str, item: &str, target: &str) -> String {
    format!(
        "¡{} {attacker} arrebata {item} a {target} y suelta una carcajada!",
        cap_article(attacker)
    )
}

pub fn nymph_theft(attacker: &str, item: &str, target: &str) -> String {
    format!(
        "¡{} {attacker} le quita {item} a {target} de un tirón y desaparece en una nube de humo!",
        cap_article(attacker)
    )
}

// ---------------------------------------------------------------------------
// models/src/items/rings.rs
// ---------------------------------------------------------------------------

pub const FANFARE: [&str; 4] = [
    "Magenta, cian y dorado brotan todos a la vez.",
    "Por un instante, la mazmorra entera es un salón de baile.",
    "¡Y todo con estilo!",
    "¡La puntuación se duplica!",
];

pub fn adornment_spent(name: &str) -> String {
    format!("{} {name} ya no tiene nada más que dar.", cap_article(name))
}

// ---------------------------------------------------------------------------
// models/src/hud.rs, models/src/score.rs
// ---------------------------------------------------------------------------

pub fn combo_word() -> &'static str {
    "¡COMBO!"
}

pub fn with_pride() -> &'static str {
    "Con orgullo."
}

pub fn with_style() -> &'static str {
    "Con estilo."
}

// ---------------------------------------------------------------------------
// models/src/catalog.rs, models/src/components.rs
// ---------------------------------------------------------------------------

pub fn wizard_now() -> &'static str {
    "¡Ahora hay un mago en la mazmorra!"
}

pub fn wizard_no_more() -> &'static str {
    "La magia de mago se ha ido."
}

pub fn welcome_new_run() -> &'static str {
    "¡Bienvenido a nihilurk! Buena suerte y a divertirse."
}

pub fn welcome_back() -> &'static str {
    "¡De vuelta a nihilurk! Buena suerte y a divertirse."
}

// ---------------------------------------------------------------------------
// engine/src/view.rs
// ---------------------------------------------------------------------------

pub fn more_prompt() -> &'static str {
    "--MÁS-- (Pulsa Espacio)"
}

pub fn you_die() -> &'static str {
    "La muerte llega..."
}

pub fn lose_title() -> &'static str {
    "DERROTA"
}

pub fn win_title() -> &'static str {
    "VICTORIA"
}

pub fn score_line(score_text: &str) -> String {
    format!("PUNTOS {score_text}")
}

pub fn press_any_key_to_depart() -> &'static str {
    "Pulsa cualquier tecla para partir."
}

// ---------------------------------------------------------------------------
// models/src/pride.rs
// ---------------------------------------------------------------------------

pub fn pride_off_refusal() -> &'static str {
    "ERROR: Nuestro orgullo jamás será tuyo."
}

// ---------------------------------------------------------------------------
// models/src/pack.rs
// ---------------------------------------------------------------------------

pub fn pack_title_browse() -> &'static str {
    " INVENTARIO "
}
pub fn pack_title_use() -> &'static str {
    " ¿USAR QUÉ? "
}
pub fn pack_title_throw() -> &'static str {
    " ¿LANZAR QUÉ? "
}
pub fn pack_title_drop() -> &'static str {
    " ¿SOLTAR QUÉ? "
}
pub fn pack_title_equip() -> &'static str {
    " ¿EQUIPAR QUÉ? "
}
pub fn pack_title_quaff() -> &'static str {
    " ¿BEBER QUÉ? "
}
pub fn pack_title_read() -> &'static str {
    " ¿LEER QUÉ? "
}
pub fn pack_title_zap() -> &'static str {
    " ¿USAR QUÉ VARITA? "
}
pub fn pack_title_wield() -> &'static str {
    " ¿EMPUÑAR QUÉ? "
}
pub fn pack_title_wear() -> &'static str {
    " ¿VESTIR QUÉ? "
}
pub fn pack_title_put_on() -> &'static str {
    " ¿PONERSE QUÉ? "
}

pub fn pack_nothing_browse() -> &'static str {
    "No hay ningún objeto en el inventario."
}
pub fn pack_nothing_use() -> &'static str {
    "No hay nada que usar."
}
pub fn pack_nothing_throw() -> &'static str {
    "No hay nada que lanzar."
}
pub fn pack_nothing_drop() -> &'static str {
    "No hay nada que soltar."
}
pub fn pack_nothing_equip() -> &'static str {
    "No hay nada que equipar."
}
pub fn pack_nothing_quaff() -> &'static str {
    "No hay nada que beber."
}
pub fn pack_nothing_read() -> &'static str {
    "No hay nada que leer."
}
pub fn pack_nothing_zap() -> &'static str {
    "No hay ninguna varita que usar."
}
pub fn pack_nothing_wield() -> &'static str {
    "No hay nada que empuñar."
}
pub fn pack_nothing_wear() -> &'static str {
    "No hay nada que vestir."
}
pub fn pack_nothing_put_on() -> &'static str {
    "No hay nada que ponerse."
}

// ---------------------------------------------------------------------------
// models/src/magicmap.rs
// ---------------------------------------------------------------------------

pub fn magicmap_row_by_row() -> &'static str {
    "La forma de la mazmorra surge de golpe en la mente."
}
pub fn magicmap_spiral() -> &'static str {
    "La mazmorra se despliega alrededor como un pergamino."
}
pub fn magicmap_explode() -> &'static str {
    "El conocimiento de la mazmorra estalla hacia afuera desde este punto."
}

// ---------------------------------------------------------------------------
// models/src/effects.rs — the EFFECTS table's `ends`/`beware` text.
// ---------------------------------------------------------------------------

pub const fn beware_aggravating_shriek() -> &'static str {
    "chillido que alerta a todos"
}
pub const fn beware_corrosive_touch() -> &'static str {
    "toque corrosivo"
}
pub const fn beware_regeneration() -> &'static str {
    "regeneración"
}
pub const fn beware_erratic_strikes() -> &'static str {
    "golpes erráticos"
}
pub const fn beware_binding_bite() -> &'static str {
    "mordisco que ata"
}
pub const fn beware_petrifying_gaze() -> &'static str {
    "mirada petrificante"
}
pub const fn beware_draining_touch() -> &'static str {
    "toque drenante"
}
pub const fn beware_venomous_bite() -> &'static str {
    "mordisco venenoso"
}
pub const fn beware_splitting_flesh() -> &'static str {
    "carne que se divide"
}
pub const fn beware_paralysing_touch() -> &'static str {
    "toque paralizante"
}
pub const fn beware_thieving_touch() -> &'static str {
    "toque ladrón"
}
pub const fn beware_fire_breath() -> &'static str {
    "aliento de fuego"
}
pub const fn beware_confusing_touch() -> &'static str {
    "toque que confunde"
}

pub const fn ends_asleep() -> &'static str {
    "La modorra se disipa y el despertar llega."
}
pub const fn ends_petrified() -> &'static str {
    "La piedra se desprende y la carne vuelve a ser suya."
}
pub const fn ends_pinned() -> &'static str {
    "La pierna se libera de un tirón fuera del cepo."
}
pub const fn ends_rooted() -> &'static str {
    "Lo que sujetaba, suelta."
}

// ---------------------------------------------------------------------------
// models/src/conditions.rs
// ---------------------------------------------------------------------------

pub fn mob_verb_line(name: &str, verb: &str) -> String {
    format!("{} {name} {verb}.", cap_article(name))
}

pub fn blind_mob_verb() -> &'static str {
    "tantea a ciegas"
}

pub fn paralyzed_mob_verb() -> &'static str {
    "se paraliza en seco"
}

pub fn blind_player_line() -> &'static str {
    "Una oscuridad cae sobre los ojos. ¡Nada es visible!"
}

pub fn paralyse_player_line() -> &'static str {
    "Los miembros se agarrotan de golpe. ¡Apenas hay movimiento posible!"
}

pub fn paralysis_lost_turn() -> &'static str {
    "El cuerpo no responde."
}

pub const fn blind_cured_line() -> &'static str {
    "La oscuridad se levanta de los ojos."
}
pub const fn blind_cured_noun() -> &'static str {
    "ceguera"
}
pub const fn blind_lifted_adjective() -> &'static str {
    "ciego"
}
pub const fn paralyzed_cured_line() -> &'static str {
    "Los miembros vuelven a responder."
}
pub const fn paralyzed_cured_noun() -> &'static str {
    "parálisis"
}
pub const fn paralyzed_lifted_adjective() -> &'static str {
    "paralizado"
}
pub const fn confused_cured_line() -> &'static str {
    "La cabeza se despeja."
}
pub const fn confused_cured_noun() -> &'static str {
    "confusión"
}
pub const fn confused_lifted_adjective() -> &'static str {
    "confundido"
}

pub const fn sluggish_cured_line() -> &'static str {
    "El plomo desaparece de las piernas."
}
pub const fn sluggish_cured_noun() -> &'static str {
    "lentitud"
}

pub const fn power_restored_line() -> &'static str {
    "La fuerza vuelve a fluir por el brazo."
}
pub const fn power_restored_noun() -> &'static str {
    "debilidad"
}

pub fn snaps_out_of(name: &str, noun: &str) -> String {
    format!("{} {name} sale de golpe de {noun}.", cap_article(name))
}

pub const fn adjective_asleep() -> &'static str {
    "dormido"
}
pub const fn adjective_pinned() -> &'static str {
    "atrapado"
}
pub const fn adjective_held() -> &'static str {
    "inmovilizado"
}
pub const fn adjective_warded() -> &'static str {
    "protegido"
}
pub const fn adjective_coiled() -> &'static str {
    "enroscado"
}
pub const fn adjective_stone() -> &'static str {
    "de piedra"
}
pub const fn adjective_stealthy() -> &'static str {
    "sigiloso"
}
pub const fn adjective_sluggish() -> &'static str {
    "lento"
}

pub fn no_longer(adjective: &str) -> String {
    format!("Eso de estar {adjective} ya quedó atrás.")
}

pub fn already_as_extreme_player(extreme: &str) -> String {
    format!("Ya no se puede estar más {extreme}.")
}

pub fn already_as_extreme_mob(name: &str, extreme: &str) -> String {
    format!(
        "{} {name} ya no puede estar más {extreme}.",
        cap_article(name)
    )
}

pub fn extreme_quick() -> &'static str {
    "veloz"
}
pub fn extreme_sluggish() -> &'static str {
    "lento"
}

pub fn haste_player_line() -> &'static str {
    "El mundo entero cae en cámara lenta."
}
pub fn slow_player_line() -> &'static str {
    "Los miembros se vuelven de plomo."
}
pub fn haste_mob_line(name: &str) -> String {
    format!(
        "{} {name} se difumina en un estallido de velocidad.",
        cap_article(name)
    )
}
pub fn slow_mob_line(name: &str) -> String {
    format!("{} {name} cae en cámara lenta.", cap_article(name))
}

// ---------------------------------------------------------------------------
// models/src/equipment.rs
// ---------------------------------------------------------------------------

pub fn donned_hand(name: &str) -> String {
    format!("Ahora empuña {} {name}.", article(name))
}
pub fn donned_body_or_finger(name: &str) -> String {
    format!("Ahora lleva puesto {} {name}.", article(name))
}
pub fn doffed_hand(name: &str) -> String {
    format!("Deja de empuñar {} {name}.", article(name))
}
pub fn doffed_body(name: &str) -> String {
    format!("Se quita {} {name}.", article(name))
}
pub fn doffed_finger(name: &str) -> String {
    format!("Se retira {} {name}.", article(name))
}
pub fn stuck_hand(name: &str) -> String {
    format!(
        "Imposible — ¡{} {name} está soldado a la mano!",
        article(name)
    )
}
pub fn stuck_body(name: &str) -> String {
    format!(
        "Imposible — ¡{} {name} se aferra al cuerpo y no sale!",
        article(name)
    )
}
pub fn stuck_finger(name: &str) -> String {
    format!(
        "Imposible — ¡{} {name} está fundido al dedo!",
        article(name)
    )
}
pub fn cursed_reveal_hand(name: &str) -> String {
    format!(
        "¡{} {name} se suelda a la mano! ¡Está maldito!",
        cap_article(name)
    )
}
pub fn cursed_reveal_body(name: &str) -> String {
    format!(
        "¡{} {name} se aferra al cuerpo! ¡Está maldito!",
        cap_article(name)
    )
}
pub fn cursed_reveal_finger(name: &str) -> String {
    format!(
        "¡{} {name} se suelda al dedo! ¡Está maldito!",
        cap_article(name)
    )
}
pub fn blocked_hand(name: &str) -> String {
    format!(
        "Imposible cambiar de arma — {} {name} no sale de la mano.",
        article(name)
    )
}
pub fn blocked_body(name: &str) -> String {
    format!(
        "Imposible cambiar de armadura — {} {name} no sale.",
        article(name)
    )
}
pub fn blocked_finger(name: &str) -> String {
    format!("Imposible — {} {name} no sale del dedo.", article(name))
}

pub fn armor_shrugs_off_corrosion() -> &'static str {
    "La armadura bebe la corrosión y la deja pasar sin daño."
}

pub fn armor_corrodes(name: &str) -> String {
    format!("¡La {name} se corroe! Ahora es más débil.")
}

// ---------------------------------------------------------------------------
// models/src/abilities.rs
// ---------------------------------------------------------------------------

pub const fn flavour_aggravates() -> &'static str {
    "¡Un aullido escapa sin permiso! Todo el piso mira hacia acá."
}
pub const fn flavour_regenerates() -> &'static str {
    "El anillo en el dedo está tibio."
}
pub const fn flavour_teleportitis() -> &'static str {
    "Algo en el dedo parece muy satisfecho de sí mismo."
}

pub fn heavy_stagger_player() -> &'static str {
    "¡El golpe deja tambaleando — no hay tiempo de reponerse para responder!"
}
pub fn heavy_stagger_mob(name: &str) -> String {
    format!(
        "¡{} {name} se tambalea, aturdido por el golpe!",
        cap_article(name)
    )
}

pub fn chaos_recoil() -> &'static str {
    "¡El filo del caos muerde de vuelta!"
}

pub fn venom_resisted_player() -> &'static str {
    "El veneno arde, pero la fuerza no cede."
}
pub fn venom_resisted_mob(name: &str) -> String {
    format!(
        "El veneno arde, pero la fuerza {} {name} no cede.",
        de_contraction(name)
    )
}
pub fn venom_took_player() -> &'static str {
    "El veneno corre por dentro — la fuerza se escurre poco a poco."
}
pub fn venom_took_mob(name: &str) -> String {
    format!(
        "El veneno corre por {} {name} — su fuerza se escurre poco a poco.",
        article(name)
    )
}

pub fn vampiric_drain_player() -> &'static str {
    "¡Un frío mortal se extiende por dentro — la vitalidad queda drenada!"
}
pub fn vampiric_drain_mob(name: &str) -> String {
    format!(
        "¡Un frío mortal se extiende por {} {name} — su vitalidad queda drenada!",
        article(name)
    )
}

pub fn bind_victim_player(name: &str) -> String {
    format!(
        "¡{} {name} clava sus fauces en la pierna — imposible dar un paso, pero los brazos están libres!",
        cap_article(name)
    )
}
pub fn bind_victim_mob(attacker_name: &str, target_name: &str) -> String {
    format!(
        "¡{} {attacker_name} clava sus fauces alrededor {} {target_name}!",
        cap_article(attacker_name),
        de_contraction(target_name)
    )
}

pub fn medusa_gaze_line() -> &'static str {
    "¡Los ojos se cruzan con los de la medusa — y la carne se vuelve piedra fría!"
}

// ---------------------------------------------------------------------------
// models/src/visibility.rs
// ---------------------------------------------------------------------------

pub fn spotted_line(phrase: &str, worn: &str) -> String {
    format!("A la vista: {phrase}{worn}.")
}

pub fn trap_spotted(_article: &str, label: &str) -> String {
    format!("Detección: una {label}.")
}

// ---------------------------------------------------------------------------
// models/src/helpers.rs
// ---------------------------------------------------------------------------

pub fn ward_turns_aside(name: &str) -> String {
    format!(
        "La protección {} {name} desvía la magia.",
        de_contraction(name)
    )
}

pub fn unharmed_by(name: &str, element_noun: &str) -> String {
    format!(
        "{} {name} no sufre daño por {element_noun}.",
        cap_article(name)
    )
}

pub fn badly_wounded() -> &'static str {
    "¡Las heridas son graves!"
}

// ---------------------------------------------------------------------------
// models/src/components.rs — Element::noun()
// ---------------------------------------------------------------------------

pub fn element_fire_noun() -> &'static str {
    "las llamas"
}
pub fn element_cold_noun() -> &'static str {
    "el frío"
}
pub fn element_drain_noun() -> &'static str {
    "magia maligna"
}

// ---------------------------------------------------------------------------
// models/src/saveload.rs
// ---------------------------------------------------------------------------

pub fn retired_enchantment_singular() -> &'static str {
    "Un encantamiento de esta partida no existe en esta versión, y se pierde."
}

pub fn retired_enchantment_plural(n: usize) -> String {
    format!("{n} encantamientos de esta partida no existen en esta versión, y se pierden.")
}

// ---------------------------------------------------------------------------
// models/src/map/levels.rs
// ---------------------------------------------------------------------------

pub fn element_seeks_the_sun() -> &'static str {
    "El Elemento de Yoord busca el sol; no permitirá bajar más."
}

pub fn cannot_go_down() -> &'static str {
    "No hay bajada posible desde aquí."
}

pub fn dungeon_lord_prevents_up() -> &'static str {
    "El poder del Señor de la Mazmorra impide subir la escalera."
}

pub fn cannot_go_up() -> &'static str {
    "No hay subida posible desde aquí."
}

pub fn climb_last_stair() -> &'static str {
    "La última escalera lleva al cielo abierto, con el Elemento de Yoord ardiendo en las manos."
}

pub fn portal_down(depth: u8) -> String {
    format!(
        "¡El Señor de la Mazmorra abre un portal bajo los pies! Una caída hacia abajo. (Profundidad {depth})"
    )
}

pub fn portal_up(depth: u8) -> String {
    format!(
        "¡El Elemento de Yoord destella y rasga un portal en lo alto! Un ascenso repentino. (Profundidad {depth})"
    )
}

pub fn trapdoor_arrival(depth: u8) -> String {
    format!("Un choque contra el piso de abajo, entre una nube de polvo. (Profundidad {depth})")
}

pub fn potion_arrival(depth: u8) -> String {
    format!(
        "La piedra de arriba se disuelve en nada y un ascenso a través de ella. (Profundidad {depth})"
    )
}

pub fn descend_stairs(depth: u8) -> String {
    format!("Descenso por la escalera. (Profundidad {depth})")
}

pub fn climb_stairs(depth: u8) -> String {
    format!("Ascenso por la escalera. (Profundidad {depth})")
}

pub fn element_wont_let_you_land() -> &'static str {
    "El Elemento de Yoord tira hacia el sol — pero la última escalera hay que subirla por cuenta propia."
}

pub fn portal_no_deeper_floor() -> &'static str {
    "El Señor de la Mazmorra araña el suelo, pero no hay ningún piso más profundo adonde arrojar."
}

// ---------------------------------------------------------------------------
// models/src/items/pickups.rs
// ---------------------------------------------------------------------------

pub fn hidden_item_found() -> &'static str {
    "¡Eh! ¡Hay algo aquí!"
}

pub fn take_element_of_yoord() -> &'static str {
    "El Elemento de Yoord queda en las manos. \"El elemento de Yoord busca el sol.\""
}

pub fn pick_up(taken: &str) -> String {
    format!("Recoge {taken}.")
}

pub fn pick_up_pickup(name: &str, line: &str) -> String {
    format!("Recoge {} {name}. {line}", article(name))
}

pub fn coin_gives_itself_up(name: &str, line: &str) -> String {
    format!(
        "{} {name} se entrega por su cuenta. {line}",
        cap_article(name)
    )
}

pub fn coin_ledger() -> &'static str {
    "Directo al registro."
}

pub fn heal_line(healed: i32) -> String {
    format!("Un calor reconfortante lo recorre todo. ({healed} PV)")
}

pub fn refill_magic_line(gained: u8) -> String {
    format!("Algo frío y brillante llena la mente. ({gained} Ma)")
}

pub fn cleanse_one() -> &'static str {
    "El sabor despeja una cosa."
}

pub fn cleanse_many(n: i32) -> String {
    format!("El sabor despeja {n} cosas.")
}

pub fn learn_spell_full() -> &'static str {
    "Algo ancestral se agita en la mente y no encuentra dónde asentarse."
}

pub fn learn_spell_all_known() -> &'static str {
    "Algo ancestral se agita en la mente y no encuentra nada nuevo que enseñar."
}

pub fn learn_spell_line(name: &str) -> String {
    format!("Algo ancestral y violento se asienta en la mente. ¡{name} queda aprendido!")
}

pub fn restore_strength_line(given: i32) -> String {
    format!("El brazo recuerda lo que era. ({given} Pod.)")
}

pub fn promise_platinum_offer() -> &'static str {
    "No se empaña. Por ahora, tampoco nadie más lo hará. (PLAT)"
}
pub fn promise_forge_offer() -> &'static str {
    "Todavía está tibio. Algo se está forjando. (FORJ)"
}
pub fn promise_platinum_broken() -> &'static str {
    "El platino se opaca. Se acabó la perfección."
}
pub fn promise_forge_broken() -> &'static str {
    "La forja se enfría."
}

pub fn pay_platinum_power() -> &'static str {
    "Sin empañarse. El platino pasa al brazo. (Pod. +1)"
}
pub fn pay_platinum_armor() -> &'static str {
    "Sin empañarse. El platino pasa a la piel. (Arm. +1)"
}
pub fn pay_forge_collects() -> &'static str {
    "La forja cobra su parte. Algo del inventario queda terminado como es debido."
}
pub fn pay_forge_nothing_worth() -> &'static str {
    "...pero no hay nada en el inventario que valga la pena terminar."
}

// ---------------------------------------------------------------------------
// models/src/items/potions.rs
// ---------------------------------------------------------------------------

pub fn potion_healing_player() -> &'static str {
    "¡Las heridas cierran y una sensación renovada lo invade todo!"
}
pub fn potion_healing_mob() -> &'static str {
    "brilla de forma extraña, con las heridas cerrando"
}
pub fn potion_extra_healing_player() -> &'static str {
    "Jamás hubo una sensación mejor que esta."
}

pub fn potion_confusion_player() -> &'static str {
    "¡El mundo entero gira! La confusión lo invade todo."
}
pub fn potion_confusion_mob() -> &'static str {
    "se tambalea, con los ojos dando vueltas"
}

pub fn potion_gain_strength_player() -> &'static str {
    "¡Una fuerza nueva se instala! Menudos músculos."
}
pub fn potion_gain_strength_mob() -> &'static str {
    "se hincha de músculo"
}

pub fn potion_gain_magic_player() -> &'static str {
    "La mente se despeja y sigue despejándose — el poder está ahí, completo."
}
pub fn potion_gain_magic_mob() -> &'static str {
    "vibra con un poder prestado"
}

pub fn potion_poison_player() -> &'static str {
    "Una náusea profunda se instala — la fuerza se escurre por completo."
}
pub fn potion_poison_mob() -> &'static str {
    "se arquea con arcadas, los miembros aflojándose"
}

pub fn potion_restore_strength_noop_player() -> &'static str {
    "Un calor recorre todo el cuerpo."
}
pub fn potion_restore_strength_noop_mob() -> &'static str {
    "tiembla"
}
pub fn potion_restore_strength_player() -> &'static str {
    "La antigua fuerza vuelve al brazo de golpe."
}
pub fn potion_restore_strength_mob() -> &'static str {
    "se yergue, con la fuerza de vuelta"
}

pub fn potion_see_invisible_player() -> &'static str {
    "Los ojos escuecen, y el aire se llena de cosas que nunca dejaron de estar ahí."
}
pub fn potion_see_invisible_mob() -> &'static str {
    "los ojos brillan, siguiendo algo invisible"
}

pub fn detect_monsters_none() -> &'static str {
    "Un silencio total responde — nada se mueve en este piso."
}
pub fn detect_monsters_some() -> &'static str {
    "Los habitantes del piso se agitan, en algún punto de la oscuridad."
}

pub fn detect_magic_none() -> &'static str {
    "El zumbido de la magia no responde — este piso no tiene ninguna."
}
pub fn detect_magic_some() -> &'static str {
    "La magia zumba desde el suelo, y su ubicación exacta queda clara del todo."
}

pub fn distant_laughter() -> &'static str {
    "Una risa lejana llega desde algún lugar."
}

pub fn raise_level_win() -> &'static str {
    "La poción arrastra hacia arriba, a través de piedra y raíz, hasta el cielo abierto. La libertad, por fin."
}

pub fn potion_fruit_juice() -> &'static str {
    "Fría, dulce y espesa. ¡Delicia!"
}
pub fn potion_water() -> &'static str {
    "Es agua. Solo agua."
}
pub fn potion_flavour_mob() -> &'static str {
    "se relame los labios"
}

// ---------------------------------------------------------------------------
// models/src/items/throwing.rs
// ---------------------------------------------------------------------------

pub fn thrown_wand_confetti(seen_name: &str) -> String {
    format!(
        "{} {seen_name} estalla en una lluvia de confeti de colores. Eso es todo. Ese es el hechizo entero.",
        cap_article(seen_name)
    )
}

pub fn thrown_wand_shatters(seen_name: &str, charges: i32) -> String {
    format!(
        "¡{} {seen_name} se hace añicos, y {charges} cargas de magia escapan a la vez!",
        cap_article(seen_name)
    )
}

pub fn very_clever() -> &'static str {
    "Muy ingenioso."
}

pub fn you_fire(phrase: &str) -> String {
    format!("Un disparo sale disparado: {phrase}.")
}
pub fn you_throw(seen_name: &str) -> String {
    format!("{} {seen_name} sale volando.", cap_article(seen_name))
}
pub fn mob_fires(thrower: &str, phrase: &str) -> String {
    format!("{} {thrower} dispara {phrase}.", cap_article(thrower))
}
pub fn mob_throws(thrower: &str, seen_name: &str) -> String {
    format!(
        "{} {thrower} lanza {} {seen_name}.",
        cap_article(thrower),
        article(seen_name)
    )
}

pub fn scroll_read_aloud(who: &str, seen_name: &str) -> String {
    format!(
        "{who} desenrolla {} {seen_name} y lo lee en voz alta.",
        article(seen_name)
    )
}

pub fn wand_clatters_unspent(seen_name: &str) -> String {
    format!(
        "{} {seen_name} cae al suelo con estrépito, con la magia todavía intacta.",
        cap_article(seen_name)
    )
}

pub fn picked_up_thrown_verb_hand() -> &'static str {
    "lo agarra y lo empuña"
}
pub fn picked_up_thrown_verb_body() -> &'static str {
    "se lo pone encima"
}
pub fn picked_up_thrown_verb_other() -> &'static str {
    "se lo desliza puesto"
}
pub fn picks_up_thrown(victim_name: &str, verb: &str) -> String {
    format!("¡{} {victim_name} {verb}!", cap_article(victim_name))
}

/// "arrow"/"arrows"/"quarrel"/"quarrels" are the only ammo-noun ids the
/// engine ever passes here, so unlike other content ids (left untranslated,
/// see `content_name`), these read naturally enough as ordinary Spanish
/// words that there's no reason not to translate them.
fn ammo_word(noun: &str) -> &str {
    match noun {
        "arrow" => "flecha",
        "arrows" => "flechas",
        "quarrel" => "virote",
        "quarrels" => "virotes",
        _ => noun,
    }
}

pub fn monster_shot_wild(shooter_name: &str, noun: &str, target_label: &str) -> String {
    format!(
        "{} {shooter_name} suelta {} {} {} — pasa lejos de {target_label}.",
        cap_article(shooter_name),
        indef_article(noun),
        ammo_word(noun),
        wild(noun)
    )
}

pub fn monster_shot_hit(
    shooter_name: &str,
    _article: &str,
    noun: &str,
    target_label: &str,
    damage: i32,
) -> String {
    format!(
        "¡{} {shooter_name} suelta {} {} contra {target_label} por {damage} de daño!",
        cap_article(shooter_name),
        indef_article(noun),
        ammo_word(noun)
    )
}

pub fn potion_shatters_floor(seen_name: &str) -> String {
    format!(
        "{} {seen_name} se hace añicos contra el suelo.",
        cap_article(seen_name)
    )
}

pub fn potion_bursts_over(seen_name: &str, victim_name: &str) -> String {
    format!(
        "¡{} {seen_name} estalla sobre {} {victim_name}, que se atraganta con un buche entero!",
        cap_article(seen_name),
        article(victim_name)
    )
}

pub fn throw_bounces_off(seen_name: &str, hit_name: &str) -> String {
    format!(
        "{} {seen_name} rebota en {} {hit_name}.",
        cap_article(seen_name),
        article(hit_name)
    )
}

pub fn throw_glances_off(seen_name: &str, hit_name: &str) -> String {
    format!(
        "{} {seen_name} roza {} {hit_name} sin efecto.",
        cap_article(seen_name),
        a_contraction(hit_name)
    )
}

pub fn throw_hits(seen_name: &str, hit_name: &str, damage: i32) -> String {
    format!(
        "{} {seen_name} golpea {} {hit_name} por {damage} de daño.",
        cap_article(seen_name),
        a_contraction(hit_name)
    )
}

// ---------------------------------------------------------------------------
// models/src/items/spells.rs
// ---------------------------------------------------------------------------

pub fn no_magic_for_that() -> &'static str {
    "No queda magia suficiente para eso."
}

pub fn you_cast(name: &str) -> String {
    format!("¡{name} sale lanzado!")
}

pub fn sting_misses() -> &'static str {
    "El dardo de veneno no encuentra nada que morder."
}
pub fn sting_glances(name: &str) -> String {
    format!("El dardo roza {} {name} sin efecto.", a_contraction(name))
}
pub fn sting_hits(name: &str, damage: i32) -> String {
    format!(
        "¡Un dardo verde de veneno pincha {} {name} por {damage} de daño!",
        a_contraction(name)
    )
}

pub fn thunderbolt_misses() -> &'static str {
    "El trueno estalla sobre piedra vacía."
}
pub fn thunderbolt_hits(name: &str, damage: i32) -> String {
    format!(
        "¡Un rayo de trueno golpea {} {name} por {damage} de daño!",
        a_contraction(name)
    )
}

pub fn cure_self_nothing_to_cure() -> &'static str {
    "No hay nada que curar por aquí."
}

pub fn bide_coil() -> &'static str {
    "Un enroscamiento reúne fuerzas para el golpe que viene."
}

pub fn breathe_fire_player() -> &'static str {
    "¡Un torrente de fuego brota hacia afuera!"
}
pub fn breathe_fire_mob(name: &str) -> String {
    format!(
        "¡Un torrente de fuego brota {} {name}!",
        de_contraction(name)
    )
}

pub fn force_lance_cast() -> &'static str {
    "¡Un puño invisible golpea a lo largo de la línea!"
}
pub fn force_lance_hits(name: &str, damage: i32) -> String {
    format!(
        "¡La lanza de fuerza golpea {} {name} por {damage} de daño!",
        a_contraction(name)
    )
}

pub fn setup_planted() -> &'static str {
    "Unas trampas de flechas quedan plantadas a los flancos, resortes listos a plena vista."
}
pub fn setup_no_room() -> &'static str {
    "No hay espacio en los flancos para una trampa."
}

pub fn lux_cast() -> &'static str {
    "¡Un fragmento de luz pura sale disparado!"
}

pub fn circle_of_death_nothing() -> &'static str {
    "Una luz cenicienta se reúne alrededor y no encuentra nada de qué alimentarse."
}
pub fn circle_of_death_cast() -> &'static str {
    "Una luz cenicienta se alza del suelo. Llamas innecesarias rugen por la sala, y \
         todo lo que tocan se vuelve gris."
}
pub fn circle_of_death_drain(drained: i32) -> String {
    format!("El círculo drena {drained} de vida.")
}

pub fn magic_ward_cast() -> &'static str {
    "Una piel fría y plateada se cierra alrededor. Solo la magia propia puede tocar ahora — \
         durante el resto de este piso."
}

pub fn heal_self_line(healed: i32) -> String {
    format!("Un calor inunda por dentro y las heridas cierran. (+{healed} PV)")
}
pub fn heal_self_full() -> &'static str {
    "La fuerza ya está al máximo."
}

pub fn meteor_strike_cast() -> &'static str {
    "El fuego sale volando hacia el cielo. No cae de vuelta donde se esperaría."
}
pub fn meteor_screams_down() -> &'static str {
    "¡Un meteoro cae con un rugido!"
}
pub fn sky_tears_open_again() -> &'static str {
    "¡El cielo se rasga otra vez!"
}

pub fn frost_nova_cast() -> &'static str {
    "¡Una ESTRELLA CIAN estalla alrededor — hielo, brillo, y de sobra de ambos!"
}

pub fn haste_self_gathers() -> &'static str {
    "El poder se reúne. El poder se reúne más."
}
pub fn haste_self_the_fast() -> &'static str {
    "Nada de rápido. Nada de veloz. Esto es LA VELOCIDAD MISMA."
}

// ---------------------------------------------------------------------------
// models/src/items/scrolls.rs
// ---------------------------------------------------------------------------

pub fn already_recognise_everything() -> &'static str {
    "Todo en el inventario ya es reconocible."
}
pub fn identify_everything() -> &'static str {
    "¡El pergamino identifica todo en el inventario!"
}

pub fn remove_curse_freed() -> &'static str {
    "Una sensación de vigilancia protectora se instala. El equipo maldito se desmorona."
}
pub fn remove_curse_nothing() -> &'static str {
    "Una sensación de vigilancia protectora se instala."
}

pub fn scare_monster_some() -> &'static str {
    "¡El pergamino arde con el patetismo puro del miedo!"
}
pub fn scare_monster_none() -> &'static str {
    "El pergamino irradia un aura amenazante, pero no hay nada aquí para sentirla."
}

pub fn blank_paper() -> &'static str {
    "El pergamino está en blanco. Alguien se llevó la última risa."
}

pub fn amnesia_poof() -> &'static str {
    "1... 2... ¡Puf!"
}
pub fn amnesia_nothing_to_forget() -> &'static str {
    "No había nada ahí para olvidar."
}
pub fn amnesia_forgotten(name: &str) -> String {
    format!("¡El saber usar {name} queda olvidado!")
}
pub fn amnesia_dungeon_slips_away() -> &'static str {
    "La mazmorra entera se escurre como un sueño a medio recordar."
}

pub fn teleport_scroll_blonk() -> &'static str {
    "¡PLOF! ¡Un salto lo lleva lejos de aquí!"
}

pub fn aggravate_scroll_shriek() -> &'static str {
    "Un chillido agudo atraviesa la mazmorra entera. Todo en este piso lo ha oído — y ya sabe dónde está."
}

pub fn create_monster_nowhere() -> &'static str {
    "El aire se cuaja — y luego se asienta. Lo que fuera a venir, lo pensó mejor."
}
pub fn create_monster_line(_article: &str, name: &str) -> String {
    format!(
        "¡El aire se cuaja en {} {name}, dientes y todo!",
        indef_article(name)
    )
}

pub fn vorpalize_fizzles() -> &'static str {
    "El pergamino se apaga sin lograr marcar ningún arma."
}
pub fn vorpalize_crumbles(wname: &str) -> String {
    format!(
        "¡{} {wname} grita de dolor y se deshace en polvo!",
        cap_article(wname)
    )
}
pub fn vorpalize_branded(wname: &str, bane: &str) -> String {
    format!(
        "{} {wname} canta con un filo de luz afilada, un presagio de muerte para cualquier {bane}.",
        cap_article(wname)
    )
}

pub fn enchant_sparks(name: &str) -> String {
    format!(
        "¡{} {name} suelta una lluvia de chispas naranjas!",
        cap_article(name)
    )
}
pub fn enchant_curse_burns(name: &str) -> String {
    format!(
        "La maldición {} {name} arde junto con ellas.",
        de_contraction(name)
    )
}
pub fn enchant_missing_armor() -> &'static str {
    "Las chispas destellan sobre piel desnuda y se apagan. No hay armadura puesta."
}
pub fn enchant_missing_weapon() -> &'static str {
    "Las chispas destellan sobre una mano vacía y se apagan."
}

pub fn confusing_touch_fresh_player() -> &'static str {
    "Un brillo violeta se enciende sobre las manos. Lo próximo que toquen se arrepentirá."
}
pub fn confusing_touch_fresh_mob() -> &'static str {
    "flexiona las garras, y un brillo violeta las recorre"
}
pub fn confusing_touch_deeper_player() -> &'static str {
    "El brillo violeta sobre las manos se intensifica. Sigue siendo un solo toque."
}
pub fn confusing_touch_deeper_mob() -> &'static str {
    "sacude las garras brillantes"
}
pub fn confusing_touch_discharge_player() -> &'static str {
    "¡El brillo violeta estalla de golpe — la sala entera se inclina!"
}
pub fn confusing_touch_discharge_mob() -> &'static str {
    "se tambalea cuando el brillo violeta estalla sobre él"
}

pub fn hold_monster_none() -> &'static str {
    "Las palabras caen como hierro — sobre absolutamente nada."
}
pub fn hold_monster_some() -> &'static str {
    "Las palabras caen como hierro. Toda criatura a la vista queda clavada donde está."
}

pub fn sleep_scroll_none() -> &'static str {
    "Una ola de somnolencia se despliega sobre una sala vacía."
}
pub fn sleep_scroll_some() -> &'static str {
    "Una ola de somnolencia se despliega, y todo a la vista cae rendido con ella."
}
pub fn sleep_backfire_player() -> &'static str {
    "Las palabras se arrastran y se espesan en la propia boca. El suelo se acerca..."
}
pub fn sleep_backfire_mob() -> &'static str {
    "se derrumba en el suelo, leyendo todavía"
}

pub fn food_detection_not_player() -> &'static str {
    "Las palabras no significan nada para eso."
}
pub fn food_detection_none() -> &'static str {
    "La búsqueda de algo simple y útil no encuentra nada — este piso está vacío de eso."
}
pub fn food_detection_some() -> &'static str {
    "El piso entrega sus sobras: la ubicación de cada cosa simple queda clara."
}

// ---------------------------------------------------------------------------
// engine/src/update.rs
// ---------------------------------------------------------------------------

pub fn stumble_foolishly() -> &'static str {
    "Un tropezón torpe, sin más."
}

pub fn pack_full() -> &'static str {
    "El inventario está lleno."
}

pub fn strain_against_rooted() -> &'static str {
    "Un forcejeo contra lo que sujeta, sin ningún resultado."
}

pub fn too_confused_right_now() -> &'static str {
    "Demasiada confusión ahora mismo para eso."
}

pub fn too_injured_now() -> &'static str {
    "Demasiado herido ahora mismo para eso."
}

pub fn nothing_to_fight() -> &'static str {
    "No hay nada contra qué luchar."
}

pub fn cant_reach_it() -> &'static str {
    "Fuera de alcance desde aquí."
}

pub fn out_of_ammo() -> &'static str {
    "Sin munición."
}

pub fn out_of_range() -> &'static str {
    "Fuera de rango."
}

pub fn no_clear_shot() -> &'static str {
    "Sin línea de tiro clara."
}

pub fn well_played() -> &'static str {
    "Bien jugado."
}

pub fn great_idea_but_no() -> &'static str {
    "¡Buena idea! Pero no."
}

pub fn you_see_nothing_there() -> &'static str {
    "No hay nada visible ahí."
}

pub fn you_see(phrase: &str, worn: &str) -> String {
    format!("A la vista: {phrase}{worn}.")
}

pub fn beware_their(phrase: &str) -> String {
    format!("Cuidado con {phrase}.")
}

pub fn you_drop(name: &str) -> String {
    format!("{} {name} cae al suelo.", cap_article(name))
}

pub fn not_while_monster_in_sight() -> &'static str {
    "No mientras haya una criatura a la vista."
}

pub fn cant_run_that_way() -> &'static str {
    "No hay carrera posible en esa dirección."
}

pub fn no_spell_there() -> &'static str {
    "No hay ningún hechizo en esa posición."
}

pub fn no_spells() -> &'static str {
    "No hay hechizos disponibles."
}

pub fn toggle_on() -> &'static str {
    "ACTIVADO"
}
pub fn toggle_off() -> &'static str {
    "DESACTIVADO"
}
pub fn auto_pickup_state(state: &str) -> String {
    format!("Recogida automática en auto-exploración: {state}.")
}

pub fn not_wielding_launcher() -> &'static str {
    "No hay ningún lanzador empuñado."
}

pub fn no_ammo_to_fire(noun: &str) -> String {
    format!("No hay {} para disparar.", ammo_word(noun))
}

pub fn not_wielding_reach_weapon() -> &'static str {
    "No hay ningún arma de alcance empuñada."
}

pub fn nothing_left_to_explore() -> &'static str {
    "Nada más queda por explorar."
}

pub fn dir_up() -> &'static str {
    "arriba"
}
pub fn dir_down() -> &'static str {
    "abajo"
}

pub fn cannot_go_direction(dir: &str) -> String {
    format!("No hay paso hacia {dir} desde aquí.")
}

pub fn cant_find_path_to_stairs(dir: &str) -> String {
    format!("Ningún camino lleva a la escalera de {dir}.")
}

pub fn travel_interrupted() -> &'static str {
    "Viaje interrumpido."
}
pub fn auto_explore_interrupted() -> &'static str {
    "Auto-exploración interrumpida."
}

pub fn arrive_at_staircase() -> &'static str {
    "La escalera queda a la vista, alcanzada."
}
pub fn you_stop() -> &'static str {
    "Un alto en el camino."
}

pub fn monster_nearby() -> &'static str {
    "Hay un monstruo cerca."
}

pub fn cant_find_path_there() -> &'static str {
    "Ningún camino lleva hasta ahí."
}
pub fn explored_everywhere() -> &'static str {
    "Todo lo explorable ya ha sido explorado."
}

pub fn already_there() -> &'static str {
    "Ese punto ya está alcanzado."
}

// ---------------------------------------------------------------------------
// models/src/items.rs
// ---------------------------------------------------------------------------

pub fn cant_use_right_now(name: &str) -> String {
    format!("{name} no se puede usar ahora mismo.")
}

pub fn you_zap(seen_name: &str) -> String {
    format!("{} {seen_name} entra en acción.", cap_article(seen_name))
}

pub fn wand_crumbles(seen_name: &str) -> String {
    format!(
        "¡{} {seen_name} se deshace en polvo!",
        cap_article(seen_name)
    )
}
pub fn you_drink(seen_name: &str) -> String {
    format!(
        "{} {seen_name} desaparece de un trago.",
        cap_article(seen_name)
    )
}
pub fn you_read(seen_name: &str) -> String {
    format!("{} {seen_name} queda leído.", cap_article(seen_name))
}
pub fn ring_shivers_apart(seen_name: &str) -> String {
    format!(
        "{} {seen_name} se deshace en mil motas relucientes.",
        cap_article(seen_name)
    )
}
pub fn item_turns_to_dust() -> &'static str {
    "¡El objeto se convierte en polvo!"
}

// ---------------------------------------------------------------------------
// models/src/combat.rs
// ---------------------------------------------------------------------------

pub fn killer_unknown() -> &'static str {
    "Asesino desconocido"
}

pub fn mob_dies(name: &str) -> String {
    format!("{} {name} muere.", cap_article(name))
}

pub fn gear_clatters_to_floor(name: &str) -> String {
    format!("{} {name} cae al suelo con estrépito.", cap_article(name))
}

pub fn lunge_hit(target_name: &str, damage: i32) -> String {
    format!(
        "¡Una estocada, hoja destellando más allá de toda guardia, ensarta {} {target_name} por {damage} de daño!",
        a_contraction(target_name)
    )
}

pub fn you_have_slain(target_name: &str) -> String {
    format!("¡{} {target_name} queda abatido!", cap_article(target_name))
}

pub fn strike_at_nothing() -> &'static str {
    "Un golpe al aire, sin nada más."
}

pub fn slain_by(attacker_name: &str) -> String {
    format!("Abatido por {} {attacker_name}", article(attacker_name))
}

pub fn excellent_hit(target_name: &str, damage: i32) -> String {
    format!(
        "¡Un golpe excelente marca {} {target_name} por {damage} de daño!",
        a_contraction(target_name)
    )
}
pub fn glancing_blow(target_name: &str) -> String {
    format!(
        "Un golpe de refilón alcanza {} {target_name}.",
        a_contraction(target_name)
    )
}
pub fn plain_hit(target_name: &str, damage: i32) -> String {
    format!(
        "Un golpe alcanza {} {target_name} por {damage} de daño.",
        a_contraction(target_name)
    )
}
pub fn garrote_kill(target_name: &str) -> String {
    format!(
        "¡La vida se apaga a la fuerza en {} {} {target_name}! Atroz.",
        article(target_name),
        helpless(target_name)
    )
}
pub fn vorpal_kill(target_name: &str) -> String {
    format!(
        "¡Zas-zas! La hoja atraviesa limpiamente {} {target_name}!",
        a_contraction(target_name)
    )
}

pub fn mob_misses(atk: &str, target_label: &str) -> String {
    format!("{atk} falla contra {target_label}.")
}
pub fn mob_hits(atk: &str, target_label: &str, damage: i32) -> String {
    format!("{atk} golpea a {target_label} por {damage} de daño.")
}
pub fn mob_strikes_you_down(atk: &str) -> String {
    format!("{atk} derriba con un golpe final...")
}
pub fn mob_kills(atk: &str, target_name: &str) -> String {
    format!("¡{atk} acaba con {} {target_name}!", article(target_name))
}

pub fn blown_up_by(_article: &str, label: &str) -> String {
    format!("Volado en pedazos por una {label}")
}

// ---------------------------------------------------------------------------
// engine/src/view.rs — HUD badges, prompts, menu titles
// ---------------------------------------------------------------------------

pub fn badge_stone() -> &'static str {
    "PIEDRA"
}
pub fn badge_asleep() -> &'static str {
    "DORMIDO"
}
pub fn badge_held() -> &'static str {
    "INMOVIL."
}
pub fn badge_ascending() -> &'static str {
    "ASCENDIENDO"
}
pub fn badge_traveling() -> &'static str {
    "VIAJANDO"
}
pub fn badge_exploring() -> &'static str {
    "EXPLORANDO"
}
pub fn badge_travel_query() -> &'static str {
    "¿VIAJAR?"
}

pub fn depth_label() -> &'static str {
    "PROF."
}

pub fn travel_cursor_prompt() -> &'static str {
    "¿Moverse hacia dónde?"
}
pub fn travel_cursor_hint() -> &'static str {
    "[hjkl/flechas mover · Intro viajar · Esc/x cancelar]"
}

pub fn quit_question() -> &'static str {
    "¿Salir de verdad?"
}
pub fn quit_answers() -> &'static str {
    "[s] sí    [n] no"
}

pub fn spells_menu_title() -> &'static str {
    " HECHIZOS "
}

// ---------------------------------------------------------------------------
// models/src/pack.rs — the Use/Throw/Drop action modal
// ---------------------------------------------------------------------------

pub fn action_use() -> &'static str {
    " Usar   "
}
pub fn action_throw() -> &'static str {
    " Lanzar "
}
pub fn action_drop() -> &'static str {
    " Soltar "
}

// ---------------------------------------------------------------------------
// models/src/items/throwing.rs
// ---------------------------------------------------------------------------

pub fn element_wont_leave_hand() -> &'static str {
    "El Elemento de Yoord no saldrá de esta mano."
}

// ---------------------------------------------------------------------------
// models/src/monsters.rs — the xeroc's disguise, a cosmetic-only category word
// ---------------------------------------------------------------------------

pub const fn mimic_look_scroll() -> &'static str {
    "pergamino"
}
pub const fn mimic_look_potion() -> &'static str {
    "poción"
}
pub const fn mimic_look_wand() -> &'static str {
    "varita"
}
pub const fn mimic_look_gold_coin() -> &'static str {
    "moneda de oro"
}
pub const fn mimic_look_ring() -> &'static str {
    "anillo"
}
pub const fn mimic_look_suit_of_armor() -> &'static str {
    "armadura completa"
}
pub const fn mimic_look_weapon() -> &'static str {
    "arma"
}

// ---------------------------------------------------------------------------
// models/src/equipment.rs
// ---------------------------------------------------------------------------

pub fn worn_tag(names: &str) -> String {
    format!(" (p. {names})")
}

// ---------------------------------------------------------------------------
// Shared "the X" / "The X" / pronoun fragments (models/src/combat.rs,
// models/src/items/theft.rs, models/src/traps.rs)
// ---------------------------------------------------------------------------

pub fn the(name: &str) -> String {
    format!("{} {name}", article(name))
}
pub fn capital_the(name: &str) -> String {
    format!("{} {name}", cap_article(name))
}
pub fn pronoun_you() -> &'static str {
    "a ti"
}
pub fn pronoun_something() -> &'static str {
    "Algo"
}

pub fn adjective_sees_unseen() -> &'static str {
    "capaz de ver lo invisible"
}

pub fn equipped_suffix() -> &'static str {
    " (E)"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_agrees_with_grammatical_gender() {
        assert_eq!(cap_article("medusa"), "La");
        assert_eq!(article("medusa"), "la");
        assert_eq!(cap_article("orc"), "El");
        assert_eq!(article("orc"), "el");
    }

    #[test]
    fn contractions_and_agreement_follow_the_same_gender() {
        assert_eq!(indef_article("medusa"), "una");
        assert_eq!(cap_indef_article("medusa"), "Una");
        assert_eq!(de_contraction("medusa"), "de la");
        assert_eq!(a_contraction("medusa"), "a la");
        assert_eq!(helpless("medusa"), "indefensa");
        assert_eq!(de_contraction("orc"), "del");
        assert!(garrote_kill("medusa").contains("en la indefensa medusa"));
    }
}

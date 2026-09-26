//! Portuguese (Brazilian). Preliminary pass -- see `lib.rs`'s module doc
//! comment; still wants review by a native speaker (see `beta_notice` in
//! `lib.rs`).
//!
//! Style note: clitic pronouns (se/te/o/a...) lean proclitic ("se sente",
//! "o lê") throughout, matching everyday Brazilian usage, rather than the
//! enclitic literary forms ("sinta-se", "lê-o"). Where a sentence would
//! otherwise force an enclitic (an infinitive or imperative with nothing
//! else to attach the clitic to), a full pronoun/noun is used instead so no
//! enclitic form is needed at all.
//!
//! `content_name` (monster/item/trap ids) and `engine/src/main.rs`'s CLI
//! output (usage/help text, flag errors, save/load prompts) are out of
//! scope: only in-game text is localized here, so those stay re-exported
//! from `en`.
//!
//! A few upstream functions (`models/src/identify.rs`, `models/src/
//! monsters.rs`) hand this crate an English-only "a"/"an" article or a
//! literal "wielding"/"wearing" verb. That's staying as-is. Functions below
//! that take an `article` parameter ignore it and hardcode a plausible
//! Portuguese article instead.

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
/// coincidentally already be a real, gendered Portuguese word -- today just
/// "medusa" (spelled the same in English), which is feminine. "arrow"/
/// "arrows" are here for a different reason: unlike other content ids, ammo
/// nouns *do* get translated (see `ammo_word`), to "flecha(s)", which is
/// feminine. "quarrel(s)" needs no entry -- "virote" is already masculine,
/// matching the default. Everything else defaults masculine. Add an id here
/// if a future one collides the same way.
fn is_feminine_name(name: &str) -> bool {
    matches!(name, "medusa" | "arrow" | "arrows")
}

/// The definite article ("o"/"a") that reads correctly directly before
/// `name`, lowercase for mid-sentence use.
fn article(name: &str) -> &'static str {
    if is_feminine_name(name) { "a" } else { "o" }
}

/// [`article`], capitalized for sentence-initial use.
fn cap_article(name: &str) -> &'static str {
    if is_feminine_name(name) { "A" } else { "O" }
}

/// The indefinite article ("um"/"uma") that reads correctly directly before
/// `name`.
fn indef_article(name: &str) -> &'static str {
    if is_feminine_name(name) { "uma" } else { "um" }
}

/// [`indef_article`], capitalized for sentence-initial use.
fn cap_indef_article(name: &str) -> &'static str {
    if is_feminine_name(name) { "Uma" } else { "Um" }
}

/// "de" + [`article`], contracted as Portuguese always does: "do"/"da".
fn de_contraction(name: &str) -> &'static str {
    if is_feminine_name(name) { "da" } else { "do" }
}

/// "em" + [`article`], contracted: "no"/"na".
fn em_contraction(name: &str) -> &'static str {
    if is_feminine_name(name) { "na" } else { "no" }
}

/// "por" + [`article`], contracted: "pelo"/"pela".
fn por_contraction(name: &str) -> &'static str {
    if is_feminine_name(name) {
        "pela"
    } else {
        "pelo"
    }
}

/// "helpless", agreeing with `name`'s gender: "indefeso"/"indefesa".
fn helpless(name: &str) -> &'static str {
    if is_feminine_name(name) {
        "indefesa"
    } else {
        "indefeso"
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
    "Vida"
}

pub fn magic_abbr() -> &'static str {
    "Magia"
}

pub fn power_abbr() -> &'static str {
    "For."
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
    format!("TEMA O LURK! (+{step} {stat})")
}

pub fn no_hands_lurk(item_name: &str) -> String {
    format!("Pelo, presas e quatro patas: nenhuma parte de um lurk seguraria {item_name}.")
}

pub fn no_hands_monster(species: &str, item_name: &str) -> String {
    format!(
        "{} {species} não tem mãos para {item_name}.",
        cap_indef_article(species)
    )
}

// ---------------------------------------------------------------------------
// models/src/monsters.rs
// ---------------------------------------------------------------------------

pub fn wearing_gear(verb: &str, name: &str) -> String {
    let verb_pt = match verb {
        "wielding" => "empunhando",
        "wearing" => "vestindo",
        other => other,
    };
    format!("Está {verb_pt} {name}.")
}

pub fn xeroc_disguise_falls() -> &'static str {
    "O disfarce cai — era um xeroc o tempo todo!"
}

// ---------------------------------------------------------------------------
// models/src/items/wands.rs
// ---------------------------------------------------------------------------

pub fn dazzle_player_line() -> &'static str {
    "O clarão deixa tudo girando — o ofuscamento toma conta!"
}

pub fn dazzle_mob_verb() -> &'static str {
    "fica ofuscado"
}

pub fn bolt_magic_missile() -> &'static str {
    "Um raio brilhante salta da varinha!"
}

pub fn bolt_lightning() -> &'static str {
    "Um forte relâmpago estala no ar!"
}

pub fn bolt_striking() -> &'static str {
    "Um punho invisível emerge, golpeando tudo em seu caminho!"
}

pub fn bolt_drain_life() -> &'static str {
    "Um filamento das trevas drena a vida em seu caminho!"
}

pub fn drain_life_gained(taken: i32) -> String {
    format!("{taken} de vida são drenados.")
}

pub fn blast_fire() -> &'static str {
    "Uma bola de fogo explode!"
}

pub fn blast_cold() -> &'static str {
    "Uma rajada de ar congelante detona!"
}

pub fn wand_does_nothing() -> &'static str {
    "A varinha não faz nada. Bem que o nome combina."
}

pub fn light_reveals(label: &str) -> String {
    format!("A luz revela {label}!")
}

pub fn light_floods_room() -> &'static str {
    "Uma luz quente inunda a sala."
}

pub fn light_races_passage() -> &'static str {
    "A luz brilha em todo o corredor."
}

pub fn polymorph_fizzles() -> &'static str {
    "A mágica da mudança se apaga contra o nada."
}

pub fn polymorph_self_player() -> &'static str {
    "Você se sente uma nova pessoa!"
}

pub fn polymorph_same_looking(old_name: &str, new_name: &str) -> String {
    format!(
        "{} {old_name} se contorce e se transforma em {} {new_name} diferente!",
        cap_article(old_name),
        indef_article(new_name)
    )
}

pub fn polymorph_different(old_name: &str, _article: &str, new_name: &str) -> String {
    format!(
        "{} {old_name} se contorce e se transforma em {} {new_name}!",
        cap_article(old_name),
        indef_article(new_name)
    )
}

pub fn nothing_to_enchant() -> &'static str {
    "Não há nada para encantar."
}

pub fn teleport_pull_finds_nothing() -> &'static str {
    "O puxão da varinha não encontra nada."
}

pub fn yanked_into_dark(name: &str) -> String {
    format!("{} {name} é arrastado para a escuridão.", cap_article(name))
}

pub fn dragged_to_your_side(name: &str) -> String {
    format!(
        "{} {name} é arrastado para perto de uma vez só!",
        cap_article(name)
    )
}

pub fn bursts_in_transit(name: &str) -> String {
    format!(
        "{} {name} é arrastado para dentro do chão e explode!",
        cap_article(name)
    )
}

pub fn teleport_self_player() -> &'static str {
    "O teletransporte manda direto para o mesmo lugar! Que viagem."
}

pub fn teleport_self_mob(name: &str) -> String {
    format!(
        "{} {name} se teletransporta direto para si mesmo.",
        cap_article(name)
    )
}

pub fn cancellation_strikes_stone() -> &'static str {
    "O raio cinza atinge apenas pedra."
}

pub fn cancellation_sputters(name: &str) -> String {
    format!("A magia {} {name} crepita e morre.", de_contraction(name))
}

pub fn cancellation_player_wave() -> &'static str {
    "Uma onda cinza se derrama por cima. O inventário emudece, o equipamento fica sem graça, e cada maldição simplesmente se solta."
}

// ---------------------------------------------------------------------------
// models/src/traps.rs
// ---------------------------------------------------------------------------

pub fn bear_trap_thrash() -> &'static str {
    "Na tentativa de escapar, a armadilha rasga a perna presa."
}

pub fn steps_on_trap(who: &str, _article: &str, label: &str) -> String {
    format!("{who} pisa em uma {label}!")
}

pub fn trap_breaks(label: &str) -> String {
    format!("A {label} se quebra!")
}

pub fn hero_coin_ultimate() -> &'static str {
    "A moeda do herói entrega tudo que sabe de uma vez."
}

pub fn relic_takes_the_hit() -> &'static str {
    "O Elemento de Yoord recebe o golpe — e responde."
}

pub fn ultimate_trick_shot_shout() -> &'static str {
    "TIRO CERTEIRO DEFINITIVO!"
}

pub fn trick_shot_shout_self() -> &'static str {
    "POR QUÊ!"
}

pub fn trick_shot_shout_other() -> &'static str {
    "PA!"
}

pub fn trick_shot_line(shout: &str) -> String {
    format!("{shout} Tiro com efeito!")
}

pub fn drops_through_trapdoor(who: &str) -> String {
    format!("{who} cai pelo alçapão e desaparece.")
}

pub fn trapdoor_grinds_shut() -> &'static str {
    "Um alçapão se abre — mas só há rocha sólida lá embaixo. Ele range e fecha de novo."
}

pub fn trapdoor_yawns_open() -> &'static str {
    "Um alçapão se escancara de repente!"
}

pub fn bear_trap_snare() -> &'static str {
    "Presas de aço se fecham com um estalo — a perna fica presa, mas os braços continuam livres!"
}

pub fn sleep_gas_snare() -> &'static str {
    "Um gás sobe ao redor. As pálpebras viram chumbo..."
}

pub fn teleport_trap_whisked() -> &'static str {
    "As paredes mudam! Um salto leva a outra parte da masmorra."
}

pub fn arrow_whistles_past(who: &str) -> String {
    format!("Uma flecha assobia perto de {who} e cai tilintando no chão.")
}

pub fn arrow_plinks(who: &str, damage: i32) -> String {
    format!("Uma flecha crava em {who} e causa {damage} de dano!")
}

pub fn dart_glances_off(who: &str) -> String {
    format!("Um dardo raspa em {who} sem efeito.")
}

pub fn dart_pricks(who: &str, damage: i32) -> String {
    format!("Um dardo envenenado fura {who} e causa {damage} de dano!")
}

pub fn dart_poison_resisted() -> &'static str {
    "O veneno arde, mas a força não cede."
}

pub fn dart_poison_took() -> &'static str {
    "O veneno corre por dentro — a força escorre aos poucos."
}

// ---------------------------------------------------------------------------
// models/src/items/theft.rs
// ---------------------------------------------------------------------------

pub fn leprechaun_theft(attacker: &str, item: &str, target: &str) -> String {
    format!(
        "{} {attacker} arranca {item} de {target} e solta uma gargalhada!",
        cap_article(attacker)
    )
}

pub fn nymph_theft(attacker: &str, item: &str, target: &str) -> String {
    format!(
        "{} {attacker} arranca {item} de {target} e some em uma nuvem de fumaça!",
        cap_article(attacker)
    )
}

// ---------------------------------------------------------------------------
// models/src/items/rings.rs
// ---------------------------------------------------------------------------

pub const FANFARE: [&str; 4] = [
    "Magenta, ciano e dourado jorram tudo de uma vez.",
    "Por um instante, a masmorra inteira é o baile.",
    "Com luz e estilo!",
    "A pontuação dobra!",
];

pub fn adornment_spent(name: &str) -> String {
    format!("{} {name} não tem mais nada a conceder.", cap_article(name))
}

// ---------------------------------------------------------------------------
// models/src/hud.rs, models/src/score.rs
// ---------------------------------------------------------------------------

pub fn combo_word() -> &'static str {
    "COMBO!"
}

pub fn with_pride() -> &'static str {
    "Com orgulho."
}

pub fn with_style() -> &'static str {
    "Com estilo."
}

// ---------------------------------------------------------------------------
// models/src/catalog.rs, models/src/components.rs
// ---------------------------------------------------------------------------

pub fn wizard_now() -> &'static str {
    "Agora há um mago na masmorra!"
}

pub fn wizard_no_more() -> &'static str {
    "A magia de mago já era."
}

pub fn welcome_new_run() -> &'static str {
    "Bem-vindo ao nihilurk! Boa sorte e divirta-se!"
}

pub fn welcome_back() -> &'static str {
    "De volta ao nihilurk! Boa sorte e divirta-se!"
}

// ---------------------------------------------------------------------------
// engine/src/view.rs
// ---------------------------------------------------------------------------

pub fn more_prompt() -> &'static str {
    "--MAIS-- (Pressione Espaço)"
}

pub fn you_die() -> &'static str {
    "A morte chega..."
}

pub fn lose_title() -> &'static str {
    "DERROTA"
}

pub fn win_title() -> &'static str {
    "VITÓRIA"
}

pub fn score_line(score_text: &str) -> String {
    format!("PONTOS {score_text}")
}

pub fn press_any_key_to_depart() -> &'static str {
    "Pressione qualquer tecla para sair."
}

// ---------------------------------------------------------------------------
// models/src/pride.rs
// ---------------------------------------------------------------------------

pub fn pride_off_refusal() -> &'static str {
    "ERRO: Ninguém jamais tomará nosso orgulho!"
}

// ---------------------------------------------------------------------------
// models/src/pack.rs
// ---------------------------------------------------------------------------

pub fn pack_title_browse() -> &'static str {
    " INVENTÁRIO "
}
pub fn pack_title_use() -> &'static str {
    " USAR O QUÊ? "
}
pub fn pack_title_throw() -> &'static str {
    " JOGAR O QUÊ? "
}
pub fn pack_title_drop() -> &'static str {
    " LARGAR O QUÊ? "
}
pub fn pack_title_equip() -> &'static str {
    " EQUIPAR O QUÊ? "
}
pub fn pack_title_quaff() -> &'static str {
    " BEBER O QUÊ? "
}
pub fn pack_title_read() -> &'static str {
    " LER O QUÊ? "
}
pub fn pack_title_zap() -> &'static str {
    " USAR QUAL VARINHA? "
}
pub fn pack_title_wield() -> &'static str {
    " EMPUNHAR O QUÊ? "
}
pub fn pack_title_wear() -> &'static str {
    " VESTIR O QUÊ? "
}
pub fn pack_title_put_on() -> &'static str {
    " COLOCAR O QUÊ? "
}

pub fn pack_nothing_browse() -> &'static str {
    "Não há nenhum item no inventário."
}
pub fn pack_nothing_use() -> &'static str {
    "Não há nada para usar."
}
pub fn pack_nothing_throw() -> &'static str {
    "Não há nada para jogar."
}
pub fn pack_nothing_drop() -> &'static str {
    "Não há nada para largar."
}
pub fn pack_nothing_equip() -> &'static str {
    "Não há nada para equipar."
}
pub fn pack_nothing_quaff() -> &'static str {
    "Não há nada para beber."
}
pub fn pack_nothing_read() -> &'static str {
    "Não há nada para ler."
}
pub fn pack_nothing_zap() -> &'static str {
    "Não há nenhuma varinha para usar."
}
pub fn pack_nothing_wield() -> &'static str {
    "Não há nada para empunhar."
}
pub fn pack_nothing_wear() -> &'static str {
    "Não há nada para vestir."
}
pub fn pack_nothing_put_on() -> &'static str {
    "Não há nada para colocar."
}

// ---------------------------------------------------------------------------
// models/src/magicmap.rs
// ---------------------------------------------------------------------------

pub fn magicmap_row_by_row() -> &'static str {
    "A forma da masmorra surge de repente na mente."
}
pub fn magicmap_spiral() -> &'static str {
    "A masmorra se desenrola ao redor como um pergaminho."
}
pub fn magicmap_explode() -> &'static str {
    "O conhecimento da masmorra explode para fora a partir deste ponto."
}

// ---------------------------------------------------------------------------
// models/src/effects.rs — the EFFECTS table's `ends`/`beware` text.
// ---------------------------------------------------------------------------

pub const fn beware_aggravating_shriek() -> &'static str {
    "grito que alerta a todos"
}
pub const fn beware_corrosive_touch() -> &'static str {
    "toque corrosivo"
}
pub const fn beware_regeneration() -> &'static str {
    "regeneração"
}
pub const fn beware_erratic_strikes() -> &'static str {
    "golpes erráticos"
}
pub const fn beware_binding_bite() -> &'static str {
    "mordida que prende"
}
pub const fn beware_petrifying_gaze() -> &'static str {
    "olhar petrificante"
}
pub const fn beware_draining_touch() -> &'static str {
    "toque drenante"
}
pub const fn beware_venomous_bite() -> &'static str {
    "mordida venenosa"
}
pub const fn beware_splitting_flesh() -> &'static str {
    "carne que se divide"
}
pub const fn beware_paralysing_touch() -> &'static str {
    "toque paralisante"
}
pub const fn beware_thieving_touch() -> &'static str {
    "toque ladrão"
}
pub const fn beware_fire_breath() -> &'static str {
    "sopro de fogo"
}
pub const fn beware_confusing_touch() -> &'static str {
    "toque que confunde"
}

pub const fn ends_asleep() -> &'static str {
    "A sonolência se dissipa e o despertar chega."
}
pub const fn ends_petrified() -> &'static str {
    "A pedra se desprende e a carne volta a ser sua."
}
pub const fn ends_pinned() -> &'static str {
    "A perna se solta com um puxão da armadilha."
}
pub const fn ends_rooted() -> &'static str {
    "O que segurava, solta."
}

// ---------------------------------------------------------------------------
// models/src/conditions.rs
// ---------------------------------------------------------------------------

pub fn mob_verb_line(name: &str, verb: &str) -> String {
    format!("{} {name} {verb}.", cap_article(name))
}

pub fn blind_mob_verb() -> &'static str {
    "tateia às cegas"
}

pub fn paralyzed_mob_verb() -> &'static str {
    "trava de repente, paralisado"
}

pub fn blind_player_line() -> &'static str {
    "Uma escuridão cai sobre os olhos. Nada fica visível!"
}

pub fn paralyse_player_line() -> &'static str {
    "Os membros travam de repente. Quase nenhum movimento é possível!"
}

pub fn paralysis_lost_turn() -> &'static str {
    "O corpo não responde."
}

pub const fn blind_cured_line() -> &'static str {
    "A escuridão se levanta dos olhos."
}
pub const fn blind_cured_noun() -> &'static str {
    "cegueira"
}
pub const fn blind_lifted_adjective() -> &'static str {
    "cego"
}
pub const fn paralyzed_cured_line() -> &'static str {
    "Os membros voltam a responder."
}
pub const fn paralyzed_cured_noun() -> &'static str {
    "paralisia"
}
pub const fn paralyzed_lifted_adjective() -> &'static str {
    "paralisado"
}
pub const fn confused_cured_line() -> &'static str {
    "A cabeça clareia."
}
pub const fn confused_cured_noun() -> &'static str {
    "confusão"
}
pub const fn confused_lifted_adjective() -> &'static str {
    "confuso"
}

pub const fn sluggish_cured_line() -> &'static str {
    "O chumbo sai das pernas."
}
pub const fn sluggish_cured_noun() -> &'static str {
    "lentidão"
}

pub const fn power_restored_line() -> &'static str {
    "A força volta a fluir pelo braço."
}
pub const fn power_restored_noun() -> &'static str {
    "fraqueza"
}

pub fn snaps_out_of(name: &str, noun: &str) -> String {
    format!("{} {name} sai de {noun} de repente.", cap_article(name))
}

pub const fn adjective_asleep() -> &'static str {
    "adormecido"
}
pub const fn adjective_pinned() -> &'static str {
    "preso"
}
pub const fn adjective_held() -> &'static str {
    "imobilizado"
}
pub const fn adjective_warded() -> &'static str {
    "protegido"
}
pub const fn adjective_coiled() -> &'static str {
    "enroscado"
}
pub const fn adjective_stone() -> &'static str {
    "de pedra"
}
pub const fn adjective_stealthy() -> &'static str {
    "furtivo"
}
pub const fn adjective_sluggish() -> &'static str {
    "lento"
}

pub fn no_longer(adjective: &str) -> String {
    format!("Isso de estar {adjective} já passou.")
}

pub fn already_as_extreme_player(extreme: &str) -> String {
    format!("Já está no limite máximo de {extreme}.")
}

pub fn already_as_extreme_mob(name: &str, extreme: &str) -> String {
    format!(
        "{} {name} já está no limite máximo de {extreme}.",
        cap_article(name)
    )
}

pub fn extreme_quick() -> &'static str {
    "rápido"
}
pub fn extreme_sluggish() -> &'static str {
    "lento"
}

pub fn haste_player_line() -> &'static str {
    "O mundo mergulha em câmera lenta ao redor."
}
pub fn slow_player_line() -> &'static str {
    "Os membros viram chumbo."
}
pub fn haste_mob_line(name: &str) -> String {
    format!(
        "{} {name} vira um borrão de velocidade repentina.",
        cap_article(name)
    )
}
pub fn slow_mob_line(name: &str) -> String {
    format!("{} {name} mergulha em câmera lenta.", cap_article(name))
}

// ---------------------------------------------------------------------------
// models/src/equipment.rs
// ---------------------------------------------------------------------------

pub fn donned_hand(name: &str) -> String {
    format!("{} {name} passa a ser empunhado.", cap_article(name))
}
pub fn donned_body_or_finger(name: &str) -> String {
    format!("{} {name} é vestido.", cap_article(name))
}
pub fn doffed_hand(name: &str) -> String {
    format!("{} {name} deixa de ser empunhado.", cap_article(name))
}
pub fn doffed_body(name: &str) -> String {
    format!("{} {name} sai do corpo.", cap_article(name))
}
pub fn doffed_finger(name: &str) -> String {
    format!("{} {name} é removido.", cap_article(name))
}
pub fn stuck_hand(name: &str) -> String {
    format!("Impossível — {} {name} está soldado à mão!", article(name))
}
pub fn stuck_body(name: &str) -> String {
    format!(
        "Impossível — {} {name} gruda no corpo e não sai!",
        article(name)
    )
}
pub fn stuck_finger(name: &str) -> String {
    format!(
        "Impossível — {} {name} está fundido ao dedo!",
        article(name)
    )
}
pub fn cursed_reveal_hand(name: &str) -> String {
    format!(
        "{} {name} se solda à mão! Está amaldiçoado!",
        cap_article(name)
    )
}
pub fn cursed_reveal_body(name: &str) -> String {
    format!(
        "{} {name} gruda no corpo! Está amaldiçoado!",
        cap_article(name)
    )
}
pub fn cursed_reveal_finger(name: &str) -> String {
    format!(
        "{} {name} se funde ao dedo! Está amaldiçoado!",
        cap_article(name)
    )
}
pub fn blocked_hand(name: &str) -> String {
    format!(
        "Impossível trocar de arma — {} {name} não sai da mão.",
        article(name)
    )
}
pub fn blocked_body(name: &str) -> String {
    format!(
        "Impossível trocar de armadura — {} {name} não sai.",
        article(name)
    )
}
pub fn blocked_finger(name: &str) -> String {
    format!("Impossível — {} {name} não sai do dedo.", article(name))
}

pub fn armor_shrugs_off_corrosion() -> &'static str {
    "A armadura bebe a corrosão e a ignora."
}

pub fn armor_corrodes(name: &str) -> String {
    format!("A {name} corrói! Agora está mais fraca.")
}

// ---------------------------------------------------------------------------
// models/src/abilities.rs
// ---------------------------------------------------------------------------

pub const fn flavour_aggravates() -> &'static str {
    "AUUUUUUUUUU! Um uivo escapa do nada! O andar inteiro vai vir olhar."
}
pub const fn flavour_regenerates() -> &'static str {
    "O anel no seu dedo está quente."
}
pub const fn flavour_teleportitis() -> &'static str {
    "Algo que você carrega parece muito satisfeito consigo mesmo."
}

pub fn heavy_stagger_player() -> &'static str {
    "O golpe deixa tudo cambaleando — não há tempo de se recompor para revidar!"
}
pub fn heavy_stagger_mob(name: &str) -> String {
    format!(
        "{} {name} cambaleia, atordoado pelo golpe!",
        cap_article(name)
    )
}

pub fn chaos_recoil() -> &'static str {
    "A lâmina do caos morde de volta!"
}

pub fn venom_resisted_player() -> &'static str {
    "O veneno arde, mas a força não cede."
}
pub fn venom_resisted_mob(name: &str) -> String {
    format!(
        "O veneno arde, mas a força {} {name} não cede.",
        de_contraction(name)
    )
}
pub fn venom_took_player() -> &'static str {
    "O veneno corre por dentro — a força escorre aos poucos."
}
pub fn venom_took_mob(name: &str) -> String {
    format!(
        "O veneno corre {} {name} — sua força escorre aos poucos.",
        por_contraction(name)
    )
}

pub fn vampiric_drain_player() -> &'static str {
    "Um frio mortal se espalha por dentro — a vitalidade é drenada!"
}
pub fn vampiric_drain_mob(name: &str) -> String {
    format!(
        "Um frio mortal se espalha {} {name} — sua vitalidade é drenada!",
        por_contraction(name)
    )
}

pub fn bind_victim_player(name: &str) -> String {
    format!(
        "{} {name} crava as presas na perna presa — nenhum passo é possível, mas os braços continuam livres!",
        cap_article(name)
    )
}
pub fn bind_victim_mob(attacker_name: &str, target_name: &str) -> String {
    format!(
        "{} {attacker_name} crava as presas ao redor {} {target_name}!",
        cap_article(attacker_name),
        de_contraction(target_name)
    )
}

pub fn medusa_gaze_line() -> &'static str {
    "Os olhos se cruzam com os da medusa — e a carne vira pedra fria!"
}

// ---------------------------------------------------------------------------
// models/src/visibility.rs
// ---------------------------------------------------------------------------

pub fn spotted_line(phrase: &str, worn: &str) -> String {
    format!("À vista: {phrase}{worn}.")
}

pub fn trap_spotted(_article: &str, label: &str) -> String {
    format!("Detecção: uma {label}.")
}

// ---------------------------------------------------------------------------
// models/src/helpers.rs
// ---------------------------------------------------------------------------

pub fn ward_turns_aside(name: &str) -> String {
    format!("A proteção {} {name} desvia a magia.", de_contraction(name))
}

pub fn unharmed_by(name: &str, element_noun: &str) -> String {
    format!(
        "{} {name} não sofre dano de {element_noun}.",
        cap_article(name)
    )
}

pub fn badly_wounded() -> &'static str {
    "Os ferimentos são graves!"
}

// ---------------------------------------------------------------------------
// models/src/components.rs — Element::noun()
// ---------------------------------------------------------------------------

pub fn element_fire_noun() -> &'static str {
    "as chamas"
}
pub fn element_cold_noun() -> &'static str {
    "o frio"
}
pub fn element_drain_noun() -> &'static str {
    "a magia desmorta"
}

// ---------------------------------------------------------------------------
// models/src/saveload.rs
// ---------------------------------------------------------------------------

pub fn retired_enchantment_singular() -> &'static str {
    "Um encantamento desta run não existe nesta versão, e se perde."
}

pub fn retired_enchantment_plural(n: usize) -> String {
    format!("{n} encantamentos não existem nesta versão e se perdem.")
}

// ---------------------------------------------------------------------------
// models/src/map/levels.rs
// ---------------------------------------------------------------------------

pub fn element_seeks_the_sun() -> &'static str {
    "O Elemento de Yoord busca o sol; ele não permite descer mais."
}

pub fn cannot_go_down() -> &'static str {
    "Não há descida possível daqui."
}

pub fn dungeon_lord_prevents_up() -> &'static str {
    "O poder do Mestre impede a subida pela escada."
}

pub fn cannot_go_up() -> &'static str {
    "Não há subida possível daqui."
}

pub fn climb_last_stair() -> &'static str {
    "A última escada leva ao céu aberto, com o Elemento de Yoord ardendo nas mãos."
}

pub fn portal_down(depth: u8) -> String {
    format!(
        "O Mestre abre um portal sob os pés! Uma queda repentina para baixo. (Profundidade {depth})"
    )
}

pub fn portal_up(depth: u8) -> String {
    format!(
        "O Elemento de Yoord brilha e rasga um portal no alto! Uma subida repentina. (Profundidade {depth})"
    )
}

pub fn trapdoor_arrival(depth: u8) -> String {
    format!(
        "Uma queda dura no andar de baixo, em meio a uma nuvem de poeira. (Profundidade {depth})"
    )
}

pub fn potion_arrival(depth: u8) -> String {
    format!(
        "A pedra acima se dissolve no nada e uma subida através dela acontece. (Profundidade {depth})"
    )
}

pub fn descend_stairs(depth: u8) -> String {
    format!("Descida pela escada. (Profundidade {depth})")
}

pub fn climb_stairs(depth: u8) -> String {
    format!("Subida pela escada. (Profundidade {depth})")
}

pub fn element_wont_let_you_land() -> &'static str {
    "O Elemento de Yoord puxa em direção ao sol — mas a última escada precisa ser subida por conta própria."
}

pub fn portal_no_deeper_floor() -> &'static str {
    "O Mestre arranha o chão, mas não há nenhum andar mais profundo para onde jogar."
}

// ---------------------------------------------------------------------------
// models/src/items/pickups.rs
// ---------------------------------------------------------------------------

pub fn hidden_item_found() -> &'static str {
    "Opa! Tem algo aqui! Daí sim!"
}

pub fn take_element_of_yoord() -> &'static str {
    "Você segura Elemento de Yoord com toda a sua força. \"O elemento de Yoord busca o sol.\""
}

pub fn pick_up(taken: &str) -> String {
    format!("Você pega {taken}.")
}

pub fn pick_up_pickup(name: &str, line: &str) -> String {
    format!("Você pega {} {name}. {line}", article(name))
}

pub fn coin_gives_itself_up(name: &str, line: &str) -> String {
    format!(
        "{} {name} se entrega por conta própria. {line}",
        cap_article(name)
    )
}

pub fn coin_ledger() -> &'static str {
    "Direto para a conta. Viva o Pix!"
}

pub fn heal_line(healed: i32) -> String {
    format!("Um calor reconfortante percorre tudo. ({healed} PV)")
}

pub fn refill_magic_line(gained: u8) -> String {
    format!("Algo frio e brilhante enche a mente. ({gained} Ma)")
}

pub fn cleanse_one() -> &'static str {
    "Você se sente melhor."
}

pub fn cleanse_many(n: i32) -> String {
    format!("Você melhora de exatamente {n} coisas.")
}

pub fn learn_spell_full() -> &'static str {
    "Algo ancestral se agita na mente e não encontra onde se acomodar."
}

pub fn learn_spell_all_known() -> &'static str {
    "Algo ancestral se agita na mente e não encontra nada de novo para ensinar."
}

pub fn learn_spell_line(name: &str) -> String {
    format!(
        "Algo ancestral e violento se instala na mente. O feitiço {name} acaba de ser aprendido!"
    )
}

pub fn restore_strength_line(given: i32) -> String {
    format!("O braço se lembra do que era. ({given} For.)")
}

pub fn promise_platinum_offer() -> &'static str {
    "Não perde o brilho. Por agora, ninguém mais vai perder também. (PLAT)"
}
pub fn promise_forge_offer() -> &'static str {
    "Ainda está quente. Algo está sendo forjado. (FORJ)"
}
pub fn promise_platinum_broken() -> &'static str {
    "A platina perde o brilho. Lá se vai a perfeição."
}
pub fn promise_forge_broken() -> &'static str {
    "A forja esfria. Lá se vai sua promessa."
}

pub fn pay_platinum_power() -> &'static str {
    "Sem perder o brilho. A platina passa para o braço. (For. +1)"
}
pub fn pay_platinum_armor() -> &'static str {
    "Sem perder o brilho. A platina passa para a pele. (Arm. +1)"
}
pub fn pay_forge_collects() -> &'static str {
    "A forja cobra sua parte. Algo do inventário fica pronto como deve ser."
}
pub fn pay_forge_nothing_worth() -> &'static str {
    "...mas não há nada no inventário que valha a pena terminar."
}

// ---------------------------------------------------------------------------
// models/src/items/potions.rs
// ---------------------------------------------------------------------------

pub fn potion_healing_player() -> &'static str {
    "Os ferimentos se fecham e uma sensação de frescor toma conta de tudo!"
}
pub fn potion_healing_mob() -> &'static str {
    "brilha de forma estranha, com os ferimentos se fechando"
}
pub fn potion_extra_healing_player() -> &'static str {
    "Você nunca se sentiu tão bem."
}

pub fn potion_confusion_player() -> &'static str {
    "O mundo inteiro gira! A confusão toma conta de tudo."
}
pub fn potion_confusion_mob() -> &'static str {
    "cambaleia, com os olhos girando"
}

pub fn potion_gain_strength_player() -> &'static str {
    "Uma força nova surge! Que músculos."
}
pub fn potion_gain_strength_mob() -> &'static str {
    "fica GRANDE!"
}

pub fn potion_gain_magic_player() -> &'static str {
    "A consciência expande! O poder esteve todo ali o tempo todo."
}
pub fn potion_gain_magic_mob() -> &'static str {
    "vibra com um poder emprestado"
}

pub fn potion_poison_player() -> &'static str {
    "Uma náusea profunda se instala — a força escorre por completo."
}
pub fn potion_poison_mob() -> &'static str {
    "se contorce com ânsia, os membros amolecendo"
}

pub fn potion_restore_strength_noop_player() -> &'static str {
    "Um calor percorre o corpo inteiro."
}
pub fn potion_restore_strength_noop_mob() -> &'static str {
    "estremece"
}
pub fn potion_restore_strength_player() -> &'static str {
    "A antiga força volta ao braço de repente."
}
pub fn potion_restore_strength_mob() -> &'static str {
    "se endireita, com a força de volta"
}

pub fn potion_see_invisible_player() -> &'static str {
    "Os olhos ardem, e o ar se enche de coisas que nunca deixaram de estar ali."
}
pub fn potion_see_invisible_mob() -> &'static str {
    "os olhos brilham, seguindo o invisível"
}

pub fn detect_monsters_none() -> &'static str {
    "Um silêncio total responde — nada se move neste andar."
}
pub fn detect_monsters_some() -> &'static str {
    "Os moradores do andar se agitam nas trevas."
}

pub fn detect_magic_none() -> &'static str {
    "O zumbido da magia não responde — este andar não tem nenhuma."
}
pub fn detect_magic_some() -> &'static str {
    "A magia zumbe pelo chão, e sua localização exata fica clara."
}

pub fn distant_laughter() -> &'static str {
    "Uma risada distante chega de algum lugar."
}

pub fn raise_level_win() -> &'static str {
    "A poção arrasta para cima, através de pedra e raiz, até o céu aberto. A liberdade, enfim."
}

pub fn potion_fruit_juice() -> &'static str {
    "Frio, doce e encorpado. Delícia!"
}
pub fn potion_water() -> &'static str {
    "É água. Só água."
}
pub fn potion_flavour_mob() -> &'static str {
    "lambe os lábios"
}

// ---------------------------------------------------------------------------
// models/src/items/throwing.rs
// ---------------------------------------------------------------------------

pub fn thrown_wand_confetti(seen_name: &str) -> String {
    format!(
        "{} {seen_name} explode em uma chuva de confete colorido. É isso. Essa é a mágica. Viva.",
        cap_article(seen_name)
    )
}

pub fn thrown_wand_shatters(seen_name: &str, charges: i32) -> String {
    format!(
        "{} {seen_name} se estilhaça, e {charges} cargas de magia escapam de uma vez!",
        cap_article(seen_name)
    )
}

pub fn very_clever() -> &'static str {
    "Muito esperto."
}

pub fn you_fire(phrase: &str) -> String {
    format!("Um disparo sai voando: {phrase}.")
}
pub fn you_throw(seen_name: &str) -> String {
    format!("{} {seen_name} sai voando.", cap_article(seen_name))
}
pub fn mob_fires(thrower: &str, phrase: &str) -> String {
    format!("{} {thrower} dispara {phrase}.", cap_article(thrower))
}
pub fn mob_throws(thrower: &str, seen_name: &str) -> String {
    format!(
        "{} {thrower} joga {} {seen_name}.",
        cap_article(thrower),
        article(seen_name)
    )
}

pub fn scroll_read_aloud(who: &str, seen_name: &str) -> String {
    format!(
        "{who} desenrola {} {seen_name} e o lê em voz alta.",
        article(seen_name)
    )
}

pub fn wand_clatters_unspent(seen_name: &str) -> String {
    format!(
        "{} {seen_name} cai no chão com estrondo, com a magia ainda intacta.",
        cap_article(seen_name)
    )
}

pub fn picked_up_thrown_verb_hand() -> &'static str {
    "agarra e empunha"
}
pub fn picked_up_thrown_verb_body() -> &'static str {
    "veste"
}
pub fn picked_up_thrown_verb_other() -> &'static str {
    "coloca"
}
pub fn picks_up_thrown(victim_name: &str, verb: &str) -> String {
    format!("{} {victim_name} {verb}!", cap_article(victim_name))
}

/// "arrow"/"arrows"/"quarrel"/"quarrels" are the only ammo-noun ids the
/// engine ever passes here, so unlike other content ids (left untranslated,
/// see `content_name`), these read naturally enough as ordinary Portuguese
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
        "{} {shooter_name} solta {} {} {} — passa longe de {target_label}.",
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
        "{} {shooter_name} solta {} {} contra {target_label} e causa {damage} de dano!",
        cap_article(shooter_name),
        indef_article(noun),
        ammo_word(noun)
    )
}

pub fn potion_shatters_floor(seen_name: &str) -> String {
    format!(
        "{} {seen_name} se estilhaça no chão.",
        cap_article(seen_name)
    )
}

pub fn potion_bursts_over(seen_name: &str, victim_name: &str) -> String {
    format!(
        "{} {seen_name} explode sobre {} {victim_name}, que se engasga com um gole inteiro!",
        cap_article(seen_name),
        article(victim_name)
    )
}

pub fn throw_bounces_off(seen_name: &str, hit_name: &str) -> String {
    format!(
        "{} {seen_name} quica {} {hit_name}.",
        cap_article(seen_name),
        em_contraction(hit_name)
    )
}

pub fn throw_glances_off(seen_name: &str, hit_name: &str) -> String {
    format!(
        "{} {seen_name} raspa {} {hit_name} sem efeito.",
        cap_article(seen_name),
        em_contraction(hit_name)
    )
}

pub fn throw_hits(seen_name: &str, hit_name: &str, damage: i32) -> String {
    format!(
        "{} {seen_name} acerta {} {hit_name} por {damage} de dano.",
        cap_article(seen_name),
        article(hit_name)
    )
}

// ---------------------------------------------------------------------------
// models/src/items/spells.rs
// ---------------------------------------------------------------------------

pub fn no_magic_for_that() -> &'static str {
    "Não sobra magia suficiente para isso."
}

pub fn you_cast(name: &str) -> String {
    format!("{name} sai lançado!")
}

pub fn sting_misses() -> &'static str {
    "O dardo de veneno não encontra nada para morder."
}
pub fn sting_glances(name: &str) -> String {
    format!("O dardo raspa {} {name} sem efeito.", em_contraction(name))
}
pub fn sting_hits(name: &str, damage: i32) -> String {
    format!(
        "Um dardo verde de veneno fura {} {name} por {damage} de dano!",
        article(name)
    )
}

pub fn thunderbolt_misses() -> &'static str {
    "O trovão estala sobre pedra vazia."
}
pub fn thunderbolt_hits(name: &str, damage: i32) -> String {
    format!(
        "Um raio de trovão atinge {} {name} por {damage} de dano!",
        article(name)
    )
}

pub fn cure_self_nothing_to_cure() -> &'static str {
    "Não há nada para curar por aqui."
}

pub fn bide_coil() -> &'static str {
    "Um enroscar reúne forças para o golpe que vem."
}

pub fn breathe_fire_player() -> &'static str {
    "Uma torrente de fogo jorra para fora!"
}
pub fn breathe_fire_mob(name: &str) -> String {
    format!(
        "Uma torrente de fogo jorra {} {name}!",
        de_contraction(name)
    )
}

pub fn force_lance_cast() -> &'static str {
    "Uma lança invisível golpeia ao longo da linha!"
}
pub fn force_lance_hits(name: &str, damage: i32) -> String {
    format!(
        "A lança de força atinge {} {name} por {damage} de dano!",
        article(name)
    )
}

pub fn setup_planted() -> &'static str {
    "Armadilhas de flecha são plantadas nos flancos."
}
pub fn setup_no_room() -> &'static str {
    "Não há espaço nos flancos para uma armadilha."
}

pub fn lux_cast() -> &'static str {
    "Um fragmento de luz pura sai voando!"
}

pub fn circle_of_death_nothing() -> &'static str {
    "Uma luz cinzenta se reúne ao redor e não encontra nada para se alimentar."
}
pub fn circle_of_death_cast() -> &'static str {
    "Uma luz cinzenta se levanta do chão. Chamas desnecessárias rugem pela sala, e \
         tudo o que tocam vira cinza."
}
pub fn circle_of_death_drain(drained: i32) -> String {
    format!("O círculo drena {drained} de vida.")
}

pub fn magic_ward_cast() -> &'static str {
    "Uma pele fria e prateada se fecha ao redor. Só a própria magia consegue tocar agora — \
         pelo resto deste andar."
}

pub fn heal_self_line(healed: i32) -> String {
    format!("Um calor inunda por dentro e os ferimentos se fecham. (+{healed} PV)")
}
pub fn heal_self_full() -> &'static str {
    "A força já está no máximo."
}

pub fn meteor_strike_cast() -> &'static str {
    "O fogo sai voando em direção ao céu. Não cai de volta onde seria de esperar."
}
pub fn meteor_screams_down() -> &'static str {
    "Um meteoro cai com um rugido!"
}
pub fn sky_tears_open_again() -> &'static str {
    "O céu se rasga de novo!"
}

pub fn frost_nova_cast() -> &'static str {
    "Uma ESTRELA CIANO explode ao redor — gelo, brilho, e de sobra dos dois!"
}

pub fn haste_self_gathers() -> &'static str {
    "O poder se reúne. O poder se reúne ainda mais."
}
pub fn haste_self_the_fast() -> &'static str {
    "Nada de rápido. Nada de veloz. Isto é A VELOCIDADE EM PESSOA."
}

// ---------------------------------------------------------------------------
// models/src/items/scrolls.rs
// ---------------------------------------------------------------------------

pub fn already_recognise_everything() -> &'static str {
    "Tudo no inventário já é reconhecido."
}
pub fn identify_everything() -> &'static str {
    "O pergaminho identifica tudo no inventário!"
}

pub fn remove_curse_freed() -> &'static str {
    "Uma sensação de vigilância protetora se instala. O equipamento amaldiçoado se desfaz."
}
pub fn remove_curse_nothing() -> &'static str {
    "Uma sensação de vigilância protetora se instala."
}

pub fn scare_monster_some() -> &'static str {
    "O pergaminho arde com o patetismo puro do medo!"
}
pub fn scare_monster_none() -> &'static str {
    "O pergaminho irradia uma aura ameaçadora, mas não há nada aqui para sentir isso."
}

pub fn blank_paper() -> &'static str {
    "O pergaminho está em branco. Alguém ficou com a última risada."
}

pub fn amnesia_poof() -> &'static str {
    "1... 2... Puf!"
}
pub fn amnesia_nothing_to_forget() -> &'static str {
    "Não havia nada ali para esquecer."
}
pub fn amnesia_forgotten(name: &str) -> String {
    format!("O saber usar {name} desaparece da memória!")
}
pub fn amnesia_dungeon_slips_away() -> &'static str {
    "A masmorra ao redor escorrega como um sonho meio esquecido."
}

pub fn teleport_scroll_blonk() -> &'static str {
    "BLONK! Um salto leva para bem longe!"
}

pub fn aggravate_scroll_shriek() -> &'static str {
    "Um grito agudo atravessa a masmorra inteira. Tudo neste andar ouviu — e já sabe onde encontrar."
}

pub fn create_monster_nowhere() -> &'static str {
    "O ar coalha — depois se acalma. O que quer que estivesse vindo, pensou melhor."
}
pub fn create_monster_line(_article: &str, name: &str) -> String {
    format!(
        "O ar coalha em {} {name}, dentes e tudo!",
        indef_article(name)
    )
}

pub fn vorpalize_fizzles() -> &'static str {
    "O pergaminho se apaga sem conseguir marcar nenhuma arma."
}
pub fn vorpalize_crumbles(wname: &str) -> String {
    format!(
        "{} {wname} grita de dor e se desfaz em pó.",
        cap_article(wname)
    )
}
pub fn vorpalize_branded(wname: &str, bane: &str) -> String {
    format!(
        "{} {wname} canta com um brilho afiado como navalha, um presságio de morte para qualquer {bane}.",
        cap_article(wname)
    )
}

pub fn enchant_sparks(name: &str) -> String {
    format!(
        "{} {name} solta uma chuva de faíscas laranjas!",
        cap_article(name)
    )
}
pub fn enchant_curse_burns(name: &str) -> String {
    format!(
        "A maldição {} {name} queima junto com elas.",
        de_contraction(name)
    )
}
pub fn enchant_missing_armor() -> &'static str {
    "As faíscas brilham sobre pele nua e se apagam. Nenhuma armadura está vestida."
}
pub fn enchant_missing_weapon() -> &'static str {
    "As faíscas brilham sobre uma mão vazia e se apagam."
}

pub fn confusing_touch_fresh_player() -> &'static str {
    "Um brilho violeta se acende sobre as mãos. A próxima coisa que tocarem vai se arrepender."
}
pub fn confusing_touch_fresh_mob() -> &'static str {
    "flexiona as garras, e um brilho violeta as percorre"
}
pub fn confusing_touch_deeper_player() -> &'static str {
    "O brilho violeta sobre as mãos se intensifica. Ainda é um único toque."
}
pub fn confusing_touch_deeper_mob() -> &'static str {
    "sacode as garras brilhantes"
}
pub fn confusing_touch_discharge_player() -> &'static str {
    "O brilho violeta explode de repente — a sala inteira se inclina!"
}
pub fn confusing_touch_discharge_mob() -> &'static str {
    "cambaleia quando o brilho violeta explode sobre ele"
}

pub fn hold_monster_none() -> &'static str {
    "As palavras caem como ferro — sobre absolutamente nada."
}
pub fn hold_monster_some() -> &'static str {
    "As palavras caem como ferro. Toda criatura à vista fica presa onde está."
}

pub fn sleep_scroll_none() -> &'static str {
    "Uma onda de sonolência se espalha sobre uma sala vazia."
}
pub fn sleep_scroll_some() -> &'static str {
    "Uma onda de sonolência se espalha, e tudo à vista cai rendido com ela."
}
pub fn sleep_backfire_player() -> &'static str {
    "As palavras se arrastam e engrossam na própria boca. O chão se aproxima..."
}
pub fn sleep_backfire_mob() -> &'static str {
    "desaba no chão, ainda lendo"
}

pub fn food_detection_not_player() -> &'static str {
    "As palavras não significam nada para isso."
}
pub fn food_detection_none() -> &'static str {
    "A busca por algo simples e útil não encontra nada — este andar está vazio disso."
}
pub fn food_detection_some() -> &'static str {
    "O andar entrega suas sobras: a localização de cada coisa simples fica clara."
}

// ---------------------------------------------------------------------------
// engine/src/update.rs
// ---------------------------------------------------------------------------

pub fn stumble_foolishly() -> &'static str {
    "Um tropeço bobo, só isso."
}

pub fn pack_full() -> &'static str {
    "O inventário está cheio."
}

pub fn strain_against_rooted() -> &'static str {
    "Um esforço contra o que prende, sem nenhum resultado."
}

pub fn too_confused_right_now() -> &'static str {
    "Confusão demais agora para isso."
}

pub fn too_injured_now() -> &'static str {
    "Ferimentos demais agora para isso."
}

pub fn nothing_to_fight() -> &'static str {
    "Não há nada para lutar."
}

pub fn cant_reach_it() -> &'static str {
    "Isso está longe demais daqui."
}

pub fn out_of_ammo() -> &'static str {
    "Sem munição."
}

pub fn out_of_range() -> &'static str {
    "Fora de alcance."
}

pub fn no_clear_shot() -> &'static str {
    "Sem linha de tiro livre."
}

pub fn well_played() -> &'static str {
    "Bem jogado."
}

pub fn great_idea_but_no() -> &'static str {
    "Boa ideia! Mas não."
}

pub fn you_see_nothing_there() -> &'static str {
    "Nada visível ali."
}

pub fn you_see(phrase: &str, worn: &str) -> String {
    format!("À vista: {phrase}{worn}.")
}

pub fn beware_their(phrase: &str) -> String {
    format!("Cuidado com {phrase}.")
}

pub fn you_drop(name: &str) -> String {
    format!("{} {name} cai no chão.", cap_article(name))
}

pub fn not_while_monster_in_sight() -> &'static str {
    "Não enquanto houver uma criatura à vista."
}

pub fn cant_run_that_way() -> &'static str {
    "Nenhuma corrida possível nessa direção."
}

pub fn no_spell_there() -> &'static str {
    "Nenhum feitiço nessa posição."
}

pub fn no_spells() -> &'static str {
    "Nenhum feitiço disponível."
}

pub fn toggle_on() -> &'static str {
    "ATIVADO"
}
pub fn toggle_off() -> &'static str {
    "DESATIVADO"
}
pub fn auto_pickup_state(state: &str) -> String {
    format!("Coleta automática na auto-exploração: {state}.")
}

pub fn not_wielding_launcher() -> &'static str {
    "Nenhum lançador empunhado."
}

pub fn no_ammo_to_fire(noun: &str) -> String {
    format!("Não há {} para disparar.", ammo_word(noun))
}

pub fn not_wielding_reach_weapon() -> &'static str {
    "Nenhuma arma de alcance empunhada."
}

pub fn nothing_left_to_explore() -> &'static str {
    "Nada mais resta para explorar."
}

pub fn dir_up() -> &'static str {
    "cima"
}
pub fn dir_down() -> &'static str {
    "baixo"
}

pub fn cannot_go_direction(dir: &str) -> String {
    format!("Nenhuma passagem para {dir} a partir daqui.")
}

pub fn cant_find_path_to_stairs(dir: &str) -> String {
    format!("Nenhum caminho leva à escada de {dir}.")
}

pub fn travel_interrupted() -> &'static str {
    "Viagem interrompida."
}
pub fn auto_explore_interrupted() -> &'static str {
    "Auto-exploração interrompida."
}

pub fn arrive_at_staircase() -> &'static str {
    "A escada fica à vista, alcançada."
}
pub fn you_stop() -> &'static str {
    "Uma parada no caminho."
}

pub fn monster_nearby() -> &'static str {
    "Há um monstro por perto."
}

pub fn cant_find_path_there() -> &'static str {
    "Nenhum caminho leva até ali."
}
pub fn explored_everywhere() -> &'static str {
    "Tudo que era explorável já foi explorado."
}

pub fn already_there() -> &'static str {
    "Esse ponto já está alcançado."
}

// ---------------------------------------------------------------------------
// models/src/items.rs
// ---------------------------------------------------------------------------

pub fn cant_use_right_now(name: &str) -> String {
    format!("{name} não pode ser usado agora.")
}

pub fn you_zap(seen_name: &str) -> String {
    format!("{} {seen_name} entra em ação.", cap_article(seen_name))
}

pub fn wand_crumbles(seen_name: &str) -> String {
    format!("{} {seen_name} se desfaz em pó!", cap_article(seen_name))
}
pub fn you_drink(seen_name: &str) -> String {
    format!(
        "{} {seen_name} desaparece de um gole.",
        cap_article(seen_name)
    )
}
pub fn you_read(seen_name: &str) -> String {
    format!("{} {seen_name} fica lido.", cap_article(seen_name))
}
pub fn ring_shivers_apart(seen_name: &str) -> String {
    format!(
        "{} {seen_name} se desfaz em mil partículas reluzentes.",
        cap_article(seen_name)
    )
}
pub fn item_turns_to_dust() -> &'static str {
    "O item vira pó!"
}

// ---------------------------------------------------------------------------
// models/src/combat.rs
// ---------------------------------------------------------------------------

pub fn killer_unknown() -> &'static str {
    "Assassino desconhecido"
}

pub fn mob_dies(name: &str) -> String {
    format!("{} {name} morre.", cap_article(name))
}

pub fn gear_clatters_to_floor(name: &str) -> String {
    format!("{} {name} cai no chão com estrondo.", cap_article(name))
}

pub fn lunge_hit(target_name: &str, damage: i32) -> String {
    format!(
        "Uma estocada, lâmina reluzindo além de qualquer guarda, espeta {} {target_name} por {damage} de dano!",
        article(target_name)
    )
}

pub fn you_have_slain(target_name: &str) -> String {
    format!(
        "{} {target_name} acaba de tombar!",
        cap_article(target_name)
    )
}

pub fn strike_at_nothing() -> &'static str {
    "Um golpe no ar, nada além disso."
}

pub fn slain_by(attacker_name: &str) -> String {
    format!("Morto {} {attacker_name}", por_contraction(attacker_name))
}

pub fn excellent_hit(target_name: &str, damage: i32) -> String {
    format!(
        "Um golpe excelente marca {} {target_name} por {damage} de dano!",
        article(target_name)
    )
}
pub fn glancing_blow(target_name: &str) -> String {
    format!(
        "Um golpe de raspão alcança {} {target_name}.",
        article(target_name)
    )
}
pub fn plain_hit(target_name: &str, damage: i32) -> String {
    format!(
        "Um golpe alcança {} {target_name} por {damage} de dano.",
        article(target_name)
    )
}
pub fn garrote_kill(target_name: &str) -> String {
    format!(
        "A vida se apaga à força {} {} {target_name}! Atroz.",
        em_contraction(target_name),
        helpless(target_name)
    )
}
pub fn vorpal_kill(target_name: &str) -> String {
    format!(
        "Zás-zás! A lâmina atravessa limpo {} {target_name}!",
        article(target_name)
    )
}

pub fn mob_misses(atk: &str, target_label: &str) -> String {
    format!("{atk} erra contra {target_label}.")
}
pub fn mob_hits(atk: &str, target_label: &str, damage: i32) -> String {
    format!("{atk} acerta {target_label} por {damage} de dano.")
}
pub fn mob_strikes_you_down(atk: &str) -> String {
    format!("{atk} derruba com um golpe final...")
}
pub fn mob_kills(atk: &str, target_name: &str) -> String {
    format!("{atk} acaba com {} {target_name}!", article(target_name))
}

pub fn blown_up_by(_article: &str, label: &str) -> String {
    format!("Explodido em pedaços por uma {label}")
}

// ---------------------------------------------------------------------------
// engine/src/view.rs — HUD badges, prompts, menu titles
// ---------------------------------------------------------------------------

pub fn badge_stone() -> &'static str {
    "PEDRA"
}
pub fn badge_asleep() -> &'static str {
    "DORMINDO"
}
pub fn badge_held() -> &'static str {
    "PRESO"
}
pub fn badge_ascending() -> &'static str {
    "ASCENDENDO"
}
pub fn badge_traveling() -> &'static str {
    "VIAJANDO"
}
pub fn badge_exploring() -> &'static str {
    "EXPLORANDO"
}
pub fn badge_travel_query() -> &'static str {
    "VIAJAR?"
}

pub fn depth_label() -> &'static str {
    "PROF."
}

pub fn travel_cursor_prompt() -> &'static str {
    "Mover para onde?"
}
pub fn travel_cursor_hint() -> &'static str {
    "[hjkl/setas mover · Enter viajar · Esc/x cancelar]"
}

pub fn quit_question() -> &'static str {
    "Sair mesmo?"
}
pub fn quit_answers() -> &'static str {
    "[s] sim    [n] não"
}

pub fn spells_menu_title() -> &'static str {
    " FEITIÇOS "
}

// ---------------------------------------------------------------------------
// models/src/pack.rs — the Use/Throw/Drop action modal
// ---------------------------------------------------------------------------

pub fn action_use() -> &'static str {
    " Usar   "
}
pub fn action_throw() -> &'static str {
    " Jogar  "
}
pub fn action_drop() -> &'static str {
    " Largar "
}

// ---------------------------------------------------------------------------
// models/src/items/throwing.rs
// ---------------------------------------------------------------------------

pub fn element_wont_leave_hand() -> &'static str {
    "O Elemento de Yoord não vai sair desta mão."
}

// ---------------------------------------------------------------------------
// models/src/monsters.rs — the xeroc's disguise, a cosmetic-only category word
// ---------------------------------------------------------------------------

pub const fn mimic_look_scroll() -> &'static str {
    "pergaminho"
}
pub const fn mimic_look_potion() -> &'static str {
    "poção"
}
pub const fn mimic_look_wand() -> &'static str {
    "varinha"
}
pub const fn mimic_look_gold_coin() -> &'static str {
    "moeda de ouro"
}
pub const fn mimic_look_ring() -> &'static str {
    "anel"
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
    format!(" (vest. {names})")
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
    "você"
}
pub fn pronoun_something() -> &'static str {
    "Algo"
}

pub fn adjective_sees_unseen() -> &'static str {
    "capaz de ver o invisível"
}

pub fn equipped_suffix() -> &'static str {
    " (E)"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_agrees_with_grammatical_gender() {
        assert_eq!(cap_article("medusa"), "A");
        assert_eq!(article("medusa"), "a");
        assert_eq!(cap_article("orc"), "O");
        assert_eq!(article("orc"), "o");
    }

    #[test]
    fn contractions_and_agreement_follow_the_same_gender() {
        assert_eq!(indef_article("medusa"), "uma");
        assert_eq!(cap_indef_article("medusa"), "Uma");
        assert_eq!(de_contraction("medusa"), "da");
        assert_eq!(em_contraction("medusa"), "na");
        assert_eq!(por_contraction("medusa"), "pela");
        assert_eq!(helpless("medusa"), "indefesa");
        assert_eq!(de_contraction("orc"), "do");
        assert!(garrote_kill("medusa").contains("na indefesa medusa"));
    }
}

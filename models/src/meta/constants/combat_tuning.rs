//! Tuning for combat: `combat` and `helpers`.

// ===========================================================================
// Combat
// ===========================================================================

/// Numbers that shape a single exchange of blows. Background:
/// `docs/explanation/combat-and-balance.md`.
pub mod combat {
    /// The player's chance, per swing, of landing an "excellent hit" that rolls
    /// [`EXCELLENT_HIT_DICE`] weapon dice instead of one. Monsters never get it.
    ///
    /// This is the player's escape hatch from a stalemate against good armour.
    /// Drop it toward 0 and a poorly-geared player facing plate has no path;
    /// raise it toward 1 and armour stops mattering.
    pub const EXCELLENT_HIT_CHANCE: f64 = 0.15;

    /// How many weapon dice an excellent hit rolls: `1d[power]` becomes
    /// `Nd[power]`, summed, *before* armour is subtracted. The more dice, the
    /// higher a lucky swing's ceiling.
    pub const EXCELLENT_HIT_DICE: i32 = 3;

    /// The floor under a player's swing: even when the armour roll eats the
    /// whole blow, the player still scrapes off this much (logged as a
    /// "glancing blow"). It keeps a fight from never moving the number.
    ///
    /// A glancing blow can never be the killing one — it leaves a foe on 1 HP —
    /// so raising this does not let chip damage finish things, only speeds the
    /// grind. Monsters have no such floor: a monster that cannot beat your
    /// armour simply cannot hurt you, and that asymmetry is why armour is worth
    /// wearing. Leave it at its minimum unless you are re-teaching that lesson.
    pub const CHIP_DAMAGE: i32 = 1;

    /// The odds that any one piece of a dead creature's gear survives to be
    /// picked up; the rest is lost in the mess. Rolled once per equipped item.
    /// Lower this and thrown-weapon retrieval gets stingier; raise it toward 1
    /// and every orc becomes a vending machine for its own sword.
    pub const GEAR_SURVIVES_DEATH: f64 = 0.5;

    /// The flat bonus the spell Bide adds to the attack roll of the very next
    /// blow its caster lands — see [`crate::effects::Bided`].
    pub const BIDE_ATTACK_BONUS: i32 = 4;

    /// How much harder a monster's melee blow lands on a creature that is
    /// [`crate::effects::Vuln`], in percent of the damage the dice settled on.
    pub const VULN_DAMAGE_PERCENT: i32 = 25;

    /// How many dice a normal attack or armour roll averages together —
    /// see `crate::combat::roll_die_bell`. At 1 it is a flat `1d[sides]`
    /// like an excellent hit; raising it narrows the spread further without
    /// moving the mean or the min/max off the plain die's.
    pub const BELL_CURVE_DICE: i32 = 2;

    /// How far the splinters of a cursed suit fly when an excellent hit
    /// shatters it, in tiles (Chebyshev). See [`crate::combat::resolve_attack`].
    pub const CURSED_SPLINTER_RADIUS: i32 = 1;

    /// The die each splinter rolls against every foe of the hero it reaches:
    /// `1d[this]`, as magic, so a ward turns it aside.
    pub const CURSED_SPLINTER_DIE: i32 = 6;
}

// ===========================================================================
// Helpers
// ===========================================================================

/// The boon companion. See [`crate::companion`].
pub mod helpers {
    /// The odds a creature accepts the right treat and becomes your Helper.
    /// The treat is eaten either way.
    pub const ACCEPT_CHANCE: f64 = 0.5;

    /// The odds a kill turns a [`crate::effects::ShapeshiftOnKill`] creature
    /// into another monster, or a [`crate::effects::MirrorOnKill`] one into
    /// what it killed. Either way it happens once.
    pub const SHAPESHIFT_CHANCE: f64 = 0.25;

    /// Scroll of create monster: the odds the creature conjured arrives
    /// already charmed, as a plain ally. Rolled against the same draw as
    /// [`CREATE_HELPER_CHANCE`] — the two never both land on one summon.
    pub const CREATE_ALLY_CHANCE: f64 = 0.13;

    /// Scroll of create monster: the odds the creature conjured arrives as
    /// your Helper outright, rather than a plain ally or a plain monster.
    pub const CREATE_HELPER_CHANCE: f64 = 0.07;
}

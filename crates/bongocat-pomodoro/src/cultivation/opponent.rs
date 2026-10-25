//! Who stands on the other side: a body, the gear on it, and the 功法 it swings.
//!
//! Every fight on this page used to be against a stat block — a guardian, a
//! wanderer, a warden, all of them naked. That made the loot a second, unrelated
//! roll: you beat a bare-handed guard and it dropped a robe nobody had been
//! wearing. This module closes that: an opponent is wound up out of the same
//! pieces a drop is, wears them, swings the 功法 it carries, and whatever it was
//! carrying is what a win takes.
//!
//! Two rules hold the design together:
//!
//! * **The body is the rung, the gear is on top of it.** Every opponent starts
//!   from the attributes a naked cultivator at that rung has — so the ladder's
//!   baseline is unchanged — and then puts its gear on. A guardian in a fine robe
//!   is a bigger guardian than the same one in rags, and that difference is the
//!   point: a fight worth preparing for is a fight that can go badly.
//! * **Both the gear and the art are capped.** Gear may add up to
//!   [`GEAR_SHARE_PERMILLE`] of the body and no more, and an art pays for
//!   [`ART_SWINGS`] swings and no more: the variance is meant to be felt within a
//!   rung, not to make one realm's guardian unbeatable and the next one free.

use super::MAX_LEVEL;
use super::arts::blueprint;
use super::duel::{GUARDIAN_NAME, WANDERER_NAME};
use super::trial::{TRIAL_OPPONENT_NAME, floor_level, floor_realm};
use super::{
    ArtGrade, ArtKind, ArtStrike, Attribute, Attributes, Derived, EquipmentRecord, Fighter, Realm,
    Rng, SLOT_COUNT, base_attributes, items::roll_equipment,
};

/// How much gear an opponent may carry on top of its body, in parts per
/// thousand of the body's own attribute points.
///
/// Eighteen hundredths: the gear is a real bonus — that is the whole point of
/// handing it to them — but it is capped, because an 极品 piece at a low realm is
/// worth several bodies and would turn the first floor of the tower into a wall
/// nobody climbs twice.
///
/// The share is read against the body's *power*, so it had to move when the
/// pairing bonus came out of the power formula: the allowance shrank with every
/// body on the ladder while the pieces did not shrink as far, and a warden whose
/// gear no longer fits is a warden softer than the floor below it.
const GEAR_SHARE_PERMILLE: u32 = 180;

/// How far the four attributes may drift apart because of the gear, in parts per
/// thousand of the body's own value of that attribute.
///
/// A quarter: the gear is allowed to make an opponent a spellcaster or a runner,
/// which is the difference the player reads in the fight, but not to the point
/// where 身法 alone turns the hit curve against the player. Everything that makes
/// a fight lopsided lives in that curve, so the drift is what gets capped.
const ATTRIBUTE_DRIFT_PERMILLE: i32 = 750;

/// How much of an opponent's damage is moved into its 功法, in parts per
/// thousand of the expected uplift.
///
/// Four tenths: a named move should be felt — three blows land half again as hard
/// — but not to the point where the art alone decides the fight, because the
/// fight is five or six blows a side and most of a blow is a realm the player no
/// longer opens. The remaining six tenths is the challenge the art is there for.
const ART_SHARE_PERMILLE: u32 = 400;

/// What an opponent's 功法 swing is worth, by grade.
///
/// The old, flatter row: 1.2× for 黄, 1.8× for 神. It is deliberately separate
/// from [`super::arts`]'s blueprint multiplier so that retuning the player's
/// ladder never moves the height of a guardian in the same breath — the player's
/// arts got steeper, and the wall they climb has to stay where it was.
const OPPONENT_ART_MULTIPLIERS: [u32; 5] = [1_200, 1_350, 1_500, 1_650, 1_800];

/// How many swings of its 功法 an opponent can pay for.
///
/// Three: an art is a moment in the fight — a blow the log can point at, and the
/// reason a fight can go badly — not a multiplier running on every round. An
/// opponent whose 内力 pool was read off its own 根骨 paid for ten or fifteen
/// swings and hit half again as hard for the whole fight, which stopped being a
/// moment and became a stat.
const ART_SWINGS: u32 = 3;

/// Which kind of fight the opponent stands in.
///
/// The three differ in what they are worth, not in what they are made of: the
/// same body-plus-gear-plus-功法 recipe, measured against a different rung of
/// the ladder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Stance {
    /// The thing standing at the end of a realm.
    Guardian,
    /// Someone met on the road.
    Wanderer,
    /// The warden of a 试炼 floor.
    Warden,
}

impl Stance {
    /// How many pieces the opponent wears.
    ///
    /// A guardian carries one — what it drops is exactly what it wore, and a
    /// breakthrough has always paid one piece. A wanderer and a warden carry two,
    /// so the one a win takes is a choice made out of what they had rather than
    /// the only thing they had.
    const fn gear_slots(self) -> usize {
        match self {
            Self::Guardian => 2,
            Self::Wanderer => 2,
            Self::Warden => 3,
        }
    }

    /// The chance it carries an 外功 at all, in parts per thousand.
    ///
    /// A guardian always does: the realm fight is the one place the page can
    /// promise a 功法 and must not break the promise. The other two carry one
    /// often enough that a fight with an art in the log is ordinary, and rarely
    /// enough that it is worth reading.
    const fn art_permille(self) -> u64 {
        match self {
            Self::Guardian => 1_000,
            Self::Wanderer => 500,
            Self::Warden => 600,
        }
    }

    /// The health and attack the body is multiplied by once it is built.
    ///
    /// These are the multipliers the three fights were already tuned with — a
    /// guardian has one and a half bodies and a heavy hand, a wanderer is
    /// smaller than the cultivator on its own rung, a warden is the rung itself.
    const fn multipliers(self) -> (u32, u32) {
        match self {
            Self::Guardian => (1_300, 1_150),
            Self::Wanderer => (800, 800),
            Self::Warden => (1_000, 1_000),
        }
    }

    /// The share of the rung's own attributes the body is worth, in parts per
    /// thousand: the tower softens its wardens, the road does not.
    const fn budget_permille(self) -> i32 {
        match self {
            Self::Guardian | Self::Wanderer => 1_000,
            Self::Warden => super::trial::TRIAL_OPPONENT_PERMILLE as i32,
        }
    }
}

/// One opponent, as the fight and the loot both read it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Opponent {
    /// The fighter the duel settles against, art and all.
    pub fighter: Fighter,
    /// What the body and the gear add up to. The body is the rung's own; the
    /// gear is on top of it, which is why a well-dressed opponent at the same
    /// rung is a harder fight than a ragged one.
    pub attributes: Attributes,
    /// What it is wearing. A won fight takes one of these — the piece is not
    /// rolled again, it is the one the opponent had on.
    pub gear: Vec<EquipmentRecord>,
    /// The 功法 it swings, by catalog id, when it carries one. A won fight takes
    /// that book.
    pub art: Option<u64>,
}

/// The guardian standing between a realm and the next one.
///
/// Deterministic in the realm: its power is printed before the fight, so the same
/// realm has to send the same opponent every time.
pub(crate) fn guardian(current: Realm, target: Realm) -> Opponent {
    opponent(
        Stance::Guardian,
        current.cap_level(),
        target.index() as u8 + 1,
        GUARDIAN_NAME.to_owned(),
        fixed_seed(Stance::Guardian, current.index() as u32),
    )
}

/// The warden of a 试炼 floor.
///
/// Deterministic in the floor's *realm* rather than in the floor itself: a tower
/// whose wardens re-rolled their gear every floor would climb up and down, and a
/// floor softer than the one below it is not a ladder. Within a realm the body
/// grows floor by floor and the gear stays the same, so the climb only ever goes
/// one way.
pub(crate) fn warden(floor: u16) -> Opponent {
    let realm = floor_realm(floor);
    opponent(
        Stance::Warden,
        floor_level(floor),
        realm.index() as u8 + 1,
        TRIAL_OPPONENT_NAME.to_owned(),
        fixed_seed(Stance::Warden, realm.index() as u32),
    )
}

/// Someone met on the road: rolled per meeting, because nobody promised who the
/// road would send.
pub(crate) fn wanderer(realm: Realm, seed: u64) -> Opponent {
    opponent(
        Stance::Wanderer,
        realm.cap_level(),
        realm.index() as u8 + 1,
        WANDERER_NAME.to_owned(),
        seed,
    )
}

/// The someone a 试炼 sends: rolled per meeting, and levelled to the
/// cultivator rather than to the realm's ceiling.
///
/// A 试炼 is a fight, not a wall: the tower's warden stands on a rung of the
/// ladder, while this one stands where the cultivator stands, so a mid-realm
/// save is not sent the realm's last layer for asking.
pub(crate) fn challenger(level: u16, tier: u8, seed: u64) -> Opponent {
    opponent(
        Stance::Wanderer,
        level.clamp(1, MAX_LEVEL),
        tier,
        WANDERER_NAME.to_owned(),
        seed,
    )
}

/// Build one opponent for a rung of the ladder.
///
/// `level` is the rung the body stands on, `tier` the realm its goods belong to
/// (one-based).
pub(crate) fn opponent(stance: Stance, level: u16, tier: u8, name: String, seed: u64) -> Opponent {
    let mut rng = Rng::seeded(seed);
    let body = scaled(base_attributes(level), stance.budget_permille());
    // The gear is a bonus on top of the body, not a slice out of it: a guardian
    // wearing a fine robe is a bigger guardian, which is what makes the loot on
    // its back worth taking and the fight worth preparing for.
    let gear = trim(stance, &body, wear(stance, &body, tier, &mut rng));
    let total = body.plus(added_attributes(&gear));
    let art = carry_art(stance, tier, &mut rng);
    let art_strike = art.map(|art_id| {
        let art = blueprint(art_id);
        ArtStrike {
            name: art.name,
            // An opponent's 功法 is a move, not a build — which is why this is
            // *not* the blueprint's multiplier. The player's art ladder was
            // retuned to be far steeper (a 神阶 art now lands near five blows
            // for one), and an opponent reading the same table would have every
            // guardian quietly doubled, raising the wall in the same commit that
            // raised the reward. Pinned to its own row, the wall stays put.
            multiplier_permille: OPPONENT_ART_MULTIPLIERS[art.grade.index()],
            cost: art.cost,
            // The opponent is not the one who trained it: a 功法 in a fighter's
            // hands is the multiplier it was printed with, and the mastery share
            // belongs to whoever put the pomodoros in.
            mastery_permille: 0,
        }
    });
    let (health_permille, attack_permille) = stance.multipliers();
    let naked = total.derived();
    // The gear's outright combat lines ride on top of the stance's own
    // multipliers: a 神装 guardian strikes harder than its robe's attributes
    // say, the same way a cultivator's does.
    let flat = added_bonus(&gear);
    let derived = Derived {
        health: naked.health.saturating_mul(health_permille) / 1_000,
        attack: paid_for(
            naked.attack.saturating_mul(attack_permille) / 1_000,
            art_strike.as_ref(),
        ),
        ..naked
    }
    .plus(flat);
    let mut fighter = Fighter::from_derived(name, derived, art_strike.clone());
    // A fighter carrying an art carries only a few swings of it.
    if let Some(strike) = &art_strike {
        fighter.inner = strike.cost.saturating_mul(ART_SWINGS);
    }
    Opponent {
        fighter,
        attributes: total,
        gear,
        art,
    }
}

/// What is left of a blow once the art's share of its uplift has been set aside.
///
/// The art is only partly free damage: three blows in six land half again as
/// hard, and [`ART_SHARE_PERMILLE`] of that uplift is paid for by a lighter hand,
/// so a named move is a swing the player feels without being the whole fight.
fn paid_for(attack: u32, art: Option<&ArtStrike>) -> u32 {
    let Some(strike) = art else {
        return attack;
    };
    let uplift =
        ART_SHARE_PERMILLE.saturating_mul(strike.multiplier_permille.saturating_sub(1_000)) / 1_000;
    attack.saturating_mul(1_000) / (1_000 + uplift).max(1)
}

/// The attributes one rung's body is worth, before gear and before the fight's
/// own multipliers.
fn scaled(attributes: Attributes, permille: i32) -> Attributes {
    let scale = |value: i32| value.saturating_mul(permille) / 1_000;
    Attributes::new(
        scale(attributes.root),
        scale(attributes.strength),
        scale(attributes.mana),
        scale(attributes.agility),
    )
}

/// The gear the opponent wears, rolled for the realm its goods belong to — a
/// guardian guarding 金丹 wears 金丹 cloth.
///
/// Kept only while the pieces stay inside [`GEAR_SHARE_PERMILLE`] of the body it
/// is standing on: a lucky 极品 roll at level one is worth several bodies, and an
/// opponent wearing it would be a wall rather than a challenge. A piece that does
/// not fit is simply not worn. The grade is rolled, not chosen, which is where
/// the run-to-run difference between two fights of the same rung comes from.
///
/// The check is the piece's *marginal* worth on the body it would join, not its
/// standalone worth: power is a weighted sum of the four attributes, so a piece
/// that feeds an attribute a body already leans on pays differently from one
/// that does not — and an allowance read against the standalone number would
/// let a piece through that the fight then reads as more.
fn wear(stance: Stance, body: &Attributes, tier: u8, rng: &mut Rng) -> Vec<EquipmentRecord> {
    // Measured in power rather than in raw points: the fight is decided on the
    // weighted sum, and two pieces worth the same points are not worth the same
    // fight when one of them lands on 身法.
    let allowance = body.power().saturating_mul(GEAR_SHARE_PERMILLE) / 1_000;
    // The wardrobe is rolled at the *realm's* strength rather than at the body's
    // own: two floors of one realm have to dress out of one pool, or a slightly
    // stronger body would roll a slightly different threshold and the deeper
    // floor could come out softer. The allowance below is what lets a stronger
    // body carry more of that one pool.
    let factor = super::drop::power_factor_permille(super::drop::realm_power(Realm::from_index(
        usize::from(tier).saturating_sub(1),
    )));
    // The wardrobe is drawn whole, before any of it is judged: the number of
    // draws must not depend on what the body can carry, or two floors of one
    // realm would dress out of two different pools and the climb could go
    // *down* — which is the one thing the tower's wardens are built not to do.
    let mut wardrobe: Vec<EquipmentRecord> = Vec::with_capacity(SLOT_COUNT * 2);
    for _ in 0..SLOT_COUNT * 2 {
        let rarity = super::drop::roll_quality(rng, factor).unwrap_or(super::Rarity::Common);
        wardrobe.push(roll_equipment(0, tier, rarity, rng));
    }
    let mut worn: Vec<EquipmentRecord> = Vec::with_capacity(stance.gear_slots());
    for item in wardrobe {
        if worn.len() >= stance.gear_slots() {
            break;
        }
        // One piece per slot: two cloaks would be a fighter wearing the same
        // slot twice, which the inventory itself does not allow.
        if worn.iter().any(|already| already.slot == item.slot) {
            continue;
        }
        let mut candidate = worn.clone();
        candidate.push(item);
        if marginal_power(body, &candidate) > allowance || drifted(body, &candidate) {
            continue;
        }
        worn = candidate;
    }
    worn
}

/// What the gear as a whole adds to the body it stands on: the attributes' own
/// power, the pair synergy they earn against the body, and the outright combat
/// numbers on top.
fn marginal_power(body: &Attributes, gear: &[EquipmentRecord]) -> u32 {
    let total = body.plus(added_attributes(gear));
    total
        .power()
        .saturating_add(added_bonus(gear).power())
        .saturating_sub(body.power())
}

/// Whether the gear as a whole has pushed one attribute too far past the body's
/// own value of it.
fn drifted(body: &Attributes, gear: &[EquipmentRecord]) -> bool {
    let worn = added_attributes(gear);
    Attribute::ALL.into_iter().any(|attribute| {
        let base = attribute.of(*body).max(1);
        attribute.of(worn).max(0) * 1_000 / base > ATTRIBUTE_DRIFT_PERMILLE
    })
}

/// The attributes every piece together add.
fn added_attributes(gear: &[EquipmentRecord]) -> Attributes {
    gear.iter().fold(Attributes::default(), |sum, item| {
        sum.plus(Attributes::new(
            item.attributes.root.max(0),
            item.attributes.strength.max(0),
            item.attributes.mana.max(0),
            item.attributes.agility.max(0),
        ))
    })
}

/// The combat numbers every piece together adds outright.
///
/// A guardian in a 神装 robe soaks and strikes harder than its attributes say,
/// exactly as a cultivator's does: the flat lines are worth what the attribute
/// behind them would have paid, on both sides of the fight.
fn added_bonus(gear: &[EquipmentRecord]) -> Derived {
    gear.iter()
        .fold(Derived::default(), |sum, item| sum.plus(item.bonus))
}

/// The backstop behind [`wear`]'s own check: drop pieces until the gear as a
/// whole fits the share it is allowed.
///
/// [`wear`] reads each piece's marginal worth before wearing it, so this fires
/// only when a piece's worth moved after it was put on — and it is trimmed from
/// the last piece rather than the first, so what an opponent keeps is the best
/// of what it rolled.
fn trim(stance: Stance, body: &Attributes, mut gear: Vec<EquipmentRecord>) -> Vec<EquipmentRecord> {
    let allowance = body.power().saturating_mul(GEAR_SHARE_PERMILLE) / 1_000;
    while !gear.is_empty() {
        // The flat combat lines count toward the share the attributes earned:
        // the allowance is a power allowance, and a 神装's outright 攻击力 is
        // power the fight reads.
        if marginal_power(body, &gear) <= allowance && !drifted(body, &gear) {
            break;
        }
        gear.pop();
    }
    let _ = stance;
    gear
}

/// The 外功 the opponent swings, when it carries one.
///
/// An external art of the realm's own grade band, never a 内功: an art that only
/// pays attributes would be invisible in the log, and what this is for is the
/// line that says "它用的是这招".
fn carry_art(stance: Stance, tier: u8, rng: &mut Rng) -> Option<u64> {
    if rng.below(1_000) >= stance.art_permille() {
        return None;
    }
    let grade = grade_for_tier(tier);
    let band: Vec<u64> = (1..=super::ART_COUNT)
        .filter(|id| {
            let art = blueprint(*id);
            art.kind == ArtKind::External && art.grade == grade
        })
        .collect();
    if band.is_empty() {
        None
    } else {
        Some(band[rng.below(band.len() as u64) as usize])
    }
}

/// The grade band a realm's goods belong to, the same band the market stocks.
fn grade_for_tier(tier: u8) -> ArtGrade {
    match tier {
        0..=2 => ArtGrade::Yellow,
        3..=4 => ArtGrade::Profound,
        5..=6 => ArtGrade::Earth,
        7..=8 => ArtGrade::Heaven,
        _ => ArtGrade::Divine,
    }
}

/// A fixed seed for the two opponents whose strength is printed before the
/// fight, so the same guardian is met every time.
pub(crate) fn fixed_seed(stance: Stance, rung: u32) -> u64 {
    let tag = match stance {
        Stance::Guardian => 0x4755_4152_4449_414E_u64,
        Stance::Wanderer => 0x5741_4E44_4552_4552_u64,
        Stance::Warden => 0x5741_5244_454E_0000_u64,
    };
    tag ^ u64::from(rung).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

//! Who stands on the other side: a body, the gear on it, and the 功法 it swings.
//!
//! Every fight on this page used to be against a stat block — a guardian, a
//! wanderer, a warden, all of them naked. That made the loot a second, unrelated
//! roll: you beat a bare-handed guard and it dropped a robe nobody had been
//! wearing. This module closes that: an opponent is wound up out of the same
//! pieces a drop is, wears them, swings the 功法 it carries, and whatever it was
//! carrying is what a win takes.
//!
//! Two rules keep the added detail from turning into a hidden difficulty spike:
//!
//! * **The gear comes out of the opponent's own strength, not on top of it.** A
//!   fighter's attribute budget is fixed by the ladder rung it stands on; its
//!   gear is a slice of that budget and the body is the rest. A guardian in a
//!   fine robe has a smaller body than the same guardian in rags, so the number
//!   the panel promises before the fight is still the number the fight is worth.
//! * **The slice is capped.** At the bottom of the ladder a single common piece
//!   is worth more than a whole level-one body, so an uncapped share would turn
//!   the first floor of the tower into a wall. Gear only appears once the budget
//!   can carry it, which is also why the shallow floors are still bare-handed.

use super::arts::blueprint;
use super::{
    ArtGrade, ArtKind, ArtStrike, Attribute, Attributes, EquipmentRecord, Fighter, Rng, SLOT_COUNT,
    base_attributes, items::roll_equipment,
};

/// How much of an opponent's own attribute budget may live in its gear, in
/// parts per thousand.
///
/// A third: enough that the gear is visibly part of what the opponent is, little
/// enough that the body remains the thing that carries the fight. Bigger and a
/// lucky roll would put a 极品 piece on a body that cannot pay for it.
const GEAR_SHARE_PERMILLE: i32 = 350;

/// How many swings of its 功法 an opponent can pay for.
///
/// Three: an art is a moment in the fight — a blow the log can point at — not a
/// multiplier running on every round. An opponent whose 内力 pool was read off
/// its own 根骨 paid for ten or fifteen swings and hit half again as hard for the
/// whole fight, which quietly raised every fight on the page; three is what makes
/// the art cost something to the opponent too.
const ART_SWINGS: u32 = 3;

/// How much of an opponent's damage is delivered through its 功法, in parts per
/// thousand.
///
/// Half: a fight on this ladder is five or six blows a side, and three of them
/// swing the art. This is what the art's damage is *paid for* out of — see
/// [`paid_for`] — so that giving an opponent a named move does not quietly make
/// every guardian on the ladder harder than the power the panel promised.
const ART_BLOW_SHARE_PERMILLE: u32 = 750;

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
            Self::Guardian => 1,
            Self::Wanderer | Self::Warden => 2,
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
            Self::Guardian => (1_500, 1_200),
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
    /// What the body and the gear add up to: the budget the rung gave it, split
    /// between the two. The gear is a slice of this, never a bonus on top.
    pub attributes: Attributes,
    /// What it is wearing. A won fight takes one of these — the piece is not
    /// rolled again, it is the one the opponent had on.
    pub gear: Vec<EquipmentRecord>,
    /// The 功法 it swings, by catalog id, when it carries one. A won fight takes
    /// that book.
    pub art: Option<u64>,
}

/// Build one opponent for a rung of the ladder.
///
/// `level` is the rung the body stands on, `tier` the realm its goods belong to
/// (one-based), and `seed` fixes the roll: a guardian and a warden are the same
/// opponent every time they are met, because their power is printed before the
/// fight and a roll would make that number a lie. A wanderer is rolled per
/// meeting, because nobody ever promised what the road would send.
pub(crate) fn opponent(stance: Stance, level: u16, tier: u8, name: String, seed: u64) -> Opponent {
    let mut rng = Rng::seeded(seed);
    let budget = scaled(base_attributes(level), stance.budget_permille());
    let gear = wear(stance, &budget, tier, &mut rng);
    // The gear takes its slice out of the budget and the body keeps the rest, so
    // the fighter is worth the same whichever way the split falls — a guardian
    // in a good robe is a smaller body in better cloth, not a bigger guardian.
    let worn = added(&gear);
    let total = without(budget, &worn).plus(worn);
    let art = carry_art(stance, tier, &mut rng);
    let art_strike = art.map(|art_id| {
        let art = blueprint(art_id);
        ArtStrike {
            name: art.name,
            multiplier_permille: art.multiplier_permille,
            cost: art.cost,
            // The opponent is not the one who trained it: a 功法 in a fighter's
            // hands is the multiplier it was printed with, and the mastery share
            // belongs to whoever put the pomodoros in.
            mastery_permille: 0,
        }
    });
    let (health_permille, attack_permille) = stance.multipliers();
    let naked = total.derived();
    let attack = paid_for(
        naked.attack.saturating_mul(attack_permille) / 1_000,
        art_strike.as_ref(),
    );
    let derived = super::Derived {
        health: naked.health.saturating_mul(health_permille) / 1_000,
        attack,
        ..naked
    };
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

/// What is left of a blow once the art's share of it has been set aside.
///
/// The art is not free damage: three blows in six land half again as hard, so the
/// hand that throws them is that much lighter. Without this the art was worth a
/// fifth more damage over a whole fight, which on this ladder — five or six blows
/// a side — is most of a blow, and most of a blow is a realm the player no longer
/// opens.
fn paid_for(attack: u32, art: Option<&ArtStrike>) -> u32 {
    let Some(strike) = art else {
        return attack;
    };
    let uplift = ART_BLOW_SHARE_PERMILLE
        .saturating_mul(strike.multiplier_permille.saturating_sub(1_000))
        / 1_000;
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

/// The gear the opponent can carry without outgrowing its own body.
///
/// Rolled at the realm its goods belong to — a guardian guarding 金丹 wears 金丹
/// cloth — and kept only while the pieces fit inside [`GEAR_SHARE_PERMILLE`] of
/// the budget. A piece that does not fit is not worn, which is what keeps a
/// level-one warden from turning up in an immortal robe it could not have
/// carried up the tower.
fn wear(stance: Stance, budget: &Attributes, tier: u8, rng: &mut Rng) -> Vec<EquipmentRecord> {
    let allowance = points(budget) * GEAR_SHARE_PERMILLE / 1_000;
    let mut worn: Vec<EquipmentRecord> = Vec::with_capacity(stance.gear_slots());
    let mut spent = 0;
    let mut tries = 0;
    while worn.len() < stance.gear_slots() && tries < SLOT_COUNT * 2 {
        tries += 1;
        let item = roll_equipment(0, tier, rng);
        // One piece per slot: two cloaks would be a fighter wearing the same
        // slot twice, which the inventory itself does not allow.
        if worn.iter().any(|already| already.slot == item.slot) {
            continue;
        }
        let worth = points(&item.attributes);
        if spent + worth > allowance {
            continue;
        }
        spent += worth;
        worn.push(item);
    }
    worn
}

/// How many attribute points a set holds.
fn points(attributes: &Attributes) -> i32 {
    Attribute::ALL
        .into_iter()
        .map(|attribute| attribute.of(*attributes).max(0))
        .fold(0, i32::saturating_add)
}

/// The attributes every piece together add.
fn added(gear: &[EquipmentRecord]) -> Attributes {
    gear.iter().fold(Attributes::default(), |sum, item| {
        sum.plus(Attributes::new(
            item.attributes.root.max(0),
            item.attributes.strength.max(0),
            item.attributes.mana.max(0),
            item.attributes.agility.max(0),
        ))
    })
}

/// What is left of the budget once the gear has taken its slice.
///
/// Per attribute and floored at zero: a body cannot owe points to its own robe,
/// and the gear is the budget's, not a loan against it.
fn without(budget: Attributes, worn: &Attributes) -> Attributes {
    Attributes::new(
        budget.root.saturating_sub(worn.root),
        budget.strength.saturating_sub(worn.strength),
        budget.mana.saturating_sub(worn.mana),
        budget.agility.saturating_sub(worn.agility),
    )
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

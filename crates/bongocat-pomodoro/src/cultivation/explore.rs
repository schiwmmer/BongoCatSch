//! 探索: one walk out of the door every ten hours.
//!
//! Exploration is the ladder's quiet faucet. A session already pays growth and
//! may drop gear, and a breakthrough pays a 功法, so walking pays the third thing
//! a cultivator runs short of: seeds. It is gated by a cooldown rather than by a
//! cost, because the thing being rationed is the player's attention, not a
//! currency.
//!
//! A walk can also go wrong: a quarter of them meet someone. That fight is not a
//! guardian — it is a wanderer, weaker, and paying less — and losing it costs
//! nothing but the walk.

use serde::{Deserialize, Serialize};

use super::Rng;

/// How long a failed breakthrough locks the attempt for, in seconds.
///
/// Twelve hours: long enough that a loss is a real setback, short enough that a
/// player who goes and trains comes back the same day.
pub const BREAKTHROUGH_COOLDOWN_SECONDS: i64 = 12 * 60 * 60;

/// How long between two walks, in seconds.
///
/// Ten hours: a day still holds two walks, but a save that walks in the morning
/// has to have done something else by the time the afternoon comes round — the
/// walk is the cheapest faucet on the page and must not become the *only* one.
pub const EXPLORATION_COOLDOWN_SECONDS: i64 = 10 * 60 * 60;

/// The chance a walk meets someone, in parts per thousand.
pub const ENCOUNTER_PERMILLE: u64 = 250;
/// What a walk turns up when it is quiet: seeds most often, gear sometimes, a
/// 功法 almost never.
pub const SEED_FIND_PERMILLE: u64 = 450;
pub const GEAR_FIND_PERMILLE: u64 = 120;
pub const ART_FIND_PERMILLE: u64 = 30;
/// What winning the fight that met you pays. Better than a quiet walk, because
/// the walk cost a fight to keep — and led by gear rather than by herbs, which
/// is the one place the two findings differ in kind: a wanderer carries what
/// they were carrying, a roadside carries what grows there.
pub const VICTORY_ART_PERMILLE: u64 = 80;
pub const VICTORY_GEAR_PERMILLE: u64 = 300;
pub const VICTORY_SEED_PERMILLE: u64 = 220;

/// A purse one source pays, as it stands at the first realm.
///
/// 灵石 travels beside an item rather than in place of it — it is money, not a
/// finding — so every source has its own purse rather than one shared roll. Two
/// things make the purses differ, and both are the design:
///
/// * **Which source it is.** A fight pays more than a walk: the walk is the
///   cheapest thing a save can do, and a save that fights has earned the purse.
/// * **How deep the ladder is.** The numbers here are the *first realm's*, and
///   the purse is multiplied by the tier it was paid at — the realm a walk was
///   taken in, the realm a guardian guarded, the realm a trial floor stands in.
///   Gear prices climb with the tier too, so the shop keeps roughly its weight
///   on the page at every depth instead of becoming pocket change at the top.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StonePurse {
    /// The fewest coins it pays at the first realm.
    pub min: u32,
    /// The most coins it pays at the first realm.
    pub max: u32,
}

impl StonePurse {
    /// The range this purse pays at `tier`, which is a one-based realm.
    ///
    /// A zero tier cannot happen on the ladder, but a document read from a hand
    /// edit can say anything, so it is read as the first realm rather than as a
    /// purse of nothing.
    pub(crate) const fn at(self, tier: u8) -> (u32, u32) {
        let tier = if tier == 0 { 1 } else { tier } as u32;
        (self.min.saturating_mul(tier), self.max.saturating_mul(tier))
    }
}

/// What a quiet walk turns up. The smallest purse of the four: a walk costs
/// nothing and is owed every ten hours, so it is a trickle beside a fight —
/// but it always pays one, because a walk that came home with nothing would read
/// as a bug rather than as a quiet road.
pub(crate) const WALK_STONES: StonePurse = StonePurse { min: 6, max: 10 };

/// What clearing a trial floor pays. Above a walk because the tower is a fight,
/// and below a wanderer because a floor is a level-matched opponent rather than
/// someone who has been out on the road longer.
pub(crate) const TRIAL_STONES: StonePurse = StonePurse { min: 12, max: 18 };

/// What beating a wanderer pays — the road's own purse, which is what an ambush
/// is for. Its finding roll leans the same way: gear over herbs.
pub(crate) const WANDERER_STONES: StonePurse = StonePurse { min: 20, max: 34 };

/// What beating a realm's guardian pays. The richest purse, because a guardian
/// guards a realm and its treasury both, and because it is the rarest fight on
/// the ladder.
pub(crate) const GUARDIAN_STONES: StonePurse = StonePurse { min: 38, max: 66 };

/// The purses are ordered on purpose — a walk under a floor, a floor under a
/// wanderer, a wanderer under a guardian — because that ordering is both the
/// difficulty and the emphasis, and a retune that broke it would flatten four
/// sources into one.
const _: () = assert!(WALK_STONES.max < TRIAL_STONES.min);
const _: () = assert!(TRIAL_STONES.max < WANDERER_STONES.min);
const _: () = assert!(WANDERER_STONES.max < GUARDIAN_STONES.min);

/// What one walk came to.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ExplorationOutcome {
    /// Nothing but the walk itself.
    Nothing,
    /// A seed came of it.
    Seed,
    /// Gear came of it.
    Gear,
    /// A 功法 came of it: the rarest thing a walk can pay.
    Art,
    /// 灵石 came of it, and nothing else did.
    Stones,
    /// Someone was met, and beaten.
    Victory,
    /// Someone was met, and not beaten. Nothing was lost but the walk.
    Defeat,
}

impl ExplorationOutcome {
    /// Whether the walk met someone.
    pub fn fought(self) -> bool {
        matches!(self, Self::Victory | Self::Defeat)
    }

    /// The catalog key naming how the walk ended.
    pub const fn outcome_key(self) -> &'static str {
        match self {
            Self::Nothing => "cultivation.explore.nothing",
            Self::Seed => "cultivation.explore.found_seed",
            Self::Gear => "cultivation.explore.found_gear",
            Self::Art => "cultivation.explore.found_art",
            Self::Stones => "cultivation.explore.found_stones",
            Self::Victory => "cultivation.explore.victory",
            Self::Defeat => "cultivation.explore.defeat",
        }
    }

    /// The catalog key naming how the walk ended, for a line that already names
    /// the finds beside it.
    ///
    /// [`Self::outcome_key`] hands back sentences that carry a `%{name}` or a
    /// `%{value}` — right in a popup, where the surface fills them in, and
    /// wrong in a history line where the finds are named *after* the outcome.
    /// Those four outcomes get placeholder-free sentences here; the other three
    /// never had one to lose.
    pub const fn record_outcome_key(self) -> &'static str {
        match self {
            Self::Nothing => "cultivation.explore.nothing",
            Self::Seed => "cultivation.record.outcome.seed",
            Self::Gear => "cultivation.record.outcome.gear",
            Self::Art => "cultivation.record.outcome.art",
            Self::Stones => "cultivation.record.outcome.stones",
            Self::Victory => "cultivation.explore.victory",
            Self::Defeat => "cultivation.explore.defeat",
        }
    }
}

/// How many walks the record keeps.
pub const WALK_RECORD_CAP: usize = 8;

/// What the last walk settled, kept after the app restarts.
///
/// The popup that shows a walk is deliberately forgotten on quit — it is a
/// moment, not a record — so without this the page could never say what the last
/// walk actually paid. Only names are stored: the record is a line of history,
/// and a line that went stale because an item was renamed would be a worse lie
/// than one that says what it found.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WalkRecord {
    /// When the walk was taken.
    pub at_unix: i64,
    /// The realm index it was taken in.
    pub realm: u8,
    /// How it ended.
    pub outcome: ExplorationOutcome,
    /// The 功法 it turned up, by name, when it turned one up.
    pub art: Option<String>,
    /// The gear it turned up, by name, when it turned some up.
    pub gear: Option<String>,
    /// The herb it turned up, by name, when it turned one up.
    pub seed: Option<String>,
    /// How many 灵石 it turned up, when it turned any up.
    ///
    /// Counted rather than named, because a purse is a number: what the walk
    /// found is worth exactly what it says, and a name would only be a number
    /// the page had to guess at again.
    #[serde(default)]
    pub stones: Option<u32>,
}

/// What one walk turns up, before any fight is settled.
///
/// One roll answers both questions — is anything there, and does anyone want it —
/// so the four quiet outcomes are exclusive and their chances add up to less than
/// one, leaving the rest of the thousand to "nothing".
pub fn roll_walk(rng: &mut Rng) -> ExplorationOutcome {
    let roll = rng.below(1_000);
    if roll < SEED_FIND_PERMILLE {
        return ExplorationOutcome::Seed;
    }
    if roll < SEED_FIND_PERMILLE + GEAR_FIND_PERMILLE {
        return ExplorationOutcome::Gear;
    }
    if roll < SEED_FIND_PERMILLE + GEAR_FIND_PERMILLE + ART_FIND_PERMILLE {
        return ExplorationOutcome::Art;
    }
    if roll < SEED_FIND_PERMILLE + GEAR_FIND_PERMILLE + ART_FIND_PERMILLE + ENCOUNTER_PERMILLE {
        // Someone is here; who wins is the fight's to say, not the walk's.
        return ExplorationOutcome::Victory;
    }
    ExplorationOutcome::Nothing
}

/// What winning the fight pays, as a second roll of its own.
pub fn roll_victory(rng: &mut Rng) -> ExplorationOutcome {
    let roll = rng.below(1_000);
    if roll < VICTORY_ART_PERMILLE {
        return ExplorationOutcome::Art;
    }
    if roll < VICTORY_ART_PERMILLE + VICTORY_GEAR_PERMILLE {
        return ExplorationOutcome::Gear;
    }
    if roll < VICTORY_ART_PERMILLE + VICTORY_GEAR_PERMILLE + VICTORY_SEED_PERMILLE {
        return ExplorationOutcome::Seed;
    }
    ExplorationOutcome::Nothing
}

/// How many 灵石 an outing turns up.
///
/// A roll of its own, beside the finding roll rather than inside it: 灵石 is a
/// currency, not one of the three things an outing finds, so the same walk can
/// come back with a herb and a handful of stones both. The amount is a short
/// range rather than a fixed figure, so two walks in the same day do not pay
/// the same purse, and it is scaled by the tier the purse was opened at.
///
/// Crate-private like the other rolls: the seeded source it takes is nobody
/// else's business, and the state machine is the only thing that draws it.
pub(crate) fn roll_stones(rng: &mut Rng, purse: StonePurse, tier: u8) -> u32 {
    let (min, max) = purse.at(tier);
    let span = max.saturating_sub(min).saturating_add(1);
    min.saturating_add(u32::try_from(rng.below(u64::from(span))).unwrap_or(0))
}

/// Whether a walk is owed: the cooldown has run out.
pub fn exploration_ready(last: Option<i64>, now_unix: i64) -> bool {
    match last {
        None => true,
        Some(last) => now_unix.saturating_sub(last) >= EXPLORATION_COOLDOWN_SECONDS,
    }
}

/// Whether the guardian can be fought again: the cooldown after a loss has run
/// out.
pub fn breakthrough_ready(blocked_until: Option<i64>, now_unix: i64) -> bool {
    match blocked_until {
        None => true,
        Some(until) => now_unix >= until,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quiet outcomes and the encounter share one roll, so together they
    /// cannot fill it: the rest of the thousand is a walk that finds nothing.
    const _: () = assert!(
        SEED_FIND_PERMILLE + GEAR_FIND_PERMILLE + ART_FIND_PERMILLE + ENCOUNTER_PERMILLE <= 1_000
    );

    #[test]
    fn a_walk_lands_on_every_outcome_sooner_or_later() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..1_000_u64 {
            seen.insert(roll_walk(&mut Rng::seeded(seed)));
        }
        for outcome in [
            ExplorationOutcome::Seed,
            ExplorationOutcome::Gear,
            ExplorationOutcome::Art,
            ExplorationOutcome::Victory,
            ExplorationOutcome::Nothing,
        ] {
            assert!(seen.contains(&outcome), "{outcome:?} never came up");
        }
        // Seeds are the common finding, a 功法 the rarest.
        let counts = |outcome| {
            (0..5_000_u64)
                .filter(|seed| roll_walk(&mut Rng::seeded(*seed)) == outcome)
                .count()
        };
        assert!(counts(ExplorationOutcome::Seed) > counts(ExplorationOutcome::Gear));
        assert!(counts(ExplorationOutcome::Gear) > counts(ExplorationOutcome::Art));
    }

    #[test]
    fn a_victory_pays_out_and_a_defeat_does_not() {
        assert!(ExplorationOutcome::Victory.fought());
        assert!(ExplorationOutcome::Defeat.fought());
        assert!(!ExplorationOutcome::Seed.fought());
    }

    /// The purse is the tier's: the same walk pays six times as much in the
    /// sixth realm, which is what keeps the shop's weight at every depth.
    #[test]
    fn a_purse_scales_with_the_tier_it_was_opened_at() {
        assert_eq!(WALK_STONES.at(1), (WALK_STONES.min, WALK_STONES.max));
        assert_eq!(WALK_STONES.at(6).0, WALK_STONES.min * 6);
        assert_eq!(WALK_STONES.at(10).1, WALK_STONES.max * 10);
        assert_eq!(
            WALK_STONES.at(0),
            WALK_STONES.at(1),
            "a hand-edited zero tier reads as the first realm"
        );
        // Every purse is opened at the tier of the thing that paid it, so a
        // deeper fight is a bigger purse by arithmetic and not by a second table.
        let mut rng = Rng::seeded(11);
        let shallow = roll_stones(&mut rng, GUARDIAN_STONES, 1);
        let mut deep = 0;
        for _ in 0..16 {
            deep = deep.max(roll_stones(&mut rng, GUARDIAN_STONES, 5));
        }
        assert!(deep > shallow * 2, "{deep} against {shallow}");
    }

    #[test]
    fn both_cooldowns_run_out() {
        assert!(exploration_ready(None, 0));
        assert!(!exploration_ready(
            Some(1_000),
            1_000 + EXPLORATION_COOLDOWN_SECONDS - 1
        ));
        assert!(exploration_ready(
            Some(1_000),
            1_000 + EXPLORATION_COOLDOWN_SECONDS
        ));
        assert!(breakthrough_ready(None, 0));
        assert!(!breakthrough_ready(Some(500), 499));
        assert!(breakthrough_ready(Some(500), 500));
    }
}

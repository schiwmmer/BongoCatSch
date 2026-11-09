//! 试炼: the fight a finished pomodoro pays for — a submodule of exploration.
//!
//! Two outings share the courtyard, and the page names them apart. The 锁妖塔
//! (see [`super::trial`]) is the ladder climbed on the day's own attempts, one
//! floor at a time, and its frontier is a number that only ever goes up. This
//! 试炼 is the other kind: every finished pomodoro earns one chance, ten at
//! most are banked, and a chance is spent on a fight tied to the road the
//! cultivator is walking — so the training itself is the faucet, and a good
//! week of focus is a week of fights.
//!
//! It is a **submodule of exploration** rather than a second economy. Its
//! opponent stands where the current region's progress stands, multiplied by
//! [`TRIAL_CHALLENGE_MIN_PERMILLE`] to [`TRIAL_CHALLENGE_MAX_PERMILLE`], and
//! its base reward is exactly what the current region pays for one hour of
//! walking. A finished pomodoro is therefore worth a short walk on the road
//! the save is already on, and the fight is worth having because the road is
//! worth walking.
//!
//! What a win finds is the 锁妖塔's base table rather than a second table: a
//! seed most often, gear sometimes, a 功法 almost never. The two differ in
//! *when* they pay, not in what they pay, which is what keeps one economy
//! behind both.

use serde::Serialize;

use super::{DuelReport, ExplorationFindView};

/// How many 试炼 chances a save may bank.
///
/// Ten: a full day of pomodoros, or a week of light ones. The ceiling is what
/// makes finishing a session worth something even when the user has no time to
/// spend the chance right then, and it is low enough that a save cannot sit on
/// a hundred and open the tower all at once.
pub const TRIAL_CHANCES_CAP: u32 = 10;

/// The weakest a 试炼's opponent is drawn at, in parts per thousand of the
/// current region's requirement.
///
/// One and a half: the opponent is always a real step past what the road asks,
/// so a 试炼 is a fight rather than a formality. The draw is a uniform between
/// this and [`TRIAL_CHALLENGE_MAX_PERMILLE`], so a 试炼's difficulty is a
/// spread around the region rather than a fixed wall.
pub const TRIAL_CHALLENGE_MIN_PERMILLE: u32 = 1_500;

/// The strongest a 试炼's opponent is drawn at, in parts per thousand of the
/// current region's requirement.
///
/// Three: the top of the spread is a body three times what the road asks, so a
/// 试炼 can be a wall — and a win over it pays for the trouble.
pub const TRIAL_CHALLENGE_MAX_PERMILLE: u32 = 3_000;

/// What one 试炼 chance came to.
///
/// The duel travels whole, the way every other fight does, because it is
/// replayed blow by blow; what the win turned up travels as the same find a
/// walk reports, since the three kinds are the same three.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProvingOutcome {
    /// Every blow of the fight that settled it.
    pub duel: DuelReport,
    /// What winning turned up. A loss finds nothing but the lesson.
    pub found: ExplorationFindView,
    /// What the opponent was worth, as the page reads it beside the verdict.
    ///
    /// Carried rather than left to the log because the popup that says "试炼获
    /// 胜" has to know who was beaten, and a surface reading the log would have
    /// to match a line by timestamp to find it.
    pub opponent_power: u32,
    /// Which region the 试炼 was fought in, as a ladder position.
    pub region: u8,
}

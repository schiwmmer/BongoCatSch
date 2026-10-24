//! 试炼: the fight a finished pomodoro pays for.
//!
//! Two dungeons share the courtyard, and the page names them apart. The
//! 锁妖塔 (see [`super::trial`]) is the ladder climbed on the day's own
//! attempts, one floor at a time, and its frontier is a number that only ever
//! goes up. This 试炼 is the other kind: every finished pomodoro earns one
//! chance, ten at most are banked, and a chance is spent on a fight levelled to
//! the cultivator — so the training itself is the faucet, and a good week of
//! focus is a week of fights.
//!
//! What a win pays is the 锁妖塔's base table rather than a second table: a seed
//! most often, gear sometimes, a 功法 almost never. The two differ in *when* they
//! pay, not in what they pay, which is what keeps one economy behind both.

use super::{DuelReport, ExplorationFindView};

/// How many 试炼 chances a save may bank.
///
/// Ten: a full day of pomodoros, or a week of light ones. The ceiling is what
/// makes finishing a session worth something even when the user has no time to
/// spend the chance right then, and it is low enough that a save cannot sit on
/// a hundred and open the tower all at once.
pub const TRIAL_CHANCES_CAP: u32 = 10;

/// What one 试炼 chance came to.
///
/// The duel travels whole, the way every other fight does, because it is
/// replayed blow by blow; what the win turned up travels as the same find a
/// walk reports, since the three kinds are the same three.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvingOutcome {
    /// Every blow of the fight that settled it.
    pub duel: DuelReport,
    /// What winning turned up. A loss finds nothing but the lesson.
    pub found: ExplorationFindView,
}

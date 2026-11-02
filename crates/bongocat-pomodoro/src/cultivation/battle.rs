//! The guardian that gates a realm, and the one comparison that decides it.
//!
//! The fight is a power comparison rather than a hit-by-hit simulation, and that
//! is a deliberate choice: the number a surface shows the user before they press
//! the button has to be the number that decides the outcome, or the panel is
//! lying about the odds. A hidden damage roll would make "your power is 1,540
//! against 1,528" a coin toss instead of a win.
//!
//! What makes the fight a fight is the tuning, not the dice: the guardian is
//! stronger than a cultivator standing at the realm's cap with nothing equipped,
//! so the way through is gear and herbs.

use serde::Serialize;

use super::{Attributes, Realm, base_attributes};

/// How much stronger a guardian is than a naked cultivator at the same cap, in
/// percent. Every guardian in the ladder shares the ratio, so no realm is
/// secretly softer than another.
const GUARDIAN_STRENGTH_PERCENT: u32 = 120;

/// The thing standing at the end of a realm.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Guardian {
    /// The realm a win opens.
    pub target: Realm,
    /// What it takes to beat it.
    pub power: u32,
}

/// The guardian gating the realm after `current`, or `None` at the top.
///
/// Its power is measured against a cultivator standing at `current`'s last
/// layer with no equipment at all, so the requirement is fixed by the ladder
/// rather than by whatever the player happens to own — and then the guardian's
/// own gear is added on top, because the fight really does send a guardian
/// wearing it. The percentage is a floor: however light the roll, the guardian
/// stays stronger than the naked cultivator it gates.
pub fn guardian_for(current: Realm) -> Option<Guardian> {
    let target = current.next()?;
    let naked = base_attributes(current.cap_level()).power();
    let floor = naked * GUARDIAN_STRENGTH_PERCENT / 100;
    // The same opponent the breakthrough will send, read for its attributes:
    // what the panel prints is the body and the gear that meet the player.
    let rival = super::opponent::guardian(current, target);
    Some(Guardian {
        target,
        power: rival.attributes.power().max(floor),
    })
}

/// What a breakthrough attempt came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct BattleReport {
    /// The power the cultivator brought: level, gear and herbs, exactly as the
    /// panel showed them.
    pub player_power: u32,
    pub guardian_power: u32,
    pub won: bool,
}

/// Settle one attempt.
///
/// Ties go to the cultivator: a power exactly equal to the guardian's is the
/// narrowest possible win, which keeps the displayed threshold honest — the
/// number shown is the number that must be reached, not exceeded.
pub fn resolve(player: Attributes, guardian: Guardian) -> BattleReport {
    let player_power = player.power();
    BattleReport {
        player_power,
        guardian_power: guardian.power,
        won: player_power >= guardian.power,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_naked_cultivator_loses_every_guardian() {
        for realm in Realm::ALL {
            let Some(guardian) = guardian_for(realm) else {
                continue;
            };
            let naked = base_attributes(realm.cap_level());
            assert!(
                !resolve(naked, guardian).won,
                "{:?}'s guardian is beatable without gear",
                realm
            );
        }
    }

    #[test]
    fn the_top_realm_has_no_guardian() {
        assert!(guardian_for(Realm::Immortal).is_none());
        assert_eq!(
            guardian_for(Realm::QiRefining).map(|guardian| guardian.target),
            Some(Realm::Foundation)
        );
    }

    #[test]
    fn reaching_the_guardian_power_exactly_is_a_win() {
        let attributes = Attributes::new(0, 500, 0, 0);
        let guardian = Guardian {
            target: Realm::Foundation,
            power: attributes.power(),
        };
        let report = resolve(attributes, guardian);
        assert_eq!(
            report.player_power,
            attributes.power(),
            "the report prints the power the body carries"
        );
        assert!(report.won);
    }
}

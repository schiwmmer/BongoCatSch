//! The fight a breakthrough is settled by.
//!
//! Not a power comparison: two fighters stand up and trade blows until one of
//! them cannot. A number the panel prints has to be the number that decides, and
//! "your power is 1,540 against 1,528" is a coin toss pretending to be an
//! answer — so the fight is the answer, and every line of it is shown.
//!
//! The order of the blows comes from 速度: each fighter fills a meter at their
//! own speed and acts when it fills, which is what makes 身法 worth anything at
//! all. Everything else is read off the shown numbers — 攻击力 and 法术强度 for
//! the damage, 法抗 to blunt it, 命中 against 闪避 to see whether it lands, 生命值
//! to survive it, 内力值 to pay for an 外功.

use serde::{Deserialize, Serialize};

use super::{Attributes, Derived, Rng, arts::external_damage};

/// How many blows the longest fight runs to before it is called.
///
/// A cap rather than a wall-clock: two evenly matched fighters with high 闪避 can
/// circle each other forever, and a fight that never ends is worse than a loss.
pub const DUEL_ROUND_CAP: u32 = 30;

/// How full a meter has to be before its owner acts.
const METER_FULL: i64 = 1_000;

/// A guard against a fight that cannot progress: no speed on either side means
/// no meter ever fills, so the loop also counts ticks.
const TICK_CAP: u32 = 4_000;

/// The catalog key that names the guardian. A surface translates it; the domain
/// never holds a translated word.
pub const GUARDIAN_NAME: &str = "cultivation.duel.guardian";

/// The catalog key that names a wanderer met while exploring.
pub const WANDERER_NAME: &str = "cultivation.duel.wanderer";

/// The catalog key that names the cultivator in a fight log.
pub const PLAYER_NAME: &str = "cultivation.duel.you";

/// An 外功 brought into a fight: what it is called, what it multiplies, what it
/// costs in 内力.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtStrike {
    pub name: String,
    pub multiplier_permille: u32,
    pub cost: u32,
}

/// One side of a fight.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Fighter {
    pub name: String,
    pub health: u32,
    pub attack: u32,
    /// The 内力 pool an 外功 spends. Empty, the fighter falls back on 攻击力.
    pub inner: u32,
    pub spell: u32,
    pub resist: u32,
    pub speed: u32,
    /// 命中, in parts per thousand.
    pub hit: u32,
    /// 闪避, in parts per thousand.
    pub evade: u32,
    /// The 外功 being used, if the fighter has one.
    pub art: Option<ArtStrike>,
}

impl Fighter {
    /// A cultivator, read off the numbers the panel already shows.
    pub fn from_derived(name: String, derived: Derived, art: Option<ArtStrike>) -> Self {
        Self {
            name,
            health: derived.health,
            attack: derived.attack,
            inner: derived.inner,
            spell: derived.spell,
            resist: derived.resist,
            speed: derived.speed,
            hit: derived.hit,
            evade: derived.evade,
            art,
        }
    }

    /// A fighter whose numbers come straight from a set of attributes.
    pub fn from_attributes(name: String, attributes: Attributes, art: Option<ArtStrike>) -> Self {
        Self::from_derived(name, attributes.derived(), art)
    }

    /// Whether the fight is over for this fighter.
    pub fn down(&self) -> bool {
        self.health == 0
    }
}

/// One blow, as the log tells it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DuelRound {
    /// The blow's position in the fight, one-based.
    pub index: u32,
    /// Who threw it.
    pub attacker: String,
    /// The 外功 spent on it, if one was.
    pub art: Option<String>,
    /// Whether the other side slipped it.
    pub dodged: bool,
    /// What it took off.
    pub damage: u32,
    /// What the target has left.
    pub target_health: u32,
}

/// How a fight came out, and every blow in it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DuelReport {
    pub rounds: Vec<DuelRound>,
    pub won: bool,
    pub player_health: u32,
    pub guardian_health: u32,
}

/// Settle one fight.
///
/// The meters are what make 速度 matter: each side fills at its own rate and acts
/// when full, so a fast fighter simply gets more turns. A tie on the meter goes
/// to the faster side, which is the same advantage stated once more.
pub fn duel(player: Fighter, guardian: Fighter, rng: &mut Rng) -> DuelReport {
    let mut player = player;
    let mut guardian = guardian;
    let player_name = player.name.clone();
    let guardian_name = guardian.name.clone();
    let mut rounds = Vec::new();
    let mut player_meter = 0_i64;
    let mut guardian_meter = 0_i64;
    let mut ticks = 0_u32;
    while !player.down() && !guardian.down() && rounds.len() < DUEL_ROUND_CAP as usize {
        ticks += 1;
        if ticks > TICK_CAP {
            break;
        }
        player_meter += i64::from(player.speed);
        guardian_meter += i64::from(guardian.speed);
        let player_acts = player_meter >= METER_FULL
            && (player_meter >= guardian_meter || guardian_meter < METER_FULL);
        let guardian_acts = !player_acts && guardian_meter >= METER_FULL;
        if !player_acts && !guardian_acts {
            continue;
        }
        let round = if player_acts {
            player_meter -= METER_FULL;
            let mut round = strike(&mut player, &mut guardian, rng);
            round.attacker = player_name.clone();
            round
        } else {
            guardian_meter -= METER_FULL;
            let mut round = strike(&mut guardian, &mut player, rng);
            round.attacker = guardian_name.clone();
            round
        };
        let index = rounds.len() as u32 + 1;
        rounds.push(DuelRound { index, ..round });
    }
    DuelReport {
        won: guardian.down() && !player.down(),
        player_health: player.health,
        guardian_health: guardian.health,
        rounds,
    }
}

/// The damage one blow does, before 命中 and 闪避 are asked.
///
/// 法术强度 is spent against 法抗, but never for nothing: a tenth of it always
/// gets through, so stacking 法抗 is a soft answer rather than an immune one.
fn blow(attacker: &Fighter, defender: &Fighter) -> u32 {
    let spell = attacker
        .spell
        .saturating_sub(defender.resist)
        .max(attacker.spell / 10);
    attacker.attack.saturating_add(spell)
}

/// Throw one blow.
fn strike(attacker: &mut Fighter, defender: &mut Fighter, rng: &mut Rng) -> DuelRound {
    let mut damage = blow(attacker, defender);
    let mut art = None;
    // An 外功 is spent whenever it can be paid for: the choice to save 内力 for
    // later is not the fighter's to make, and a fighter who cannot pay falls
    // back on the bare hand rather than standing still.
    if let Some(strike) = attacker.art.clone()
        && attacker.inner >= strike.cost
    {
        attacker.inner -= strike.cost;
        damage = external_damage(damage, strike.multiplier_permille);
        art = Some(strike.name);
    }
    // 命中 against 闪避, never a certainty and never hopeless.
    let chance = (i64::from(attacker.hit) - i64::from(defender.evade)).clamp(50, 950);
    let dodged = rng.below(1_000) >= chance as u64;
    if dodged {
        damage = 0;
    }
    defender.health = defender.health.saturating_sub(damage);
    DuelRound {
        index: 0,
        attacker: String::new(),
        art,
        dodged,
        damage,
        target_health: defender.health,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute(name: &str, health: u32, attack: u32, speed: u32) -> Fighter {
        Fighter {
            name: name.to_owned(),
            health,
            attack,
            inner: 0,
            spell: 0,
            resist: 0,
            speed,
            hit: 950,
            evade: 0,
            art: None,
        }
    }

    #[test]
    fn the_stronger_fighter_wins() {
        let strong = brute("strong", 500, 60, 10);
        let weak = brute("weak", 200, 20, 10);
        let report = duel(strong, weak, &mut Rng::seeded(1));
        assert!(report.won);
        assert_eq!(report.guardian_health, 0);
        assert!(report.player_health > 0);
        assert!(!report.rounds.is_empty(), "the log says what happened");
    }

    #[test]
    fn the_faster_fighter_strikes_first_and_more_often() {
        let swift = brute("swift", 5_000, 1, 100);
        let slow = brute("slow", 5_000, 100, 1);
        let report = duel(swift.clone(), slow.clone(), &mut Rng::seeded(2));
        assert_eq!(
            report.rounds.first().map(|round| round.attacker.as_str()),
            Some("swift"),
            "speed is what decides who acts"
        );
        let swift_blows = report
            .rounds
            .iter()
            .filter(|round| round.attacker == "swift")
            .count();
        let slow_blows = report.rounds.len() - swift_blows;
        assert!(
            swift_blows > slow_blows,
            "a hundred to one in speed must mean more turns: {swift_blows} vs {slow_blows}"
        );
    }

    #[test]
    fn spell_resist_blunts_a_blow_but_never_empties_it() {
        let caster = Fighter {
            spell: 100,
            attack: 0,
            ..brute("caster", 1_000, 0, 10)
        };
        let naked = Fighter {
            resist: 0,
            ..brute("naked", 1_000, 0, 10)
        };
        let warded = Fighter {
            resist: 1_000,
            ..brute("warded", 1_000, 0, 10)
        };
        assert_eq!(blow(&caster, &naked), 100);
        assert_eq!(
            blow(&caster, &warded),
            10,
            "a tenth of 法术强度 always gets through"
        );
    }

    #[test]
    fn an_external_art_is_spent_while_the_inner_force_lasts() {
        let mut fighter = Fighter {
            inner: 25,
            art: Some(ArtStrike {
                name: "降龙十八掌".to_owned(),
                multiplier_permille: 2_000,
                cost: 10,
            }),
            ..brute("fighter", 1_000, 100, 10)
        };
        let mut target = brute("target", 10_000, 0, 10);
        let first = strike(&mut fighter, &mut target, &mut Rng::seeded(3));
        assert_eq!(first.art.as_deref(), Some("降龙十八掌"));
        assert!(first.damage >= 200, "the art multiplies the blow");
        assert_eq!(fighter.inner, 15);
        // Two more blows and the pool cannot pay; the bare hand is still there.
        let _ = strike(&mut fighter, &mut target, &mut Rng::seeded(4));
        let third = strike(&mut fighter, &mut target, &mut Rng::seeded(5));
        assert_eq!(third.art, None, "no 内力 left to spend");
        assert!(third.damage > 0, "a fighter out of 内力 still fights");
    }

    #[test]
    fn a_fight_that_cannot_end_still_ends() {
        // Both sides miss everything: no damage can land, so the round cap is
        // what stops the fight.
        let left = Fighter {
            hit: 50,
            ..brute("left", 1_000, 10, 10)
        };
        let right = Fighter {
            evade: 1_000,
            hit: 50,
            ..brute("right", 1_000, 10, 10)
        };
        let report = duel(left, right, &mut Rng::seeded(6));
        assert!(report.rounds.len() <= DUEL_ROUND_CAP as usize);
        assert!(!report.won, "nobody went down");
    }
}

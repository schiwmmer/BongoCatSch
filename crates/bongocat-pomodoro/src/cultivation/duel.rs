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
    /// What a completed 功法 adds to every blow, in parts per thousand of the
    /// wielder's 攻击力. Zero for an 外功 whose last 阶 is still owed.
    #[serde(default)]
    pub mastery_permille: u32,
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
    /// 命中, a plain number read against the other side's 闪避.
    pub hit: u32,
    /// 闪避, a plain number the attacker's 命中 is read against.
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

/// How many fights the record keeps.
///
/// Eight is what the page has room to draw without turning into a scroll of
/// history nobody reads, and it is long enough that a run of losses still leaves
/// the wins visible at the top.
pub const DUEL_RECORD_CAP: usize = 8;

/// Which fight a record line is about.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FightKind {
    /// The guardian waiting at a realm's last layer.
    Guardian,
    /// Someone met on a walk.
    Wanderer,
    /// The warden of a 试炼 floor.
    Trial,
}

impl FightKind {
    /// The catalog key naming this kind of fight.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Guardian => "cultivation.record.guardian",
            Self::Wanderer => "cultivation.record.wanderer",
            Self::Trial => "cultivation.trial.title",
        }
    }
}

/// One fought fight, as the page's record reads it.
///
/// The record is one line per fight and nothing more: who it was against, whether
/// it was won, and where it happened. The blows are not kept — the popup has
/// already shown them, and a save that carried every round of every fight would
/// be a second, worse copy of the combat log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DuelRecord {
    pub won: bool,
    pub kind: FightKind,
    /// The realm index the fight was fought in.
    pub realm: u8,
    /// When it was fought.
    pub at_unix: i64,
}

/// How a fight came out, and every blow in it.
///
/// The two sides travel whole — every number they walked in with — because the
/// replay is not only a log: the popup shows what each side brought, and a
/// report that carried only the final health could not say what the fight was
/// between. `health` here is the health they started with; where a side stands
/// mid-replay is [`Self::health_after`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DuelReport {
    pub rounds: Vec<DuelRound>,
    pub won: bool,
    /// The cultivator as they stood at the first blow.
    pub player: Fighter,
    /// Whoever they were fighting, on the same terms.
    pub opponent: Fighter,
}

impl DuelReport {
    /// The health each side has left once `revealed` blows have been shown.
    ///
    /// The bars start from the health the sides walked in with and take the
    /// damage the shown blows did, so a replay half-way through reads as a
    /// fight half-fought rather than as two full bars and a spoilered ending.
    /// A slipped blow did no damage, so it moves nothing.
    pub fn health_after(&self, revealed: usize) -> (u32, u32) {
        let mut player = self.player.health;
        let mut opponent = self.opponent.health;
        // Who swung is the name the round carries, and the cultivator's name is
        // whatever this report says it is — compared against the report rather
        // than against a constant, so a fight between two other things still
        // folds correctly.
        let player_name = self.player.name.as_str();
        for round in self.rounds.iter().take(revealed) {
            let target = if round.attacker == player_name {
                &mut opponent
            } else {
                &mut player
            };
            *target = target.saturating_sub(round.damage);
        }
        (player, opponent)
    }
}

/// Settle one fight.
///
/// The meters are what make 速度 matter: each side fills at its own rate and acts
/// when full, so a fast fighter simply gets more turns. A tie on the meter goes
/// to the faster side, which is the same advantage stated once more.
pub fn duel(player: Fighter, guardian: Fighter, rng: &mut Rng) -> DuelReport {
    // The two sides as they walked in, kept for the report: the fight mutates
    // its own copies, and what a surface shows beside the replay is the
    // numbers the fight began between.
    let (walked_in, mut player, mut guardian) =
        ((player.clone(), guardian.clone()), player, guardian);
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
    let (player_side, opponent_side) = walked_in;
    DuelReport {
        won: guardian.down() && !player.down(),
        player: player_side,
        opponent: opponent_side,
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

/// The chance one blow lands between even fighters, in parts per thousand.
///
/// Eighty percent: two cultivators of the same depth land most of their blows,
/// but a fight where nothing is ever slipped is arithmetic rather than a fight.
pub const EVEN_HIT_CHANCE_PERMILLE: i64 = 800;

/// The floor and the ceiling the chance is held to, in parts per thousand.
///
/// A blow that can never land is a fight that cannot end, and a blow that can
/// never be slipped is a stat nobody would pay for; the same two walls the old
/// percentage model had, kept because the reasons for them did not change.
const HIT_CHANCE_FLOOR_PERMILLE: i64 = 100;
const HIT_CHANCE_CEILING_PERMILLE: i64 = 950;

/// The exponent the chance's curve is cut at, over one thousand.
///
/// The curve is `EVEN × ratio^k` with `ratio = attacker 命中 / defender 闪避`.
/// `k = ln 0.625 / ln 0.5 ≈ 0.678` is the one exponent that makes a two-to-one
/// gap worth exactly a forty-point swing: at ratio ½ the chance is 500‰, so a
/// fighter whose 命中 is half the other side's 闪避 lands one blow in two, and
/// evenly matched fighters land four in five. Read the ratio the other way and
/// the same curve pays the advantage back, up to the ceiling.
const HIT_CHANCE_EXPONENT_NUM: u32 = 678;
const HIT_CHANCE_EXPONENT_DEN: f64 = 1_000.0;

/// The chance one blow lands, in parts per thousand.
///
/// Read off the *ratio* of the attacker's 命中 to the defender's 闪避 rather
/// than off their difference: a difference model makes a hundred points mean
/// everything early and nothing late, while a ratio means what it says at every
/// depth — even is even, double is double, and the numbers the panel prints are
/// the numbers the curve eats.
pub fn hit_chance_permille(attacker_hit: u32, defender_evade: u32) -> i64 {
    if attacker_hit == 0 {
        return HIT_CHANCE_FLOOR_PERMILLE;
    }
    if defender_evade == 0 {
        return HIT_CHANCE_CEILING_PERMILLE;
    }
    let ratio = f64::from(attacker_hit) / f64::from(defender_evade);
    let exponent = f64::from(HIT_CHANCE_EXPONENT_NUM) / HIT_CHANCE_EXPONENT_DEN;
    let chance = (EVEN_HIT_CHANCE_PERMILLE as f64) * ratio.powf(exponent);
    (chance.round() as i64).clamp(HIT_CHANCE_FLOOR_PERMILLE, HIT_CHANCE_CEILING_PERMILLE)
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
        // A completed 外功 adds its mastery share of 攻击力 to every blow it
        // lands: the multiplier is what the 阶 bought, this is what finishing
        // the last one bought.
        let mastered = attacker.attack.saturating_mul(strike.mastery_permille) / 1_000;
        damage = damage.saturating_add(mastered);
        art = Some(strike.name);
    }
    // 命中 against 闪避, never a certainty and never hopeless.
    let chance = hit_chance_permille(attacker.hit, defender.evade);
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
        let report = duel(strong.clone(), weak.clone(), &mut Rng::seeded(1));
        assert!(report.won);
        // The sides travel whole, so the report says what the fight was
        // between as well as how it ended.
        assert_eq!(report.player.attack, 60);
        assert_eq!(report.opponent.attack, 20);
        let (player, opponent) = report.health_after(report.rounds.len());
        assert_eq!(opponent, 0, "the loser is at zero once every blow is shown");
        assert!(player > 0);
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
                mastery_permille: 0,
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

    /// A fighter whose 外功 has reached its last 阶 hits harder with it than
    /// the same art one 阶 short does — the mastery share is paid on top of the
    /// multiplier, and only while the art is being swung.
    #[test]
    fn a_completed_external_art_adds_its_mastery_share() {
        fn striker(mastery_permille: u32) -> Fighter {
            Fighter {
                inner: 100,
                art: Some(ArtStrike {
                    name: "掌法".to_owned(),
                    multiplier_permille: 1_200,
                    cost: 10,
                    mastery_permille,
                }),
                ..brute("fighter", 10_000, 1_000, 10)
            }
        }
        let mut plain_target = brute("target", 100_000, 0, 10);
        let mut mastered_target = brute("target", 100_000, 0, 10);
        let plain = strike(&mut striker(0), &mut plain_target, &mut Rng::seeded(11));
        let mastered = strike(
            &mut striker(150),
            &mut mastered_target,
            &mut Rng::seeded(11),
        );
        assert!(!plain.dodged, "the probe needs a blow that lands");
        // 1,000 攻击力 through a 120% art is 1,200; the mastery share adds
        // fifteen percent of the 攻击力 behind it.
        assert_eq!(plain.damage, 1_200);
        assert_eq!(mastered.damage, 1_350, "the completed art hits harder");
        // Out of 内力 the art — and its mastery share with it — is not spent.
        let mut spent = Fighter {
            inner: 0,
            ..striker(150)
        };
        let bare = strike(&mut spent, &mut plain_target, &mut Rng::seeded(12));
        assert_eq!(bare.art, None, "no 内力, no art");
        assert_eq!(bare.damage, 1_000, "and no mastery share either");
    }

    #[test]
    fn the_hit_chance_curve_holds_its_two_anchors() {
        // Even fighters land four blows in five.
        assert_eq!(hit_chance_permille(100, 100), 800);
        assert_eq!(hit_chance_permille(2_000, 2_000), 800);
        // A two-to-one disadvantage lands half — the anchor the exponent was
        // cut to make exact.
        assert_eq!(hit_chance_permille(100, 200), 500);
        assert_eq!(hit_chance_permille(300, 600), 500);
        // And the same gap the other way pays back past the ceiling.
        assert_eq!(hit_chance_permille(200, 100), 950);
        // Degenerate sides still sit inside the walls.
        assert_eq!(hit_chance_permille(0, 100), 100);
        assert_eq!(hit_chance_permille(100, 0), 950);
        // A wide gap bottoms out toward the floor rather than reaching zero.
        assert!(hit_chance_permille(100, 1_000) < 200);
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

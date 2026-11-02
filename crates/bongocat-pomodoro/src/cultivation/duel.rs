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

use super::{Attributes, Derived, Rng, arts::StrikeEffect, arts::external_damage};

/// How many blows the longest fight runs to before it is called.
///
/// A cap rather than a wall-clock: two evenly matched fighters with high 闪避 can
/// circle each other forever, and a fight that never ends is worse than a loss.
pub const DUEL_ROUND_CAP: u32 = 30;

/// How full a meter has to be before its owner acts.
///
/// The floor is the pace a fight of slow fighters shares. A fight is also
/// capped at one action per tick per side, so a bar every speed on the field
/// clears every tick would hand the turn to whoever's bank is ahead — and the
/// banks then read the fight, not the speeds: a 1.6-to-1 edge in 速度
/// compounded into a fourteen-to-one edge in turns, the slower side's bank
/// wiped by every action it finally won. The bar is therefore raised to the
/// faster side's speed (see `duel`), which puts the slower side back on the
/// clock at the ratio of the speeds themselves.
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
    /// How many blows one use throws. One for a 外功 that lands a single blow,
    /// three for one that throws a flurry of them.
    #[serde(default = "one_blow")]
    pub hits: u8,
    /// The band each blow is drawn from, in parts per thousand of the printed
    /// worth. `(0, 0)` is a move that pays exactly what it says.
    #[serde(default)]
    pub band_permille: (u32, u32),
    /// What the first blow that lands does to the rest of the fight.
    #[serde(default)]
    pub effect: Option<StrikeEffect>,
    /// Whether the move has already laid its effect on the fight: it rides on
    /// the first blow that gets through and nowhere else.
    #[serde(default)]
    pub marked: bool,
    /// What a completed 功法 adds to every blow, in parts per thousand of the
    /// wielder's 攻击力. Zero for an 外功 whose last 阶 is still owed.
    #[serde(default)]
    pub mastery_permille: u32,
}

/// The number of blows an 外功 written before shapes existed throws.
const fn one_blow() -> u8 {
    1
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
    // The bar this fight fills to: the shared floor, or the faster side's
    // speed, whichever asks for more. A tick pays at most one action a side,
    // so a bar both speeds clear every tick would make the banks — not the
    // speeds — hand out the turns, and a modest edge in 速度 would shut the
    // slower fighter out of the fight almost entirely.
    let meter_full = METER_FULL
        .max(i64::from(player.speed))
        .max(i64::from(guardian.speed));
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
        let player_acts = player_meter >= meter_full
            && (player_meter >= guardian_meter || guardian_meter < meter_full);
        let guardian_acts = !player_acts && guardian_meter >= meter_full;
        if !player_acts && !guardian_acts {
            continue;
        }
        let round = if player_acts {
            player_meter -= meter_full;
            let mut round = strike(&mut player, &mut guardian, rng);
            round.attacker = player_name.clone();
            round
        } else {
            guardian_meter -= meter_full;
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

/// Draw one blow's worth out of a 外功's band, in parts per thousand of the
/// wielder's 攻击力.
///
/// The band is what makes the same 外功 worth a different number each time it is
/// thrown: a 单 hits for about its printed worth, a 连击 throws three draws at a
/// third apiece, and a 蓄力 draws once from a band twice as wide. A band of
/// `(0, 0)` is a move that pays exactly what its row says — the shape every
/// 外功 had before shapes existed.
fn strike_multiplier(strike: &ArtStrike, rng: &mut Rng) -> u32 {
    let (lo, hi) = strike.band_permille;
    if hi <= lo {
        return strike.multiplier_permille;
    }
    let span = hi - lo;
    let drawn = lo + (rng.below(u64::from(span) + 1) as u32);
    u64::from(strike.multiplier_permille)
        .saturating_mul(u64::from(drawn))
        .saturating_div(1_000) as u32
}

/// Lay a 外功's effect on the fight: the side it names keeps it from here.
///
/// An effect is *not* a second swing — it is a change to what the rest of the
/// fight reads, so a fight against a 寒劲 art is a fight against a slow hand
/// rather than against the same swing twice. Each move lays its own once, on
/// the first blow that gets through.
fn lay_effect(attacker: &mut Fighter, defender: &mut Fighter, effect: Option<StrikeEffect>) {
    let Some(effect) = effect else {
        return;
    };
    match effect {
        StrikeEffect::Slow { permille } => {
            let lost = defender.speed.saturating_mul(permille) / 1_000;
            defender.speed = defender.speed.saturating_sub(lost.max(1));
        }
        StrikeEffect::Haste { permille } => {
            let gained = attacker.speed.saturating_mul(permille) / 1_000;
            attacker.speed = attacker.speed.saturating_add(gained.max(1));
        }
        StrikeEffect::Focus { hit } => {
            attacker.hit = attacker.hit.saturating_add(hit);
        }
    }
}

/// Throw one blow.
fn strike(attacker: &mut Fighter, defender: &mut Fighter, rng: &mut Rng) -> DuelRound {
    // 命中 against 闪避, never a certainty and never hopeless — asked before
    // the blow is thrown, because a move that misses carries nothing with it.
    let chance = hit_chance_permille(attacker.hit, defender.evade);
    let dodged = rng.below(1_000) >= chance as u64;
    let mut damage = blow(attacker, defender);
    let mut art = None;
    // An 外功 is spent whenever it can be paid for: the choice to save 内力 for
    // later is not the fighter's to make, and a fighter who cannot pay falls
    // back on the bare hand rather than standing still.
    if let Some(strike) = attacker.art.clone()
        && attacker.inner >= strike.cost
    {
        attacker.inner -= strike.cost;
        let mut total = 0_u32;
        for _ in 0..strike.hits.max(1) {
            // The worth is drawn per blow rather than read off the row: two 玄阶
            // 外功 of the same worth are two different swings, and a 蓄力 is the
            // same 外功 being a different swing every time it is thrown.
            let rolled = strike_multiplier(&strike, rng);
            total = total.saturating_add(external_damage(damage, rolled));
        }
        damage = total;
        // A completed 外功 adds its mastery share of 攻击力 to every blow it
        // lands: the multiplier is what the 阶 bought, this is what finishing
        // the last one bought.
        let mastered = attacker.attack.saturating_mul(strike.mastery_permille) / 1_000;
        damage = damage.saturating_add(mastered);
        // The effect rides on the first blow that gets through, and stays. A
        // blow that slipped carries nothing: an art that laid its mark on a
        // miss would be an art that could not be dodged out of.
        if !dodged && !strike.marked {
            if let Some(held) = attacker.art.as_mut() {
                held.marked = true;
            }
            lay_effect(attacker, defender, strike.effect);
        }
        art = Some(strike.name);
    }
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

    /// Two deep fighters whose speeds both clear the shared pace: the slower
    /// one still has to be on the clock. A bar the banks alone decided once
    /// compounded a 1.6-to-1 edge in 速度 into a fourteen-to-one edge in turns
    /// — the slower side swung twice in thirty rounds, and a burst that deep
    /// ends most fights before its first turn ever comes.
    #[test]
    fn a_much_faster_fighter_still_shares_the_fight() {
        let swift = brute("swift", 1_000_000, 1, 2_336);
        let slow = brute("slow", 1_000_000, 1, 1_460);
        let report = duel(swift.clone(), slow.clone(), &mut Rng::seeded(3));
        assert_eq!(
            report.rounds.len(),
            DUEL_ROUND_CAP as usize,
            "two near-unhurtable bodies run the fight to its cap"
        );
        let swift_blows = report
            .rounds
            .iter()
            .filter(|round| round.attacker == "swift")
            .count();
        let slow_blows = report.rounds.len() - swift_blows;
        assert!(
            swift_blows > slow_blows,
            "the faster side still earns more turns: {swift_blows} vs {slow_blows}"
        );
        assert!(
            slow_blows * 4 >= report.rounds.len(),
            "1.6 to 1 in speed is not a landslide in turns: {swift_blows} vs {slow_blows}"
        );
    }

    /// One 外功 is not one number: the same swing is drawn from a band, so two
    /// throws of it are worth two different amounts.
    #[test]
    fn a_move_is_drawn_from_its_band_and_not_read_off_the_row() {
        let striker = || Fighter {
            inner: 1_000,
            art: Some(ArtStrike {
                name: "蓄力".to_owned(),
                multiplier_permille: 1_000,
                cost: 10,
                hits: 1,
                // The widest band there is: a sixth of a swing to nearly two.
                band_permille: (550, 1_700),
                effect: None,
                marked: false,
                mastery_permille: 0,
            }),
            ..brute("fighter", 10_000, 100, 10)
        };
        let mut seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for seed in 0..80u64 {
            let mut attacker = striker();
            let mut target = brute("target", 1_000_000, 0, 10);
            let round = strike(&mut attacker, &mut target, &mut Rng::seeded(seed));
            // A slipped blow is no blow at all: it says nothing about the band.
            if !round.dodged {
                seen.insert(round.damage);
            }
        }
        assert!(
            seen.len() > 5,
            "the same 外功 is the same number every time: {seen:?}"
        );
        // And the band is a *band*: nothing outside it, however many times it
        // is thrown.
        let low = 100 * 550 / 1_000;
        let high = 100 * 1_700 / 1_000;
        assert!(
            seen.iter().all(|damage| (low..=high).contains(damage)),
            "a blow landed outside its band: {seen:?} against {low}..={high}"
        );
    }

    /// A 连击 throws its worth as three blows rather than one: the same damage
    /// altogether, counted out in three.
    #[test]
    fn a_flurry_throws_three_blows() {
        let mut attacker = Fighter {
            inner: 1_000,
            art: Some(ArtStrike {
                name: "连击".to_owned(),
                multiplier_permille: 1_500,
                cost: 10,
                hits: 3,
                band_permille: (280, 340),
                effect: None,
                marked: false,
                mastery_permille: 0,
            }),
            ..brute("fighter", 10_000, 100, 10)
        };
        let mut target = brute("target", 1_000_000, 0, 10);
        let round = strike(&mut attacker, &mut target, &mut Rng::seeded(1));
        // Three draws at a third apiece over 150% of a hundred: somewhere near
        // a hundred and fifty, and never less than the three together can pay.
        assert!(
            round.damage >= 100 * 3 * 280 * 15 / 10_000,
            "a flurry pays its three: {}",
            round.damage
        );
    }

    /// An effect rides on the first blow that gets through, and only on that
    /// one: a move that laid its mark on every hit would be a move that could
    /// not be dodged out of.
    #[test]
    fn an_effect_lands_once_and_stays() {
        let mut attacker = Fighter {
            inner: 1_000,
            art: Some(ArtStrike {
                name: "寒劲".to_owned(),
                multiplier_permille: 1_000,
                cost: 10,
                hits: 1,
                band_permille: (0, 0),
                effect: Some(StrikeEffect::Slow { permille: 200 }),
                marked: false,
                mastery_permille: 0,
            }),
            ..brute("fighter", 10_000, 100, 10)
        };
        let mut target = brute("target", 1_000_000, 0, 100);
        let before = target.speed;
        strike(&mut attacker, &mut target, &mut Rng::seeded(5));
        let chilled = target.speed;
        assert!(
            chilled < before,
            "the chill bit: {chilled} against {before}"
        );
        strike(&mut attacker, &mut target, &mut Rng::seeded(6));
        assert_eq!(
            target.speed, chilled,
            "the chill does not stack: a second blow adds nothing"
        );
        assert!(
            attacker.art.as_ref().is_some_and(|held| held.marked),
            "the move remembers it has laid its mark"
        );
        // 凝神 and 轻身 are the other side of the same coin: they ride on the
        // thrower, not on the target.
        let mut attacker = Fighter {
            inner: 1_000,
            art: Some(ArtStrike {
                name: "凝神".to_owned(),
                multiplier_permille: 1_000,
                cost: 10,
                hits: 1,
                band_permille: (0, 0),
                effect: Some(StrikeEffect::Focus { hit: 80 }),
                marked: false,
                mastery_permille: 0,
            }),
            ..brute("fighter", 10_000, 100, 10)
        };
        let focus_before = attacker.hit;
        let mut target = brute("target", 1_000_000, 0, 10);
        strike(&mut attacker, &mut target, &mut Rng::seeded(7));
        assert!(
            attacker.hit > focus_before,
            "the aim steadied: {} against {}",
            attacker.hit,
            focus_before
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
                hits: 1,
                band_permille: (0, 0),
                effect: None,
                marked: false,
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
                    hits: 1,
                    band_permille: (0, 0),
                    effect: None,
                    marked: false,
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

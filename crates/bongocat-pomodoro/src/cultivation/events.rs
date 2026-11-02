//! 事件: the page's memory of what has happened.
//!
//! A popup is a moment — it is forgotten at quit, and a fight report or a walk's
//! settlement that only lived in a popup might as well not have happened. The
//! duel record and the last walk kept the two things the page used to show, but
//! a cultivator's week is more than fights and walks: sessions finish, 功法 turn
//! over, guardian falls, goods are bought and sold, crops come up. This is one
//! log for all of it, newest first, capped — a save that carried every event
//! since the first pomodoro would be a diary nobody reads.
//!
//! The log carries catalog keys and neutral values (numbers, names, whole
//! sub-records) rather than translated text: the domain never holds a sentence,
//! the surface does the saying.

use serde::{Deserialize, Serialize};

use super::ArtOutcome;
use super::Attribute;
use super::daily::DailyKind;
use super::explore::ExplorationOutcome;

/// How many events the log keeps, newest first.
///
/// Fifty is the history rather than the screenful: the page itself still shows
/// the last five, and everything older waits in the popup, which is where a
/// record of a week of sessions, walks and trades is actually read. Sixteen was
/// a few days; a save that plays daily outgrew it in a fortnight and lost the
/// beginning of its own week.
pub const EVENT_LOG_CAP: usize = 50;

/// One thing that happened, as the page's event log reads it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventRecord {
    /// When it happened.
    pub at_unix: i64,
    /// What happened.
    pub detail: EventDetail,
}

/// What one event was, told the way the surface has to say it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EventDetail {
    /// A pomodoro finished and paid its growth.
    Session {
        /// What the session paid, in growth.
        growth: u32,
    },
    /// A pomodoro finished as a gathering session and paid a site's yield.
    ///
    /// A gathering session is the other half of the mode fork: it pays the
    /// site's resources and the trade's xp, and nothing of the 修习 ladder. The
    /// line is separate from [`Self::Session`] because the two pay different
    /// things, and the history has to be able to tell them apart.
    SessionGather {
        /// The site the session worked.
        site: super::GatherSite,
        /// What the purse gained, after the 仓库's ceiling.
        gained: super::ResourceBag,
        /// The 伐木 xp the session banked.
        woodcut_xp: u32,
        /// The 采石 xp the session banked.
        stonecut_xp: u32,
    },
    /// An expedition came home and paid the site's yield.
    ///
    /// The passive half of gathering: the cultivator was *out* for the hours
    /// chosen, and the settle pays what the realm's hourly rate made over that
    /// time. Separate from [`Self::SessionGather`] because a session is a
    /// pomodoro and an expedition is a real outing — the history has to tell
    /// the two apart.
    Expedition {
        /// The site the expedition worked.
        site: super::GatherSite,
        /// The length it was out for, in hours.
        hours: u8,
        /// What the purse gained, after the 仓库's ceiling.
        gained: super::ResourceBag,
        /// The 伐木 xp the expedition banked.
        woodcut_xp: u32,
        /// The 采石 xp the expedition banked.
        stonecut_xp: u32,
    },
    /// A finished pomodoro's ordinary find: the gear or the herb the drop roll
    /// paid beside the growth.
    ///
    /// The jackpot has a line of its own ([`Self::Bonus`]); this is the ordinary
    /// one, which used to exist only in the notice above the log. A herb that
    /// arrived that way had no history at all, so a satchel could gain a seed
    /// the user was never told about.
    Found {
        /// The piece of gear it turned up, by name.
        #[serde(default)]
        gear: Option<String>,
        /// The herb it turned up, by name.
        #[serde(default)]
        seed: Option<String>,
    },
    /// A job came home and was paid for: the purse, the hours it cost, the grade
    /// that sized it, and how the work came off.
    Work {
        /// What it paid, in 灵石. The wage is the wage.
        stones: u32,
        /// How long it took, in hours.
        hours: u8,
        /// The job's grade, the way every other line names one.
        grade: super::ArtGrade,
        /// The chance the job came off at, in parts per thousand.
        success_rate: u64,
        /// The attribute the job paid, and how many points, if the gain came up.
        gain: Option<(super::DerivedSlot, u32)>,
    },
    /// One or more whole days passed without a session and were charged for.
    ///
    /// The charge is the only thing the ladder does that takes growth while
    /// nobody is looking, so it belongs in the durable log rather than only in a
    /// notice the next quit forgets. A user who wonders where a level went has
    /// to be able to read the day it went on.
    Absence {
        /// How many days the charge covered.
        days: u32,
        /// What it took, in growth. Zero when the total was already at the
        /// realm's floor, which the line still says rather than hiding.
        charged: u32,
    },
    /// A walk came back. The finds travel by name, the way the walk record has
    /// always carried them.
    Walk {
        outcome: ExplorationOutcome,
        art: Option<String>,
        gear: Option<String>,
        seed: Option<String>,
        stones: Option<u32>,
    },
    /// A finished session trained the selected 功法书 and it came to something.
    Training {
        name: String,
        outcome: ArtOutcome,
        rank: u8,
    },
    /// A finished session with no 功法 in study trained the body instead: 练功,
    /// and it paid attribute points.
    TrainBody {
        /// How many attribute points the session paid.
        points: u32,
    },
    /// The guardian was fought. A wanderer fight is part of the walk that met
    /// it, so it needs no line of its own.
    ///
    /// `stage` is the realm the win opened, so the line can name it: the page
    /// used to say "破境成功，晋入元婴" in a sticky notice above this log, and
    /// carrying the realm here is what let that notice go without the log
    /// losing the name. An old save's events predate the field and read as the
    /// bare line they always said.
    Breakthrough {
        won: bool,
        #[serde(default)]
        stage: Option<u8>,
        /// The purse the win turned up, when it turned one up.
        #[serde(default)]
        stones: Option<u32>,
        /// The piece of gear the win took off the guardian, by name.
        ///
        /// A breakthrough pays three things beside the realm — a 功法, a piece
        /// of gear and sometimes a herb — and a line that named only the purse
        /// left the bag disagreeing with the log. Older events predate the
        /// fields and read as a win that simply said less.
        #[serde(default)]
        gear: Option<String>,
        /// The 功法 the win paid, by name.
        #[serde(default)]
        art: Option<String>,
        /// The herb the win turned up, by name.
        #[serde(default)]
        seed: Option<String>,
    },
    /// A 试炼 floor was climbed. The warden is the tower's own; what the win
    /// turned up travels by name, the way a walk's finds do.
    Trial {
        floor: u16,
        won: bool,
        art: Option<String>,
        gear: Option<String>,
        seed: Option<String>,
        /// The floor's purse, when it paid one. An old save's events predate
        /// the field and read as no money at all, which is what they said.
        #[serde(default)]
        stones: Option<u32>,
    },
    /// The tower was swept: one overwhelmingly strong cultivator cleared
    /// several floors in one action, every fight skipped because none of them
    /// was a contest. What the swept floors turned up is the counts — ten floors
    /// can turn up several of a kind, and a single named find would have to drop
    /// the others on the floor.
    TrialSweep {
        from_floor: u16,
        floors: u16,
        #[serde(default)]
        seeds: u32,
        #[serde(default)]
        gear: u32,
        #[serde(default)]
        arts: u32,
        #[serde(default)]
        stones: u32,
    },
    /// A 试炼 was fought with a banked chance. A loss finds nothing but the
    /// lesson, so the three names are only ever filled by a win.
    Proving {
        won: bool,
        art: Option<String>,
        gear: Option<String>,
        seed: Option<String>,
        /// The purse the win paid, which every fight pays and this one used to
        /// keep out of the log: a win whose roll came up empty read as a win
        /// that paid nothing at all. An old save's events predate the field and
        /// read as exactly that, which is what they were.
        #[serde(default)]
        stones: Option<u32>,
    },
    /// Something was bought at the market, for this much.
    ///
    /// `stones` is what the purchase paid; an old save's events predate the
    /// field and read as zero, which the surface renders as the old bare line.
    Bought {
        what: SaleKind,
        #[serde(default)]
        stones: u32,
    },
    /// Something was sold at the market, for this much.
    Sold { what: SaleKind, stones: u32 },
    /// A crop was pulled by a save that predates the log knowing which herb it
    /// was. It is kept under the tag those saves wrote so they still load; only
    /// that old bare line is ever read from it.
    #[serde(rename = "Harvest")]
    HarvestBare,
    /// A crop was pulled: which herb, its tier, and the attribute points it paid.
    ///
    /// The name travels beside the attribute because the plot is empty once the
    /// crop is out: a line that said only "a crop came up" leaves the user with
    /// nothing to check against a garden that is now bare.
    Harvested {
        attribute: Attribute,
        /// The 潜质 herb this crop grew, when it was a herb seed rather than an
        /// attribute seed. The line reads the herb, never the `attribute`,
        /// which is a don't-care carried only to keep the shape of the record.
        #[serde(default)]
        herb: Option<super::AptitudeHerb>,
        #[serde(default)]
        tier: u8,
        /// The grade the crop was planted at, which is what its points were
        /// multiplied by. An old save's harvests predate the field and read as
        /// 地阶 — the numbers they already paid.
        #[serde(default)]
        grade: super::SeedGrade,
        #[serde(default)]
        name: Option<String>,
        /// What the harvest added to that attribute, permanently. Zero for a
        /// herb crop, which feeds the body instead.
        points: i32,
        /// What the pull added to the garden's 造化 — the currency the next rank
        /// is bought with.
        ///
        /// A harvest pays twice over: the points go into the body, and the same
        /// number becomes the garden's own progress. The line used to print only
        /// the first, which left a garden bar that had moved with nothing in the
        /// log saying why. Older harvests predate the field and read as a pull
        /// that banked nothing, which the surface says by not mentioning it.
        #[serde(default)]
        xp: i32,
        /// The seed the pull gave back, by name.
        ///
        /// A harvest rolls for a seed of its own, and that roll is the only
        /// thing keeping a garden from running dry — so it belongs on the same
        /// line as the points, not only in the notice the next quit forgets.
        /// Older harvests predate the field and read as a pull that gave none.
        #[serde(default)]
        seed: Option<String>,
    },
    /// A planting did not come up: the seed went into the ground and died there.
    ///
    /// The garden used to have nothing to report but successes, which is a log
    /// that quietly disagrees with the satchel — a seed is gone and no line
    /// says where to. The odds ride along because "it failed" and "it failed at
    /// ninety-five percent" are not the same news, and the failure still taught
    /// something, so its 造化 does too.
    Withered {
        attribute: Attribute,
        /// The 潜质 herb the seed would have grown, when it was a herb seed; a
        /// herb seed that dies reads as the herb, never as its `attribute`.
        #[serde(default)]
        herb: Option<super::AptitudeHerb>,
        #[serde(default)]
        tier: u8,
        #[serde(default)]
        grade: super::SeedGrade,
        #[serde(default)]
        name: Option<String>,
        /// The odds it went in at, in permille.
        #[serde(default)]
        permille: u32,
        /// The 造化 a dead seedling still pays: see [`super::wither_points`].
        #[serde(default)]
        xp: i32,
    },
    /// The garden was raised a rank, for this much 灵石.
    GardenRank {
        /// The rank it stands at now.
        rank: u8,
        /// What the rank cost, in 灵石.
        #[serde(default)]
        stones: u32,
    },
    /// The day opened with something: an ambush, a gift, or a coin in the dust.
    ///
    /// One line for all three because it is one thing that happened to the
    /// user, not three unrelated records: the kind says which, and the finds
    /// travel beside it the way a walk's do.
    Daily {
        kind: DailyKind,
        /// Whether the ambush was beaten. True for the two kinds that are not
        /// a fight, so the line never has to ask twice.
        won: bool,
        #[serde(default)]
        art: Option<String>,
        #[serde(default)]
        gear: Option<String>,
        #[serde(default)]
        seed: Option<String>,
        #[serde(default)]
        stones: Option<u32>,
    },
    /// A star was attempted on a piece of gear, and came off or did not.
    ///
    /// One line for both directions: the attempt is the event, and a failure
    /// that left no line would make a save's stone purse disagree with its own
    /// history. The cost travels with it because the stones are spent either
    /// way.
    Star {
        /// The piece's name, the way every other line names a thing.
        name: String,
        /// The star the piece stands at now.
        star: u8,
        /// Whether the attempt came off.
        success: bool,
        /// What the attempt cost: the 灵石 wager and the resources the forge
        /// burned. A star is the same event whatever it cost, but a line that
        /// says a star was won and not what it cost would let the purse disagree
        /// with its own history.
        #[serde(default)]
        cost: super::Cost,
    },
    /// A building was raised a rank.
    BuildingUpgrade {
        /// The building raised.
        building: super::Building,
        /// The rank it stands at now.
        level: u8,
        /// What the rank cost.
        #[serde(default)]
        cost: super::Cost,
    },
    /// 潜质 moved: a herb was fed and a gate was paid.
    ///
    /// `gained_tenths` is the move the user saw, not the roll: a herb at the top
    /// of the band pays less than its band, and a line that printed the roll
    /// would disagree with the number beside it.
    Aptitude {
        herb: super::AptitudeHerb,
        gained_tenths: i32,
        /// Where 潜质 stands after the move, in tenths.
        value_tenths: i32,
        /// How many 洗髓丹 the move crossed.
        #[serde(default)]
        marrow: u32,
    },
    /// A 洗髓丹 was found, and where.
    MarrowFound {
        /// The catalog key of the source: a fight, a walk, or the day's event.
        source: MarrowSource,
    },
    /// 千分之一: one finished pomodoro paid something beside its growth — a
    /// purse, a herb, a book or a piece of gear, in whatever shape the roll took.
    ///
    /// A line of its own rather than a note on the session, because it is the
    /// rarer of the two things the same pomodoro did and the one a user will go
    /// looking for later.
    Bonus { find: BonusFind },
}

/// What one 千分之一 paid, in the shape it came.
///
/// The same five shapes a session's drop can take, told in the terms the log
/// already speaks: a name where the thing has one, and the numbers a herb needs
/// to be planted or priced.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BonusFind {
    Equipment {
        name: Option<String>,
    },
    Seed {
        attribute: Attribute,
        tier: u8,
        grade: super::SeedGrade,
    },
    Art {
        name: String,
    },
    Herb {
        herb: super::AptitudeHerb,
    },
    Stones {
        stones: u32,
    },
}

/// Where a 洗髓丹 came from, as the log says it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarrowSource {
    /// A fight the cultivator won.
    Combat,
    /// A walk, or the morning event.
    Fortune,
    /// Bought off the shelf — the only channel that names a price.
    Shop,
}

/// Something sold or bought, named the way the log can carry it.
///
/// A name when the thing has one — most gear and every 功法 does — and the kind
/// to fall back on when it does not, because "sold 装备" beats a blank. A seed
/// carries its tier beside the name, because a purchase that says "a seed"
/// without saying which tier bought what is a line the buyer cannot check
/// against the price; an old save's seeds predate the field and read as zero.
///
/// Each variant also carries the thing's own numbers where they can be had —
/// the four attributes a garment adds, the attribute a herb raises, the id of a
/// book — so a purchase line can say what was gained rather than only what was
/// paid. They are optional because an old save's events predate them, and a
/// line that cannot say more says exactly what it used to.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SaleKind {
    Equipment {
        name: Option<String>,
        /// The four attributes the piece carried, star multiplier and all.
        #[serde(default)]
        attributes: Option<super::Attributes>,
    },
    Seed {
        name: Option<String>,
        #[serde(default)]
        tier: u8,
        /// The herb's grade. An old save's sales predate the field and read as
        /// 地阶, the grade their numbers were the anchor for.
        #[serde(default)]
        grade: super::SeedGrade,
        /// Which attribute the herb raises when it is harvested.
        #[serde(default)]
        attribute: Option<super::Attribute>,
    },
    Art {
        name: Option<String>,
        /// The book's id, so the line can print what one 阶 of it pays.
        #[serde(default)]
        art_id: Option<u64>,
    },
    /// A garden plot bought past the four free ones.
    Plot,
    /// One 潜质 item: 悟心草、慧心草 or 洗髓丹.
    Herb { herb: super::AptitudeHerb },
}

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
use super::daily::DailyKind;
use super::explore::ExplorationOutcome;

/// How many events the log keeps, newest first.
///
/// Sixteen is a screenful of lines and a few days of play: long enough that a
/// session, a walk and a sale all stay visible together, short enough that the
/// page reads at a glance.
pub const EVENT_LOG_CAP: usize = 16;

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
    /// A crop was pulled.
    Harvest,
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
}

/// Something sold or bought, named the way the log can carry it.
///
/// A name when the thing has one — most gear and every 功法 does — and the kind
/// to fall back on when it does not, because "sold 装备" beats a blank. A seed
/// carries its tier beside the name, because a purchase that says "a seed"
/// without saying which tier bought what is a line the buyer cannot check
/// against the price; an old save's seeds predate the field and read as zero.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SaleKind {
    Equipment {
        name: Option<String>,
    },
    Seed {
        name: Option<String>,
        #[serde(default)]
        tier: u8,
    },
    Art {
        name: Option<String>,
    },
    /// A garden plot bought past the four free ones.
    Plot,
}

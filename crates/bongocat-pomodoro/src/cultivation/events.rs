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
    Breakthrough { won: bool },
    /// Something was bought at the market.
    Bought { what: SaleKind },
    /// Something was sold at the market, for this much.
    Sold { what: SaleKind, stones: u32 },
    /// A crop was pulled.
    Harvest,
}

/// Something sold or bought, named the way the log can carry it.
///
/// A name when the thing has one — most gear and every 功法 does — and the kind
/// to fall back on when it does not, because "sold 装备" beats a blank.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SaleKind {
    Equipment { name: Option<String> },
    Seed { name: Option<String> },
    Art { name: Option<String> },
}

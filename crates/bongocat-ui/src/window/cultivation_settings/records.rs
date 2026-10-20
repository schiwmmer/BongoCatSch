//! The event log: the last sixteen things that happened.

use super::*;

/// The page's log of what has happened, newest first.
///
/// One line per event and nothing more: sessions, walks and their finds, 功法
/// turning over, goods bought and sold, crops pulled. The popups that showed
/// these are forgotten at quit, so this is the only place a save can be asked
/// what it has been through — and a line that only kept the fights would be a
/// flatterer's history of a cultivator's week.
pub(super) fn record_section(
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut column = div()
        .id("cultivation-record")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.record.title", tokens))
        // Marked so a frame can be asked whether the record is on screen.
        .test_support();
    if cultivation.events.is_empty() {
        return column.child(
            div()
                .id("cultivation-record-empty")
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.record.empty"))
                .test_support(),
        );
    }
    for (index, event) in cultivation.events.iter().enumerate() {
        column = column.child(
            div()
                .id(format!("cultivation-event-{index}"))
                .text_xs()
                .text_color(tokens.muted)
                .child(event_line(&event.detail, locale))
                .test_support(),
        );
    }
    column
}

/// One event, as a line the user reads.
fn event_line(detail: &EventDetail, locale: &str) -> String {
    match detail {
        EventDetail::Session { growth } => format_text(
            locale,
            "cultivation.event.session",
            &[("growth", growth.to_string())],
        ),
        EventDetail::Walk {
            outcome,
            art,
            gear,
            seed,
            stones,
        } => walk_line(
            *outcome,
            art.as_deref(),
            gear.as_deref(),
            seed.as_deref(),
            *stones,
            locale,
        ),
        EventDetail::Training {
            name,
            outcome,
            rank,
        } => {
            let key = match outcome {
                ArtOutcome::Failed => "cultivation.event.train_failed",
                ArtOutcome::Progressed => "cultivation.event.train_progress",
                ArtOutcome::RankedUp => "cultivation.event.train_ranked",
                ArtOutcome::Mastered => "cultivation.event.train_mastered",
            };
            format_text(
                locale,
                key,
                &[("name", name.clone()), ("rank", rank.to_string())],
            )
        }
        EventDetail::Breakthrough { won } => text(
            locale,
            if *won {
                "cultivation.event.breakthrough_won"
            } else {
                "cultivation.event.breakthrough_lost"
            },
        )
        .to_string(),
        EventDetail::Bought { what } => format_text(
            locale,
            "cultivation.event.bought",
            &[("what", sale_label(what, locale))],
        ),
        EventDetail::Sold { what, stones } => format_text(
            locale,
            "cultivation.event.sold",
            &[
                ("what", sale_label(what, locale)),
                ("stones", stones.to_string()),
            ],
        ),
        EventDetail::Harvest => text(locale, "cultivation.event.harvest").to_string(),
    }
}

/// What a sold or bought thing is called: its name when it has one, the kind
/// when it does not.
fn sale_label(what: &SaleKind, locale: &str) -> String {
    let (name, kind_key) = match what {
        SaleKind::Equipment { name } => (name.clone(), "cultivation.shop.equipment"),
        SaleKind::Seed { name } => (name.clone(), "cultivation.shop.seed"),
        SaleKind::Art { name } => (name.clone(), "cultivation.shop.art"),
    };
    name.unwrap_or_else(|| text(locale, kind_key).to_string())
}

/// One walk's settlement, as a single line: how it ended and what it turned up.
///
/// A walk can pay three things at once — and the 灵石 beside them, which is
/// rolled beside the finding rather than in place of it — so the line names each
/// in turn rather than saying "something was found". A record that does not say
/// what was found is not worth keeping. The outcome is read through the
/// record's own keys: the popup's outcome sentences carry a `%{name}` or a
/// `%{value}` for the surface to fill, and a history line that names the finds
/// beside the outcome would print the placeholder unfilled.
fn walk_line(
    outcome: ExplorationOutcome,
    art: Option<&str>,
    gear: Option<&str>,
    seed: Option<&str>,
    stones: Option<u32>,
    locale: &str,
) -> String {
    let mut line = format_text(
        locale,
        "cultivation.event.walk",
        &[(
            "outcome",
            text(locale, outcome.record_outcome_key()).to_string(),
        )],
    );
    for found in [
        art.map(|name| ("cultivation.record.found_art", name)),
        gear.map(|name| ("cultivation.record.found_gear", name)),
        seed.map(|name| ("cultivation.record.found_seed", name)),
    ]
    .into_iter()
    .flatten()
    {
        line.push_str(&format!(
            " · {}",
            format_text(locale, found.0, &[("name", found.1.to_owned())])
        ));
    }
    if let Some(amount) = stones {
        line.push_str(&format!(
            " · {}",
            format_text(
                locale,
                "cultivation.record.found_stones",
                &[("value", amount.to_string())]
            )
        ));
    }
    line
}

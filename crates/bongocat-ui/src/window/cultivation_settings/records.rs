//! The event log: the last five on the page, the full sixteen in the popup.

use super::*;
use std::rc::Rc;

/// How many events the page lists before the popup takes over.
///
/// Five is what fits between the notices and the attributes panel without
/// pushing the rest of the page off the first screenful; everything older is
/// one click away rather than always on the page.
const VISIBLE_EVENT_COUNT: usize = 5;

/// How wide the history popup is drawn.
const EVENTS_PANEL_WIDTH: f32 = 420.0;

/// How tall the history list may grow before it scrolls.
const EVENTS_LIST_MAX_HEIGHT: f32 = 360.0;

/// What closing the history popup does.
pub(in crate::window) type EventsClose = Rc<dyn Fn(&mut Window, &mut App)>;

/// The page's log of what has happened, newest first.
///
/// The last five events sit on the page and the rest hang behind the
/// "show all" control: a week of sessions, walks and trades is a wall of text
/// on a page that is already a ladder, while a history nobody can open is a
/// record that only kept what fit. The popups that showed these are forgotten
/// at quit, so this is the only place a save can be asked what it has been
/// through — and a line that only kept the fights would be a flatterer's
/// history of a cultivator's week.
pub(super) fn record_section(
    view: &Entity<SettingsView>,
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
    for (index, event) in cultivation
        .events
        .iter()
        .take(VISIBLE_EVENT_COUNT)
        .enumerate()
    {
        column = column.child(
            div()
                .id(format!("cultivation-event-{index}"))
                .text_xs()
                .text_color(tokens.muted)
                .child(event_line(&event.detail, locale))
                .test_support(),
        );
    }
    let total = cultivation.events.len();
    if total > VISIBLE_EVENT_COUNT {
        column = column.child(
            div()
                .id("cultivation-record-show-all")
                .text_xs()
                .text_color(tokens.accent)
                .child(format_text(
                    locale,
                    "cultivation.record.show_all",
                    &[("count", total.to_string())],
                ))
                .test_support()
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.cultivation_events_open = true;
                            cx.notify();
                        });
                    }
                }),
        );
    }
    column
}

/// The full history, drawn over the settings surface.
///
/// The same layout as the fight popup: a backdrop that closes on a click of
/// its own, a panel that occludes the pointer so a click inside never reaches
/// the backdrop, and a list that scrolls once it outgrows the window. Every
/// event the save still remembers is in the list, oldest lines at the bottom,
/// because a history that hides its own beginning is not full.
pub(in crate::window) fn events_overlay(
    events: &[EventRecord],
    locale: &str,
    tokens: Tokens,
    on_close: EventsClose,
) -> gpui_kit::AnyElement {
    let close_from_backdrop = on_close.clone();
    let mut list = div()
        .id("cultivation-events-list")
        .flex()
        .flex_col()
        .gap_1()
        .min_h(px(48.0))
        .max_h(px(EVENTS_LIST_MAX_HEIGHT))
        .overflow_y_scroll()
        .p_2()
        .rounded_md()
        .bg(tokens.overlay)
        .test_support();
    if events.is_empty() {
        list = list.child(
            div()
                .id("cultivation-history-empty")
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.record.empty"))
                .test_support(),
        );
    }
    for (index, event) in events.iter().enumerate() {
        list = list.child(
            div()
                .id(format!("cultivation-history-event-{index}"))
                .text_xs()
                .text_color(tokens.muted)
                .child(event_line(&event.detail, locale))
                .test_support(),
        );
    }
    div()
        .id("cultivation-events-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .bg(tokens.overlay)
        .on_click(move |_, window, cx| (close_from_backdrop)(window, cx))
        .child(
            div()
                .id("cultivation-events-panel")
                .w(px(EVENTS_PANEL_WIDTH))
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_lg()
                .bg(tokens.canvas)
                .border_1()
                .border_color(tokens.border)
                .occlude()
                .child(
                    div()
                        .id("cultivation-events-header")
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(tokens.text)
                                .child(text(locale, "cultivation.record.history")),
                        )
                        .child(
                            Button::new("cultivation-events-close")
                                .ghost()
                                .small()
                                .label(text(locale, "cultivation.duel.close"))
                                .on_click(move |_, window, cx| (on_close)(window, cx)),
                        ),
                )
                .child(list)
                .test_support(),
        )
        .into_any_element()
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

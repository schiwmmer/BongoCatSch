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
    // The offset is read once per render rather than once per line: every line
    // on the page belongs to the same local clock.
    let offset = bongocat_platform::local_utc_offset_seconds();
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
                .child(event_line_with_stamp(event, locale, offset))
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
                .child(event_line_with_stamp(
                    event,
                    locale,
                    bongocat_platform::local_utc_offset_seconds(),
                ))
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

/// One event as the page prints it: the local instant it happened, then the line.
///
/// The record outlives the day it was written on, so a list without its clocks
/// reads as one undated column: "the herb I pulled yesterday, or the one from
/// this morning" is a question only the stamp answers. The instant comes from
/// the same calendar the rest of the product reads, so a line written at 23:50
/// and one written at 00:10 are two different local days here as they are
/// everywhere else.
fn event_line_with_stamp(event: &EventRecord, locale: &str, offset_seconds: i32) -> String {
    format!(
        "{} {}",
        crate::pomodoro_window::view::format_local_timestamp(event.at_unix, offset_seconds),
        event_line(&event.detail, locale)
    )
}

/// One event, as a line the user reads.
fn event_line(detail: &EventDetail, locale: &str) -> String {
    match detail {
        EventDetail::Session { growth } => format_text(
            locale,
            "cultivation.event.session",
            &[("growth", growth.to_string())],
        ),
        EventDetail::Absence { days, charged } => format_text(
            locale,
            "cultivation.event.absence",
            &[("days", days.to_string()), ("charged", charged.to_string())],
        ),
        EventDetail::Work {
            stones,
            hours,
            grade,
            multiplier,
        } => format_text(
            locale,
            "cultivation.event.work",
            &[
                ("stones", stones.to_string()),
                ("hours", hours.to_string()),
                ("grade", text(locale, grade.name_key()).to_string()),
                ("multiplier", multiplier.to_string()),
            ],
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
        EventDetail::Aptitude {
            herb,
            gained_tenths,
            value_tenths,
            marrow,
        } => {
            let mut line = format_text(
                locale,
                "cultivation.event.aptitude",
                &[
                    ("herb", text(locale, herb.name_key()).to_string()),
                    ("gain", super::tenths_text(*gained_tenths)),
                    ("value", super::tenths_text(*value_tenths)),
                ],
            );
            if *marrow > 0 {
                line = format_text(
                    locale,
                    "cultivation.event.aptitude_gate",
                    &[("line", line), ("pills", marrow.to_string())],
                );
            }
            line
        }
        EventDetail::MarrowFound { source } => {
            let key = match source {
                MarrowSource::Combat => "cultivation.marrow_source.combat",
                MarrowSource::Fortune => "cultivation.marrow_source.fortune",
                MarrowSource::Shop => "cultivation.marrow_source.shop",
            };
            format_text(
                locale,
                "cultivation.event.marrow_found",
                &[("source", text(locale, key).to_string())],
            )
        }
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
        EventDetail::Breakthrough {
            won,
            stage,
            stones,
            gear,
            art,
            seed,
        } => {
            // A win names the realm it opened, the way the sticky notice above
            // this log used to: the notice is gone because the log is the same
            // line said twice, but the realm it named is not lost.
            let mut line = match (*won, *stage) {
                (true, Some(stage)) => format_text(
                    locale,
                    "cultivation.breakthrough.won",
                    &[(
                        "stage",
                        text(locale, Realm::from_index(usize::from(stage)).name_key()).to_string(),
                    )],
                ),
                _ => text(
                    locale,
                    if *won {
                        "cultivation.event.breakthrough_won"
                    } else {
                        "cultivation.event.breakthrough_lost"
                    },
                )
                .to_string(),
            };
            // Everything a won realm handed over rides on the line: the 功法 it
            // always pays, the gear it always pays, the herb it sometimes pays
            // and the purse on top. A log that named only the realm left the
            // bag and the history disagreeing about what the fight was worth.
            if *won {
                push_finds(
                    &mut line,
                    art.as_deref(),
                    gear.as_deref(),
                    seed.as_deref(),
                    locale,
                );
            }
            push_stones(&mut line, *stones, locale);
            line
        }
        EventDetail::Trial {
            floor,
            won,
            art,
            gear,
            seed,
            stones,
        } => trial_line(
            *floor,
            *won,
            art.as_deref(),
            gear.as_deref(),
            seed.as_deref(),
            *stones,
            locale,
        ),
        EventDetail::Proving {
            won,
            art,
            gear,
            seed,
        } => {
            let mut line = format_text(
                locale,
                if *won {
                    "cultivation.proving.won"
                } else {
                    "cultivation.proving.lost"
                },
                &[],
            );
            if *won {
                push_finds(
                    &mut line,
                    art.as_deref(),
                    gear.as_deref(),
                    seed.as_deref(),
                    locale,
                );
            }
            line
        }
        EventDetail::Daily {
            kind,
            won,
            art,
            gear,
            seed,
            stones,
        } => {
            // One line for the whole morning: how the day opened, then what it
            // left behind. A lost ambush says so instead of naming the attacker,
            // because the attacker is a 游荡者 every time.
            let key = match (kind, won) {
                (DailyKind::Ambush, true) => "cultivation.daily.ambush",
                (DailyKind::Ambush, false) => "cultivation.daily.ambush_lost",
                (DailyKind::Gift, _) => "cultivation.daily.gift",
                (DailyKind::Trinket, _) => "cultivation.daily.trinket",
            };
            let mut line = text(locale, key).to_string();
            push_finds(
                &mut line,
                art.as_deref(),
                gear.as_deref(),
                seed.as_deref(),
                locale,
            );
            push_stones(&mut line, *stones, locale);
            line
        }
        EventDetail::Bought { what, stones } => {
            // An old save's purchases predate the price and read as zero; the
            // bare line is what those events said, so that is what they keep
            // saying rather than "paid 0".
            let what = sale_line_label(what, locale);
            if *stones > 0 {
                format_text(
                    locale,
                    "cultivation.event.bought",
                    &[("what", what), ("stones", stones.to_string())],
                )
            } else {
                format_text(locale, "cultivation.event.bought_bare", &[("what", what)])
            }
        }
        EventDetail::Sold { what, stones } => format_text(
            locale,
            "cultivation.event.sold",
            &[
                ("what", sale_line_label(what, locale)),
                ("stones", stones.to_string()),
            ],
        ),
        // One line for the wager either way: the attempt is the event, and a
        // failure that left no line would make the purse disagree with its own
        // history. The cost rides with it because the stones are spent either
        // way.
        EventDetail::Star {
            name,
            star,
            success,
            cost,
        } => {
            let key = if *success {
                "cultivation.event.star_success"
            } else {
                "cultivation.event.star_failed"
            };
            let stars = "★".repeat((*star).min(MAX_STAR) as usize);
            format_text(
                locale,
                key,
                &[
                    ("name", name.clone()),
                    ("star", star.to_string()),
                    ("stars", stars),
                    ("stones", cost.to_string()),
                ],
            )
        }
        // A crop pulled by a save from before the log knew which herb it was:
        // the bare line is what those events said, so that is what they keep
        // saying rather than a line with a blank where the name would go.
        EventDetail::HarvestBare => text(locale, "cultivation.event.harvest").to_string(),
        EventDetail::Harvested {
            attribute,
            tier,
            grade,
            name,
            points,
            seed,
        } => {
            let mut line = format_text(
                locale,
                "cultivation.event.harvested",
                &[
                    (
                        "seed",
                        seed_label(locale, *attribute, *tier, *grade, name.as_deref()),
                    ),
                    ("attribute", text(locale, attribute.name_key()).to_string()),
                    ("points", points.to_string()),
                ],
            );
            // The seed a pull gives back is half of what a harvest is worth,
            // and it used to live only in the notice the next quit forgot.
            if let Some(returned) = seed {
                line.push_str(&format!(
                    " · {}",
                    format_text(
                        locale,
                        "cultivation.notice.seed_return",
                        &[("seed", returned.clone())]
                    )
                ));
            }
            line
        }
        // 千分之一: what the pomodoro paid beside its growth. One line, and it
        // names the shape the luck took rather than saying "something".
        EventDetail::Bonus { find } => format_text(
            locale,
            "cultivation.event.bonus",
            &[("find", bonus_find_label(find, locale))],
        ),
    }
}

/// What one 千分之一 handed over, as a phrase a line can carry.
fn bonus_find_label(find: &BonusFind, locale: &str) -> String {
    match find {
        BonusFind::Equipment { name } => name
            .clone()
            .unwrap_or_else(|| text(locale, "cultivation.shop.equipment").to_string()),
        BonusFind::Seed {
            attribute,
            tier,
            grade,
        } => seed_label(locale, *attribute, *tier, *grade, None),
        BonusFind::Art { name } => name.clone(),
        BonusFind::Herb { herb } => text(locale, herb.name_key()).to_string(),
        BonusFind::Stones { stones } => format_text(
            locale,
            "cultivation.event.bonus_stones",
            &[("stones", stones.to_string())],
        ),
    }
}

/// What a sold or bought thing is called: its name when it has one, the kind
/// when it does not.
///
/// A seed says which tier it is beside its name, because "bought a seed" is a
/// line the buyer cannot check against the price; an old save's seeds predate
/// the tier and fall back to what they always said.
fn sale_label(what: &SaleKind, locale: &str) -> String {
    match what {
        SaleKind::Equipment { name, .. } => name
            .clone()
            .unwrap_or_else(|| text(locale, "cultivation.shop.equipment").to_string()),
        SaleKind::Seed {
            name, tier, grade, ..
        } => {
            let grade_name = text(locale, grade.name_key()).to_string();
            match (name.as_deref(), *tier) {
                (Some(name), tier) if tier > 0 => format_text(
                    locale,
                    "cultivation.seed.label_named",
                    &[
                        ("name", name.to_owned()),
                        ("tier", tier.to_string()),
                        ("grade", grade_name),
                    ],
                ),
                (None, tier) if tier > 0 => format_text(
                    locale,
                    "cultivation.shop.seed_tier",
                    &[("tier", tier.to_string()), ("grade", grade_name)],
                ),
                (Some(name), _) => name.to_owned(),
                (None, _) => text(locale, "cultivation.shop.seed").to_string(),
            }
        }
        SaleKind::Art { name, .. } => name
            .clone()
            .unwrap_or_else(|| text(locale, "cultivation.shop.art").to_string()),
        SaleKind::Plot => text(locale, "cultivation.shop.plot").to_string(),
        SaleKind::Herb { herb } => text(locale, herb.name_key()).to_string(),
    }
}

/// A bought or sold thing with what it is worth beside its name.
///
/// The name alone answers "what did I pay for", and the user's next question is
/// always "and what did I get": a herb's name says which attribute it raises
/// only if the reader already knows the catalog, and a garment's name says
/// nothing about its numbers. So the thing's own figures ride in brackets after
/// the name — the same figures the shelf and the satchel print, read through the
/// same catalog.
fn sale_line_label(what: &SaleKind, locale: &str) -> String {
    let name = sale_label(what, locale);
    match sale_effect(what, locale) {
        Some(effect) => format_text(
            locale,
            "cultivation.event.with_effect",
            &[("what", name), ("effect", effect)],
        ),
        None => name,
    }
}

/// What a bought or sold thing is worth, as a phrase a line can carry.
///
/// `None` where the event predates the field or the thing has no numbers: an old
/// save's purchase line then reads exactly as it did before.
fn sale_effect(what: &SaleKind, locale: &str) -> Option<String> {
    match what {
        SaleKind::Equipment {
            attributes: Some(attributes),
            ..
        } => Some(format_text(
            locale,
            "cultivation.item.details",
            &[
                ("root", attributes.root.to_string()),
                ("strength", attributes.strength.to_string()),
                ("mana", attributes.mana.to_string()),
                ("agility", attributes.agility.to_string()),
            ],
        )),
        SaleKind::Seed {
            attribute: Some(attribute),
            tier,
            grade,
            ..
        } => Some(format_text(
            locale,
            "cultivation.seed.harvest_line",
            &[
                ("attribute", text(locale, attribute.name_key()).to_string()),
                ("value", harvest_points(*tier, *grade).to_string()),
            ],
        )),
        SaleKind::Art {
            art_id: Some(art_id),
            ..
        } => {
            let art = blueprint(*art_id);
            Some(super::shop::art_numbers(
                art.kind,
                &art.gain,
                &art.bonus,
                art.multiplier_permille,
                art.cost,
                locale,
            ))
        }
        SaleKind::Herb { herb } => Some(super::shop::herb_effect(*herb, locale)),
        _ => None,
    }
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
    push_finds(&mut line, art, gear, seed, locale);
    push_stones(&mut line, stones, locale);
    line
}

/// One 试炼 floor, as a single line: whether it was cleared and what it paid.
///
/// The finds are named beside the floor for the same reason a walk names them:
/// the tower pays the three kinds a walk does, and a line that said only "floor
/// nine cleared" would leave the user guessing what came of it.
fn trial_line(
    floor: u16,
    won: bool,
    art: Option<&str>,
    gear: Option<&str>,
    seed: Option<&str>,
    stones: Option<u32>,
    locale: &str,
) -> String {
    let mut line = format_text(
        locale,
        if won {
            "cultivation.trial.won"
        } else {
            "cultivation.trial.lost"
        },
        &[("floor", floor.to_string())],
    );
    if won {
        push_finds(&mut line, art, gear, seed, locale);
        push_stones(&mut line, stones, locale);
    }
    line
}

/// Name the purse a win turned up, when it turned one up.
///
/// Money is not a find — it rides beside one — so it is a segment of its own on
/// the same line rather than a fourth kind of thing to have found.
fn push_stones(line: &mut String, stones: Option<u32>, locale: &str) {
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
}

/// Name what a walk or a floor turned up, one segment per find.
///
/// Shared by the two lines that can pay all three kinds at once, so the wording
/// of "found an art" cannot drift between them: one find, one sentence.
fn push_finds(
    line: &mut String,
    art: Option<&str>,
    gear: Option<&str>,
    seed: Option<&str>,
    locale: &str,
) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line carries the instant it happened, in the local calendar the
    /// task rows use: a record that outlives the day it was written on is a
    /// list of undated lines without it.
    #[test]
    fn a_line_says_when_it_happened_before_what_happened() {
        let event = EventRecord {
            at_unix: 0,
            detail: EventDetail::Session { growth: 500 },
        };
        let line = event_line_with_stamp(&event, "zh-CN", 0);
        assert!(
            line.starts_with("1970-01-01 00:00 "),
            "the stamp leads the line: {line}"
        );
        assert!(line.ends_with("成长 +500"), "and the event follows: {line}");
    }

    /// A harvest names the seed it gave back, not only the points it paid: the
    /// returned herb is the half of a pull that keeps a garden going, and it
    /// used to exist only in a notice the next quit forgot.
    #[test]
    fn a_harvest_names_the_seed_it_gave_back() {
        let detail = EventDetail::Harvested {
            attribute: Attribute::Mana,
            tier: 3,
            grade: SeedGrade::Earth,
            name: Some("青芝".to_owned()),
            points: 10,
            seed: Some("云芝".to_owned()),
        };
        let line = event_line(&detail, "zh-CN");
        assert!(line.contains("青芝"), "the crop it pulled: {line}");
        assert!(line.contains("云芝"), "and the seed it gave back: {line}");
    }

    /// A won realm names everything it handed over — 功法, gear, herb and purse
    /// — because a bag that disagrees with the log is a bag the user cannot
    /// check.
    #[test]
    fn a_breakthrough_names_the_finds_it_paid() {
        let detail = EventDetail::Breakthrough {
            won: true,
            stage: Some(1),
            stones: Some(40),
            gear: Some("青锋".to_owned()),
            art: Some("玄冰诀".to_owned()),
            seed: Some("云芝".to_owned()),
        };
        let line = event_line(&detail, "zh-CN");
        for name in ["玄冰诀", "青锋", "云芝", "40"] {
            assert!(line.contains(name), "{name} is on the line: {line}");
        }
    }

    /// A bought herb says what a harvest of it pays — the attribute and the
    /// points — beside its name: a purchase line that only named the herb left
    /// the user to work the numbers out from the grade.
    #[test]
    fn a_bought_seed_says_what_a_harvest_pays() {
        let what = SaleKind::Seed {
            name: Some("青芝".to_owned()),
            tier: 3,
            grade: SeedGrade::Earth,
            attribute: Some(Attribute::Mana),
        };
        let line = sale_line_label(&what, "zh-CN");
        assert!(line.contains("青芝"), "the herb is named: {line}");
        assert!(line.contains("法力"), "the attribute it raises: {line}");
        assert!(
            line.contains(&harvest_points(3, SeedGrade::Earth).to_string()),
            "and how much: {line}"
        );
    }

    /// A bought garment prints its four numbers, read off the same line the
    /// satchel prints them on.
    #[test]
    fn a_bought_garment_prints_its_attributes() {
        let what = SaleKind::Equipment {
            name: Some("风矛".to_owned()),
            attributes: Some(Attributes::new(3, 5, 0, 2)),
        };
        let line = sale_line_label(&what, "zh-CN");
        assert!(line.contains("风矛"), "the piece is named: {line}");
        for value in ["3", "5", "2"] {
            assert!(line.contains(value), "an attribute reads {value}: {line}");
        }
    }

    /// An old save's purchase carried none of this, and its line still reads as
    /// the bare name it always did.
    #[test]
    fn an_old_purchase_line_keeps_its_bare_name() {
        let what = SaleKind::Seed {
            name: Some("白虎茸".to_owned()),
            tier: 0,
            grade: SeedGrade::Earth,
            attribute: None,
        };
        assert_eq!(sale_line_label(&what, "zh-CN"), "白虎茸");
        let old = SaleKind::Equipment {
            name: Some("玄铁剑".to_owned()),
            attributes: None,
        };
        assert_eq!(sale_line_label(&old, "zh-CN"), "玄铁剑");
    }
}

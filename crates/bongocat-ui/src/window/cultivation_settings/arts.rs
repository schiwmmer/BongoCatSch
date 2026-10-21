//! The 功法 shelf: the book being trained, the arts already learned,
//! and selling a tome for 灵石.

use super::*;

/// The 功法书 the cultivator is still learning from, and which one the next
/// pomodoro trains.
///
/// Selecting is the whole interaction: the training itself happens when a
/// session finishes, so the row says how far along it is and what the next 阶
/// asks for rather than offering a button that does the learning.
pub(super) fn arts_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-arts")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.arts.title", tokens));
    if cultivation.arts.is_empty() {
        return panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.arts.empty")),
        );
    }
    panel = panel.children(
        cultivation
            .arts
            .iter()
            .map(|art| art_row(view, art, locale, tokens)),
    );
    panel
}

/// The 功法 the cultivator has learned outright.
///
/// A separate panel because it is a separate thing: a book is still being
/// trained and can be sold, while a learned 功法 is kept for good and pays its
/// mastery for as long as the save lives. Putting them in one list would make
/// the second look like the first.
pub(super) fn mastered_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-mastered-arts")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.mastered.title", tokens))
        // Marked so a frame can be asked whether the panel is on screen.
        .test_support();
    if cultivation.mastered_arts.is_empty() {
        return panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.mastered.empty")),
        );
    }
    panel = panel.children(
        cultivation
            .mastered_arts
            .iter()
            .map(|art| mastered_art_row(view, art, locale, tokens)),
    );
    panel
}

/// One learned 功法, as one line: name and grade on the left, what it pays on
/// the right, and the tome's sale beside them.
///
/// Selling a learned 功法 sells the tome, not the knowledge: nothing it pays
/// is ever taken away, so the row keeps drawing the same numbers after the
/// sale — and the sale happens once, so a collected tome reads 已售 instead of
/// a price.
fn mastered_art_row(
    view: &Entity<SettingsView>,
    art: &MasteredArtView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let grade = text(locale, art.grade.name_key());
    let head = format!("{} · {}", art.name, grade);
    let payoff = mastered_art_payoff(art, locale);
    let art_id = art.id;
    let sell_button = Button::new(format!("cultivation-sell-mastered-art-{}", art.id))
        .ghost()
        .small();
    div()
        .id(format!("cultivation-mastered-art-{}", art.id))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .py_1()
        .child(div().flex_1().min_w_0().truncate().text_xs().child(head))
        .child(
            div()
                .id(format!("cultivation-mastered-art-payoff-{}", art.id))
                .text_xs()
                .text_color(tokens.muted)
                .child(payoff)
                .test_support(),
        )
        .child(if art.sold {
            sell_button
                .label(text(locale, "cultivation.shop.sold"))
                .disabled(true)
        } else {
            sell_button
                .label(sell_label(locale, art_sell_price(art.id)))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::SellArt { art_id });
                            view.poll_pomodoro(cx);
                        });
                    }
                })
        })
        .test_support()
}

/// What a learned 功法 pays, as the concrete numbers the attributes panel
/// counts: every 阶 of an 内功 with its mastery share folded in, or the swing
/// an 外功 lands and what it costs.
fn mastered_art_payoff(art: &MasteredArtView, locale: &str) -> String {
    let kind = text(locale, art.kind.name_key()).to_string();
    let parts = match art.kind {
        ArtKind::Internal => {
            let payout = mastered_art_payout(art.id);
            Attribute::ALL
                .into_iter()
                .filter_map(|attribute| {
                    let value = attribute.of(payout);
                    (value > 0).then(|| {
                        format_text(
                            locale,
                            "cultivation.art.gain",
                            &[
                                ("attribute", text(locale, attribute.name_key()).to_string()),
                                ("value", value.to_string()),
                            ],
                        )
                    })
                })
                .collect::<Vec<_>>()
        }
        ArtKind::External => vec![
            format_text(
                locale,
                "cultivation.art.multiplier",
                &[("value", (art.multiplier_permille / 10).to_string())],
            ),
            format_text(
                locale,
                "cultivation.art.cost",
                &[("value", art.cost.to_string())],
            ),
            format_text(
                locale,
                "cultivation.art.mastery_damage",
                &[("value", (art.mastery_permille / 10).to_string())],
            ),
        ],
    };
    format!("{kind} · {}", parts.join(" · "))
}

/// One 功法, as two lines: the name, grade and payoff with the two buttons a
/// book needs on the first, and where the training stands on the second.
///
/// The second line is the 阶 the book asks about: how many the grade has, what
/// the next one costs in pomodoros, and how far into it the training is. The
/// row once carried only the first line because which 功法 and what it pays
/// are what a user weighs — but a ladder that does not say how far along it is
/// is a ladder the user cannot plan a pomodoro around, so the counts are back
/// under the numbers they belong to.
fn art_row(
    view: &Entity<SettingsView>,
    art: &ArtView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let grade = text(locale, art.grade.name_key());
    let head = format!("{} · {}", art.name, grade);
    let payoff = art_payoff(art, locale);
    let art_id = art.id;
    // A finished book has no next 阶 to price, so it says it stands complete
    // rather than borrowing a cost it does not have.
    let progress_line = if art.rank >= art.max_rank {
        format_text(
            locale,
            "cultivation.art.complete",
            &[("total", art.max_rank.to_string())],
        )
    } else {
        format_text(
            locale,
            "cultivation.art.progress",
            &[
                ("rank", art.rank.to_string()),
                ("total", art.max_rank.to_string()),
                ("need", art.required.to_string()),
                ("progress", art.progress.to_string()),
            ],
        )
    };
    div()
        .id(format!("cultivation-art-{}", art.id))
        .flex()
        .flex_col()
        .py_1()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(div().flex_1().min_w_0().truncate().text_xs().child(head))
                .child(
                    div()
                        .id(format!("cultivation-art-payoff-{}", art.id))
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(payoff)
                        .test_support(),
                )
                .child(
                    Button::new(format!("cultivation-art-select-{}", art.id))
                        .ghost()
                        .small()
                        .label(text(
                            locale,
                            if art.selected {
                                "cultivation.art.selected"
                            } else {
                                "cultivation.art.select"
                            },
                        ))
                        .disabled(art.selected)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    let _ = view.pomodoro.try_publish(PomodoroCommand::SelectArt {
                                        art_id: Some(art_id),
                                    });
                                    view.poll_pomodoro(cx);
                                });
                            }
                        }),
                )
                .child(
                    // The trained 功法 is not for sale: 修炼 reads it every session,
                    // and the button is disabled rather than hidden so that the
                    // reason it cannot be sold is a thing the user can see.
                    Button::new(format!("cultivation-sell-art-{}", art.id))
                        .ghost()
                        .small()
                        .label(sell_label(locale, art_sell_price(art.id)))
                        .disabled(art.selected)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    let _ = view
                                        .pomodoro
                                        .try_publish(PomodoroCommand::SellArt { art_id });
                                    view.poll_pomodoro(cx);
                                });
                            }
                        }),
                ),
        )
        .child(
            div()
                .id(format!("cultivation-art-progress-{}", art.id))
                .text_xs()
                .text_color(tokens.muted)
                .child(progress_line)
                .test_support(),
        )
        .test_support()
}

/// What training this 功法 actually pays: attributes for an 内功, damage and a
/// cost for an 外功 — and, once every 阶 is in, the mastery bonus on top.
///
/// The kind leads the line, because the numbers that follow mean different
/// things for the two kinds: an 内功's figures are what each 阶 pays, an 外功's
/// are what one swing costs and lands.
fn art_payoff(art: &ArtView, locale: &str) -> String {
    let kind = text(locale, art.kind.name_key()).to_string();
    let mut parts = match art.kind {
        ArtKind::Internal => Attribute::ALL
            .into_iter()
            .filter_map(|attribute| {
                let value = attribute.of(art.gain);
                (value > 0).then(|| {
                    format_text(
                        locale,
                        "cultivation.art.gain",
                        &[
                            ("attribute", text(locale, attribute.name_key()).to_string()),
                            ("value", value.to_string()),
                        ],
                    )
                })
            })
            .collect::<Vec<_>>(),
        ArtKind::External => vec![
            format_text(
                locale,
                "cultivation.art.multiplier",
                &[("value", (art.multiplier_permille / 10).to_string())],
            ),
            format_text(
                locale,
                "cultivation.art.cost",
                &[("value", art.cost.to_string())],
            ),
        ],
    };
    // A completed 功法 pays its mastery bonus on top of what its 阶 already
    // bought, and the row says so: "圆满" beside the same numbers as an
    // unfinished art reads as a label rather than as a reward.
    if art.mastery_permille > 0 {
        let key = match art.kind {
            ArtKind::Internal => "cultivation.art.mastery_attribute",
            ArtKind::External => "cultivation.art.mastery_damage",
        };
        parts.push(format_text(
            locale,
            key,
            &[("value", (art.mastery_permille / 10).to_string())],
        ));
    }
    format!("{kind} · {}", parts.join(" · "))
}

/// What the 集市 pays for a 功法 of that id.
///
/// Grade-priced like every other 功法 rather than read off the row, so the number
/// on the button is the number the sale pays even if the catalog is retuned.
fn art_sell_price(art_id: u64) -> u32 {
    ShopOffer::Art { art_id }.sell_price()
}

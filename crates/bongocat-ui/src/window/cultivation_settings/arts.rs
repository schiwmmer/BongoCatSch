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
    // The families the shelf holds, each on its own line: two books of one 前缀
    // are a family and pay, so the panel says what the shelf bought. A shelf
    // with no family on it prints nothing.
    if !cultivation.art_sets.is_empty() {
        panel = panel.child(
            div()
                .id("cultivation-art-sets")
                .flex()
                .flex_col()
                .gap_0p5()
                .child(section_title(locale, "cultivation.art_sets", tokens))
                .children(cultivation.art_sets.iter().map(|set_view| {
                    div()
                        .text_xs()
                        .text_color(tokens.accent)
                        .child(set_bonus_line(
                            locale,
                            bongocat_pomodoro::art_prefix_name(set_view.prefix),
                            set_view.books,
                            &set_view.bonus,
                        ))
                })),
        );
    }
    panel
}

/// The 功法 the cultivator has learned outright.
///
/// A separate panel because it is a separate thing: a book is still being
/// trained and can be sold, while a learned 功法 is kept for good and pays its
/// mastery for as long as the save lives. Putting them in one list would make
/// the second look like the first.
/// How many learned 功法 the panel shows before it is unfolded.
const MASTERED_VISIBLE_COUNT: usize = 3;

pub(super) fn mastered_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    open: bool,
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
    // The three most recently trained are the three being worked on; the rest
    // are one click away rather than always on a page that is already a ladder.
    let total = cultivation.mastered_arts.len();
    let shown = if open {
        total
    } else {
        total.min(MASTERED_VISIBLE_COUNT)
    };
    panel = panel.children(
        cultivation
            .mastered_arts
            .iter()
            .take(shown)
            .map(|art| mastered_art_row(view, art, locale, tokens)),
    );
    if total > MASTERED_VISIBLE_COUNT {
        let key = if open {
            "cultivation.mastered.show_less"
        } else {
            "cultivation.mastered.show_all"
        };
        let label = if open {
            text(locale, key).to_string()
        } else {
            format_text(locale, key, &[("count", total.to_string())])
        };
        panel = panel.child(
            div()
                .id("cultivation-mastered-toggle")
                .text_xs()
                .text_color(tokens.accent)
                .child(label)
                .test_support()
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.cultivation_mastered_open = !view.cultivation_mastered_open;
                            cx.notify();
                        });
                    }
                }),
        );
    }
    panel
}

/// One learned 功法, as one line: name and grade on the left, what it pays on
/// the right, and the tome's sale beside them.
///
/// Selling a learned 功法 sells the tome, not the knowledge: nothing it pays
/// is ever taken away, so the row keeps drawing the same numbers after the
/// sale — and the sale happens once, so a collected tome reads 已售 instead of
/// a price. A book still being trained reports here too, as far as it has
/// gone: the 阶 it stands at is in the name, it pays what it has paid for and
/// no mastery yet, and there is no sale beside it — the shelf owns the
/// selling, and a second sell button under the same roof would be two doors
/// to the same room.
fn mastered_art_row(
    view: &Entity<SettingsView>,
    art: &MasteredArtView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    // An unfinished book says where it stands, so "已习得" cannot be read as
    // "已圆满": the name carries the 阶 the training has reached. The grade is a
    // separate element so it can keep its own colour while the 阶 share stays
    // muted.
    let grade_text = text(locale, art.grade.name_key());
    let grade_paint = grade_color(art.grade);
    let art_name = art.name.clone();
    let tail = if art.complete {
        String::new()
    } else {
        format_text(
            locale,
            "cultivation.mastered.progress",
            &[
                ("rank", art.rank.to_string()),
                ("total", art.max_rank.to_string()),
            ],
        )
    };
    let payoff = mastered_art_payoff(art, locale);
    let mut row = div()
        .id(format!("cultivation-mastered-art-{}", art.id))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .py_1()
        .child(
            div()
                .id(format!("cultivation-mastered-art-name-{}", art.id))
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .text_xs()
                .child(div().min_w_0().truncate().child(art_name))
                .child(div().text_color(grade_paint).child(grade_text))
                .child(div().text_color(tokens.muted).child(tail))
                .test_support(),
        )
        .child(
            div()
                .id(format!("cultivation-mastered-art-payoff-{}", art.id))
                .text_xs()
                .text_color(tokens.muted)
                .child(payoff)
                .test_support(),
        );
    if art.complete {
        let art_id = art.id;
        let sell_button = Button::new(format!("cultivation-sell-mastered-art-{}", art.id))
            .ghost()
            .small();
        row = row.child(if art.sold {
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
        });
    }
    row.test_support()
}

/// What a learned 功法 pays, as the concrete numbers the attributes panel
/// counts: every 阶 of an 内功 with its mastery share folded in, or the swing
/// an 外功 lands and what it costs. A book still being trained pays what it
/// has paid for — its 阶 so far, and no mastery while any is missing.
fn mastered_art_payoff(art: &MasteredArtView, locale: &str) -> String {
    let kind = text(locale, art.kind.name_key()).to_string();
    let parts = match art.kind {
        ArtKind::Internal => {
            let payout = if art.complete {
                mastered_art_payout(art.id)
            } else {
                art_payout_at(art.id, art.rank)
            };
            // The outright combat numbers ride beside the attributes: an 内功
            // that pays 攻击力 outright pays it however it was paid, and the
            // panel shows both halves.
            let bonus = if art.complete {
                mastered_art_bonus_payout(art.id)
            } else {
                art_bonus_at(art.id, art.rank)
            };
            let mut parts: Vec<String> = Attribute::ALL
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
                .collect();
            parts.extend(bonus_lines(locale, &bonus));
            if let Some(line) = aptitude_line(locale, art.aptitude_tenths) {
                parts.push(line);
            }
            parts
        }
        ArtKind::External => {
            let mut parts = vec![
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
            ];
            // The move itself: how many blows, the band each is drawn from, and
            // what a landing one lays on. Two 玄阶 外功 of one worth are not the
            // same swing twice, and the row is where that has to be readable.
            if let Some(line) = strike_line(
                locale,
                art.hits,
                art.multiplier_permille,
                art.band_permille,
                art.effect,
            ) {
                parts.push(line);
            }
            // The mastery share belongs to a complete 功法 alone: an unfinished
            // one quoting a bonus of zero would be quoting a number that
            // decides nothing.
            if art.complete {
                parts.push(format_text(
                    locale,
                    "cultivation.art.mastery_damage",
                    &[("value", (art.mastery_permille / 10).to_string())],
                ));
            }
            parts
        }
    };
    format!("{kind} · {}", parts.join(" · "))
}

/// What an 外功's move is, as one line: how many blows a use throws, the band
/// each is drawn from, and what a landing one lays on.
///
/// The printed worth is the *centre* of the band, so the two numbers the row
/// quotes are the two ends of what the swing can be — the same 外功 is a
/// different number every time it is thrown, and a row that quoted only the
/// centre would be quoting a swing that never happens.
fn strike_line(
    locale: &str,
    hits: u8,
    multiplier_permille: u32,
    band: (u32, u32),
    effect: Option<StrikeEffect>,
) -> Option<String> {
    let percent = |permille: u32| ((permille + 5) / 10).to_string();
    let (lo, hi) = band;
    let mut parts: Vec<String> = Vec::new();
    if hits > 1 {
        parts.push(format_text(
            locale,
            "cultivation.art.blows",
            &[("count", hits.to_string())],
        ));
    }
    if hi > lo {
        parts.push(format_text(
            locale,
            "cultivation.art.band",
            &[
                (
                    "min",
                    percent(multiplier_permille.saturating_mul(lo) / 1_000),
                ),
                (
                    "max",
                    percent(multiplier_permille.saturating_mul(hi) / 1_000),
                ),
            ],
        ));
    }
    if let Some(effect) = effect {
        parts.push(match effect {
            StrikeEffect::Slow { permille } => format_text(
                locale,
                "cultivation.art.effect_slow",
                &[("value", percent(permille))],
            ),
            StrikeEffect::Haste { permille } => format_text(
                locale,
                "cultivation.art.effect_haste",
                &[("value", percent(permille))],
            ),
            StrikeEffect::Focus { hit } => format_text(
                locale,
                "cultivation.art.effect_focus",
                &[("value", hit.to_string())],
            ),
        });
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
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
    // The grade is painted its own colour, so five grades on a shelf are five
    // colours before they are five words: 黄、蓝、紫、橙、红, in the order the
    // ladder runs.
    let grade_text = text(locale, art.grade.name_key());
    let grade_paint = grade_color(art.grade);
    let art_name = art.name.clone();
    let payoff = art_payoff(art, locale);
    let art_id = art.id;
    // A finished book has no next 阶 to price, so it says it stands complete
    // rather than borrowing a cost it does not have.
    // The rate rides on the same line as the cost: "one more pomodoro, and
    // this attempt lands one time in two" is the pair the user is actually
    // weighing, and a percentage on a line of its own would be a second row
    // for a number that only means anything beside its cost.
    //
    // The rate is held in parts per thousand, and it is read through the one
    // percent conversion the page uses everywhere: 405‰ prints `40.5`. Dividing
    // the *integer* by ten first — which is what the row used to do — printed a
    // 40.5% rate as `4.0`, ten times too small, and swallowed the failure
    // compensation with it: 405‰ and 409‰ both truncated to 40, so a run of
    // failures looked like it had bought nothing.
    let success_line = format_text(
        locale,
        "cultivation.art.success",
        &[("rate", permille_percent_label(art.success_permille))],
    );
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
    // The failures this book has cost are on the line beside the rate they
    // bought: the rate is the number the user acts on, and this is why it is not
    // the one 潜质 alone would promise. The count is the whole 功法's, so it is
    // read once and stays put as the 阶 turn over.
    let progress_line = if art.rank >= art.max_rank {
        progress_line
    } else if art.failures > 0 {
        format!(
            "{progress_line} · {success_line} · {}",
            format_text(
                locale,
                "cultivation.art.failures",
                &[("count", art.failures.to_string())],
            )
        )
    } else {
        format!("{progress_line} · {success_line}")
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
                .child(
                    div()
                        .id(format!("cultivation-art-name-{}", art.id))
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .child(div().min_w_0().truncate().child(art_name))
                        .child(div().text_color(grade_paint).child(grade_text))
                        // How many copies of this book are on the shelf. A
                        // spare is invisible until one is held, and a count on
                        // every row would read as a stack of everything.
                        .children((art.copies > 1).then(|| {
                            div()
                                .id(format!("cultivation-art-copies-{}", art.id))
                                .flex_none()
                                .text_color(tokens.muted)
                                .child(format_text(
                                    locale,
                                    "cultivation.art.copies",
                                    &[("count", art.copies.to_string())],
                                ))
                                .test_support()
                        }))
                        .test_support(),
                )
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
                    // The last copy of the trained 功法 is not for sale: 修炼 reads
                    // it every session, and the button is disabled rather than
                    // hidden so that the reason it cannot be sold is a thing the
                    // user can see. A spare is a different book on the same
                    // shelf, so it sells even while the other copy is read.
                    Button::new(format!("cultivation-sell-art-{}", art.id))
                        .ghost()
                        .small()
                        .label(sell_label(locale, art_sell_price(art.id)))
                        .disabled(art.selected && art.copies <= 1)
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
        ArtKind::Internal => {
            // The outright combat numbers one 阶 pays, beside the attributes:
            // a combat-shaped 内功 has no attribute gain at all, and its row
            // has to say what it pays instead of reading as empty.
            let mut parts: Vec<String> = Attribute::ALL
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
                .collect();
            parts.extend(bonus_lines(locale, &art.bonus));
            parts
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
        ],
    };
    // The move itself, on the shelf and on the row alike: a 外功 is not only a
    // worth, it is a number of blows and a band to draw them from.
    if let Some(line) = strike_line(
        locale,
        art.hits,
        art.multiplier_permille,
        art.band_permille,
        art.effect,
    ) {
        parts.push(line);
    }
    // The ten 潜质 books say so on their own row: it is the one number a build
    // is built around, and a reader who had to divine it from the fact that the
    // book is one of ten would never find it.
    if let Some(line) = aptitude_line(locale, art.aptitude_tenths) {
        parts.push(line);
    }
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

/// What one 阶 of a 潜质 book pays, as a line — `None` for every other book.
///
/// Tenths are printed the way the panel prints 潜质: one decimal, so 0.8 reads
/// as the eight tenths it is rather than as a rounded point.
fn aptitude_line(locale: &str, tenths: i32) -> Option<String> {
    (tenths > 0).then(|| {
        format_text(
            locale,
            "cultivation.art.aptitude",
            &[("value", format!("{}.{}", tenths / 10, tenths % 10))],
        )
    })
}

/// What the 集市 pays for a 功法 of that id.
///
/// Grade-priced like every other 功法 rather than read off the row, so the number
/// on the button is the number the sale pays even if the catalog is retuned.
fn art_sell_price(art_id: u64) -> u32 {
    ShopOffer::Art { art_id }.sell_price()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this guards: the rate is held in parts per thousand and the row
    /// used to divide the *integer* by ten before dividing by ten again, so a
    /// 40.5% attempt printed as `4.0` — ten times too small — and the failure
    /// compensation vanished behind the truncation, because 405‰ and 409‰ both
    /// lost their half-percent in the same integer division.
    #[test]
    fn the_rate_prints_as_the_percent_it_is() {
        // 玄阶 at 潜质 −3.6: 420‰ × 1.01^−3.6.
        assert_eq!(permille_percent_label(405), "40.5");
        // The same attempt after one failure on this 阶: ×1.01 more.
        assert_eq!(permille_percent_label(409), "40.9");
        assert_ne!(
            permille_percent_label(405),
            permille_percent_label(409),
            "a run of failures has to be readable on the line"
        );
        // A whole percent drops its decimal, the way the star line does.
        assert_eq!(permille_percent_label(420), "42");
        assert_eq!(permille_percent_label(995), "99.5");
        assert_eq!(permille_percent_label(1), "0.1");
    }

    /// The two numbers the screenshot in the bug report read, checked against
    /// the domain's own arithmetic rather than against the row: a 玄阶 book at
    /// 潜质 −3.6 fails 59.5% of the time, and one failure in it lifts the next
    /// attempt by about four parts in a thousand.
    #[test]
    fn the_profound_rate_at_negative_aptitude_is_what_the_row_says() {
        let base =
            bongocat_pomodoro::success_permille(bongocat_pomodoro::ArtGrade::Profound, -36, 0);
        assert_eq!(base, 405, "420‰ × 1.01^-3.6");
        let after =
            bongocat_pomodoro::success_permille(bongocat_pomodoro::ArtGrade::Profound, -36, 1);
        assert_eq!(after, 409, "one failure on the book, additively");
        assert!(
            permille_percent_label(after) != permille_percent_label(base),
            "the row has to show the compensation: {} vs {}",
            permille_percent_label(after),
            permille_percent_label(base)
        );
    }
}

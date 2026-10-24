//! The daily market: the rolled shelf, one row per offer, and buying.

use super::garden::seed_facts;
use super::*;

/// The 集市: what the purse holds, what is on today's shelf, and what a sale pays.
///
/// Two halves on one panel because they are one loop: the seed rows and the gear
/// rows carry a sell button, the shelf carries a buy button, and the number at the
/// top is what both move. A shop that lived apart from the inventory would ask
/// the user to go and remember what they were holding.
pub(super) fn shop_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    open_offer: Option<usize>,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-shop")
        .flex()
        .flex_col()
        .gap_2()
        // Marked so a frame can be asked whether the market is on screen at all.
        .test_support()
        .child(section_title(locale, "cultivation.shop.title", tokens))
        .child(
            div()
                .id("cultivation-shop-stones")
                .text_xs()
                .text_color(tokens.accent)
                .child(format_text(
                    locale,
                    "cultivation.shop.stones",
                    &[("stones", cultivation.stones.to_string())],
                ))
                .test_support(),
        )
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.shop.shelf")),
        );

    if cultivation.shop_offers.is_empty() {
        panel = panel.child(
            div()
                .id("cultivation-shop-empty")
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.shop.empty"))
                .test_support(),
        );
        return panel;
    }
    panel = panel.child(
        div().flex().flex_col().gap_1().children(
            cultivation
                .shop_offers
                .iter()
                .map(|entry| shop_row(view, entry, open_offer, locale, tokens)),
        ),
    );
    panel
}

/// One line of the shelf: the goods on the left, the price on the right.
///
/// The name side is a click: it opens the offer's detail under the row, the
/// same one-open-at-a-time rule the inventory rows follow. The price side is
/// the buy — one click, one price, and no state that changes with the purse:
/// 灵石不够 is the refusal the shop answers with, not a different button. A
/// bought row keeps its place and says so rather than vanishing: a shelf that
/// reshuffles under the cursor is a shelf where the wrong thing gets bought.
fn shop_row(
    view: &Entity<SettingsView>,
    entry: &ShopOfferView,
    open_offer: Option<usize>,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let label = shop_offer_label(&entry.offer, locale);
    let slot = entry.slot;
    // A piece of gear on the shelf is painted in its quality's colour, the
    // same colour the same piece wears in the satchel: the shelf and the
    // inventory are one market, and a grade that read differently on the two
    // sides would be two grades.
    let name_color = match &entry.offer {
        ShopOffer::Equipment(item) if !entry.bought => quality_color(item.rarity),
        // A 功法 on the shelf wears its grade's colour too: 黄、蓝、紫、橙、红
        // is how a buyer tells five books apart before reading five names.
        ShopOffer::Art { art_id } if !entry.bought => grade_color(blueprint(*art_id).grade),
        _ => tokens.text,
    };
    let price = format_text(
        locale,
        "cultivation.shop.price",
        &[("stones", entry.price.to_string())],
    );
    let name = div()
        .id(format!("cultivation-shop-name-{}", slot))
        .flex_1()
        .min_w_0()
        .truncate()
        .text_xs()
        .text_color(if entry.bought {
            tokens.muted
        } else {
            name_color
        })
        .child(label)
        .on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.cultivation_open_offer = if view.cultivation_open_offer == Some(slot) {
                        None
                    } else {
                        Some(slot)
                    };
                    cx.notify();
                });
            }
        })
        .test_support();
    let main_line = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(name)
        // The cut, said beside the price it is a cut of: a row that only showed
        // the cheaper number would be a row whose discount is invisible, and one
        // that only showed the old price would be a row that has not moved.
        .children(special_badge(entry, locale, tokens))
        .child(if entry.bought {
            div()
                .id(format!("cultivation-shop-bought-{}", slot))
                .text_xs()
                .text_color(tokens.accent)
                .child(text(locale, "cultivation.shop.bought"))
                .test_support()
                .into_any_element()
        } else {
            Button::new(format!("cultivation-shop-buy-{}", slot))
                .ghost()
                .small()
                .label(price)
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::BuyFromShop { slot });
                            view.poll_pomodoro(cx);
                        });
                    }
                })
                .into_any_element()
        });
    let mut row = div()
        .id(format!("cultivation-shop-row-{}", slot))
        .flex()
        .flex_col()
        .gap_1()
        .child(main_line)
        .test_support();
    if open_offer == Some(slot) {
        row = row.child(shop_offer_detail(
            &entry.offer,
            entry.sell_price,
            locale,
            tokens,
        ));
    }
    row
}

/// The row's special, when it has one: what fraction of the list price it is cut
/// to, and the price it was cut from.
///
/// Both numbers, because either alone is a half-truth: the fraction without the
/// price is a discount on a number the user has to go and remember, and the
/// price without the fraction is a row that looks like it was always this cheap.
fn special_badge(
    entry: &ShopOfferView,
    locale: &str,
    tokens: Tokens,
) -> Option<gpui_kit::AnyElement> {
    (entry.discount_permille > 0).then(|| {
        div()
            .id(format!("cultivation-shop-special-{}", entry.slot))
            .text_xs()
            .text_color(tokens.accent)
            .child(format_text(
                locale,
                "cultivation.shop.special",
                &[
                    ("value", zhe_label(entry.discount_permille)),
                    ("list", entry.list_price.to_string()),
                ],
            ))
            .test_support()
            .into_any_element()
    })
}

/// A cut as the fraction of the list price it leaves, one decimal at most:
/// seven tenths reads `7`, six and a half reads `6.5`.
fn zhe_label(permille: u32) -> String {
    let whole = permille / 100;
    let tenth = (permille % 100) / 10;
    if tenth == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{tenth}")
    }
}

/// One offer's detail, shown under its row when the name is clicked: the
/// attribute breakdown a garment carries, or the one line a herb and a 功法
/// need to say what they are worth.
fn shop_offer_detail(
    offer: &ShopOffer,
    sell_price: u32,
    locale: &str,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    let resale = format_text(
        locale,
        "cultivation.shop.detail_sell",
        &[("stones", sell_price.to_string())],
    );
    match offer {
        ShopOffer::Equipment(item) => item_detail(
            &ItemView {
                id: item.id,
                slot: item.slot,
                tier: item.tier,
                rarity: item.rarity,
                attributes: item.effective_attributes(),
                bonus: item.effective_bonus(),
                power: item.power(),
                equipped: false,
                star: item.star(),
                effect: item.effect,
                // Nothing on the shelf is starred, and a shelf row cannot be
                // wagered on: the wager belongs to what the buyer already owns.
                star_cost: None,
                name: item.name.clone(),
                sell_price,
            },
            locale,
            tokens,
        )
        .into_any_element(),
        ShopOffer::Seed {
            attribute,
            tier,
            name,
        } => div()
            .flex()
            .flex_col()
            .gap_1()
            .text_xs()
            .text_color(tokens.muted)
            // What the seed pays when it comes up, then what a sale would bring:
            // the two numbers a buyer is choosing between when they look at the
            // price on the button.
            .child(seed_facts(locale, *attribute, *tier))
            .child(format!(
                "{} · {}",
                seed_label(locale, *attribute, *tier, name.as_deref()),
                resale
            ))
            .into_any_element(),
        ShopOffer::Art { art_id } => {
            let art = blueprint(*art_id);
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(format!(
                    "{} · {} · {} · {}",
                    text(locale, art.kind.name_key()),
                    text(locale, art.grade.name_key()),
                    art_numbers(
                        art.kind,
                        &art.gain,
                        &art.bonus,
                        art.multiplier_permille,
                        art.cost,
                        locale
                    ),
                    resale
                ))
                .into_any_element()
        }
        ShopOffer::Herb { herb } => div()
            .flex()
            .flex_col()
            .gap_1()
            .text_xs()
            .text_color(tokens.muted)
            // What the herb does, then what a sale would bring: the same two
            // facts every row carries, in the terms 潜质 is bought in.
            .child(format_text(
                locale,
                "cultivation.shop.herb_facts",
                &[
                    ("herb", text(locale, herb.name_key()).to_string()),
                    ("effect", herb_effect(*herb, locale)),
                ],
            ))
            .child(resale)
            .into_any_element(),
    }
}

/// What one 潜质 item does, as the shelf says it.
fn herb_effect(herb: AptitudeHerb, locale: &str) -> String {
    match herb.gain_tenths() {
        Some((low, high)) => format_text(
            locale,
            "cultivation.shop.herb_gain",
            &[
                ("low", super::tenths_text(low)),
                ("high", super::tenths_text(high)),
            ],
        ),
        None => text(locale, "cultivation.shop.herb_marrow_gain").to_string(),
    }
}

/// The numbers one 功法 pays, as the payoff lines spell them: an 内功's per-阶
/// gains — attributes and outright combat numbers both — or an 外功's swing and
/// its cost.
fn art_numbers(
    kind: ArtKind,
    gain: &Attributes,
    bonus: &Derived,
    multiplier_permille: u32,
    cost: u32,
    locale: &str,
) -> String {
    match kind {
        ArtKind::Internal => {
            let mut parts: Vec<String> = Attribute::ALL
                .into_iter()
                .filter_map(|attribute| {
                    let value = attribute.of(*gain);
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
            parts.extend(bonus_lines(locale, bonus));
            parts.join(" · ")
        }
        ArtKind::External => format!(
            "{} · {}",
            format_text(
                locale,
                "cultivation.art.multiplier",
                &[("value", (multiplier_permille / 10).to_string())],
            ),
            format_text(
                locale,
                "cultivation.art.cost",
                &[("value", cost.to_string())],
            ),
        ),
    }
}

/// What one offer is called on the shelf.
///
/// The same names the inventory uses, so a thing bought from the shelf and the
/// same thing found on a walk read as the same object on both sides of the page.
fn shop_offer_label(offer: &ShopOffer, locale: &str) -> String {
    match offer {
        ShopOffer::Equipment(item) => item_label(
            locale,
            &ItemView {
                id: item.id,
                slot: item.slot,
                tier: item.tier,
                rarity: item.rarity,
                attributes: item.effective_attributes(),
                bonus: item.effective_bonus(),
                power: item.power(),
                equipped: false,
                star: item.star(),
                effect: item.effect,
                star_cost: None,
                name: item.name.clone(),
                sell_price: offer.sell_price(),
            },
        ),
        ShopOffer::Seed {
            attribute,
            tier,
            name,
        } => seed_label(locale, *attribute, *tier, name.as_deref()),
        ShopOffer::Art { art_id } => blueprint(*art_id).name.clone(),
        ShopOffer::Herb { herb } => text(locale, herb.name_key()).to_string(),
    }
}

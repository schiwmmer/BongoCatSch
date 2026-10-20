//! The daily market: the rolled shelf, one row per offer, and buying.

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
            tokens.text
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
    let mut row = div()
        .id(format!("cultivation-shop-row-{}", slot))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(name)
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
        })
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
                attributes: item.attributes(),
                power: item.power(),
                equipped: false,
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
            .text_xs()
            .text_color(tokens.muted)
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
                        art.multiplier_permille,
                        art.cost,
                        locale
                    ),
                    resale
                ))
                .into_any_element()
        }
    }
}

/// The numbers one 功法 pays, as the payoff lines spell them: an 内功's per-阶
/// gains, an 外功's swing and its cost.
fn art_numbers(
    kind: ArtKind,
    gain: &Attributes,
    multiplier_permille: u32,
    cost: u32,
    locale: &str,
) -> String {
    match kind {
        ArtKind::Internal => Attribute::ALL
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
            .collect::<Vec<_>>()
            .join(" · "),
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
                attributes: item.attributes(),
                power: item.power(),
                equipped: false,
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
    }
}

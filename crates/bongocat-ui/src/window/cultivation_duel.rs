//! The fight a breakthrough or a walk settled, replayed blow by blow.
//!
//! A breakthrough is the one thing on the ladder the user cannot buy their way
//! through, so it is shown rather than summarised: two fighters trade blows in
//! the order their 速度 earns them, and every line of that is on the page. The
//! blows are paced out instead of dumped at once — a fight read as a wall of
//! text is a fight nobody reads — and the log can be replayed or skipped.
//!
//! The popup is drawn over the settings surface rather than through the window
//! dialog: the window-level dialogs in this app hang off the root state, and the
//! one that would host this does not raise on this window. A sibling of the
//! settings tree in the same positioned root needs no root state to appear.

use super::*;
use bongocat_i18n::{format_text, text};
use bongocat_pomodoro::{
    BreakthroughOutcome, DuelReport, DuelRound, ExplorationFindView, ExplorationOutcome,
    GUARDIAN_NAME, PLAYER_NAME, WANDERER_NAME,
};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::Sizable as _;
use std::time::{Duration, Instant};

/// How far apart two blows of the replay are shown.
///
/// Fast enough that a thirty-blow fight is over in a few seconds, slow enough
/// that a reader can follow who is hitting whom.
pub(super) const REVEAL_INTERVAL: Duration = Duration::from_millis(250);

/// What one button of the popup does. A type of its own because the three of
/// them are handed around together, and a bare `Rc<dyn Fn(..)>` in every
/// signature is noise.
type DuelAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// How wide the fight log is drawn.
const PANEL_WIDTH: f32 = 560.0;

/// How tall the log may grow before it scrolls.
const LOG_MAX_HEIGHT: f32 = 360.0;

/// Which of the two paying actions the fight being replayed settled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CultivationDuelSource {
    /// The guardian at the end of a realm.
    Breakthrough,
    /// A walk out of the door.
    Exploration,
}

/// What the fight being replayed settled.
///
/// The two sources pay differently and say so differently, so the payload
/// carries which one it was rather than making the render guess from the data.
#[derive(Clone, Debug)]
pub(crate) enum CultivationDuelPayload {
    /// A breakthrough: the fight, and what the win paid or the loss cost.
    Breakthrough(BreakthroughOutcome),
    /// A walk: what it came to, what it turned up, and the fight it met.
    Exploration(
        ExplorationOutcome,
        Option<ExplorationFindView>,
        Option<DuelReport>,
    ),
}

/// The fight the cultivation page is replaying.
pub(crate) struct CultivationDuel {
    pub(crate) source: CultivationDuelSource,
    pub(crate) payload: CultivationDuelPayload,
    /// How many blows have been shown so far.
    pub(crate) revealed: usize,
    /// When the replay started, which is what paces the blows.
    pub(crate) started: Instant,
}

impl CultivationDuel {
    pub(crate) fn new(source: CultivationDuelSource, payload: CultivationDuelPayload) -> Self {
        Self {
            source,
            payload,
            revealed: 0,
            started: Instant::now(),
        }
    }

    /// The blows of the fight, when one settled it.
    fn rounds(&self) -> &[DuelRound] {
        match &self.payload {
            CultivationDuelPayload::Breakthrough(outcome) => &outcome.duel.rounds,
            CultivationDuelPayload::Exploration(_, _, duel) => {
                duel.as_ref().map_or(&[], |duel| &duel.rounds)
            }
        }
    }

    /// The catalog key naming whoever the cultivator is fighting.
    fn opponent(&self) -> &'static str {
        match self.source {
            CultivationDuelSource::Breakthrough => GUARDIAN_NAME,
            CultivationDuelSource::Exploration => WANDERER_NAME,
        }
    }

    /// Whether the cultivator came out of it standing. A walk that met nobody is
    /// neither a win nor a loss, and says so by there being no verdict to draw.
    fn won(&self) -> bool {
        match &self.payload {
            CultivationDuelPayload::Breakthrough(outcome) => outcome.duel.won,
            CultivationDuelPayload::Exploration(outcome, _, _) => match outcome {
                ExplorationOutcome::Victory => true,
                ExplorationOutcome::Defeat => false,
                _ => false,
            },
        }
    }

    /// Whether the replay settled a fight at all.
    fn fought(&self) -> bool {
        !self.rounds().is_empty()
    }

    /// Reveal the blows the elapsed time has earned.
    ///
    /// Called from the poll that already wakes for the garden, so a replay costs
    /// no timer of its own. The elapsed time rather than a per-tick step is what
    /// keeps the pacing even when the poll and the interval drift apart.
    pub(crate) fn advance(&mut self) -> bool {
        let owed = (self.started.elapsed().as_millis() / REVEAL_INTERVAL.as_millis()) as usize;
        let owed = owed.min(self.rounds().len());
        if owed <= self.revealed {
            return false;
        }
        self.revealed = owed;
        true
    }

    /// Show every blow at once, and say whether any were left to show.
    pub(crate) fn reveal_all(&mut self) -> bool {
        let total = self.rounds().len();
        if self.revealed >= total {
            return false;
        }
        self.revealed = total;
        true
    }

    /// Whether the replay has run its course.
    fn finished(&self) -> bool {
        self.revealed >= self.rounds().len()
    }

    /// Start the replay over from the first blow.
    pub(crate) fn replay(&mut self) {
        self.revealed = 0;
        self.started = Instant::now();
    }
}

/// What the three buttons of the popup do. The panel holds no state of its own,
/// so it asks the view through these. Shared rather than owned: the close
/// action hangs off both the backdrop and the header's button.
pub(super) struct CultivationDuelActions {
    pub(super) on_replay: DuelAction,
    pub(super) on_skip: DuelAction,
    pub(super) on_close: DuelAction,
}

/// The fight, drawn over the settings surface.
pub(super) fn render(
    duel: &CultivationDuel,
    locale: &str,
    tokens: Tokens,
    actions: CultivationDuelActions,
) -> gpui_kit::AnyElement {
    let verdict_color = if duel.won() {
        tokens.accent
    } else {
        tokens.danger
    };
    let shown = duel.revealed.min(duel.rounds().len());
    let rounds: Vec<String> = duel.rounds()[..shown]
        .iter()
        .map(|round| round_line(round, duel.opponent(), locale))
        .collect();
    let on_replay = actions.on_replay;
    let on_skip = actions.on_skip;
    let on_close = actions.on_close;
    let close_from_backdrop = on_close.clone();
    div()
        .id("cultivation-duel-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .bg(tokens.overlay)
        // A click on the backdrop and nowhere else closes the popup: it is a
        // report, not a mode, and the way out should not have to be aimed at a
        // button. The panel occludes the pointer, so a click inside it never
        // reaches here.
        .on_click(move |_, window, cx| (close_from_backdrop)(window, cx))
        .child(
            div()
                .id("cultivation-duel-panel")
                .w(px(PANEL_WIDTH))
                .max_h(px(LOG_MAX_HEIGHT + 200.0))
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_lg()
                .bg(tokens.canvas)
                .border_1()
                .border_color(tokens.border)
                .occlude()
                .child(header(duel, locale, tokens, on_replay, on_close))
                .child(log(rounds, duel.fought(), locale, tokens, on_skip))
                .child(verdict(duel, locale, verdict_color, tokens))
                .test_support(),
        )
        .into_any_element()
}

/// The title, and the two buttons that act on the replay.
fn header(
    duel: &CultivationDuel,
    locale: &str,
    tokens: Tokens,
    on_replay: DuelAction,
    on_close: DuelAction,
) -> impl IntoElement {
    let title = title_line(duel, locale);
    div()
        .id("cultivation-duel-header")
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(tokens.text)
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    Button::new("cultivation-duel-replay")
                        .ghost()
                        .small()
                        .label(text(locale, "cultivation.duel.replay"))
                        .on_click(move |_, window, cx| (on_replay)(window, cx)),
                )
                .child(
                    Button::new("cultivation-duel-close")
                        .ghost()
                        .small()
                        .label(text(locale, "cultivation.duel.close"))
                        .on_click(move |_, window, cx| (on_close)(window, cx)),
                ),
        )
}

/// What the fight is called, and — for a breakthrough — the two powers that
/// stood opposite each other, which is the number the page already taught the
/// user to read.
fn title_line(duel: &CultivationDuel, locale: &str) -> String {
    match &duel.payload {
        CultivationDuelPayload::Breakthrough(outcome) => format!(
            "{} · {}",
            text(locale, "cultivation.duel.title_breakthrough"),
            format_text(
                locale,
                "cultivation.duel.power",
                &[
                    ("player", outcome.report.player_power.to_string()),
                    ("guardian", outcome.report.guardian_power.to_string()),
                ],
            )
        ),
        CultivationDuelPayload::Exploration(_, _, fought) => {
            let key = if fought.is_some() {
                "cultivation.duel.title_exploration"
            } else {
                "cultivation.explore.title"
            };
            text(locale, key).to_string()
        }
    }
}

/// The blows so far, one line each, scrolling once the fight runs long.
///
/// `fought` says whether there is a fight to replay at all: a walk that met
/// nobody has no blows, and a log that promised a replay of nothing would be
/// lying about the walk.
fn log(
    rounds: Vec<String>,
    fought: bool,
    locale: &str,
    tokens: Tokens,
    on_skip: DuelAction,
) -> impl IntoElement {
    let mut log = div()
        .id("cultivation-duel-log")
        .flex()
        .flex_col()
        .gap_1()
        .min_h(px(48.0))
        .max_h(px(LOG_MAX_HEIGHT))
        .overflow_y_scroll()
        .p_2()
        .rounded_md()
        .bg(tokens.overlay);
    if fought {
        log = log
            // Tapping the log skips to the last blow: a reader who has seen
            // enough should not have to wait out the pacing.
            .on_click(move |_, window, cx| (on_skip)(window, cx));
        if rounds.is_empty() {
            log = log.child(
                div()
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, "cultivation.duel.playing")),
            );
        }
    }
    for line in rounds {
        log = log.child(div().text_xs().text_color(tokens.text).child(line));
    }
    log
}

/// How the fight came out, and what it paid or cost — once the last blow has
/// been shown, because a verdict under a half-read fight spoils it.
fn verdict(duel: &CultivationDuel, locale: &str, color: Hsla, tokens: Tokens) -> impl IntoElement {
    let mut column = div()
        .id("cultivation-duel-verdict")
        .flex()
        .flex_col()
        .gap_1();
    if duel.finished() {
        if duel.fought() {
            column = column.child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(color)
                    .child(text(
                        locale,
                        if duel.won() {
                            "cultivation.duel.win"
                        } else {
                            "cultivation.duel.lose"
                        },
                    )),
            );
        }
        column = column.children(payoff_lines(duel, locale, tokens));
    }
    column
}

/// What the settled fight left behind, as the domain recorded it.
fn payoff_lines(duel: &CultivationDuel, locale: &str, tokens: Tokens) -> Vec<gpui_kit::AnyElement> {
    let mut lines = Vec::new();
    match &duel.payload {
        CultivationDuelPayload::Breakthrough(outcome) => {
            if outcome.duel.won {
                let name = outcome
                    .granted_art
                    .clone()
                    .unwrap_or_else(|| text(locale, "cultivation.arts.empty").to_string());
                lines.push(payoff(
                    "cultivation-duel-pay-art",
                    format_text(locale, "cultivation.duel.reward_art", &[("name", name)]),
                    tokens.accent,
                ));
                if let Some(reward) = &outcome.reward {
                    lines.push(payoff(
                        "cultivation-duel-pay-gear",
                        format_text(
                            locale,
                            "cultivation.duel.reward_gear",
                            &[("name", gear_label(locale, reward))],
                        ),
                        tokens.accent,
                    ));
                }
            } else if let Some(lost) = &outcome.lost {
                lines.push(payoff(
                    "cultivation-duel-lost-gear",
                    format_text(
                        locale,
                        "cultivation.duel.lost_gear",
                        &[("name", lost.clone())],
                    ),
                    tokens.danger,
                ));
            }
        }
        CultivationDuelPayload::Exploration(outcome, found, _) => {
            let key = match outcome {
                ExplorationOutcome::Nothing => Some("cultivation.explore.nothing"),
                ExplorationOutcome::Victory => Some("cultivation.explore.victory"),
                ExplorationOutcome::Defeat => Some("cultivation.explore.defeat"),
                _ => None,
            };
            if let Some(key) = key {
                let color = match outcome {
                    ExplorationOutcome::Nothing => tokens.muted,
                    ExplorationOutcome::Victory => tokens.accent,
                    _ => tokens.danger,
                };
                lines.push(payoff(
                    "cultivation-duel-walk",
                    text(locale, key).to_string(),
                    color,
                ));
            }
            match found {
                Some(ExplorationFindView::Seed(seed)) => lines.push(payoff(
                    "cultivation-duel-walk-seed",
                    format_text(
                        locale,
                        "cultivation.explore.found_seed",
                        &[("name", seed_label(locale, seed))],
                    ),
                    tokens.accent,
                )),
                Some(ExplorationFindView::Gear(item)) => lines.push(payoff(
                    "cultivation-duel-walk-gear",
                    format_text(
                        locale,
                        "cultivation.explore.found_gear",
                        &[("name", item_label(locale, item))],
                    ),
                    tokens.accent,
                )),
                Some(ExplorationFindView::Art(name)) => lines.push(payoff(
                    "cultivation-duel-walk-art",
                    format_text(
                        locale,
                        "cultivation.explore.found_art",
                        &[("name", name.clone())],
                    ),
                    tokens.accent,
                )),
                None => {}
            }
        }
    }
    lines
}

/// One line of what the fight left behind.
fn payoff(id: &'static str, line: String, color: Hsla) -> gpui_kit::AnyElement {
    div()
        .id(id)
        .text_xs()
        .text_color(color)
        .child(line)
        .into_any_element()
}

/// One blow, as the log tells it: who swung, with what, and what it left the
/// other side with.
fn round_line(round: &DuelRound, opponent: &str, locale: &str) -> String {
    let attacker = text(locale, &round.attacker).to_string();
    // The target is whoever did not swing. The names are catalog keys the domain
    // wrote, so the comparison is on the key rather than on a translated word.
    let target = if round.attacker == PLAYER_NAME {
        text(locale, opponent)
    } else {
        text(locale, PLAYER_NAME)
    };
    let head = format_text(
        locale,
        "cultivation.duel.blow_index",
        &[("index", round.index.to_string())],
    );
    let body = if round.dodged {
        format_text(
            locale,
            "cultivation.duel.dodge",
            &[("attacker", attacker), ("target", target.to_string())],
        )
    } else {
        let art = round
            .art
            .clone()
            .unwrap_or_else(|| text(locale, "cultivation.duel.blow").to_string());
        format_text(
            locale,
            "cultivation.duel.hit",
            &[
                ("attacker", attacker),
                ("art", art),
                ("damage", round.damage.to_string()),
                ("target", target.to_string()),
                ("health", round.target_health.to_string()),
            ],
        )
    };
    format!("{head} {body}")
}

/// A found item's display line, shared with the page's own rows.
fn item_label(locale: &str, item: &bongocat_pomodoro::ItemView) -> String {
    super::cultivation_settings::item_label(locale, item)
}

/// A found seed's display line, shared with the page's own rows.
fn seed_label(locale: &str, seed: &bongocat_pomodoro::SeedView) -> String {
    super::cultivation_settings::seed_label(locale, seed.attribute, seed.tier, seed.name.as_deref())
}

/// The gear a win paid, named when the drop carried a name.
fn gear_label(locale: &str, item: &bongocat_pomodoro::EquipmentRecord) -> String {
    match item.name.as_deref() {
        Some(name) => name.to_string(),
        None => text(locale, item.slot.name_key()).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bongocat_pomodoro::{DuelReport, DuelRound};

    /// A report of `blows` blows, all of them the cultivator's.
    fn report(blows: u32) -> DuelReport {
        DuelReport {
            rounds: (1..=blows)
                .map(|index| DuelRound {
                    index,
                    attacker: PLAYER_NAME.to_owned(),
                    art: None,
                    dodged: false,
                    damage: 10,
                    target_health: 100 - index,
                })
                .collect(),
            won: true,
            player_health: 90,
            guardian_health: 0,
        }
    }

    /// A walk that met someone and beat them.
    fn walk(blows: u32) -> CultivationDuel {
        CultivationDuel::new(
            CultivationDuelSource::Exploration,
            CultivationDuelPayload::Exploration(
                ExplorationOutcome::Victory,
                None,
                Some(report(blows)),
            ),
        )
    }

    #[test]
    fn the_blows_are_paced_and_can_be_skipped_and_replayed() {
        let mut duel = walk(6);
        assert!(!duel.advance(), "the first blow waits out its interval");
        assert_eq!(
            duel.revealed, 0,
            "nothing is shown before the first interval"
        );
        // One interval in, that interval's blow is owed.
        duel.started = Instant::now() - REVEAL_INTERVAL;
        assert!(duel.advance(), "the interval that has passed owes its blow");
        assert!(
            duel.revealed >= 1,
            "at least the blow the interval paid for"
        );
        assert!(duel.reveal_all(), "skipping shows the rest");
        assert_eq!(duel.revealed, 6);
        assert!(!duel.reveal_all(), "there is nothing left to skip to");
        duel.replay();
        assert_eq!(duel.revealed, 0, "a replay starts over from the first blow");
        duel.reveal_all();
        assert!(duel.finished(), "every blow has been shown");
    }

    #[test]
    fn a_walk_that_met_nobody_has_no_blows_to_show() {
        let mut duel = CultivationDuel::new(
            CultivationDuelSource::Exploration,
            CultivationDuelPayload::Exploration(ExplorationOutcome::Nothing, None, None),
        );
        assert!(duel.rounds().is_empty(), "there was no fight");
        assert!(!duel.fought(), "and so no verdict to draw");
        assert!(
            !duel.reveal_all(),
            "skipping a walk that found nothing does nothing"
        );
    }

    #[test]
    fn a_breakthrough_names_the_guardian_and_a_walk_names_the_wanderer() {
        let guardian = CultivationDuel::new(
            CultivationDuelSource::Breakthrough,
            CultivationDuelPayload::Breakthrough(BreakthroughOutcome {
                report: bongocat_pomodoro::BattleReport {
                    player_power: 10,
                    guardian_power: 9,
                    won: true,
                },
                target: bongocat_pomodoro::Realm::Foundation,
                reward: None,
                duel: report(3),
                lost: None,
                granted_art: Some("一门功法".to_owned()),
            }),
        );
        assert_eq!(guardian.opponent(), GUARDIAN_NAME);
        assert_eq!(walk(3).opponent(), WANDERER_NAME);
    }
}

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

use super::cultivation_settings::{CombatStats, derived_rows};
use super::*;
use bongocat_i18n::{format_text, text};
use bongocat_pomodoro::{
    ArtStrike, BreakthroughOutcome, DailyOutcome, DuelReport, DuelRound, ExplorationFindView,
    ExplorationOutcome, Fighter, GUARDIAN_NAME, PLAYER_NAME, TRIAL_OPPONENT_NAME, TrialOutcome,
    WANDERER_NAME,
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

/// How wide the fight is drawn.
///
/// Wide enough for the two columns of numbers to stand beside each other, which
/// is the point of the layout: what each side brought, read across.
const PANEL_WIDTH: f32 = 640.0;

/// How tall the whole panel may grow. The settings window is not tall enough
/// for more, and a report that has to be scrolled to be closed is a report in
/// the way.
const PANEL_MAX_HEIGHT: f32 = 560.0;

/// How wide one health bar is drawn.
const HEALTH_BAR_WIDTH: f32 = 260.0;

/// How tall the log may grow before it scrolls.
const LOG_MAX_HEIGHT: f32 = 200.0;

/// Which of the two paying actions the fight being replayed settled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CultivationDuelSource {
    /// The guardian at the end of a realm.
    Breakthrough,
    /// A walk out of the door.
    Exploration,
    /// One floor of the 试炼 tower.
    Trial,
    /// The morning ambush the day opened with.
    Daily,
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
    /// A 试炼 floor: the fight the warden settled, and what clearing it paid.
    Trial(TrialOutcome),
    /// The morning ambush: the fight, and what driving it off paid.
    Daily(DailyOutcome),
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
            CultivationDuelPayload::Exploration(_, _, duel) => duel
                .as_ref()
                .filter(|_| self.settled_a_fight())
                .map_or(&[], |duel| &duel.rounds),
            CultivationDuelPayload::Trial(outcome) => &outcome.duel.rounds,
            CultivationDuelPayload::Daily(outcome) => {
                outcome.duel.as_ref().map_or(&[], |duel| &duel.rounds)
            }
        }
    }

    /// Whether this payload settled a fight at all.
    ///
    /// A walk that met nobody has no fight, whatever report the payload still
    /// carries beside it: the outcome is what says whether a fight happened.
    /// The domain clears the report for a quiet walk and the page refuses to
    /// pair one with a quiet walk's outcome — this is the same rule read once
    /// more where the popup draws, so a report that ever slipped through both
    /// would still draw no fight, no log and no verdict.
    fn settled_a_fight(&self) -> bool {
        match &self.payload {
            CultivationDuelPayload::Exploration(outcome, _, _) => matches!(
                outcome,
                ExplorationOutcome::Victory | ExplorationOutcome::Defeat
            ),
            _ => true,
        }
    }

    /// The catalog key naming whoever the cultivator is fighting.
    fn opponent(&self) -> &'static str {
        match self.source {
            CultivationDuelSource::Breakthrough => GUARDIAN_NAME,
            CultivationDuelSource::Exploration => WANDERER_NAME,
            CultivationDuelSource::Trial => TRIAL_OPPONENT_NAME,
            CultivationDuelSource::Daily => WANDERER_NAME,
        }
    }

    /// The report, when the payload settled a fight at all.
    fn report(&self) -> Option<&DuelReport> {
        match &self.payload {
            CultivationDuelPayload::Breakthrough(outcome) => Some(&outcome.duel),
            CultivationDuelPayload::Exploration(_, _, duel) => {
                duel.as_ref().filter(|_| self.settled_a_fight())
            }
            CultivationDuelPayload::Trial(outcome) => Some(&outcome.duel),
            CultivationDuelPayload::Daily(outcome) => outcome.duel.as_ref(),
        }
    }

    /// The two fighters, when the payload settled a fight at all.
    ///
    /// A walk that met nobody has nobody to show, and a popup that drew empty
    /// columns for it would be padding.
    fn sides(&self) -> Option<(&Fighter, &Fighter)> {
        match &self.payload {
            CultivationDuelPayload::Breakthrough(outcome) => {
                Some((&outcome.duel.player, &outcome.duel.opponent))
            }
            CultivationDuelPayload::Exploration(_, _, duel) => duel
                .as_ref()
                .filter(|_| self.settled_a_fight())
                .map(|duel| (&duel.player, &duel.opponent)),
            CultivationDuelPayload::Trial(outcome) => {
                Some((&outcome.duel.player, &outcome.duel.opponent))
            }
            CultivationDuelPayload::Daily(outcome) => outcome
                .duel
                .as_ref()
                .map(|duel| (&duel.player, &duel.opponent)),
        }
    }

    /// Whether the cultivator came out of it standing. A walk that met nobody is
    /// neither a win nor a loss, and says so by there being no verdict to draw:
    /// `fought()` is false for it, so the `false` below is never painted as a
    /// defeat — a quiet walk is not a lost fight.
    fn won(&self) -> bool {
        match &self.payload {
            CultivationDuelPayload::Breakthrough(outcome) => outcome.duel.won,
            CultivationDuelPayload::Exploration(outcome, _, _) => match outcome {
                ExplorationOutcome::Victory => true,
                ExplorationOutcome::Defeat => false,
                _ => false,
            },
            CultivationDuelPayload::Trial(outcome) => outcome.duel.won,
            CultivationDuelPayload::Daily(outcome) => outcome.won,
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
///
/// Laid out the way a fight is watched rather than the way it is logged: the
/// two health bars on top, the two sides' numbers under them — the cultivator
/// on the left, whoever they are fighting on the right — and the blow-by-blow
/// log below both. The verdict waits until the last blow has been shown.
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
                .max_h(px(PANEL_MAX_HEIGHT))
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
                .children(health_bars(duel, locale, tokens))
                .children(fighter_columns(duel, locale, tokens))
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
        CultivationDuelPayload::Exploration(_, _, _) => {
            // The title follows the same rule the log and the verdict do: a
            // walk that met nobody is an 探索, whatever report rides beside it.
            let key = if duel.fought() {
                "cultivation.duel.title_exploration"
            } else {
                "cultivation.explore.title"
            };
            text(locale, key).to_string()
        }
        // The floor is the whole point of the fight, so it is in the title
        // beside the name rather than buried in the payoff.
        CultivationDuelPayload::Trial(outcome) => format!(
            "{} · {}",
            text(locale, "cultivation.duel.title_trial"),
            format_text(
                locale,
                "cultivation.trial.floor",
                &[
                    ("floor", outcome.floor.to_string()),
                    (
                        "power",
                        bongocat_i18n::compact_number(u64::from(bongocat_pomodoro::warden_power(
                            outcome.floor
                        )))
                    ),
                ],
            )
        ),
        CultivationDuelPayload::Daily(_) => {
            text(locale, "cultivation.duel.title_daily").to_string()
        }
    }
}

/// The two health bars, side by side, as the replay has them.
///
/// The bars start full and take the damage the shown blows did, so a fight
/// half-replayed reads as a fight half-fought rather than as two full bars and
/// a spoilered ending. Shown above the two columns because it is the one
/// comparison the whole popup turns on.
fn health_bars(duel: &CultivationDuel, locale: &str, tokens: Tokens) -> Option<impl IntoElement> {
    let report = duel.report()?;
    let (player_health, opponent_health) = report.health_after(duel.revealed);
    Some(
        div()
            .id("cultivation-duel-health")
            .flex()
            .flex_row()
            .gap_3()
            .child(health_column(
                "cultivation-duel-health-player",
                text(locale, "cultivation.duel.side_you").to_string(),
                &report.player,
                player_health,
                tokens.accent,
                locale,
                tokens,
            ))
            .child(health_column(
                "cultivation-duel-health-opponent",
                text(locale, "cultivation.duel.side_foe").to_string(),
                &report.opponent,
                opponent_health,
                tokens.danger,
                locale,
                tokens,
            )),
    )
}

/// One side of the comparison: who it is, how much health it has left, and the
/// bar that says it at a glance.
///
/// The fighter carries the name and the health it walked in with, so the column
/// is given the fighter and what is left of it rather than a copy of its numbers
/// — the bar can only be as full as the fighter it belongs to.
fn health_column(
    id: &'static str,
    side: String,
    fighter: &Fighter,
    health: u32,
    color: Hsla,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let max = fighter.health;
    let fraction = if max == 0 {
        0.0
    } else {
        health as f32 / max as f32
    };
    div()
        .id(id)
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(tokens.text)
                        .child(format!("{side} · {}", text(locale, &fighter.name))),
                )
                .child(div().text_xs().text_color(tokens.muted).child(format_text(
                    locale,
                    "cultivation.duel.hp",
                    &[("health", health.to_string()), ("max", max.to_string())],
                ))),
        )
        .child(health_bar(tokens, fraction, color))
        // Marked so a frame can be asked whether the two halves of the
        // comparison are both on screen, the way the page's own rows are.
        .test_support()
}

/// The bar itself. A fixed pixel width rather than a percentage, the same way
/// the growth bar on the page is drawn: it sits beside the numbers that say the
/// same thing exactly, and a fixed width needs no layout pass to agree.
fn health_bar(tokens: Tokens, fraction: f32, color: Hsla) -> impl IntoElement {
    div()
        .w(px(HEALTH_BAR_WIDTH))
        .h(px(8.0))
        .rounded_full()
        .bg(tokens.border)
        .child(
            div()
                .w(px(HEALTH_BAR_WIDTH * fraction.clamp(0.0, 1.0)))
                .h(px(8.0))
                .rounded_full()
                .bg(color),
        )
}

/// The two sides' numbers, left and right: what the cultivator brought, and
/// what they were up against.
///
/// The same eight the page's 外显属性 panel shows, read off the fighters the
/// report carries — so the popup says what the fight was actually between, and
/// a walk that met nobody shows no columns at all.
fn fighter_columns(
    duel: &CultivationDuel,
    locale: &str,
    tokens: Tokens,
) -> Option<impl IntoElement> {
    let (player, opponent) = duel.sides()?;
    Some(
        div()
            .id("cultivation-duel-sides")
            .flex()
            .flex_row()
            .gap_3()
            .child(fighter_column(
                "cultivation-duel-side-player",
                text(locale, "cultivation.duel.side_you").to_string(),
                player,
                locale,
                tokens,
            ))
            .child(fighter_column(
                "cultivation-duel-side-opponent",
                text(locale, "cultivation.duel.side_foe").to_string(),
                opponent,
                locale,
                tokens,
            )),
    )
}

/// One side's numbers: who it is, the eight the fight used, and the 外功 it is
/// swinging when it has one.
fn fighter_column(
    id: &'static str,
    side: String,
    fighter: &Fighter,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let stats = CombatStats::of_fighter(fighter);
    let rows = derived_rows(&stats, locale);
    let mut column = div()
        .id(id)
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(tokens.text)
                .child(format!("{side} · {}", text(locale, &fighter.name))),
        )
        .children(
            rows.into_iter()
                .map(|(_, line)| div().text_xs().text_color(tokens.muted).child(line)),
        );
    if let Some(art) = &fighter.art {
        column = column.child(art_line(format!("{id}-art"), art, locale, tokens));
    }
    // Marked so a frame can be asked whether both sides' numbers are on screen.
    column.test_support()
}

/// The 外功 a side is swinging: what it multiplies by, what it costs, and what
/// its last 阶 added on top.
fn art_line(id: String, art: &ArtStrike, locale: &str, tokens: Tokens) -> impl IntoElement {
    let mut line = format!(
        "{}：{}",
        text(locale, "cultivation.art.kind.external"),
        art.name
    );
    line.push_str(&format!(
        " · {}",
        format_text(
            locale,
            "cultivation.art.multiplier",
            &[("value", (art.multiplier_permille / 10).to_string())],
        )
    ));
    line.push_str(&format!(
        " · {}",
        format_text(
            locale,
            "cultivation.art.cost",
            &[("value", art.cost.to_string())],
        )
    ));
    if art.mastery_permille > 0 {
        line.push_str(&format!(
            " · {}",
            format_text(
                locale,
                "cultivation.art.mastery_damage",
                &[("value", (art.mastery_permille / 10).to_string())],
            )
        ));
    }
    div().id(id).text_xs().text_color(tokens.accent).child(line)
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
    // Marked so a frame can be asked whether the log is on screen at all.
    log.test_support()
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
                if let Some(amount) = outcome.stones {
                    lines.push(payoff(
                        "cultivation-duel-pay-stones",
                        format_text(
                            locale,
                            "cultivation.duel.reward_stones",
                            &[("value", amount.to_string())],
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
        CultivationDuelPayload::Trial(outcome) => {
            lines.push(payoff(
                "cultivation-duel-trial-floor",
                format_text(
                    locale,
                    if outcome.duel.won {
                        "cultivation.trial.won"
                    } else {
                        "cultivation.trial.lost"
                    },
                    &[("floor", outcome.floor.to_string())],
                ),
                if outcome.duel.won {
                    tokens.accent
                } else {
                    tokens.danger
                },
            ));
            if let Some(seed) = &outcome.found.seed {
                lines.push(payoff(
                    "cultivation-duel-trial-seed",
                    format_text(
                        locale,
                        "cultivation.explore.found_seed",
                        &[("name", seed_label(locale, seed))],
                    ),
                    tokens.accent,
                ));
            }
            if let Some(item) = &outcome.found.gear {
                lines.push(payoff(
                    "cultivation-duel-trial-gear",
                    format_text(
                        locale,
                        "cultivation.explore.found_gear",
                        &[("name", item_label(locale, item))],
                    ),
                    tokens.accent,
                ));
            }
            if let Some(name) = &outcome.found.art {
                lines.push(payoff(
                    "cultivation-duel-trial-art",
                    format_text(
                        locale,
                        "cultivation.explore.found_art",
                        &[("name", name.clone())],
                    ),
                    tokens.accent,
                ));
            }
            if let Some(amount) = outcome.found.stones {
                lines.push(payoff(
                    "cultivation-duel-trial-stones",
                    format_text(
                        locale,
                        "cultivation.duel.reward_stones",
                        &[("value", amount.to_string())],
                    ),
                    tokens.accent,
                ));
            }
        }
        CultivationDuelPayload::Daily(outcome) => {
            let key = if outcome.won {
                "cultivation.daily.ambush"
            } else {
                "cultivation.daily.ambush_lost"
            };
            lines.push(payoff(
                "cultivation-duel-daily-verdict",
                text(locale, key).to_string(),
                if outcome.won {
                    tokens.accent
                } else {
                    tokens.danger
                },
            ));
            if let Some(seed) = &outcome.found.seed {
                lines.push(payoff(
                    "cultivation-duel-daily-seed",
                    format_text(
                        locale,
                        "cultivation.explore.found_seed",
                        &[("name", seed_label(locale, seed))],
                    ),
                    tokens.accent,
                ));
            }
            if let Some(item) = &outcome.found.gear {
                lines.push(payoff(
                    "cultivation-duel-daily-gear",
                    format_text(
                        locale,
                        "cultivation.explore.found_gear",
                        &[("name", item_label(locale, item))],
                    ),
                    tokens.accent,
                ));
            }
            if let Some(name) = &outcome.found.art {
                lines.push(payoff(
                    "cultivation-duel-daily-art",
                    format_text(
                        locale,
                        "cultivation.explore.found_art",
                        &[("name", name.clone())],
                    ),
                    tokens.accent,
                ));
            }
            if let Some(amount) = outcome.found.stones {
                lines.push(payoff(
                    "cultivation-duel-daily-stones",
                    format_text(
                        locale,
                        "cultivation.duel.reward_stones",
                        &[("value", amount.to_string())],
                    ),
                    tokens.accent,
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
            // Each thing the walk turned up gets its own line: the stones are
            // rolled beside the finding rather than in place of it, so a walk
            // can come back with a herb and a handful of money both.
            if let Some(found) = found {
                if let Some(seed) = &found.seed {
                    lines.push(payoff(
                        "cultivation-duel-walk-seed",
                        format_text(
                            locale,
                            "cultivation.explore.found_seed",
                            &[("name", seed_label(locale, seed))],
                        ),
                        tokens.accent,
                    ));
                }
                if let Some(item) = &found.gear {
                    lines.push(payoff(
                        "cultivation-duel-walk-gear",
                        format_text(
                            locale,
                            "cultivation.explore.found_gear",
                            &[("name", item_label(locale, item))],
                        ),
                        tokens.accent,
                    ));
                }
                if let Some(name) = &found.art {
                    lines.push(payoff(
                        "cultivation-duel-walk-art",
                        format_text(
                            locale,
                            "cultivation.explore.found_art",
                            &[("name", name.clone())],
                        ),
                        tokens.accent,
                    ));
                }
                if let Some(amount) = found.stones {
                    lines.push(payoff(
                        "cultivation-duel-walk-stones",
                        format_text(
                            locale,
                            "cultivation.explore.found_stones",
                            &[("value", amount.to_string())],
                        ),
                        tokens.accent,
                    ));
                }
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
        // Marked so a frame can be asked what a settled fight paid.
        .test_support()
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

    /// One side of a test fight: `health` of flesh and an even hand.
    fn side(name: &str, health: u32) -> Fighter {
        Fighter {
            name: name.to_owned(),
            health,
            attack: 10,
            inner: 0,
            spell: 0,
            resist: 0,
            speed: 10,
            hit: 900,
            evade: 0,
            art: None,
        }
    }

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
            // The names are the catalog keys the domain writes, the way a real
            // report carries them: the fold asks who swung by name.
            player: side(PLAYER_NAME, 100),
            opponent: side("opponent", 100),
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
        assert!(duel.sides().is_none(), "and nobody to draw columns for");
        assert!(
            !duel.reveal_all(),
            "skipping a walk that found nothing does nothing"
        );
    }

    /// The bug this guards: a quiet walk that arrived carrying an older walk's
    /// report drew that fight's log and called the afternoon 败北 under it. The
    /// outcome is what says whether a fight happened, so a report beside a
    /// quiet walk draws nothing at all.
    #[test]
    fn a_quiet_walk_ignores_an_older_walks_report() {
        let duel = CultivationDuel::new(
            CultivationDuelSource::Exploration,
            CultivationDuelPayload::Exploration(ExplorationOutcome::Stones, None, Some(report(6))),
        );
        assert!(
            duel.rounds().is_empty(),
            "a walk that met nobody has no blows to replay"
        );
        assert!(!duel.fought(), "and so no verdict to draw");
        assert!(duel.report().is_none(), "and no bars to draw");
        assert!(duel.sides().is_none(), "and no columns to draw");
    }

    /// The bars over the log follow the blows that have been shown: a replay
    /// half-way through is a fight half-fought, not two full bars and a
    /// spoilered ending.
    #[test]
    fn the_health_bars_follow_the_blows_that_have_been_shown() {
        let mut duel = walk(6);
        let whole = duel.report().expect("the walk settled a fight").clone();
        assert_eq!(whole.health_after(0), (100, 100), "both sides start whole");
        assert_eq!(
            whole.health_after(3),
            (100, 70),
            "three of the six blows are the foe's to take"
        );
        duel.reveal_all();
        assert_eq!(
            whole.health_after(duel.revealed),
            (100, 40),
            "the whole fight is fought"
        );
    }

    /// The columns carry the numbers each side fought with, so the popup can
    /// say what the fight was between and not only how it ended.
    #[test]
    fn the_columns_carry_the_numbers_each_side_fought_with() {
        let duel = walk(3);
        let (player, opponent) = duel.sides().expect("the walk settled a fight");
        assert_eq!(player.name, PLAYER_NAME);
        assert_eq!(opponent.name, "opponent");
        assert_eq!(player.health, 100, "the health the bars start from");
        assert_eq!(opponent.attack, 10);
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
                stones: None,
            }),
        );
        assert_eq!(guardian.opponent(), GUARDIAN_NAME);
        assert_eq!(walk(3).opponent(), WANDERER_NAME);
    }
}

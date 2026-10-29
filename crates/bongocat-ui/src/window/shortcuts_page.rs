use super::*;

use super::behavior_name::{
    behavior_name_edit_control, behavior_name_editor, behavior_name_key, behavior_name_part_id,
};
use gpui_kit::AnyElement;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::Sizable as _;

/// Which set of shortcut bindings one group of the page renders.
///
/// The two sets used to be tabs. They are titled groups now: `SettingPage`
/// cannot host child pages, but the settings component renders every titled
/// group of a page that has more than one group as a second-level sidebar
/// entry, so the sidebar lists both scopes under "Shortcuts" while the body
/// renders them one after the other.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ShortcutScope {
    Window,
    Model,
}

impl ShortcutScope {
    /// Whether this scope's bindings may reach the platform table.
    ///
    /// Both gates are configured positively and rendered as "enable …"
    /// switches, so the switch and the command it sends
    /// all read the same configuration field — there is no inversion to keep in
    /// step anywhere.
    pub(super) fn is_enabled(self, snapshot: &SettingsSnapshot) -> bool {
        match self {
            Self::Window => snapshot.command_shortcuts_enabled,
            Self::Model => snapshot.behavior_shortcuts_enabled,
        }
    }

    /// The label of this scope's gate.
    ///
    /// The visible row reads one string, so it cannot drift apart from the
    /// catalog. The literal is written out in full rather than assembled from a
    /// suffix. The row carries
    /// no description: "enable …" already says what the switch does, and a
    /// second line would only be the label in other words.
    pub(super) fn gate_label(self, language: SettingsLanguage) -> &'static str {
        match self {
            Self::Window => bongocat_i18n::text(
                language.catalog_locale(),
                "shortcuts.switches.enable_window_shortcuts.label",
            ),
            Self::Model => bongocat_i18n::text(
                language.catalog_locale(),
                "shortcuts.switches.enable_model_behavior_shortcuts.label",
            ),
        }
    }

    /// The scope a shortcut target belongs to.
    ///
    /// The one mapping from a target to the switch that gates its row: the
    /// render layer and the mutating methods read it through
    /// [`Self::is_enabled`] instead of re-matching the target type.
    pub(super) fn for_target(target: &ShortcutCaptureTarget) -> Self {
        match target {
            ShortcutCaptureTarget::Command(_) => Self::Window,
            ShortcutCaptureTarget::ModelBehavior { .. } => Self::Model,
        }
    }

    /// The switch that turns this scope's bindings off and on again.
    ///
    /// It is the first row of the scope's group, above the bindings it gates,
    /// because the gate used to live on the old Interaction page and a list of
    /// chords that silently does nothing is the state that page produced.
    /// Switching it back on never needs the chords to be recorded again: no
    /// gate rewrites the bindings, they only stay out of the platform table.
    fn gate_item(
        self,
        language: SettingsLanguage,
        view: Entity<SettingsView>,
        gate: SettingGate,
    ) -> SettingItem {
        SettingItem::new(
            self.gate_label(language),
            SettingField::switch(
                {
                    let view = view.clone();
                    move |app| {
                        view.read(app)
                            .snapshot
                            .as_ref()
                            .is_some_and(|snapshot| self.is_enabled(snapshot))
                    }
                },
                {
                    let view = view.clone();
                    move |enabled, app| {
                        view.update(app, |view, cx| match self {
                            Self::Window => view.set_command_shortcuts_enabled(enabled, cx),
                            Self::Model => view.set_behavior_shortcuts_enabled(enabled, cx),
                        });
                    }
                },
            ),
        )
        // The switch is disabled by the unified gate rule's switch arm: only
        // structurally blocked editing dims it, never its own state — a gate
        // that dimmed itself while off could never be turned back on.
        .disabled(gate.disables_switch())
    }

    /// The name of this scope, used as its group heading and its sidebar entry.
    pub(super) fn title(self, language: SettingsLanguage) -> &'static str {
        bongocat_i18n::text(
            language.catalog_locale(),
            match self {
                Self::Window => "shortcuts.scopes.window",
                Self::Model => "shortcuts.scopes.model",
            },
        )
    }

    /// The rows this scope binds.
    ///
    /// The window scope always lists every application command, so only the
    /// model scope can come back empty — when the active model declares no
    /// behavior.
    pub(super) fn rows(
        self,
        shortcuts: &SettingsShortcuts,
        active_model: Option<&SettingsModelKey>,
        entries: &[SettingsModelEntry],
        behavior_names: &[SettingsModelBehaviorName],
    ) -> Vec<ShortcutRow> {
        match self {
            Self::Window => window_shortcut_rows(shortcuts),
            Self::Model => shortcut_behavior_rows(shortcuts, active_model, entries, behavior_names),
        }
    }

    /// Where this scope's rows start in the page-wide row order.
    ///
    /// Both scopes are one page, so their capture and clear controls share one
    /// keyboard tab order is based on the combined list of rows.
    pub(super) fn row_index_offset(self, shortcuts: &SettingsShortcuts) -> usize {
        match self {
            Self::Window => 0,
            Self::Model => window_shortcut_rows(shortcuts).len(),
        }
    }

    /// The message to show while this scope has no rows at all.
    pub(super) fn empty_message(self, language: SettingsLanguage) -> Option<&'static str> {
        match self {
            Self::Window => None,
            Self::Model => Some(bongocat_i18n::text(
                language.catalog_locale(),
                "models.behaviors.empty",
            )),
        }
    }
}

/// One scope of the shortcut page as a titled group.
///
/// `gate` is the scope's [`SettingGate`] (see `setting_gate`): the stable
/// structural editing state combined with this scope's switch. It
/// deliberately excludes the transient in-flight `pending` flag — that one
/// flips on and off around every save, and threading it into every row dimmed
/// and re-enabled the whole page on each control change, which read as the
/// page visibly refreshing. Requests that land while another is in flight
/// already no-op through `start_request` and `shortcut_commands_available`.
pub(super) fn group(
    scope: ShortcutScope,
    language: SettingsLanguage,
    view: Entity<SettingsView>,
    gate: SettingGate,
) -> SettingGroup {
    // The group heading names the scope in the body and in the sidebar, so the
    // custom item below carries no label of its own: a `SettingItem` with a
    // title would print the scope name a second time, directly under the
    // heading. Search matches an item by its title or keywords, and a custom
    // element has no title, so the page and scope names are passed as keywords
    // to keep the page reachable through the sidebar's search box.
    let keywords = [
        SettingsNavigationPage::Shortcuts.title(language),
        scope.title(language).into(),
    ];
    SettingGroup::new()
        .title(scope.title(language))
        .item(scope.gate_item(language, view.clone(), gate))
        .item(
            SettingItem::render({
                let view = view.clone();
                move |_: &RenderOptions, window: &mut Window, app: &mut App| {
                    let snapshot = view.read(app).snapshot.clone();
                    let tokens = Tokens::from_theme(app);
                    view.update(app, move |view, cx| {
                        content(view, window, cx, snapshot.as_ref(), scope, gate, tokens)
                    })
                    .into_any_element()
                }
            })
            .keywords(keywords),
        )
}

pub(super) fn content(
    view: &SettingsView,
    window: &Window,
    cx: &mut Context<SettingsView>,
    snapshot: Option<&SettingsSnapshot>,
    scope: ShortcutScope,
    gate: SettingGate,
    tokens: Tokens,
) -> Stateful<Div> {
    let language = snapshot.map_or(SettingsLanguage::English, |snapshot| {
        snapshot.resolved_language
    });
    div()
        .min_w_0()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .text_color(tokens.text)
        .id(match scope {
            ShortcutScope::Window => "window-shortcuts-content",
            ShortcutScope::Model => "model-shortcuts-content",
        })
        .when_some(snapshot, |content, snapshot| {
            let rows = scope.rows(
                &snapshot.shortcuts,
                snapshot.active_model.as_ref(),
                &snapshot.model_catalog.entries,
                &snapshot.model_behavior_names,
            );
            let row_index_offset = scope.row_index_offset(&snapshot.shortcuts);
            let empty_message = scope.empty_message(language).filter(|_| rows.is_empty());
            content
                .when_some(empty_message, |content, message| {
                    content.child(div().text_sm().text_color(tokens.muted).child(message))
                })
                .children(rows.iter().enumerate().map(|(index, row)| {
                    shortcut_row(
                        view,
                        window,
                        cx,
                        language,
                        gate,
                        tokens,
                        scope,
                        row.clone(),
                        row_index_offset + index,
                    )
                }))
                .when(
                    matches!(scope, ShortcutScope::Model) && !rows.is_empty(),
                    |content| content.child(behavior_palettes(cx, language, gate, tokens, &rows)),
                )
        })
}

/// How many names one row of a behavior palette holds.
const BEHAVIOR_PALETTE_COLUMNS: usize = 4;

/// The behavior palettes: every expression and every motion of the active model
/// as a block of names, four to a row, that plays the behavior when clicked.
///
/// The shortcut rows above already bind a chord to each behavior, but a row
/// costs a whole line, so a model with twenty behaviors reads as twenty
/// lines of chords and nothing else. The palette is the same list in the shape
/// the model package declares it — expressions in one block, motions in the
/// other — and a name is the entire control: there is no chord to read and
/// nothing to record, so pressing one only ever plays.
fn behavior_palettes(
    cx: &mut Context<SettingsView>,
    language: SettingsLanguage,
    gate: SettingGate,
    tokens: Tokens,
    rows: &[ShortcutRow],
) -> Div {
    let disabled = gate.disables_controls();
    let mut palettes = div().flex().flex_col().gap_3().w_full().child(
        div()
            .text_xs()
            .text_color(tokens.muted)
            .child(bongocat_i18n::text(
                language.catalog_locale(),
                "shortcuts.behavior_palette.hint",
            )),
    );
    for (expressions, heading) in [
        (true, "shortcuts.behavior_palette.expressions"),
        (false, "shortcuts.behavior_palette.motions"),
    ] {
        let blocks = palette_blocks(rows, expressions);
        if blocks.is_empty() {
            continue;
        }
        let mut block = div().flex().flex_col().gap_2().w_full().child(
            div()
                .text_sm()
                .text_color(tokens.text)
                .child(bongocat_i18n::text(language.catalog_locale(), heading)),
        );
        for line in blocks {
            let mut row_line = div().flex().flex_row().gap_2().w_full();
            for row in line {
                row_line =
                    row_line.child(behavior_palette_chip(cx, language, row, disabled, tokens));
            }
            block = block.child(row_line);
        }
        palettes = palettes.child(block);
    }
    palettes
}

/// The behavior palette as the ModelBehavior page draws it.
///
/// The page previews the active model's behaviors, and this is the same list
/// the Shortcuts page's model scope draws: expressions first, then motions,
/// four names to a row, each playing its behavior when clicked. It reads the
/// active model from the snapshot and draws nothing when the model declares no
/// behavior, so a page with nothing to preview still reads as the model-wide
/// settings it is.
pub(super) fn model_behavior_palette(
    cx: &mut Context<SettingsView>,
    language: SettingsLanguage,
    gate: SettingGate,
    tokens: Tokens,
    snapshot: Option<&SettingsSnapshot>,
) -> Div {
    let Some(snapshot) = snapshot else {
        return div();
    };
    let rows = ShortcutScope::Model.rows(
        &snapshot.shortcuts,
        snapshot.active_model.as_ref(),
        &snapshot.model_catalog.entries,
        &snapshot.model_behavior_names,
    );
    if rows.is_empty() {
        return div();
    }
    behavior_palettes(cx, language, gate, tokens, &rows)
}

/// One kind's names, in rows of [`BEHAVIOR_PALETTE_COLUMNS`], in the order the
/// model package declares them.
///
/// The palette draws the two kinds as two blocks — expressions, then motions —
/// so the kind a name belongs to is decided here rather than by the caller
/// re-reading each row's behavior. An empty result means the model has none of
/// that kind, and the palette draws no heading for an empty block.
fn palette_blocks(rows: &[ShortcutRow], expressions: bool) -> Vec<Vec<&ShortcutRow>> {
    rows.iter()
        .filter(|row| {
            let is_expression = row.playable.as_ref().is_some_and(|playable| {
                matches!(playable.behavior, SettingsModelBehavior::Expression { .. })
            });
            is_expression == expressions
        })
        .collect::<Vec<_>>()
        .chunks(BEHAVIOR_PALETTE_COLUMNS)
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// One name in a palette: a chip the whole width of its column.
///
/// The chip is a name and nothing else, so it can be as narrow as a column of
/// four and still say which behavior it plays. It is inert while the gate is
/// shut rather than merely painted lighter: a chip that looks clickable and
/// does nothing is worse than one that is visibly off.
fn behavior_palette_chip(
    cx: &mut Context<SettingsView>,
    language: SettingsLanguage,
    row: &ShortcutRow,
    disabled: bool,
    tokens: Tokens,
) -> AnyElement {
    let Some(playable) = row.playable.clone() else {
        return div().into_any_element();
    };
    let target = row.target.clone();
    let id = match &row.target {
        ShortcutCaptureTarget::ModelBehavior { behavior_id, .. } => {
            format!("behavior-palette-{behavior_id}")
        }
        ShortcutCaptureTarget::Command(command) => format!("behavior-palette-{command}"),
    };
    let chip = div()
        .id(id)
        .test_support()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(tokens.border)
        .bg(tokens.overlay)
        .child(
            div()
                .text_xs()
                .text_color(tokens.text)
                .truncate()
                .child(row.name(language)),
        );
    if disabled {
        return chip.into_any_element();
    }
    chip.cursor_pointer()
        .hover(|style| style.border_color(tokens.accent))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.play_shortcut_behavior(
                target.clone(),
                playable.model.clone(),
                playable.behavior.clone(),
                cx,
            );
        }))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn shortcut_row(
    view: &SettingsView,
    window: &Window,
    cx: &mut Context<SettingsView>,
    language: SettingsLanguage,
    gate: SettingGate,
    tokens: Tokens,
    scope: ShortcutScope,
    row: ShortcutRow,
    row_index: usize,
) -> Div {
    let target_name = row.name(language);
    let target = row.target;
    let playable = row.playable;
    let shortcut = row.shortcut;
    let capture = view
        .shortcut_capture
        .as_ref()
        .filter(|capture| capture.target == target);
    let capturing = capture.is_some();
    // The unified gate rule's control arm: the row dims and drops its
    // interaction handlers while editing is structurally blocked or this
    // scope's switch is off. A disabled row is inert, not merely painted
    // lighter — the mutators guard on the same predicate.
    let row_disabled = gate.disables_controls();
    // The pencil beside the name is the rename entry, and it is the only one: the
    // name itself stays plain text, so reading a row never risks starting an edit.
    // A row gains a name without gaining a fourth control in its tab order: the
    // frame's chord, its play button and its clear button keep their positions and
    // their tab indices exactly as they were, and a row nobody renamed still reads
    // as plain text because the pencil only appears on a behavior row that can
    // actually be renamed.
    let renamable = playable.is_some() && !row_disabled;
    let behavior_id = match &target {
        ShortcutCaptureTarget::ModelBehavior { behavior_id, .. } => Some(behavior_id.clone()),
        ShortcutCaptureTarget::Command(_) => None,
    };
    let editing = match (&playable, behavior_id.as_deref()) {
        (Some(playable), Some(behavior_id)) => view
            .behavior_name
            .as_ref()
            .filter(|draft| draft.is_for(&playable.model, behavior_id)),
        _ => None,
    };
    let label = match editing {
        Some(draft) => behavior_name_editor(draft, cx),
        None => {
            let key = behavior_name_key(&target);
            let mut label = div()
                .id(behavior_name_part_id(&key, "label"))
                .test_support()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_1()
                .rounded_sm()
                // The name is its own element so it can be measured against the
                // pencil beside it, and so a name longer than the row gives up
                // characters instead of pushing the chord frame off the window.
                .child(
                    div()
                        .id(behavior_name_part_id(&key, "name"))
                        .test_support()
                        .min_w_0()
                        .truncate()
                        .child(target_name),
                );
            if renamable {
                label = label.child(behavior_name_edit_control(
                    &target,
                    bongocat_i18n::text(
                        language.catalog_locale(),
                        "shortcuts.behavior_names.rename.tooltip",
                    ),
                    tokens.accent,
                    cx,
                ));
            }
            label.into_any_element()
        }
    };
    let focus = view
        .shortcut_row_focus
        .get(&target)
        .expect("shortcut row focus is synchronized")
        .clone();
    let play_focus = view
        .shortcut_play_focus
        .get(&target)
        .expect("shortcut play focus is synchronized")
        .clone();
    let clear_focus = view
        .shortcut_clear_focus
        .get(&target)
        .expect("shortcut clear focus is synchronized")
        .clone();
    let keyboard_target = target.clone();
    let clear_target = target.clone();
    let clear_key_target = target.clone();
    let clear_key_focus = clear_focus.clone();
    // The two scopes are one page and their element ids live in one namespace,
    // so the scope prefix keeps the ids apart without folding the row index
    // (which is already unique across the page) into the name.
    let (capture_id, play_id, clear_id) = match scope {
        ShortcutScope::Window => (
            ("capture-window-shortcut", row_index),
            ("play-window-shortcut", row_index),
            ("clear-window-shortcut", row_index),
        ),
        ShortcutScope::Model => (
            ("capture-model-shortcut", row_index),
            ("play-model-shortcut", row_index),
            ("clear-model-shortcut", row_index),
        ),
    };
    let chord = if let Some(capture) = capture {
        shortcut_capture_preview(&capture.modifiers, &capture.keys)
            .map(|shortcut| shortcut_display(&shortcut))
            .unwrap_or_else(|| {
                bongocat_i18n::text(
                    language.catalog_locale(),
                    "shortcuts.capture.press_to_record",
                )
                .to_owned()
            })
    } else if let Some(shortcut) = shortcut.as_ref() {
        shortcut_display(shortcut)
    } else {
        bongocat_i18n::text(
            language.catalog_locale(),
            "shortcuts.capture.click_to_record",
        )
        .to_owned()
    };
    // The frame around the chord is the group's: the play and clear controls sit
    // inside it, at the leading and trailing edges, so the row reads as one
    // control the way an input group's addons do. The chord itself keeps the
    // middle and stays centred, which is where it was when the clear button
    // lived outside the frame.
    let mut frame = div()
        .id(capture_id)
        // Observed before the focus binding, so the rendered test can click the
        // frame itself: the controls sit inside it, and "a press that is not on
        // one of them records a chord" is only measurable against its bounds.
        .test_support()
        .key_context("SettingsControl")
        .track_focus(&focus)
        .tab_index(shortcut_capture_tab_index(row_index))
        .h(px(32.))
        .min_w(px(180.))
        .flex()
        .items_center()
        .gap_1()
        .px_1()
        .border_1()
        .border_color(if capturing {
            tokens.accent
        } else {
            tokens.border
        })
        .rounded_md()
        .when(capturing, |this| this.focus_ring_style(window, cx))
        .cursor_pointer()
        .text_color(if capturing || shortcut.is_some() {
            tokens.text
        } else {
            tokens.muted
        })
        .when(row_disabled, |this| this.cursor_default());
    if let Some(playable) = playable {
        frame = frame.child(shortcut_play_control(
            cx,
            language,
            play_id.into(),
            play_focus,
            target.clone(),
            playable,
            shortcut_play_tab_index(row_index),
            row_disabled,
        ));
    }
    frame = frame
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(chord),
        )
        .child(shortcut_clear_control(
            cx,
            language,
            clear_id.into(),
            clear_focus,
            clear_key_focus,
            clear_target,
            clear_key_target,
            shortcut_clear_tab_index(row_index),
            // The clear control keeps its place on every row, the way an input
            // group's trailing button does, and only greys out when there is
            // nothing to clear — a control that came and went would move the
            // chord every time a binding was recorded or removed.
            row_disabled || shortcut.is_none(),
        ));
    if !row_disabled {
        frame = frame
            .on_click(cx.listener(move |view, _, window, cx| {
                view.begin_shortcut_capture(target.clone(), window, cx);
            }))
            .when(capturing, |this| {
                this.on_mouse_down_out(cx.listener(|view, _, window, cx| {
                    view.cancel_shortcut_capture(cx);
                    window.blur(cx);
                }))
            })
            .on_key_down(cx.listener(move |view, event, window, cx| {
                if view.shortcut_capture.is_none() && is_activation_key(event) {
                    cx.stop_propagation();
                    view.begin_shortcut_capture(keyboard_target.clone(), window, cx);
                }
            }));
    }
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_sm()
        // The unified gate rule dims the whole row — label included — the
        // same way `SettingItem::disabled` dims a packaged field's title and
        // description, so a gated setting reads as one unavailable entry
        // rather than a live label sitting next to a dead control.
        .when(row_disabled, |this| this.opacity(0.5))
        .child(label)
        .child(frame)
}

/// The compact icon button that lives inside a shortcut row's frame.
///
/// The row's controls are the only ones in the settings window that sit inside
/// another control's border, so they take the extra-small size that fits the
/// frame's height rather than the standard icon button the models page renders
/// beside a card. Ghost keeps the frame's own border the only one the row shows.
/// [`icon_command_control`] supplies the wrapper the control is queried and
/// tabbed through by.
fn shortcut_icon_button(
    id: &'static str,
    label: &'static str,
    icon: impl Into<Icon>,
    disabled: bool,
) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .tooltip(label)
        .disabled(disabled)
}

/// The play control that sits before the chord on a model behavior row.
///
/// Only a model behavior row has something to play, so an application command
/// renders no control here at all and its frame simply starts with the chord.
///
/// The press is stopped before it reaches the frame: the frame starts a capture
/// on any click inside it, and pressing play is not a request to record a chord.
/// The stop lives on a wrapper around the control rather than on the control
/// itself, because stopping an event also stops the handlers that come after it
/// — including the control's own click.
#[allow(clippy::too_many_arguments)]
fn shortcut_play_control(
    cx: &mut Context<SettingsView>,
    language: SettingsLanguage,
    id: ElementId,
    focus: FocusHandle,
    target: ShortcutCaptureTarget,
    playable: PlayableBehavior,
    tab_index: isize,
    disabled: bool,
) -> impl IntoElement {
    let focus_for_click = focus.clone();
    let focus_for_key = focus.clone();
    let click_target = target.clone();
    let key_target = target.clone();
    let click_model = playable.model.clone();
    let key_model = playable.model;
    let click_behavior = playable.behavior.clone();
    let key_behavior = playable.behavior;
    div()
        .flex_none()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            icon_command_control(
                &focus,
                tab_index,
                shortcut_icon_button(
                    "play-model-behavior-control",
                    bongocat_i18n::text(language.catalog_locale(), "shortcuts.actions.play"),
                    gpui_kit::assets::IconName::Play,
                    disabled,
                ),
            )
            .id(id)
            .test_support()
            .when(!disabled, |this| {
                this.on_click(cx.listener(move |view, _, window, cx| {
                    window.focus(&focus_for_click, cx);
                    view.play_shortcut_behavior(
                        click_target.clone(),
                        click_model.clone(),
                        click_behavior.clone(),
                        cx,
                    );
                }))
                .on_key_down(cx.listener(move |view, event, window, cx| {
                    if is_activation_key(event) {
                        cx.stop_propagation();
                        window.focus(&focus_for_key, cx);
                        view.play_shortcut_behavior(
                            key_target.clone(),
                            key_model.clone(),
                            key_behavior.clone(),
                            cx,
                        );
                    }
                }))
            }),
        )
}

/// The clear control that sits after the chord.
///
/// It replaces the standalone button the row used to render beside the frame,
/// and keeps the same two jobs: drop this row's binding, and put the row's
/// keyboard focus back on the control it was pressed from. Like the play
/// control's, its press is stopped by a wrapper so it cannot reach the frame.
#[allow(clippy::too_many_arguments)]
fn shortcut_clear_control(
    cx: &mut Context<SettingsView>,
    language: SettingsLanguage,
    id: ElementId,
    focus: FocusHandle,
    key_focus: FocusHandle,
    target: ShortcutCaptureTarget,
    key_target: ShortcutCaptureTarget,
    tab_index: isize,
    disabled: bool,
) -> impl IntoElement {
    let focus_for_click = focus.clone();
    div()
        .flex_none()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            icon_command_control(
                &focus,
                tab_index,
                shortcut_icon_button(
                    "clear-shortcut-control",
                    bongocat_i18n::text(language.catalog_locale(), "shortcuts.actions.clear"),
                    gpui_kit::assets::IconName::Close,
                    disabled,
                ),
            )
            .id(id)
            .test_support()
            .when(!disabled, |this| {
                this.on_click(cx.listener(move |view, _, window, cx| {
                    window.focus(&focus_for_click, cx);
                    view.clear_shortcut(target.clone(), cx);
                }))
                .on_key_down(cx.listener(move |view, event, window, cx| {
                    if is_activation_key(event) {
                        cx.stop_propagation();
                        window.focus(&key_focus, cx);
                        view.clear_shortcut(key_target.clone(), cx);
                    }
                }))
            }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal palette row for one behavior, enough for `palette_blocks` to
    /// sort it by kind.
    fn palette_row(behavior: SettingsModelBehavior) -> ShortcutRow {
        let model = SettingsModelKey {
            id: "standard".to_owned(),
            origin: SettingsModelOrigin::BuiltIn,
        };
        ShortcutRow {
            target: ShortcutCaptureTarget::ModelBehavior {
                model: model.clone(),
                behavior_id: model_behavior_id(&behavior),
            },
            behavior: None,
            playable: Some(PlayableBehavior { model, behavior }),
            shortcut: None,
            custom_name: None,
        }
    }

    /// The palette separates the two kinds and wraps each at four names to a
    /// row: a model with two motions and five expressions reads as one row of
    /// motions and two rows of expressions.
    ///
    /// The blocks are drawn expressions-first by the palette's own loop (the
    /// two headings are ordered that way), so the kind separation here is the
    /// part a render test would have to reach through the DOM to check — this
    /// is the same decision in a form a test can hold on to.
    #[test]
    fn palette_blocks_separate_kinds_and_wrap_at_four() {
        let rows: Vec<ShortcutRow> = [
            SettingsModelBehavior::Motion {
                group: "CAT_motion".to_owned(),
                index: 0,
            },
            SettingsModelBehavior::Motion {
                group: "CAT_motion".to_owned(),
                index: 1,
            },
            SettingsModelBehavior::Expression {
                name: "a".to_owned(),
            },
            SettingsModelBehavior::Expression {
                name: "b".to_owned(),
            },
            SettingsModelBehavior::Expression {
                name: "c".to_owned(),
            },
            SettingsModelBehavior::Expression {
                name: "d".to_owned(),
            },
            SettingsModelBehavior::Expression {
                name: "e".to_owned(),
            },
        ]
        .into_iter()
        .map(palette_row)
        .collect();

        let expressions = palette_blocks(&rows, true);
        let motions = palette_blocks(&rows, false);
        assert_eq!(
            expressions.iter().flatten().count(),
            5,
            "five expressions are all in the expression block"
        );
        assert_eq!(
            motions.iter().flatten().count(),
            2,
            "two motions are all in the motion block"
        );
        assert_eq!(expressions.len(), 2, "five expressions wrap to two rows");
        assert_eq!(motions.len(), 1, "two motions fit on one row");
        assert!(
            expressions
                .iter()
                .all(|chunk| chunk.len() <= BEHAVIOR_PALETTE_COLUMNS)
                && motions
                    .iter()
                    .all(|chunk| chunk.len() <= BEHAVIOR_PALETTE_COLUMNS),
            "no row holds more than four names"
        );
    }
}

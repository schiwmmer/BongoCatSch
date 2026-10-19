use super::*;

/// How often the settings window re-reads the pomodoro snapshot while it is open.
///
/// The client keeps a revision, so a quiet window costs nothing: the poll only
/// `cx.notify()`s when the revision actually moved, which is what makes the
/// calendar and the task list track a running countdown without a per-tick clone.
const POMODORO_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

impl SettingsView {
    pub(super) fn set_navigation_memory(&mut self, memory: SettingsNavigationMemory) {
        self.navigation_memory = memory;
    }

    /// The display language for the frame being rendered.
    ///
    /// The snapshot answers as soon as it exists; before that the window renders the
    /// language it was opened with, so the first frame is already the user's. See
    /// [`SettingsWindowSeed`].
    pub(super) fn display_language(&self) -> SettingsLanguage {
        self.snapshot
            .as_ref()
            .map_or(self.seed.language, |snapshot| snapshot.resolved_language)
    }

    /// The appearance for the frame being rendered, with the same rule as
    /// [`Self::display_language`].
    pub(super) fn display_appearance_theme(&self) -> SettingsTheme {
        self.snapshot
            .as_ref()
            .map_or(self.seed.appearance_theme, |snapshot| {
                snapshot.appearance_theme
            })
    }

    pub(super) fn sync_component_theme(
        &mut self,
        theme: SettingsTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.applied_theme == Some(theme) {
            return;
        }
        apply_component_theme(theme, window, cx);
        self.applied_theme = Some(theme);
    }

    pub(super) fn sync_component_inputs(
        &mut self,
        snapshot: &SettingsSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.set_window_title(bongocat_i18n::text(
            snapshot.resolved_language.catalog_locale(),
            "navigation.settings.title",
        ));
        self.syncing_component_inputs = true;
        #[cfg(target_os = "windows")]
        self.window_background_color.update(cx, |picker, cx| {
            let rgb = self
                .window_background_debouncer
                .pending_value()
                .copied()
                .unwrap_or(snapshot.overlay.window_background_color);
            let color = super::window_mode::rgb_color(rgb);
            if picker.value() != Some(color) {
                picker.set_value(color, window, cx);
            }
        });
        self.language_select.update(cx, |select, cx| {
            select.set_items(
                SearchableVec::new(
                    SettingsLanguage::ALL
                        .into_iter()
                        .map(|language| {
                            crate::settings_language_display_name(
                                language,
                                snapshot.resolved_language,
                            )
                        })
                        .collect::<Vec<_>>(),
                ),
                window,
                cx,
            );
            select.set_selected_value(
                &crate::settings_language_display_name(
                    snapshot.language,
                    snapshot.resolved_language,
                ),
                window,
                cx,
            )
        });
        self.theme_select.update(cx, |select, cx| {
            select.set_items(
                SearchableVec::new(theme_options(snapshot.resolved_language)),
                window,
                cx,
            );
            select.set_selected_value(
                &theme_display_name(snapshot.appearance_theme, snapshot.resolved_language),
                window,
                cx,
            )
        });
        self.logging_level_select.update(cx, |select, cx| {
            select.set_items(
                SearchableVec::new(logging_level_options(snapshot.resolved_language)),
                window,
                cx,
            );
            select.set_selected_value(
                &logging_level_display_name(snapshot.logging.level, snapshot.resolved_language),
                window,
                cx,
            )
        });
        self.random_behavior_mode_select.update(cx, |select, cx| {
            select.set_items(
                SearchableVec::new(random_behavior_mode_options(snapshot.resolved_language)),
                window,
                cx,
            );
            select.set_selected_value(
                &random_behavior_mode_display_name(
                    snapshot.random_behavior.mode,
                    snapshot.resolved_language,
                ),
                window,
                cx,
            )
        });
        for (state, select) in [
            (
                GamepadConnectionState::Connected,
                &self.gamepad_connected_model_select,
            ),
            (
                GamepadConnectionState::Disconnected,
                &self.gamepad_disconnected_model_select,
            ),
        ] {
            let configured = state.target(&snapshot.gamepad_auto_switch);
            let options =
                gamepad_auto_switch_choices(Some(snapshot), state, snapshot.resolved_language);
            // The list is rebuilt from the catalog, so the committed value has to
            // be looked up in the new list rather than carried over by position.
            let selected = options
                .iter()
                .find(|option| option.target == *configured)
                .cloned();
            select.update(cx, |select, cx| {
                select.set_items(SearchableVec::new(options), window, cx);
                if let Some(selected) = selected {
                    select.set_selected_value(&selected, window, cx);
                }
            });
        }
        self.syncing_component_inputs = false;
    }

    pub(super) fn new(
        client: SettingsClient,
        seed: SettingsWindowSeed,
        request_quit: Rc<dyn Fn(&mut App)>,
        request_update: SettingsWindowRequest,
        pomodoro: PomodoroClient,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        #[cfg(target_os = "windows")]
        let window_background_color = cx.new(|cx| {
            gpui_kit::component::color_picker::ColorPickerState::new(window, cx)
                .default_value(super::window_mode::rgb_color([0, 255, 0]))
        });
        #[cfg(target_os = "windows")]
        cx.subscribe_in(
            &window_background_color,
            window,
            |view, _, event: &gpui_kit::component::color_picker::ColorPickerEvent, _, cx| {
                let gpui_kit::component::color_picker::ColorPickerEvent::Change(Some(color)) =
                    event
                else {
                    return;
                };
                view.set_window_background_color(*color, cx);
            },
        )
        .detach();
        // Every component built here is seeded with the window's language for the same
        // reason as the frame itself: they are constructed before the first snapshot
        // exists, and a component built in the default language would be replaced —
        // visibly — by the values the first snapshot carries.
        let language_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(
                    SettingsLanguage::ALL
                        .into_iter()
                        .map(|language| {
                            crate::settings_language_display_name(language, seed.language)
                        })
                        .collect::<Vec<_>>(),
                ),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let theme_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(theme_options(seed.language)),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let logging_level_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(logging_level_options(seed.language)),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        // The mode dropdown opens on the first catalogue entry, which is the
        // product default: a fresh configuration has never had its mode chosen,
        // so the first frame already shows the state the configuration is in.
        let random_behavior_mode_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(random_behavior_mode_options(seed.language)),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        // Both gamepad auto switch dropdowns open on their "last used" choice,
        // which is the configured default: the first frame already shows the
        // state a fresh configuration is in.
        let gamepad_connected_model_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(gamepad_auto_switch_options(
                    &[],
                    GamepadConnectionState::Connected,
                    None,
                    seed.language,
                )),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let gamepad_disconnected_model_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(gamepad_auto_switch_options(
                    &[],
                    GamepadConnectionState::Disconnected,
                    None,
                    seed.language,
                )),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &language_select,
            |view, _, event: &SelectEvent<SearchableVec<&'static str>>, cx| {
                if view.syncing_component_inputs {
                    return;
                }
                let display_language = view.display_language();
                if let SelectEvent::Confirm(Some(name)) = event
                    && let Some(language) =
                        crate::settings_language_from_display_name(name, display_language)
                {
                    view.set_language(language, cx);
                }
            },
        )
        .detach();
        cx.subscribe(
            &theme_select,
            |view, _, event: &SelectEvent<SearchableVec<&'static str>>, cx| {
                if view.syncing_component_inputs {
                    return;
                }
                let display_language = view.display_language();
                if let SelectEvent::Confirm(Some(name)) = event
                    && let Some(theme) = theme_from_display_name(name, display_language)
                {
                    view.set_appearance_theme(theme, cx);
                }
            },
        )
        .detach();
        cx.subscribe(
            &logging_level_select,
            |view, _, event: &SelectEvent<SearchableVec<&'static str>>, cx| {
                if view.syncing_component_inputs {
                    return;
                }
                let display_language = view.display_language();
                if let SelectEvent::Confirm(Some(name)) = event
                    && let Some(level) = logging_level_from_display_name(name, display_language)
                {
                    view.set_logging_level(level, cx);
                }
            },
        )
        .detach();
        cx.subscribe(
            &random_behavior_mode_select,
            |view, _, event: &SelectEvent<SearchableVec<&'static str>>, cx| {
                if view.syncing_component_inputs {
                    return;
                }
                let display_language = view.display_language();
                if let SelectEvent::Confirm(Some(name)) = event
                    && let Some(mode) =
                        random_behavior_mode_from_display_name(name, display_language)
                {
                    view.set_random_behavior_mode(mode, cx);
                }
            },
        )
        .detach();
        for (state, select) in [
            (
                GamepadConnectionState::Connected,
                &gamepad_connected_model_select,
            ),
            (
                GamepadConnectionState::Disconnected,
                &gamepad_disconnected_model_select,
            ),
        ] {
            cx.subscribe(
                select,
                move |view, _, event: &SelectEvent<SearchableVec<GamepadModelChoice>>, cx| {
                    if view.syncing_component_inputs {
                        return;
                    }
                    if let SelectEvent::Confirm(Some(choice)) = event {
                        view.set_gamepad_auto_switch_model(state, choice.target.clone(), cx);
                    }
                },
            )
            .detach();
        }
        // The pomodoro search box keeps its own cursor, so it lives as an entity
        // the page renders and reads; its text is mirrored into `pomodoro_search`
        // on every keystroke for the task-list filter.
        let pomodoro_search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(bongocat_i18n::text(
                seed.language.catalog_locale(),
                "pomodoro.settings.search_placeholder",
            ))
        });
        cx.subscribe(
            &pomodoro_search_input,
            |view, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.pomodoro_search = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        )
        .detach();
        // The task editor's two fields, seeded with the window's language the
        // same way the search box is. Enter in the name field submits the
        // editor, so creating a task never needs the pointer after the first
        // click; the count field stays a plain field.
        let pomodoro_editor_name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(bongocat_i18n::text(
                    seed.language.catalog_locale(),
                    "pomodoro.picker.task_name_placeholder",
                ))
                .submit_on_enter(true)
        });
        cx.subscribe(
            &pomodoro_editor_name,
            |view, _input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    view.submit_pomodoro_task_editor(cx);
                }
            },
        )
        .detach();
        let pomodoro_editor_sessions = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(crate::pomodoro_window::view::DEFAULT_EXPECTED_SESSIONS.to_string())
                .placeholder(bongocat_i18n::text(
                    seed.language.catalog_locale(),
                    "pomodoro.picker.estimated_sessions",
                ))
        });
        // The pomodoro snapshot advances on a worker thread, so the page re-reads
        // it on a light timer and re-renders only when the revision moves.
        cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            loop {
                executor.timer(POMODORO_POLL_INTERVAL).await;
                if this.update(cx, |view, cx| view.poll_pomodoro(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();

        Self {
            #[cfg(target_os = "windows")]
            window_background_color,
            #[cfg(target_os = "windows")]
            window_background_debouncer: crate::SettingsPatchDebouncer::default(),
            #[cfg(target_os = "windows")]
            window_background_timer_generation: 0,
            client,
            pomodoro,
            // The pomodoro page opens on the task list, with no search filter and
            // showing the month the user is in right now. The month is derived
            // from the local UTC offset so the calendar opens on the day the
            // user would write on a paper calendar, not on a UTC one.
            pomodoro_search: String::new(),
            pomodoro_view: PomodoroSettingsView::List,
            pomodoro_display_month: {
                let offset = bongocat_platform::local_utc_offset_seconds();
                let now_unix = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs() as i64)
                    .unwrap_or(0);
                let today = bongocat_pomodoro::CalendarDay::from_unix(now_unix, offset).to_civil();
                (today.year, today.month)
            },
            pomodoro_search_input,
            // No editor is open when the window opens: the list is the thing
            // the user came to look at, and the fields only exist while a
            // task is being created or edited.
            pomodoro_editor: None,
            pomodoro_editor_name,
            pomodoro_editor_sessions,
            pomodoro_observed_revision: 0,
            cultivation_minute_observed: None,
            cultivation_open_item: None,
            pomodoro_done_expanded: false,
            pomodoro_chart_days: 7,
            // No day is picked when the page opens: the chart ends on today
            // and the day-detail strip stays hidden until a cell is clicked.
            pomodoro_selected_day: None,
            seed,
            snapshot: None,
            pending: None,
            pending_notification: None,
            model_import_success_pending: false,
            model_import_failed_pending: false,
            about_copy_success_pending: false,
            model_import: ModelImportDraft::default(),
            model_drag: None,
            check_for_updates_interval_debouncer: crate::SettingsPatchDebouncer::default(),
            check_for_updates_interval_timer_generation: 0,
            overlay_scale_debouncer: crate::SettingsPatchDebouncer::default(),
            overlay_scale_timer_generation: 0,
            overlay_opacity_debouncer: crate::SettingsPatchDebouncer::default(),
            overlay_opacity_timer_generation: 0,
            overlay_corner_radius_debouncer: crate::SettingsPatchDebouncer::default(),
            overlay_corner_radius_timer_generation: 0,
            overlay_hover_hide_delay_debouncer: crate::SettingsPatchDebouncer::default(),
            overlay_hover_hide_delay_timer_generation: 0,
            overlay_idle_hide_delay_debouncer: crate::SettingsPatchDebouncer::default(),
            overlay_idle_hide_delay_timer_generation: 0,
            gamepad_dead_zone_debouncer: crate::SettingsPatchDebouncer::default(),
            gamepad_dead_zone_timer_generation: 0,
            maximum_fps_debouncer: crate::SettingsPatchDebouncer::default(),
            maximum_fps_timer_generation: 0,
            random_behavior_debouncer: crate::SettingsPatchDebouncer::default(),
            random_behavior_timer_generation: 0,
            logging_settings_debouncer: crate::SettingsPatchDebouncer::default(),
            logging_settings_timer_generation: 0,
            flush_pending_requested: false,
            quit_after_flush: false,
            model_delete_confirmation: None,
            model_row_focus: BTreeMap::new(),
            model_edit: None,
            behavior_name: None,
            model_catalog_error_reported: false,
            shortcut_capture: None,
            shortcut_capture_blur_subscription: None,
            shortcut_row_focus: BTreeMap::new(),
            shortcut_rows: BTreeMap::new(),
            shortcut_play_focus: BTreeMap::new(),
            shortcut_clear_focus: BTreeMap::new(),
            hold_modifier_recording: false,
            hold_modifier_recorder_focus: cx.focus_handle(),
            hold_modifier_clear_focus: cx.focus_handle(),
            hold_modifier_blur_subscription: None,
            hold_modifier_recording_generation: 0,
            window_hidden: true,
            navigation_memory: SettingsNavigationMemory::default(),
            applied_theme: None,
            language_select,
            theme_select,
            logging_level_select,
            random_behavior_mode_select,
            gamepad_connected_model_select,
            gamepad_disconnected_model_select,
            request_quit,
            request_update,
            overlay_focus: cx.focus_handle().tab_index(10).tab_stop(true),
            import_card_focus: cx.focus_handle().tab_index(20).tab_stop(true),
            pending_model_reveal: BTreeSet::new(),
            completed_model_cover_captures: BTreeSet::new(),
            syncing_component_inputs: false,
        }
    }

    /// Re-read the pomodoro snapshot revision and re-render only if it moved.
    ///
    /// Called from a light timer while the settings window is open. The pomodoro
    /// worker advances the revision on another thread, so comparing against the
    /// last seen value is what keeps the task list and calendar fresh without
    /// cloning the snapshot on every idle tick. The tick is gated on the
    /// pomodoro page actually being the visible one — the sidebar reporter
    /// writes the active page into the process navigation memory on every
    /// frame, so reading it back here costs one integer compare and stops the
    /// timer from waking for a page the user cannot see.
    pub(super) fn poll_pomodoro(&mut self, cx: &mut Context<Self>) {
        let page = self.navigation_memory.page_index();
        // The timer serves both pages that read the pomodoro worker: the task
        // list and the cultivation ladder it feeds.
        if page != SettingsNavigationPage::Pomodoro.index()
            && page != SettingsNavigationPage::Cultivation.index()
        {
            return;
        }
        let revision = self.pomodoro.revision();
        if revision == self.pomodoro_observed_revision {
            self.poll_ripening(cx);
            return;
        }
        self.pomodoro_observed_revision = revision;
        cx.notify();
    }

    /// Wake the cultivation page when a growing crop crosses a minute.
    ///
    /// A crop ripens on the wall clock, not on a worker change, so the revision
    /// never moves while one is in the ground. Without this the plot would keep
    /// reading "ripe in 3 min" and never turn pickable until something else
    /// happened to repaint the page. Only the cultivation page pays the
    /// snapshot clone, and only once per displayed minute.
    fn poll_ripening(&mut self, cx: &mut Context<Self>) {
        if self.navigation_memory.page_index() != SettingsNavigationPage::Cultivation.index() {
            self.cultivation_minute_observed = None;
            return;
        }
        let now = now_unix();
        let snapshot = self.pomodoro.snapshot();
        let key = ripening_minute(&snapshot.cultivation.plots, now);
        if key == self.cultivation_minute_observed {
            return;
        }
        self.cultivation_minute_observed = key;
        cx.notify();
    }
}

/// The whole minutes left on the soonest crop, or `None` when nothing is in the
/// ground.
///
/// This is the value whose change means the garden's labels changed: a ripe crop
/// reads as zero rather than counting backwards past its instant, so the tick
/// stops repainting once everything is pickable.
fn ripening_minute(plots: &[bongocat_pomodoro::PlotView], now_unix: i64) -> Option<i64> {
    let soonest = plots
        .iter()
        .filter_map(|plot| plot.crop.as_ref().map(|crop| crop.ready_at_unix))
        .min()?;
    Some((soonest - now_unix).max(0) / 60)
}

#[cfg(test)]
mod tests {
    use super::ripening_minute;
    use bongocat_pomodoro::{Attribute, CropView, PlotView};

    fn plot(ready_at_unix: i64) -> PlotView {
        PlotView {
            crop: Some(CropView {
                attribute: Attribute::Root,
                tier: 1,
                ready_at_unix,
            }),
        }
    }

    #[test]
    fn an_empty_garden_has_no_countdown_to_draw() {
        assert_eq!(ripening_minute(&[PlotView { crop: None }], 1_000), None);
    }

    #[test]
    fn the_soonest_crop_is_the_one_the_page_counts_down() {
        let plots = [plot(2_000), plot(1_000), PlotView { crop: None }];
        assert_eq!(
            ripening_minute(&plots, 700),
            Some(5),
            "300s left rounds down"
        );
    }

    #[test]
    fn a_ripe_crop_reads_as_zero_and_stays_there() {
        let plots = [plot(1_000)];
        assert_eq!(ripening_minute(&plots, 1_000), Some(0));
        // Past the instant the key must not keep changing, or the tick would
        // repaint the page every half second for as long as a crop sits unread.
        assert_eq!(ripening_minute(&plots, 9_999), Some(0));
    }
}

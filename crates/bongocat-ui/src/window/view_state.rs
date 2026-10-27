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
            cultivation_open_offer: None,
            cultivation_open_seed: None,
            cultivation_open_plot: None,
            // The event history starts closed: the page's five lines are the
            // record until the user asks for more.
            cultivation_events_open: false,
            cultivation_mastered_open: false,
            cultivation_ladder_open: false,
            cultivation_explore_hours: 8,
            cultivation_daily_seen_day: None,
            // No fight is being replayed when the window opens, and none is
            // waiting on the worker either.
            cultivation_duel: None,
            cultivation_duel_pending: None,
            // Nothing has been refused yet, so there is no error waiting to be
            // shown and no count to compare against.
            cultivation_refusal_pending: None,
            cultivation_refusal_seen: 0,
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
        } else {
            self.pomodoro_observed_revision = revision;
            cx.notify();
        }
        // A fight the user asked for is settled by the worker on its own thread:
        // this is where the page learns it has a report to show.
        self.open_answered_cultivation_duel(cx);
        // The day's own event opens itself, so it is watched here rather than
        // asked for: an ambush is the one fight nobody pressed a button for.
        self.open_daily_event_duel(cx);
        // And this is where a replay being read is paced out.
        self.advance_cultivation_duel(cx);
        // A refused purchase or a walk that is not owed is reported here: the
        // worker says why, and the page says it out loud.
        self.open_cultivation_refusal(cx);
    }

    /// Show the ambush the day opened with, once.
    ///
    /// Three things have to be true at once, and each one closes a way the
    /// popup used to come back: the event has to be *today's* — a window that
    /// opened on a later day never replays an older ambush; it has to be a
    /// fight — a gift or a quiet morning is a line on the log, not a dialog;
    /// and it has not to have been seen yet — the domain writes the day down
    /// when the popup is acknowledged, so a relaunch the same afternoon finds
    /// the field behind it. The snapshot revision cannot be used for any of
    /// this, because a running pomodoro moves it every second.
    fn open_daily_event_duel(&mut self, cx: &mut Context<Self>) {
        let cultivation = self.pomodoro.snapshot().cultivation;
        let today = cultivation.day;
        if cultivation.daily_event_day != Some(today)
            || cultivation.daily_event_seen_day == Some(today)
            || self.cultivation_daily_seen_day == Some(today)
        {
            return;
        }
        let Some(event) = cultivation
            .last_daily_event
            .filter(|event| event.duel.is_some())
        else {
            return;
        };
        self.cultivation_daily_seen_day = Some(today);
        // The acknowledgement is written down before the popup opens: the
        // domain is the one that remembers, and a popup that was shown but not
        // recorded would be shown again on the next launch.
        let _ = self
            .pomodoro
            .try_publish(PomodoroCommand::AcknowledgeDailyEvent { day: today });
        self.cultivation_duel = Some(CultivationDuel::new(
            CultivationDuelSource::Daily,
            cultivation_duel::CultivationDuelPayload::Daily(event),
        ));
        cx.notify();
    }

    /// Show why an action was refused, once.
    ///
    /// The domain counts the refusals, and this watches that count: a count
    /// that has not moved is the same reason still sitting in the snapshot, and
    /// a count that has moved is a new refusal that has to be said. The
    /// snapshot revision cannot be used for this — a running pomodoro bumps it
    /// every second.
    fn open_cultivation_refusal(&mut self, cx: &mut Context<Self>) {
        let cultivation = &self.pomodoro.snapshot().cultivation;
        let count = cultivation.refusal_count;
        if count == self.cultivation_refusal_seen {
            return;
        }
        // A count that went *down* means the worker was restarted and the
        // ladder it holds is a new one: the baseline moves rather than the
        // page announcing a refusal that never happened.
        if count < self.cultivation_refusal_seen {
            self.cultivation_refusal_seen = count;
            return;
        }
        self.cultivation_refusal_seen = count;
        let Some(refusal) = cultivation.last_refusal else {
            return;
        };
        self.cultivation_refusal_pending = Some((refusal, count));
        cx.notify();
    }

    /// Show the fight or walk the user asked for, once the worker has answered.
    ///
    /// The count the domain keeps — fights joined, walks taken — is what says
    /// the command was applied rather than refused: the snapshot revision moves
    /// every second while a pomodoro runs, so waiting on it would open the
    /// popup on a timer tick with the previous attempt's report.
    fn open_answered_cultivation_duel(&mut self, cx: &mut Context<Self>) {
        let Some((source, asked_at)) = self.cultivation_duel_pending else {
            return;
        };
        let cultivation = &self.pomodoro.snapshot().cultivation;
        let (count, payload) = match source {
            CultivationDuelSource::Breakthrough => (
                cultivation.breakthrough_attempts,
                cultivation
                    .last_breakthrough
                    .clone()
                    .map(cultivation_duel::CultivationDuelPayload::Breakthrough),
            ),
            CultivationDuelSource::Exploration => (
                cultivation.explorations,
                cultivation.last_exploration.map(|outcome| {
                    // The fight rides beside the walk only when this walk
                    // settled one: a quiet walk's popup must not replay an
                    // older walk's battle under this walk's verdict. The
                    // domain clears the report for a walk that met nobody;
                    // this is the same rule read once more, at the one place
                    // the three fields become one popup.
                    let duel = matches!(
                        outcome,
                        bongocat_pomodoro::ExplorationOutcome::Victory
                            | bongocat_pomodoro::ExplorationOutcome::Defeat
                    )
                    .then(|| cultivation.last_exploration_duel.clone())
                    .flatten();
                    cultivation_duel::CultivationDuelPayload::Exploration(
                        outcome,
                        cultivation.last_exploration_found.clone(),
                        duel,
                    )
                }),
            ),
            CultivationDuelSource::Trial => (
                cultivation.trial_attempts,
                cultivation
                    .last_trial
                    .clone()
                    .map(cultivation_duel::CultivationDuelPayload::Trial),
            ),
            CultivationDuelSource::Proving => (
                cultivation.provings,
                cultivation
                    .last_proving
                    .clone()
                    .map(cultivation_duel::CultivationDuelPayload::Proving),
            ),
            // The morning ambush is never asked for: it is opened by the poll
            // below, which watches the day's own count instead.
            CultivationDuelSource::Daily => return,
        };
        if count <= asked_at {
            // Not answered yet: the next tick asks again.
            return;
        }
        self.cultivation_duel_pending = None;
        if let Some(payload) = payload {
            self.cultivation_duel = Some(CultivationDuel::new(source, payload));
        }
        cx.notify();
    }

    /// Pace out the fight the cultivation page is replaying.
    ///
    /// A fight dumped on the page all at once is a wall of text: the blows are
    /// revealed an interval apart, so the replay reads the way the fight
    /// happened. The poll that already wakes for the garden also serves this, so
    /// a replay costs no timer of its own.
    fn advance_cultivation_duel(&mut self, cx: &mut Context<Self>) {
        let Some(duel) = self.cultivation_duel.as_mut() else {
            return;
        };
        if duel.advance() {
            cx.notify();
        }
    }

    /// Ask the worker for a fight or a walk, and remember that the answer is
    /// owed a popup.
    ///
    /// The count of that action is taken before the command is published, so the
    /// poll can tell an answered command from a refused one: a running pomodoro
    /// bumps the snapshot revision every second, and the revision cannot say
    /// which of the two moved.
    pub(super) fn ask_cultivation(
        &mut self,
        source: CultivationDuelSource,
        command: bongocat_pomodoro::PomodoroCommand,
        cx: &mut Context<Self>,
    ) {
        let cultivation = &self.pomodoro.snapshot().cultivation;
        let count = match source {
            CultivationDuelSource::Breakthrough => cultivation.breakthrough_attempts,
            CultivationDuelSource::Exploration => cultivation.explorations,
            CultivationDuelSource::Trial => cultivation.trial_attempts,
            CultivationDuelSource::Proving => cultivation.provings,
            // Nothing asks for the morning event; it asks for the user.
            CultivationDuelSource::Daily => return,
        };
        let _ = self.pomodoro.try_publish(command);
        self.cultivation_duel_pending = Some((source, count));
        self.poll_pomodoro(cx);
    }

    /// Wake the cultivation page when a minute the page is counting down to
    /// goes by.
    ///
    /// A crop ripens on the wall clock, not on a worker change, and so do the
    /// two cooldowns, so the revision never moves while any of them runs. Without
    /// this the plot would keep reading "ripe in 3 min" and never turn pickable,
    /// and the fight button would stay locked with a stale countdown beside it,
    /// until something else happened to repaint the page. Only the cultivation
    /// page pays the snapshot clone, and only once per displayed minute.
    fn poll_ripening(&mut self, cx: &mut Context<Self>) {
        if self.navigation_memory.page_index() != SettingsNavigationPage::Cultivation.index() {
            self.cultivation_minute_observed = None;
            return;
        }
        let now = now_unix();
        let snapshot = self.pomodoro.snapshot();
        let key = cultivation_minute(&snapshot.cultivation, now);
        if key == self.cultivation_minute_observed {
            return;
        }
        self.cultivation_minute_observed = key;
        cx.notify();
    }
}

/// The whole minutes left on the soonest thing the cultivation page counts down:
/// a ripening crop, a walk being owed, or a fight being held — or `None` when
/// nothing is in the ground and both cooldowns have run out.
///
/// This is the value whose change means the page's labels changed: a ripe crop
/// reads as zero rather than counting backwards past its instant, so the tick
/// stops repainting once everything is pickable and both buttons are live.
fn cultivation_minute(
    cultivation: &bongocat_pomodoro::CultivationView,
    now_unix: i64,
) -> Option<i64> {
    let crops = cultivation
        .plots
        .iter()
        .filter_map(|plot| plot.crop.as_ref().map(|crop| crop.ready_at_unix));
    let cooldowns = [
        cultivation.exploration_ready_at,
        cultivation.breakthrough_ready_at,
    ]
    .into_iter()
    .flatten();
    crops
        .chain(cooldowns)
        .map(|at| at.saturating_sub(now_unix).max(0) / 60)
        .min()
}

#[cfg(test)]
mod tests {
    use super::cultivation_minute;
    use bongocat_pomodoro::{Attribute, CropView, CultivationView, PlotView};

    fn plot(ready_at_unix: i64) -> PlotView {
        PlotView {
            crop: Some(CropView {
                attribute: Attribute::Root,
                tier: 1,
                grade: bongocat_pomodoro::SeedGrade::Earth,
                name: None,
                ready_at_unix,
                herb: None,
            }),
        }
    }

    /// A view with nothing in the ground and no cooldown running: the page has
    /// no countdown to draw at all.
    fn quiet() -> CultivationView {
        let mut cultivation = bongocat_pomodoro::Cultivation::default().view();
        cultivation.plots = vec![PlotView { crop: None }];
        cultivation
    }

    #[test]
    fn an_empty_garden_has_no_countdown_to_draw() {
        assert_eq!(cultivation_minute(&quiet(), 1_000), None);
    }

    #[test]
    fn the_soonest_crop_is_the_one_the_page_counts_down() {
        let mut cultivation = quiet();
        cultivation.plots = vec![plot(2_000), plot(1_000), PlotView { crop: None }];
        assert_eq!(
            cultivation_minute(&cultivation, 700),
            Some(5),
            "300s left rounds down"
        );
    }

    #[test]
    fn a_ripe_crop_reads_as_zero_and_stays_there() {
        let mut cultivation = quiet();
        cultivation.plots = vec![plot(1_000)];
        assert_eq!(cultivation_minute(&cultivation, 1_000), Some(0));
        // Past the instant the key must not keep changing, or the tick would
        // repaint the page every half second for as long as a crop sits unread.
        assert_eq!(cultivation_minute(&cultivation, 9_999), Some(0));
    }

    #[test]
    fn a_cooldown_the_page_counts_down_wakes_the_page_too() {
        let mut cultivation = quiet();
        // A walk owed in an hour and a half, with nothing in the ground.
        cultivation.exploration_ready_at = Some(1_000 + 5_400);
        assert_eq!(cultivation_minute(&cultivation, 1_000), Some(90));
        // The fight being held is the sooner of the two, so it is the one drawn.
        cultivation.breakthrough_ready_at = Some(1_000 + 600);
        assert_eq!(cultivation_minute(&cultivation, 1_000), Some(10));
    }

    #[test]
    fn a_cooldown_that_has_run_out_stops_waking_the_page() {
        let mut cultivation = quiet();
        cultivation.exploration_ready_at = Some(1_000);
        assert_eq!(
            cultivation_minute(&cultivation, 9_999),
            Some(0),
            "it reads as zero rather than counting backwards"
        );
    }
}

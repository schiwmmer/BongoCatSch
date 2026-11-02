//! Recording, displaying and gating the shortcut bindings.

use super::*;

#[test]
fn shortcut_capture_canonicalizes_modifiers_and_named_keys() {
    let mut modifiers = Modifiers {
        control: true,
        shift: true,
        ..Modifiers::default()
    };
    assert_eq!(
        captured_shortcut("arrowleft", modifiers).as_deref(),
        Some("Control+Shift+ArrowLeft")
    );

    modifiers = Modifiers::default();
    modifiers.control = true;
    assert_eq!(
        captured_shortcut("return", modifiers).as_deref(),
        Some("Control+Enter")
    );

    assert_eq!(
        captured_shortcut("f12", Modifiers::default()).as_deref(),
        Some("F12")
    );
}

#[test]
fn shortcut_capture_rejects_unmodified_non_function_and_unsupported_keys() {
    for key_name in ["a", "1", "return", "arrowleft", "space", "delete"] {
        assert!(
            captured_shortcut(key_name, Modifiers::default()).is_none(),
            "{key_name} must not be captured without a modifier"
        );
    }
    assert!(captured_shortcut("shift", Modifiers::default()).is_none());
    assert!(captured_shortcut("media-play", Modifiers::default()).is_none());
}

#[test]
fn shortcut_capture_clears_temporary_input_after_a_conflict() {
    let mut capture =
        ShortcutCapture::new(ShortcutCaptureTarget::Command("toggle_overlay".to_owned()));
    capture.modifiers.platform = true;
    capture.keys.insert("L".to_owned());

    capture.clear_temporary_input();

    assert_eq!(capture.modifiers, Modifiers::default());
    assert!(capture.keys.is_empty());
}

#[test]
fn shortcut_capture_previews_incomplete_and_unsupported_combinations() {
    let keys = BTreeSet::from(["A".to_owned()]);
    let mut modifiers = Modifiers::default();
    assert_eq!(
        shortcut_capture_preview(&modifiers, &keys).as_deref(),
        Some("A")
    );
    assert!(shortcut_from_capture(&modifiers, &keys).is_none());

    modifiers.control = true;
    assert_eq!(
        shortcut_from_capture(&modifiers, &keys).as_deref(),
        Some("Control+A")
    );

    let keys = BTreeSet::new();
    assert_eq!(
        shortcut_capture_preview(&modifiers, &keys).as_deref(),
        Some("Control")
    );
    assert!(shortcut_from_capture(&modifiers, &keys).is_none());
}

#[test]
fn macos_shortcut_display_uses_legacy_symbols() {
    for (shortcut, expected) in [
        ("Control+Alt+Shift+Meta+P", "⌃ ⌥ ⇧ ⌘ P"),
        ("Escape", "⎋"),
        ("Backspace", "⌫"),
        ("Tab", "⇥"),
        ("Enter", "↩︎"),
        ("Space", "␣"),
        ("Control+ArrowLeft", "⌃ ←"),
        ("Meta+BracketLeft", "⌘ ["),
    ] {
        assert_eq!(
            format_shortcut_display(shortcut, true),
            expected,
            "{shortcut}"
        );
    }
}

#[test]
fn non_macos_shortcut_display_preserves_canonical_names() {
    let shortcut = "Control+Alt+ArrowLeft";
    assert_eq!(format_shortcut_display(shortcut, false), shortcut);
}

#[test]
fn shortcut_display_uses_the_compiled_platform() {
    let expected = if cfg!(target_os = "macos") {
        "⌘ P"
    } else {
        "Meta+P"
    };
    assert_eq!(shortcut_display("Meta+P"), expected);
}

#[test]
fn shortcut_capture_conflict_preview_is_order_independent() {
    let shortcuts = SettingsShortcuts {
        commands: vec![
            SettingsShortcutBinding {
                command: "toggle_overlay".to_owned(),
                shortcut: "ctrl+b".to_owned(),
            },
            SettingsShortcutBinding {
                command: "toggle_ignore_mouse_input".to_owned(),
                shortcut: "Control+B".to_owned(),
            },
        ],
        model_behaviors: Vec::new(),
    };
    assert_eq!(
        conflicting_shortcut(&shortcuts).as_deref(),
        Some("Control+B")
    );
}

/// Conflict is a property of bindings that are live at the same moment, not of
/// the persisted configuration as a whole. Every model counts its own behavior
/// defaults from the first digit of the primary modifier, so the same chord
/// routinely appears under two models; only the model the user is on can
/// conflict with itself, a command, or nothing else.
#[test]
fn shortcut_capture_conflicts_are_scoped_to_one_model() {
    let binding =
        |model_id: &str, behavior_id: &str, shortcut: &str| SettingsModelBehaviorBinding {
            model: settings_model_key(model_id, SettingsModelOrigin::BuiltIn),
            behavior_id: behavior_id.to_owned(),
            shortcut: shortcut.to_owned(),
        };

    let cross_model = SettingsShortcuts {
        commands: Vec::new(),
        model_behaviors: vec![
            binding("standard", "motion:CAT_motion:0", "Control+1"),
            binding("keyboard", "motion:CAT_motion:0", "ctrl+1"),
        ],
    };
    assert_eq!(conflicting_shortcut(&cross_model), None);

    let same_model = SettingsShortcuts {
        commands: Vec::new(),
        model_behaviors: vec![
            binding("standard", "motion:CAT_motion:0", "Control+1"),
            binding("standard", "motion:CAT_motion:1", "Control+1"),
        ],
    };
    assert_eq!(
        conflicting_shortcut(&same_model).as_deref(),
        Some("Control+1")
    );

    let shadows_a_command = SettingsShortcuts {
        commands: vec![SettingsShortcutBinding {
            command: "toggle_overlay".to_owned(),
            shortcut: "Control+1".to_owned(),
        }],
        model_behaviors: vec![binding("keyboard", "motion:CAT_motion:0", "Control+1")],
    };
    assert_eq!(
        conflicting_shortcut(&shadows_a_command).as_deref(),
        Some("Control+1")
    );
}

#[test]
fn shortcut_capture_targets_have_independent_tab_stops() {
    let active_model = SettingsModelKey {
        id: "standard".to_owned(),
        origin: SettingsModelOrigin::BuiltIn,
    };
    let shortcuts = SettingsShortcuts {
        commands: vec![
            SettingsShortcutBinding {
                command: "toggle_overlay".to_owned(),
                shortcut: "Control+B".to_owned(),
            },
            SettingsShortcutBinding {
                command: "open_settings".to_owned(),
                shortcut: "Control+S".to_owned(),
            },
        ],
        model_behaviors: vec![
            SettingsModelBehaviorBinding {
                model: settings_model_key("standard", SettingsModelOrigin::BuiltIn),
                behavior_id: "motion:tap:0".to_owned(),
                shortcut: "Control+M".to_owned(),
            },
            SettingsModelBehaviorBinding {
                model: settings_model_key("keyboard", SettingsModelOrigin::BuiltIn),
                behavior_id: "expression:ignored".to_owned(),
                shortcut: "Control+I".to_owned(),
            },
        ],
    };
    let entries = vec![
        model_entry(
            "standard",
            SettingsModelOrigin::BuiltIn,
            SettingsModelAvailability::Ready {
                behaviors: vec![
                    SettingsModelBehavior::Motion {
                        group: "tap".to_owned(),
                        index: 0,
                    },
                    SettingsModelBehavior::Expression {
                        name: "happy".to_owned(),
                    },
                ],
            },
        ),
        model_entry(
            "keyboard",
            SettingsModelOrigin::BuiltIn,
            SettingsModelAvailability::Ready {
                behaviors: vec![SettingsModelBehavior::Expression {
                    name: "ignored".to_owned(),
                }],
            },
        ),
    ];
    let targets = shortcut_targets(&shortcuts, Some(&active_model), &entries);
    assert_eq!(targets.len(), 7);
    // A row's controls are numbered as one group of three, in reading order,
    // with the play slot in the middle. The stride is fixed rather than counted
    // from the controls a row actually renders — an application command leaves
    // its play slot empty — so a row's numbers never move because of what
    // another row shows.
    assert_eq!(shortcut_capture_tab_index(0), 100);
    assert_eq!(shortcut_play_tab_index(0), 101);
    assert_eq!(shortcut_clear_tab_index(0), 102);
    assert_eq!(shortcut_capture_tab_index(1), 103);
    assert_eq!(shortcut_capture_tab_index(2), 106);
    assert_eq!(shortcut_clear_tab_index(2), 108);
    let indices = (0..targets.len())
        .flat_map(|row| {
            [
                shortcut_capture_tab_index(row),
                shortcut_play_tab_index(row),
                shortcut_clear_tab_index(row),
            ]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        indices.iter().collect::<BTreeSet<_>>().len(),
        indices.len(),
        "a row's three controls must hold three tab indices no other row uses"
    );
    assert_eq!(targets.into_iter().collect::<BTreeSet<_>>().len(), 7);
}

/// Only a model behavior row has something to play.
///
/// The application commands are named by what they switch — window visibility,
/// settings, input gates — not by anything the model can perform, so a play
/// control on those rows would be a button with no action behind it.
#[test]
fn only_a_model_behavior_row_carries_a_playable_behavior() {
    let model = SettingsModelKey {
        id: "standard".to_owned(),
        origin: SettingsModelOrigin::BuiltIn,
    };
    let behavior = SettingsModelBehavior::Motion {
        group: "CAT_motion".to_owned(),
        index: 0,
    };
    let entries = vec![model_entry(
        "standard",
        SettingsModelOrigin::BuiltIn,
        SettingsModelAvailability::Ready {
            behaviors: vec![
                behavior.clone(),
                SettingsModelBehavior::Expression {
                    name: "happy".to_owned(),
                },
            ],
        },
    )];

    let rows = shortcut_rows(&SettingsShortcuts::default(), Some(&model), &entries, &[]);
    assert!(
        rows[..5].iter().all(|row| row.playable.is_none()),
        "the application command rows must offer no play control"
    );
    assert_eq!(
        rows[5].playable.as_ref().map(|playable| &playable.model),
        Some(&model),
        "the play control must play the row's own model, not a looked-up one"
    );
    assert_eq!(
        rows[5].playable.as_ref().map(|p| &p.behavior),
        Some(&behavior)
    );
    assert_eq!(
        rows[6].playable.as_ref().map(|p| &p.behavior),
        Some(&SettingsModelBehavior::Expression {
            name: "happy".to_owned(),
        })
    );
}

/// Pressing either control inside a row's frame does its own job and does not
/// start recording a chord; pressing the frame itself still records.
///
/// The frame wraps both controls, and it starts a capture on any click inside
/// it, so each control has to stop the press before it reaches the frame. The
/// clear control used to live outside the frame, where that could not happen;
/// that is exactly why it is covered here next to play. Both checks settle the
/// executor before reading the channel: a command is sent from a spawned task,
/// so an unsettled executor would report an empty channel no matter what the
/// page did.
#[gpui_kit::test]
fn the_controls_inside_a_shortcut_row_act_without_recording(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (client, endpoint) = crate::SettingsClient::bounded(4);
    let mut seeded = crate::tests::snapshot(1, false, true);
    let behavior = SettingsModelBehavior::Motion {
        group: "CAT_motion".to_owned(),
        index: 0,
    };
    seeded.model_catalog.entries = vec![model_entry(
        "standard",
        SettingsModelOrigin::BuiltIn,
        SettingsModelAvailability::Ready {
            behaviors: vec![behavior.clone()],
        },
    )];
    let model = SettingsModelKey {
        id: "standard".to_owned(),
        origin: SettingsModelOrigin::BuiltIn,
    };
    // The model scope's rows are numbered after the window scope's, so this
    // row's controls are the ones the offset names.
    let row_index = ShortcutScope::Model.row_index_offset(&seeded.shortcuts);
    let snapshot = seeded.clone();
    let entries = seeded.model_catalog.entries.clone();
    let active = seeded.active_model.clone();

    let built: Rc<RefCell<Option<Entity<SettingsView>>>> = Rc::new(RefCell::new(None));
    let capture = Rc::clone(&built);
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            SettingsView::new(
                client,
                SettingsWindowSeed {
                    language: SettingsLanguage::English,
                    appearance_theme: SettingsTheme::System,
                },
                Rc::new(|_| {}),
                Rc::new(|_| {}),
                PomodoroClient::unavailable(),
                window,
                cx,
            )
        });
        view.update(cx, |view, cx| {
            view.snapshot = Some(snapshot.clone());
            view.sync_shortcut_row_focus(
                &snapshot.shortcuts,
                active.as_ref(),
                &entries,
                &snapshot.model_behavior_names,
                false,
                cx,
            );
        });
        capture.borrow_mut().replace(view.clone());
        Root::new(
            cx.new(|_| ShortcutsPageHarness {
                view,
                snapshot: Some(snapshot),
                scope: ShortcutScope::Model,
                gate: SettingGate::new(false, true),
            }),
            window,
            cx,
        )
    });
    let view = built
        .borrow_mut()
        .take()
        .expect("the window builder must hand the page out");

    visual.update(|window, cx| window.render_frame(cx));
    // The probe is the point of the surrounding assertions: a harness that drew
    // nothing would satisfy every "no command was sent" check below.
    let harness = rendered_bounds(visual, "shortcuts-harness".into());
    assert!(
        harness.size.width > px(0.) && harness.size.height > px(0.),
        "the harness must have drawn the page, not an empty frame"
    );

    let play = rendered_bounds(visual, ElementId::from(("play-model-shortcut", row_index)));
    visual.simulate_click(play.center(), Modifiers::default());
    assert!(
        view.read_with(visual, |view, _| view.shortcut_capture.is_none()
            && view.pending.is_none()),
        "pressing play must not start recording a chord"
    );
    visual.run_until_parked();
    match endpoint
        .try_recv()
        .expect("pressing play must reach the service")
    {
        crate::SettingsCommand::PreviewModelBehavior {
            model: sent,
            behavior: sent_behavior,
            ..
        } => {
            assert_eq!(sent, model, "the preview must name the row's own model");
            assert_eq!(sent_behavior, behavior);
        }
        crate::SettingsCommand::SuspendShortcutCapture { .. } => {
            panic!("pressing play must not start recording a chord")
        }
        _ => panic!("pressing play must ask the service to play the behavior"),
    }

    // The clear control is inside the frame too, and its own job still runs:
    // the row has no binding, so it asks for nothing — but the press must not
    // have been read as a request to record either.
    let clear = rendered_bounds(visual, ElementId::from(("clear-model-shortcut", row_index)));
    visual.simulate_click(clear.center(), Modifiers::default());
    assert!(
        view.read_with(visual, |view, _| view.shortcut_capture.is_none()),
        "pressing clear must not start recording a chord"
    );
    visual.run_until_parked();
    assert!(
        endpoint.try_recv().is_err(),
        "clearing a row with no binding must ask the service for nothing"
    );

    // The frame itself still records: the controls sit inside it, so a press
    // that is not on one of them reaches the frame and starts a capture. The
    // recording only begins once the service has suspended the platform table,
    // and this test never answers the request, so the observable result is the
    // operation in flight — that is what the request was for.
    let frame = rendered_bounds(
        visual,
        ElementId::from(("capture-model-shortcut", row_index)),
    );
    visual.simulate_click(frame.center(), Modifiers::default());
    assert!(
        matches!(
            view.read_with(visual, |view, _| view.pending),
            Some(PendingOperation::BeginShortcutCapture)
        ),
        "pressing the frame must start recording a chord"
    );
    assert!(
        view.read_with(visual, |view, _| view.shortcut_capture.is_none()),
        "recording must not begin before the service has suspended the table"
    );
    visual.run_until_parked();
    assert!(
        matches!(
            endpoint.try_recv(),
            Ok(crate::SettingsCommand::SuspendShortcutCapture { .. })
        ),
        "pressing the frame must ask the service to suspend shortcut capture"
    );
}

/// The frame is one control: it is sized by what it holds, and the two controls
/// stay inside its border.
///
/// This is the shape the input-group design gives a row, and only the laid-out
/// frame can show it: the frame is sized by its content rather than pinned to
/// the floor under it, both controls are drawn *inside* the frame and in reading
/// order — the clear button used to sit outside it — and the controls are small
/// enough for the frame's height instead of being the thing that sets it.
///
/// The row under test is a model behavior row with no binding. A recorded chord
/// is drawn in the compiled platform's own spelling — `F12` on every target, but
/// `Control+Shift+Alt+Super+ArrowLeft` only where the platform writes modifiers
/// as words — so only the placeholder text has a width this test can depend on.
#[gpui_kit::test]
fn a_shortcut_rows_frame_is_sized_by_its_chord_and_holds_its_controls(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (client, _endpoint) = crate::SettingsClient::bounded(4);
    let mut seeded = crate::tests::snapshot(1, false, true);
    seeded.model_catalog.entries = vec![model_entry(
        "standard",
        SettingsModelOrigin::BuiltIn,
        SettingsModelAvailability::Ready {
            behaviors: vec![SettingsModelBehavior::Motion {
                group: "CAT_motion".to_owned(),
                index: 0,
            }],
        },
    )];
    let row_index = ShortcutScope::Model.row_index_offset(&seeded.shortcuts);
    let snapshot = seeded.clone();
    let entries = seeded.model_catalog.entries.clone();
    let active = seeded.active_model.clone();

    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            SettingsView::new(
                client,
                SettingsWindowSeed {
                    language: SettingsLanguage::English,
                    appearance_theme: SettingsTheme::System,
                },
                Rc::new(|_| {}),
                Rc::new(|_| {}),
                PomodoroClient::unavailable(),
                window,
                cx,
            )
        });
        view.update(cx, |view, cx| {
            view.snapshot = Some(snapshot.clone());
            view.sync_shortcut_row_focus(
                &snapshot.shortcuts,
                active.as_ref(),
                &entries,
                &snapshot.model_behavior_names,
                false,
                cx,
            );
        });
        Root::new(
            cx.new(|_| ShortcutsPageHarness {
                view,
                snapshot: Some(snapshot),
                scope: ShortcutScope::Model,
                gate: SettingGate::new(false, true),
            }),
            window,
            cx,
        )
    });

    visual.update(|window, cx| window.render_frame(cx));
    let frame = rendered_bounds(
        visual,
        ElementId::from(("capture-model-shortcut", row_index)),
    );
    let play = rendered_bounds(visual, ElementId::from(("play-model-shortcut", row_index)));
    let clear = rendered_bounds(visual, ElementId::from(("clear-model-shortcut", row_index)));

    assert!(
        frame.size.width > px(180.),
        "the row in the frame must size it past its floor; measured {}",
        frame.size.width
    );
    assert!(
        play.left() >= frame.left() && clear.right() <= frame.right(),
        "both controls must be drawn inside the frame: frame {}..{}, play from {}, clear to {}",
        frame.left(),
        frame.right(),
        play.left(),
        clear.right()
    );
    assert!(
        play.right() <= clear.left(),
        "the play control must read before the clear control: play ends {}, clear starts {}",
        play.right(),
        clear.left()
    );
    assert!(
        play.size.height < frame.size.height && clear.size.height < frame.size.height,
        "the controls must fit inside the frame's height, not fill it: frame {}, play {}, \
         clear {}",
        frame.size.height,
        play.size.height,
        clear.size.height
    );
}

#[test]
fn window_shortcuts_are_visible_and_recordable_without_saved_bindings() {
    let mut shortcuts = SettingsShortcuts::default();
    let rows = window_shortcut_rows(&shortcuts);
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|row| row.shortcut.is_none()));

    let target = ShortcutCaptureTarget::Command("open_settings".to_owned());
    assert!(replace_shortcut(
        &mut shortcuts,
        &target,
        "Control+Shift+S".to_owned(),
    ));
    assert_eq!(shortcuts.commands.len(), 1);
    assert_eq!(shortcuts.commands[0].command, "open_settings");
    assert_eq!(
        window_shortcut_rows(&shortcuts)[1].shortcut.as_deref(),
        Some("Control+Shift+S")
    );
}

#[test]
fn captured_shortcut_updates_stable_identity_after_reordering() {
    let target = ShortcutCaptureTarget::Command("toggle_overlay".to_owned());
    let mut shortcuts = SettingsShortcuts {
        commands: vec![
            SettingsShortcutBinding {
                command: "open_settings".to_owned(),
                shortcut: "Control+S".to_owned(),
            },
            SettingsShortcutBinding {
                command: "toggle_overlay".to_owned(),
                shortcut: "Control+B".to_owned(),
            },
        ],
        model_behaviors: Vec::new(),
    };

    assert!(replace_shortcut(
        &mut shortcuts,
        &target,
        "Control+O".to_owned()
    ));
    assert_eq!(shortcuts.commands[0].shortcut, "Control+S");
    assert_eq!(shortcuts.commands[1].shortcut, "Control+O");
    assert!(!replace_shortcut(
        &mut shortcuts,
        &ShortcutCaptureTarget::Command("missing".to_owned()),
        "Control+X".to_owned()
    ));
}

#[test]
fn behavior_shortcut_capture_creates_and_clear_removes_a_binding() {
    let target = ShortcutCaptureTarget::ModelBehavior {
        model: settings_model_key("standard", SettingsModelOrigin::BuiltIn),
        behavior_id: "expression:happy".to_owned(),
    };
    let mut shortcuts = SettingsShortcuts::default();

    assert!(replace_shortcut(
        &mut shortcuts,
        &target,
        "Control+Alt+H".to_owned(),
    ));
    assert_eq!(shortcuts.model_behaviors.len(), 1);
    assert_eq!(shortcuts.model_behaviors[0].shortcut, "Control+Alt+H");
    assert!(clear_shortcut(&mut shortcuts, &target));
    assert!(shortcuts.model_behaviors.is_empty());
    assert!(!clear_shortcut(&mut shortcuts, &target));
}

#[test]
fn command_shortcut_clear_removes_only_the_selected_binding() {
    let mut shortcuts = SettingsShortcuts {
        commands: vec![
            SettingsShortcutBinding {
                command: "toggle_overlay".to_owned(),
                shortcut: "Control+B".to_owned(),
            },
            SettingsShortcutBinding {
                command: "open_settings".to_owned(),
                shortcut: "Control+S".to_owned(),
            },
        ],
        model_behaviors: Vec::new(),
    };
    assert!(clear_shortcut(
        &mut shortcuts,
        &ShortcutCaptureTarget::Command("toggle_overlay".to_owned()),
    ));
    assert_eq!(shortcuts.commands.len(), 1);
    assert_eq!(shortcuts.commands[0].command, "open_settings");
}

/// The ModelBehavior page's palette, drawn the way the page's settings item
/// builds it: through `model_behavior_palette`, so the controls under test are
/// the ones the product draws.
struct ModelBehaviorPaletteHarness {
    view: Entity<SettingsView>,
    snapshot: Option<SettingsSnapshot>,
    gate: SettingGate,
}

impl Render for ModelBehaviorPaletteHarness {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.snapshot.clone();
        let gate = self.gate;
        let tokens = Tokens::from_theme(cx);
        div()
            .id("model-behavior-palette-harness")
            .test_support()
            .child(self.view.clone().update(cx, move |_view, cx| {
                shortcuts_page::model_behavior_palette(
                    cx,
                    SettingsLanguage::English,
                    gate,
                    tokens,
                    snapshot.as_ref(),
                )
            }))
    }
}

#[gpui_kit::test]
fn the_model_behavior_page_lists_the_active_models_behaviors(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (client, _endpoint) = crate::SettingsClient::bounded(4);
    let mut seeded = crate::tests::snapshot(1, false, true);
    let expression = SettingsModelBehavior::Expression {
        name: "a".to_owned(),
    };
    let motion = SettingsModelBehavior::Motion {
        group: "CAT_motion".to_owned(),
        index: 0,
    };
    seeded.model_catalog.entries = vec![model_entry(
        "standard",
        SettingsModelOrigin::BuiltIn,
        SettingsModelAvailability::Ready {
            behaviors: vec![expression.clone(), motion.clone()],
        },
    )];
    let snapshot = seeded.clone();

    let built = Rc::new(RefCell::new(None));
    let capture = Rc::clone(&built);
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            SettingsView::new(
                client,
                SettingsWindowSeed {
                    language: SettingsLanguage::English,
                    appearance_theme: SettingsTheme::System,
                },
                Rc::new(|_| {}),
                Rc::new(|_| {}),
                PomodoroClient::unavailable(),
                window,
                cx,
            )
        });
        view.update(cx, |view, _cx| {
            view.snapshot = Some(snapshot.clone());
        });
        capture.borrow_mut().replace(view.clone());
        Root::new(
            cx.new(|_| ModelBehaviorPaletteHarness {
                view,
                snapshot: Some(snapshot),
                gate: SettingGate::new(false, true),
            }),
            window,
            cx,
        )
    });
    let _view = built
        .borrow_mut()
        .take()
        .expect("the window builder must hand the page out");

    visual.update(|window, cx| window.render_frame(cx));
    // The probe is the point of the surrounding assertions: a page that drew
    // nothing would satisfy every "the chip is present" check below.
    let harness = rendered_bounds(visual, "model-behavior-palette-harness".into());
    assert!(
        harness.size.width > px(0.) && harness.size.height > px(0.),
        "the palette must draw, not an empty frame"
    );

    // Each behavior is a chip carrying the behavior's own id, so finding the
    // chip proves the page listed that behavior and made it clickable.
    let expression_chip = rendered_bounds(
        visual,
        format!("behavior-palette-{}", model_behavior_id(&expression)).into(),
    );
    let motion_chip = rendered_bounds(
        visual,
        format!("behavior-palette-{}", model_behavior_id(&motion)).into(),
    );
    assert!(
        expression_chip.size.width > px(0.) && motion_chip.size.width > px(0.),
        "both behaviors must be listed as clickable chips"
    );
}

/// The trial palette plays a behavior even while the model behavior shortcuts
/// switch is off.
///
/// A trial is not a shortcut binding, so the switch that gates the shortcut
/// rows must not take it away: the chip stays clickable and still asks the
/// service to play the behavior. This is what a trial is for — seeing what a
/// model can do before binding anything to it. The test proves it by turning
/// the switch off and clicking a chip anyway.
#[gpui_kit::test]
fn the_behavior_palette_plays_even_with_the_shortcut_switch_off(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (client, endpoint) = crate::SettingsClient::bounded(4);
    let mut seeded = crate::tests::snapshot(1, false, true);
    // The switch the palette would have been gated by is off.
    seeded.behavior_shortcuts_enabled = false;
    let expression = SettingsModelBehavior::Expression {
        name: "happy".to_owned(),
    };
    seeded.model_catalog.entries = vec![model_entry(
        "standard",
        SettingsModelOrigin::BuiltIn,
        SettingsModelAvailability::Ready {
            behaviors: vec![expression.clone()],
        },
    )];
    let snapshot = seeded.clone();

    let built = Rc::new(RefCell::new(None));
    let capture = Rc::clone(&built);
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            SettingsView::new(
                client,
                SettingsWindowSeed {
                    language: SettingsLanguage::English,
                    appearance_theme: SettingsTheme::System,
                },
                Rc::new(|_| {}),
                Rc::new(|_| {}),
                PomodoroClient::unavailable(),
                window,
                cx,
            )
        });
        view.update(cx, |view, _cx| {
            view.snapshot = Some(snapshot.clone());
        });
        capture.borrow_mut().replace(view.clone());
        Root::new(
            cx.new(|_| ModelBehaviorPaletteHarness {
                view,
                snapshot: Some(snapshot),
                // The gate reports the switch off; only the structural block
                // would disable the trial.
                gate: SettingGate::new(false, false),
            }),
            window,
            cx,
        )
    });
    let _view = built
        .borrow_mut()
        .take()
        .expect("the window builder must hand the page out");

    visual.update(|window, cx| window.render_frame(cx));
    let chip = rendered_bounds(
        visual,
        format!("behavior-palette-{}", model_behavior_id(&expression)).into(),
    );
    assert!(
        chip.size.width > px(0.) && chip.size.height > px(0.),
        "the chip must be drawn even with the switch off"
    );
    visual.simulate_click(chip.center(), Modifiers::default());
    visual.run_until_parked();
    match endpoint
        .try_recv()
        .expect("clicking the chip must reach the service")
    {
        crate::SettingsCommand::PreviewModelBehavior { behavior: sent, .. } => {
            assert_eq!(
                sent, expression,
                "the chip must play the behavior it names, switch off or not"
            );
        }
        _ => panic!("clicking the chip must ask the service to play the behavior"),
    }
}

/// A trial keeps working while a settings save is in flight.
///
/// `pending` flips on and off around every save. The shared editing predicate
/// deliberately leaves it out of every gate, because threading it through the
/// page dims and re-enables every control on each change. The trial path still
/// routed through the *binding* availability check, which does read `pending`,
/// so a click landing during a save was silently dropped: no playback and no
/// error. A trial is not a binding, so it answers only to structural blocks.
#[gpui_kit::test]
fn the_behavior_palette_plays_while_a_save_is_in_flight(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (client, endpoint) = crate::SettingsClient::bounded(4);
    let mut seeded = crate::tests::snapshot(1, false, true);
    let expression = SettingsModelBehavior::Expression {
        name: "surprised.exp3.json".to_owned(),
    };
    seeded.model_catalog.entries = vec![model_entry(
        "standard",
        SettingsModelOrigin::BuiltIn,
        SettingsModelAvailability::Ready {
            behaviors: vec![expression.clone()],
        },
    )];
    let snapshot = seeded.clone();

    let built = Rc::new(RefCell::new(None));
    let capture = Rc::clone(&built);
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            SettingsView::new(
                client,
                SettingsWindowSeed {
                    language: SettingsLanguage::English,
                    appearance_theme: SettingsTheme::System,
                },
                Rc::new(|_| {}),
                Rc::new(|_| {}),
                PomodoroClient::unavailable(),
                window,
                cx,
            )
        });
        view.update(cx, |view, _cx| {
            view.snapshot = Some(snapshot.clone());
            // A save is in flight: the transient state every page gate is
            // required to ignore.
            view.pending = Some(PendingOperation::ModelSelection);
        });
        capture.borrow_mut().replace(view.clone());
        Root::new(
            cx.new(|_| ModelBehaviorPaletteHarness {
                view,
                snapshot: Some(snapshot),
                gate: SettingGate::new(false, false),
            }),
            window,
            cx,
        )
    });
    let _view = built
        .borrow_mut()
        .take()
        .expect("the window builder must hand the page out");

    visual.update(|window, cx| window.render_frame(cx));
    let chip = rendered_bounds(
        visual,
        format!("behavior-palette-{}", model_behavior_id(&expression)).into(),
    );
    assert!(
        chip.size.width > px(0.) && chip.size.height > px(0.),
        "the chip must stay drawn while a save is in flight"
    );
    visual.simulate_click(chip.center(), Modifiers::default());
    visual.run_until_parked();
    match endpoint
        .try_recv()
        .expect("a trial click landing during a save must still reach the service")
    {
        crate::SettingsCommand::PreviewModelBehavior { behavior: sent, .. } => {
            assert_eq!(
                sent, expression,
                "the trial must play the behavior it names, save in flight or not"
            );
        }
        _ => panic!("clicking the chip must ask the service to play the behavior"),
    }
}

/// The palette shows each real act once and names shipped resources by what
/// they do, instead of printing file names. The two built-in motion groups
/// declare identical clips, so grouping by real clip is what removes the twins;
/// the shortcut rows and their behavior_ids are deliberately untouched.
#[test]
fn the_palette_deduplicates_motion_groups_and_names_preset_behaviors() {
    use super::super::shortcuts_page::{ShortcutScope, palette_behavior_name, palette_blocks};

    let behaviors = vec![
        SettingsModelBehavior::Motion {
            group: "CAT_motion".to_owned(),
            index: 2,
        },
        SettingsModelBehavior::Motion {
            group: "CAT_motion_lock".to_owned(),
            index: 2,
        },
        SettingsModelBehavior::Expression {
            name: "surprised.exp3.json".to_owned(),
        },
        SettingsModelBehavior::Expression {
            name: "angry.exp3.json".to_owned(),
        },
    ];
    let mut seeded = crate::tests::snapshot(1, false, true);
    seeded.model_catalog.entries = vec![model_entry(
        "standard",
        SettingsModelOrigin::BuiltIn,
        SettingsModelAvailability::Ready { behaviors },
    )];
    let rows = ShortcutScope::Model.rows(
        &seeded.shortcuts,
        seeded.active_model.as_ref(),
        &seeded.model_catalog.entries,
        &seeded.model_behavior_names,
    );

    let motions = palette_blocks(&rows, false);
    let expressions = palette_blocks(&rows, true);
    assert_eq!(
        motions.iter().map(|line| line.len()).sum::<usize>(),
        1,
        "the two shipped groups declare the same clip; the palette shows it once"
    );
    assert_eq!(
        expressions.iter().map(|line| line.len()).sum::<usize>(),
        2,
        "two distinct expressions stay two chips"
    );
    assert_eq!(
        palette_behavior_name(SettingsLanguage::ChineseSimplified, motions[0][0]),
        "点头"
    );
    let names: Vec<String> = expressions
        .iter()
        .flatten()
        .map(|row| palette_behavior_name(SettingsLanguage::ChineseSimplified, row))
        .collect();
    assert!(
        names.contains(&"惊讶".to_owned()),
        "surprised must read as 惊讶"
    );
    assert!(
        names.contains(&"生气".to_owned()),
        "angry must read as 生气"
    );
    assert!(
        !names.iter().any(|name| name.contains(".exp3.json")),
        "a shipped expression must never print its file name"
    );
}

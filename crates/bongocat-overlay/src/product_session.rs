//! The session, and the input it owns.
//!
//! One session owns the overlay's window, its GPU surface and its input
//! registration, and it is the only thing that can shut them down in order. A
//! denied input permission is reported once as a degraded state rather than
//! retried every frame, because a permission the user has declined will not
//! become granted by asking again.

use super::*;

impl HasWindowHandle for ProductOverlaySession {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

pub(crate) fn input_start_failure_diagnostics(
    error: PlatformInputError,
) -> PlatformInputDiagnostics {
    PlatformInputDiagnostics {
        service_status: match error {
            PlatformInputError::PermissionDenied => PlatformInputServiceStatus::PermissionDenied,
            PlatformInputError::BackendUnavailable => {
                PlatformInputServiceStatus::BackendUnavailable
            }
            _ => PlatformInputServiceStatus::Failed,
        },
        service_error_code: Some(error.as_str()),
        service_start_attempts: 1,
        ..PlatformInputDiagnostics::default()
    }
}

pub(crate) fn start_platform_input<T>(
    diagnostics_producer: &PlatformInputDiagnosticsProducer,
    start: impl FnOnce() -> Result<T, PlatformInputError>,
) -> (Option<T>, Option<PlatformInputError>) {
    match start() {
        Ok(service) => (Some(service), None),
        Err(error) => {
            let _ = diagnostics_producer.publish(input_start_failure_diagnostics(error));
            (None, Some(error))
        }
    }
}

pub struct ProductOverlaySession {
    pub(crate) inner: macos::ProductOverlaySession,
}

/// A right-click on the model window. The application owns menu presentation
/// and action handling; the overlay only reports this platform input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayContextMenuRequest;

/// The end of a right-button drag that resized the model window.
///
/// The overlay reports only the scale the drag settled on. Window geometry is
/// already published through the placement path the frame loop owns, and the
/// application is the only owner of configuration, so the scale is a request
/// rather than a write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayResizeOutcome {
    /// The scale the drag ended on, clamped to the `25–400` configuration
    /// contract by the state machine.
    pub scale_percent: u16,
}

/// The countdown the overlay draws above the cat.
///
/// It mirrors the domain's read model rather than carrying it: the overlay draws
/// a surface, it does not own the timer, and giving it the domain type would make
/// the renderer depend on a feature the model has nothing to do with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayPomodoroTimer {
    pub remaining_seconds: u32,
    pub paused: bool,
}

/// A finished focus session the overlay should perform.
///
/// `sequence` is what makes it a *request* rather than a state: the overlay plays
/// a performance when the sequence changes, so a session that finished while the
/// application was not looking is still performed exactly once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayPomodoroCompletion {
    pub sequence: u64,
}

/// The cultivation copy the overlay draws, composed by the application.
///
/// The copy arrives already translated: the overlay has no locale of its own,
/// and it is the application that owns the language the user chose. The overlay
/// is a renderer of text here, not a catalog reader.
///
/// The progress is in parts per thousand rather than a fraction so the value
/// stays comparable — a surface compares what it was handed to decide whether to
/// redraw at all.
///
/// The realm itself is deliberately not part of it: the mouse pad belongs to
/// the model, and a number the code cannot measure the pad against ends up
/// printed over the model rather than on it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OverlayCultivationText {
    /// The hover text: the realm and the growth behind it on the first line,
    /// the pomodoros done and still to go on the second.
    pub hud: String,
    /// How far through the current layer the cultivator is, in parts per
    /// thousand. The HUD divides it back down for its bar.
    pub hud_progress_permille: u16,
}

/// What a right-click on the countdown asks for.
///
/// The overlay reports the request and the application owns the menu, exactly as
/// the model window's context menu does: presenting a menu is a product action,
/// and a panel that popped its own would make two surfaces disagree about how the
/// product behaves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayPomodoroMenuRequest {
    /// The countdown is running: offer to pause it.
    OfferPause,
    /// The countdown is paused: offer to resume or abandon it.
    OfferResumeAndAbandon,
}

/// Optional application-owned event handoffs consumed by the native overlay.
/// They carry no overlay state and are never used to render or mutate config.
pub struct OverlayInteractionSinks {
    pub context_menu_sender: Option<SyncSender<OverlayContextMenuRequest>>,
    pub resize_sender: Option<SyncSender<OverlayResizeOutcome>>,
    /// Where a right-click on the countdown is reported.
    pub pomodoro_menu_sender: Option<SyncSender<OverlayPomodoroMenuRequest>>,
}

impl ProductOverlaySession {
    pub fn start(
        runtime_client: RuntimeClient,
        input_producer: InputProducer,
        cursor_producer: CursorProducer,
        gamepad_axis_producer: GamepadAxisProducer,
        render_consumer: RenderConsumer,
        options: OverlaySessionOptions,
    ) -> Result<Self, OverlayError> {
        Self::start_with_interaction_sinks(
            runtime_client,
            input_producer,
            cursor_producer,
            gamepad_axis_producer,
            render_consumer,
            options,
            OverlayInteractionSinks {
                context_menu_sender: None,
                resize_sender: None,
                pomodoro_menu_sender: None,
            },
        )
    }

    pub fn start_with_interaction_sinks(
        runtime_client: RuntimeClient,
        input_producer: InputProducer,
        cursor_producer: CursorProducer,
        gamepad_axis_producer: GamepadAxisProducer,
        render_consumer: RenderConsumer,
        options: OverlaySessionOptions,
        interaction_sinks: OverlayInteractionSinks,
    ) -> Result<Self, OverlayError> {
        {
            macos::ProductOverlaySession::start(
                runtime_client,
                input_producer,
                cursor_producer,
                gamepad_axis_producer,
                render_consumer,
                options,
                interaction_sinks,
            )
            .map(|inner| Self { inner })
        }
    }

    pub fn run_for(&mut self, duration: Duration) -> Result<(), OverlayError> {
        self.inner.run_for(duration)
    }

    pub fn tick(&mut self) -> Result<OverlayTickOutcome, OverlayError> {
        self.inner.tick()
    }

    pub fn window_bounds(&self) -> Result<OverlayWindowBounds, OverlayError> {
        self.inner.window_bounds()
    }

    pub fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }

    pub fn model_generation(&self) -> u64 {
        self.inner.model_generation()
    }

    /// Publish what the pomodoro worker last reported.
    ///
    /// The countdown and the completion are handed in rather than pulled from a
    /// worker the overlay does not own: the timer has its own thread and its own
    /// document, and the overlay draws a surface for it. `completion` is a
    /// *request* — the overlay performs a session once per sequence it has not
    /// played — so a session that finished while the application was busy is
    /// still performed, exactly once.
    ///
    /// macOS only. The countdown and the performance are macOS surfaces.
    pub fn set_pomodoro_state(
        &mut self,
        timer: Option<OverlayPomodoroTimer>,
        completion: Option<OverlayPomodoroCompletion>,
    ) {
        self.inner.set_pomodoro_state(timer, completion);
    }

    /// Publish the cultivation copy the pad plate and the hover HUD draw.
    ///
    /// macOS only: both surfaces are AppKit panels.
    pub fn set_cultivation_text(&mut self, copy: Option<OverlayCultivationText>) {
        self.inner.set_cultivation_text(copy);
    }

    /// The right-click the countdown raised since the last call, taken once.
    pub fn take_pomodoro_menu_request(&self) -> Option<OverlayPomodoroMenuRequest> {
        self.inner.take_pomodoro_menu_request()
    }

    pub fn stop_input(&mut self) -> Result<(), OverlayError> {
        self.inner.stop_input()
    }

    pub fn finish_after_runtime_shutdown(self) -> Result<ProductOverlayReport, OverlayError> {
        self.inner.finish_after_runtime_shutdown()
    }
}

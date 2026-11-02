//! The pomodoro window: the handle the application holds and the view behind
//! it.
//!
//! The window answers exactly one question — "what am I working on?" — so it
//! offers the three ways a user can answer it: name a new task, pick up the one
//! left unfinished, or start one from the list. Ticking a task off happens
//! right here too, one checkbox per row, so a finished task never needs a trip
//! to the settings window; the list itself — search and the monthly history —
//! still belongs to the settings window.
//!
//! It is a singleton like the update window: the tray and the model-window
//! context menu both route through here, so a second request focuses the open
//! window instead of stacking another one.

mod render;
#[cfg(test)]
mod tests;
pub(crate) mod view;

use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::{SettingsClient, SettingsLanguage, SettingsTheme, window::apply_component_theme};
use bongocat_i18n::text;
use bongocat_pomodoro::PomodoroClient;
use gpui_kit::component::{Root, Theme};
use gpui_kit::{
    Anchor, App, AppContext, Context, Render, TitlebarOptions, WeakEntity, Window, WindowBounds,
    WindowHandle, WindowOptions, prelude::*, px, size,
};

pub use view::PomodoroView;

/// How often the window re-reads the countdown.
///
/// The tick is short because the seconds digit moves every second and a stale
/// digit is a countdown nobody trusts; the language and theme poll is slower
/// because those change only when the user changes them.
const TIMER_POLL_INTERVAL: Duration = Duration::from_millis(250);
const SETTINGS_POLL_INTERVAL: Duration = Duration::from_secs(1);

const WINDOW_WIDTH: f32 = 480.0;
const WINDOW_MIN_WIDTH: f32 = 420.0;
const WINDOW_HEIGHT: f32 = 600.0;
const WINDOW_MIN_HEIGHT: f32 = 400.0;

/// Every user-visible string on this window.
///
/// The contract is kept beside the window and asserted by the render tests, so a
/// key the frame stopped using cannot sit in the catalog forever and a key the
/// frame started using cannot be missing from one language.
#[cfg(test)]
pub(super) const POMODORO_LOCALIZED_KEYS: &[&str] = &[
    "pomodoro.window.title",
    "pomodoro.picker.new_task",
    "pomodoro.picker.continue_last",
    "pomodoro.picker.task_name",
    "pomodoro.picker.task_name_placeholder",
    "pomodoro.picker.estimated_sessions",
    "pomodoro.picker.create",
    "pomodoro.picker.close",
    "pomodoro.settings.task_editor_title",
    "pomodoro.settings.save",
    "pomodoro.settings.cancel",
    "pomodoro.settings.task_edit",
    "pomodoro.settings.task_duplicate",
    "pomodoro.settings.task_delete",
    "pomodoro.settings.done_group",
    "pomodoro.task.start",
    "pomodoro.task.running_now",
    "pomodoro.task.progress",
    "pomodoro.task.progress_compact",
    "pomodoro.task.created_at",
    "pomodoro.task.none",
    "pomodoro.task.section",
    "pomodoro.picker.session_length",
    "pomodoro.worker_lost",
];

/// The open pomodoro window.
#[derive(Clone)]
pub struct PomodoroWindowHandle {
    pub(super) window: WindowHandle<Root>,
    pub(super) view: WeakEntity<PomodoroView>,
}

impl PomodoroWindowHandle {
    pub fn update<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut PomodoroView, &mut Window, &mut Context<PomodoroView>) -> R,
    ) -> gpui_kit::Result<R>
    where
        C: AppContext,
    {
        self.window.update(cx, |_, window, cx| {
            self.view.update(cx, |view, cx| update(view, window, cx))
        })?
    }

    pub fn view(&self) -> &WeakEntity<PomodoroView> {
        &self.view
    }

    pub fn window(&self) -> &WindowHandle<Root> {
        &self.window
    }

    /// Bring the window to the front.
    pub fn activate(&self, cx: &mut App) -> gpui_kit::Result<()> {
        self.window
            .update(cx, |_, window, _| window.activate_window())
    }

    /// Whether the window still exists.
    ///
    /// A handle outlives the window it was opened for, so a caller that needs to
    /// know whether the user still has it on screen asks here.
    pub fn is_open(&self) -> bool {
        self.view.upgrade().is_some()
    }
}

impl PartialEq for PomodoroWindowHandle {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window
    }
}

impl Eq for PomodoroWindowHandle {}

/// Open the pomodoro window.
///
/// `language` and `appearance_theme` are the display language and the appearance
/// at open time; the window keeps itself in sync with the settings snapshot
/// afterwards, so neither a language nor a theme change needs a reopen.
pub fn open_pomodoro_window(
    client: PomodoroClient,
    settings_client: SettingsClient,
    language: SettingsLanguage,
    appearance_theme: SettingsTheme,
    cx: &mut App,
) -> Result<PomodoroWindowHandle, String> {
    let bounds = WindowBounds::Windowed(gpui_kit::Bounds::centered(
        None,
        size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
        cx,
    ));
    let view_slot = Rc::new(RefCell::new(None));
    let opened_view = Rc::clone(&view_slot);
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                window_min_size: Some(size(px(WINDOW_MIN_WIDTH), px(WINDOW_MIN_HEIGHT))),
                titlebar: Some(TitlebarOptions {
                    title: Some(text(language.catalog_locale(), "pomodoro.window.title").into()),
                    ..Default::default()
                }),
                focus: true,
                show: false,
                ..Default::default()
            },
            move |window, cx| {
                if !cx.has_global::<Theme>() {
                    gpui_kit::init(cx);
                }
                Theme::global_mut(cx).notification.placement = Anchor::BottomRight;
                apply_component_theme(appearance_theme, window, cx);
                let view = cx.new(|cx| {
                    let view = PomodoroView::new(
                        client,
                        settings_client,
                        language,
                        appearance_theme,
                        window,
                        cx,
                    );
                    view.start_polling(cx);
                    view.start_settings_polling(cx);
                    view
                });
                opened_view.borrow_mut().replace(view.clone());
                let appearance_view = view.downgrade();
                window
                    .observe_window_appearance(move |window, cx| {
                        // The callback is the system announcing that it changed. That is
                        // only the product's business while the product follows the
                        // system: a pinned preference was already handed to the platform,
                        // and it does not move because the system did.
                        let theme = appearance_view
                            .upgrade()
                            .map_or(SettingsTheme::System, |view| view.read(cx).appearance_theme);
                        if theme == SettingsTheme::System {
                            apply_component_theme(theme, window, cx);
                        }
                    })
                    .detach();
                let close_view = view.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    let _ = close_view.update(cx, |view, cx| {
                        view.applied_theme = None;
                        cx.notify();
                    });
                    true
                });
                let focus = view.read(cx).primary_focus.clone();
                window.focus(&focus, cx);
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .map_err(|error| error.to_string())?;
    let view = view_slot
        .borrow_mut()
        .take()
        .ok_or_else(|| "pomodoro view was not created".to_owned())?;
    // The window is created with `show: false` so it never flashes at its
    // unpositioned frame; activating the handle is what raises it. Without
    // this the first open produced a window that existed but stayed hidden —
    // the user had to trigger the menu a second time, which took the
    // singleton's "already open" branch, to see anything at all.
    let pomodoro_handle = PomodoroWindowHandle {
        window: handle,
        view: view.downgrade(),
    };
    let _ = pomodoro_handle.activate(cx);
    cx.activate(true);
    Ok(pomodoro_handle)
}

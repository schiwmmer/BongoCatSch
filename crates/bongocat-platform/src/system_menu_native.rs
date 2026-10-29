use crate::{PomodoroMenuMode, SystemMenuAction, SystemMenuError, SystemMenuPresentation};
use image::ImageReader;
use muda::{
    CheckMenuItem, ContextMenu, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem,
    Submenu,
};
use objc2::MainThreadMarker;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    io::Cursor,
    sync::mpsc::{self, Receiver, Sender},
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconId};

const TRAY_ICON_ID: &str = "bongocat.system-menu";
const OPEN_SETTINGS_ID: &str = "bongocat.open-settings";
const MODEL_WINDOW_SUBMENU_ID: &str = "bongocat.model-window";
const TOGGLE_OVERLAY_VISIBILITY_ID: &str = "bongocat.toggle-overlay";
const TOGGLE_CLICK_THROUGH_ID: &str = "bongocat.toggle-click-through";
const TOGGLE_ALWAYS_ON_TOP_ID: &str = "bongocat.toggle-always-on-top";
const TOGGLE_HIDE_ON_POINTER_HOVER_ID: &str = "bongocat.toggle-hide-on-pointer-hover";
const CHECK_FOR_UPDATES_ID: &str = "bongocat.check-for-updates";
const QUIT_ID: &str = "bongocat.quit";
const START_POMODORO_ID: &str = "bongocat.start-pomodoro";
const START_CULTIVATION_ID: &str = "bongocat.start-cultivation";
const PAUSE_POMODORO_ID: &str = "bongocat.pomodoro-pause";
const RESUME_POMODORO_ID: &str = "bongocat.pomodoro-resume";
const ABANDON_POMODORO_ID: &str = "bongocat.pomodoro-abandon";

const STATUS_ICON_BYTES: &[u8] = include_bytes!("../../../resources/icons/tray-macos.png");

/// The platform owner for both native menu surfaces.
///
/// The tray and model-window context surfaces intentionally share one native
/// menu tree and one set of item instances. The owner keeps the menu rooted
/// with the tray icon while also allowing the overlay to present that same
/// menu at its own `NSView`.
pub struct SystemMenu {
    // Keep the tray icon first: the native menu handles must be released after
    // the status item stops using them.
    tray_icon: TrayIcon,
    /// The tray's menu tree.
    ///
    /// The model-window popup is rebuilt per right-click, and the tray keeps its
    /// own handle on the tree.
    model_window: Submenu,
    items: NativeMenuItems,
    /// The labels the model-window menu is rebuilt from.
    ///
    /// The tray menu is built once and lives as long as the status item, but the
    /// model-window menu is rebuilt per popup: its pomodoro rows follow the
    /// session, and a menu built once would keep offering to start a session
    /// that is already running.
    presentation: SystemMenuPresentation,
    sender: Sender<SystemMenuAction>,
    receiver: Receiver<SystemMenuAction>,
    visible: bool,
}

struct NativeMenuItems {
    open_settings: MenuItem,
    toggle_overlay: CheckMenuItem,
    toggle_click_through: CheckMenuItem,
    toggle_always_on_top: CheckMenuItem,
    toggle_hide_on_pointer_hover: CheckMenuItem,
    check_for_updates: Option<MenuItem>,
    start_pomodoro: MenuItem,
    pause_pomodoro: MenuItem,
    resume_pomodoro: MenuItem,
    abandon_pomodoro: MenuItem,
    start_cultivation: MenuItem,
    quit: MenuItem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MenuEntry {
    OpenSettings,
    Separator,
    ModelWindow,
    ToggleOverlay,
    ToggleClickThrough,
    ToggleAlwaysOnTop,
    ToggleHideOnPointerHover,
    CheckForUpdates,
    StartPomodoro,
    PausePomodoro,
    ResumePomodoro,
    AbandonPomodoro,
    StartCultivation,
    Quit,
}

const MENU_ENTRIES: &[MenuEntry] = &[
    MenuEntry::OpenSettings,
    // The pomodoro row is a primary action beside Settings. Placing it before the
    // first separator keeps it out of the separator-collapsing rule below.
    MenuEntry::StartPomodoro,
    MenuEntry::StartCultivation,
    MenuEntry::Separator,
    MenuEntry::ModelWindow,
    MenuEntry::Separator,
    MenuEntry::CheckForUpdates,
    MenuEntry::Separator,
    MenuEntry::Quit,
];

const MODEL_WINDOW_ENTRIES: &[MenuEntry] = &[
    MenuEntry::ToggleOverlay,
    MenuEntry::ToggleClickThrough,
    MenuEntry::ToggleAlwaysOnTop,
    MenuEntry::ToggleHideOnPointerHover,
];

/// The model-window menu for a session in the given state.
///
/// Everything else in the menu is fixed; only the pomodoro rows move, and they
/// are rebuilt per popup rather than mutated in place so a menu that is open
/// cannot change under the pointer.
fn context_menu_entries(mode: PomodoroMenuMode) -> Vec<MenuEntry> {
    let pomodoro: &[MenuEntry] = match mode {
        PomodoroMenuMode::Idle => &[MenuEntry::StartPomodoro],
        PomodoroMenuMode::Running => &[MenuEntry::PausePomodoro, MenuEntry::AbandonPomodoro],
        PomodoroMenuMode::Paused => &[MenuEntry::ResumePomodoro, MenuEntry::AbandonPomodoro],
    };
    let mut entries = vec![MenuEntry::OpenSettings];
    entries.extend_from_slice(pomodoro);
    entries.push(MenuEntry::StartCultivation);
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::ModelWindow);
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::CheckForUpdates);
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Quit);
    entries
}

impl NativeMenuItems {
    fn new(presentation: &SystemMenuPresentation, include_update: bool) -> Self {
        Self {
            open_settings: MenuItem::with_id(
                OPEN_SETTINGS_ID,
                &presentation.open_settings,
                true,
                None,
            ),
            // Keep one fixed "Hide model window" row. Its check state is the
            // inverse of runtime visibility, so there is no separate Show row.
            toggle_overlay: CheckMenuItem::with_id(
                TOGGLE_OVERLAY_VISIBILITY_ID,
                &presentation.hide_overlay,
                true,
                visibility_checked(presentation.overlay_visible),
                None,
            ),
            toggle_click_through: CheckMenuItem::with_id(
                TOGGLE_CLICK_THROUGH_ID,
                &presentation.click_through,
                true,
                presentation.click_through_enabled,
                None,
            ),
            toggle_always_on_top: CheckMenuItem::with_id(
                TOGGLE_ALWAYS_ON_TOP_ID,
                &presentation.always_on_top,
                true,
                presentation.always_on_top_enabled,
                None,
            ),
            toggle_hide_on_pointer_hover: CheckMenuItem::with_id(
                TOGGLE_HIDE_ON_POINTER_HOVER_ID,
                &presentation.hide_on_pointer_hover,
                true,
                presentation.hide_on_pointer_hover_enabled,
                None,
            ),
            // Update availability is a build/channel fact and does not change
            // during the lifetime of this owner. Do not leave a permanently
            // disabled row in a production tray menu when the channel cannot
            // produce a release.
            check_for_updates: include_update.then(|| {
                MenuItem::with_id(
                    CHECK_FOR_UPDATES_ID,
                    &presentation.check_for_updates,
                    presentation.update_check_available,
                    None,
                )
            }),
            start_pomodoro: MenuItem::with_id(
                START_POMODORO_ID,
                &presentation.start_pomodoro,
                true,
                None,
            ),
            pause_pomodoro: MenuItem::with_id(
                PAUSE_POMODORO_ID,
                &presentation.pause_pomodoro,
                true,
                None,
            ),
            resume_pomodoro: MenuItem::with_id(
                RESUME_POMODORO_ID,
                &presentation.resume_pomodoro,
                true,
                None,
            ),
            abandon_pomodoro: MenuItem::with_id(
                ABANDON_POMODORO_ID,
                &presentation.abandon_pomodoro,
                true,
                None,
            ),
            start_cultivation: MenuItem::with_id(
                START_CULTIVATION_ID,
                &presentation.start_cultivation,
                true,
                None,
            ),
            quit: MenuItem::with_id(QUIT_ID, &presentation.quit, true, None),
        }
    }

    fn update(&self, presentation: &SystemMenuPresentation) {
        self.open_settings.set_text(&presentation.open_settings);
        self.toggle_overlay.set_text(&presentation.hide_overlay);
        self.toggle_overlay
            .set_checked(visibility_checked(presentation.overlay_visible));
        self.toggle_click_through
            .set_text(&presentation.click_through);
        self.toggle_click_through
            .set_checked(presentation.click_through_enabled);
        self.toggle_always_on_top
            .set_text(&presentation.always_on_top);
        self.toggle_always_on_top
            .set_checked(presentation.always_on_top_enabled);
        self.toggle_hide_on_pointer_hover
            .set_text(&presentation.hide_on_pointer_hover);
        self.toggle_hide_on_pointer_hover
            .set_checked(presentation.hide_on_pointer_hover_enabled);
        if let Some(check_for_updates) = &self.check_for_updates {
            check_for_updates.set_text(&presentation.check_for_updates);
            check_for_updates.set_enabled(presentation.update_check_available);
        }
        self.start_pomodoro.set_text(&presentation.start_pomodoro);
        self.pause_pomodoro.set_text(&presentation.pause_pomodoro);
        self.resume_pomodoro.set_text(&presentation.resume_pomodoro);
        self.abandon_pomodoro
            .set_text(&presentation.abandon_pomodoro);
        self.start_cultivation
            .set_text(&presentation.start_cultivation);
        self.quit.set_text(&presentation.quit);
    }
}

impl SystemMenu {
    pub fn start_with_presentation(
        visible: bool,
        presentation: SystemMenuPresentation,
    ) -> Result<Self, SystemMenuError> {
        MainThreadMarker::new().ok_or(SystemMenuError::WrongThread)?;

        let icon = load_status_icon()?;
        let items = NativeMenuItems::new(&presentation, presentation.update_check_available);
        let model_window = Submenu::with_id(
            MenuId::new(MODEL_WINDOW_SUBMENU_ID),
            &presentation.model_window,
            true,
        );
        append_model_window_entries(&model_window, &items)?;

        // One menu tree serves both the tray and the model-window context
        // surface. The optional update row is omitted when its channel cannot
        // produce a release.
        let menu = Menu::new();
        append_menu_entries(&menu, &items, &model_window, MENU_ENTRIES)?;

        let tray_icon_id = TrayIconId::new(TRAY_ICON_ID);
        let tray_icon_builder = TrayIconBuilder::new()
            .with_id(tray_icon_id.clone())
            .with_menu(Box::new(menu.clone()))
            .with_tooltip(&presentation.tooltip)
            .with_menu_on_left_click(cfg!(target_os = "macos"))
            .with_menu_on_right_click(true);
        let tray_icon_builder = tray_icon_builder.with_icon_templated(icon);
        let tray_icon = tray_icon_builder.build().map_err(|error| {
            if matches!(error, tray_icon::Error::NotMainThread) {
                return SystemMenuError::WrongThread;
            }
            SystemMenuError::StatusItemCreateFailed
        })?;

        if !visible {
            tray_icon
                .set_visible(false)
                .map_err(|_| SystemMenuError::StatusItemUpdateFailed)?;
        }

        let (sender, receiver) = mpsc::channel();
        Ok(Self {
            tray_icon,
            model_window,
            items,
            presentation,
            sender,
            receiver,
            visible,
        })
    }

    /// Applies a new settings snapshot to both native menu surfaces.
    ///
    /// `SystemMenuPresentation::tooltip` is a creation-time input: it is applied once by
    /// [`Self::start_with_presentation`] and deliberately not re-applied here. The tooltip
    /// must therefore stay invariant for the lifetime of this owner; changing it
    /// requires replacing the tray owner, which ADR-0031 forbids while the app is running.
    pub fn set_presentation(
        &mut self,
        presentation: SystemMenuPresentation,
    ) -> Result<(), SystemMenuError> {
        MainThreadMarker::new().ok_or(SystemMenuError::WrongThread)?;

        self.items.update(&presentation);
        self.model_window.set_text(&presentation.model_window);
        self.presentation = presentation;
        Ok(())
    }

    pub fn try_recv(&self) -> Option<SystemMenuAction> {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(action) = action_for_menu_id(&event.id) {
                let _ = self.sender.send(action);
            }
        }


        self.receiver.try_recv().ok()
    }

    /// Present a small menu of timer actions at the current pointer location.
    ///
    /// The menu is built per request from what the caller offers rather than held
    /// as a tree of its own: a running countdown offers a pause and a paused one
    /// offers a resume and an abandon, so a menu kept as fixed state would have to
    /// be rewritten on every transition anyway.
    ///
    /// macOS only. The countdown is a macOS surface.
    pub fn show_pomodoro_menu(
        &self,
        window: &impl HasWindowHandle,
        entries: &[(&str, &str)],
    ) -> Result<(), SystemMenuError> {
        let window = window
            .window_handle()
            .map_err(|_| SystemMenuError::WindowHandleUnavailable)?;
        let RawWindowHandle::AppKit(handle) = window.as_raw() else {
            return Err(SystemMenuError::UnsupportedWindowHandle);
        };
        let _main_thread = MainThreadMarker::new().ok_or(SystemMenuError::WrongThread)?;
        let menu = Menu::new();
        for (id, label) in entries {
            let item = MenuItem::with_id(*id, *label, true, None);
            menu.append(&item)
                .map_err(|_| SystemMenuError::MenuItemCreateFailed)?;
            // The item is appended by reference and the menu retains it, so the
            // local handle goes out of scope with the loop iteration.
            drop(item);
        }
        // SAFETY: the caller's `HasWindowHandle` keeps the content NSView alive
        // for the duration of this synchronous popup.
        let _ = unsafe { menu.show_context_menu_for_nsview(handle.ns_view.as_ptr(), None) };
        // `muda` routes every menu event to one process-wide receiver, which is
        // what the tray menu is read from too. The ids below are ones no other
        // menu carries, so an event drained here can only be this one's choice.
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(action) = action_for_menu_id(&event.id) {
                let _ = self.sender.send(action);
                break;
            }
        }
        Ok(())
    }

    /// Present the model-window context menu at the current pointer location.
    ///
    /// `mode` says which pomodoro rows the menu carries. macOS rebuilds the menu
    /// for the popup because those rows follow the session.
    pub fn show_context_menu_for_window(
        &self,
        window: &impl HasWindowHandle,
        mode: PomodoroMenuMode,
    ) -> Result<(), SystemMenuError> {
        let window = window
            .window_handle()
            .map_err(|_| SystemMenuError::WindowHandleUnavailable)?;

        match window.as_raw() {
            RawWindowHandle::AppKit(handle) => {
                let _main_thread = MainThreadMarker::new().ok_or(SystemMenuError::WrongThread)?;
                let menu = self.context_menu(mode)?;
                // SAFETY: the overlay's `HasWindowHandle` implementation keeps its
                // content NSView alive for the duration of this synchronous popup,
                // and `menu` outlives the call.
                let _ = unsafe { menu.show_context_menu_for_nsview(handle.ns_view.as_ptr(), None) };
                while let Ok(event) = MenuEvent::receiver().try_recv() {
                    if let Some(action) = action_for_menu_id(&event.id) {
                        let _ = self.sender.send(action);
                        break;
                    }
                }
                Ok(())
            }
            _ => Err(SystemMenuError::UnsupportedWindowHandle),
        }
    }

    /// The model-window menu for one popup.
    ///
    /// Built fresh rather than cloned so the pomodoro rows can differ from the
    /// tray's: the row a right-click lands on is the one that describes what the
    /// click would do right now.
    fn context_menu(&self, mode: PomodoroMenuMode) -> Result<Menu, SystemMenuError> {
        let items =
            NativeMenuItems::new(&self.presentation, self.items.check_for_updates.is_some());
        let model_window = Submenu::with_id(
            MenuId::new(MODEL_WINDOW_SUBMENU_ID),
            &self.presentation.model_window,
            true,
        );
        append_model_window_entries(&model_window, &items)?;
        let menu = Menu::new();
        append_menu_entries(&menu, &items, &model_window, &context_menu_entries(mode))?;
        Ok(menu)
    }

    #[doc(hidden)]
    pub fn request_action_for_smoke(
        &self,
        action: SystemMenuAction,
    ) -> Result<(), SystemMenuError> {
        self.sender
            .send(action)
            .map_err(|_| SystemMenuError::EventQueueClosed)
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<(), SystemMenuError> {
        if visible == self.visible {
            return Ok(());
        }
        self.tray_icon
            .set_visible(visible)
            .map_err(|_| SystemMenuError::StatusItemUpdateFailed)?;
        self.visible = visible;
        Ok(())
    }

    pub fn shutdown(mut self) -> Result<(), SystemMenuError> {
        let result = if self.visible {
            self.tray_icon
                .set_visible(false)
                .map_err(|_| SystemMenuError::StatusItemUpdateFailed)
        } else {
            Ok(())
        };
        self.visible = false;
        result
    }
}

impl Drop for SystemMenu {
    fn drop(&mut self) {
        let _ = self.tray_icon.set_visible(false);
    }
}

fn append_menu_entries(
    menu: &Menu,
    items: &NativeMenuItems,
    model_window: &Submenu,
    entries: &[MenuEntry],
) -> Result<(), SystemMenuError> {
    let mut previous_was_separator = false;
    for entry in entries {
        let entry = *entry;
        if entry == MenuEntry::CheckForUpdates && items.check_for_updates.is_none() {
            continue;
        }
        if entry == MenuEntry::Separator {
            // The update row is optional. Collapse the two separators around it
            // instead of leaving an empty section in an unavailable channel.
            if previous_was_separator {
                continue;
            }
            append_item(menu, &PredefinedMenuItem::separator())?;
            previous_was_separator = true;
            continue;
        }
        previous_was_separator = false;
        match entry {
            MenuEntry::OpenSettings => append_item(menu, &items.open_settings)?,
            MenuEntry::Separator => unreachable!("separator handled above"),
            MenuEntry::ModelWindow => append_item(menu, model_window)?,
            MenuEntry::ToggleOverlay => append_item(menu, &items.toggle_overlay)?,
            MenuEntry::ToggleClickThrough => append_item(menu, &items.toggle_click_through)?,
            MenuEntry::ToggleAlwaysOnTop => append_item(menu, &items.toggle_always_on_top)?,
            MenuEntry::ToggleHideOnPointerHover => {
                append_item(menu, &items.toggle_hide_on_pointer_hover)?
            }
            MenuEntry::CheckForUpdates => append_item(
                menu,
                items
                    .check_for_updates
                    .as_ref()
                    .ok_or(SystemMenuError::MenuCreateFailed)?,
            )?,
            MenuEntry::StartPomodoro => append_item(menu, &items.start_pomodoro)?,
            MenuEntry::PausePomodoro => append_item(menu, &items.pause_pomodoro)?,
            MenuEntry::ResumePomodoro => append_item(menu, &items.resume_pomodoro)?,
            MenuEntry::AbandonPomodoro => append_item(menu, &items.abandon_pomodoro)?,
            MenuEntry::StartCultivation => append_item(menu, &items.start_cultivation)?,
            MenuEntry::Quit => append_item(menu, &items.quit)?,
        }
    }
    Ok(())
}

fn append_model_window_entries(
    submenu: &Submenu,
    items: &NativeMenuItems,
) -> Result<(), SystemMenuError> {
    for entry in MODEL_WINDOW_ENTRIES {
        let item: &dyn IsMenuItem = match *entry {
            MenuEntry::ToggleOverlay => &items.toggle_overlay,
            MenuEntry::ToggleClickThrough => &items.toggle_click_through,
            MenuEntry::ToggleAlwaysOnTop => &items.toggle_always_on_top,
            MenuEntry::ToggleHideOnPointerHover => &items.toggle_hide_on_pointer_hover,
            _ => return Err(SystemMenuError::MenuCreateFailed),
        };
        submenu
            .append(item)
            .map_err(|_| SystemMenuError::MenuCreateFailed)?;
    }
    Ok(())
}

fn append_item(menu: &Menu, item: &dyn IsMenuItem) -> Result<(), SystemMenuError> {
    menu.append(item)
        .map_err(|_| SystemMenuError::MenuCreateFailed)
}

fn load_status_icon() -> Result<Icon, SystemMenuError> {
    let image = ImageReader::new(Cursor::new(STATUS_ICON_BYTES))
        .with_guessed_format()
        .map_err(|_| SystemMenuError::StatusIconImageLoadFailed)?
        .decode()
        .map_err(|_| SystemMenuError::StatusIconImageLoadFailed)?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|_| SystemMenuError::StatusIconImageLoadFailed)
}

fn visibility_checked(overlay_visible: bool) -> bool {
    !overlay_visible
}

fn action_for_menu_id(id: &MenuId) -> Option<SystemMenuAction> {
    Some(match id.0.as_str() {
        OPEN_SETTINGS_ID => SystemMenuAction::OpenSettings,
        TOGGLE_OVERLAY_VISIBILITY_ID => SystemMenuAction::ToggleOverlayVisibility,
        TOGGLE_CLICK_THROUGH_ID => SystemMenuAction::ToggleClickThrough,
        TOGGLE_ALWAYS_ON_TOP_ID => SystemMenuAction::ToggleAlwaysOnTop,
        TOGGLE_HIDE_ON_POINTER_HOVER_ID => SystemMenuAction::ToggleHideOnPointerHover,
        CHECK_FOR_UPDATES_ID => SystemMenuAction::CheckForUpdates,
        START_POMODORO_ID => SystemMenuAction::StartPomodoro,
        START_CULTIVATION_ID => SystemMenuAction::StartCultivation,
        PAUSE_POMODORO_ID => SystemMenuAction::PausePomodoro,
        RESUME_POMODORO_ID => SystemMenuAction::ResumePomodoro,
        ABANDON_POMODORO_ID => SystemMenuAction::AbandonPomodoro,
        QUIT_ID => SystemMenuAction::Quit,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        MENU_ENTRIES, MODEL_WINDOW_ENTRIES, MenuEntry, SystemMenuAction, action_for_menu_id,
        visibility_checked,
    };
    use muda::MenuId;

    #[test]
    fn visibility_is_rendered_as_a_check_state_meaning_hidden() {
        assert!(visibility_checked(false));
        assert!(!visibility_checked(true));
    }

    #[test]
    fn both_menu_surfaces_use_the_same_layout() {
        assert!(MENU_ENTRIES.contains(&MenuEntry::ModelWindow));
        assert!(MENU_ENTRIES.contains(&MenuEntry::CheckForUpdates));

        for entry in [
            MenuEntry::OpenSettings,
            MenuEntry::ModelWindow,
            MenuEntry::CheckForUpdates,
            MenuEntry::Quit,
        ] {
            assert!(MENU_ENTRIES.contains(&entry));
        }

        for entry in [
            MenuEntry::ToggleOverlay,
            MenuEntry::ToggleClickThrough,
            MenuEntry::ToggleAlwaysOnTop,
            MenuEntry::ToggleHideOnPointerHover,
        ] {
            assert!(MODEL_WINDOW_ENTRIES.contains(&entry));
        }
    }

    #[test]
    fn menu_action_mapping_keeps_only_the_current_action_set() {
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.open-settings")),
            Some(SystemMenuAction::OpenSettings)
        );
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.toggle-overlay")),
            Some(SystemMenuAction::ToggleOverlayVisibility)
        );
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.toggle-click-through")),
            Some(SystemMenuAction::ToggleClickThrough)
        );
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.toggle-always-on-top")),
            Some(SystemMenuAction::ToggleAlwaysOnTop)
        );
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.toggle-hide-on-pointer-hover")),
            Some(SystemMenuAction::ToggleHideOnPointerHover)
        );
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.check-for-updates")),
            Some(SystemMenuAction::CheckForUpdates)
        );
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.quit")),
            Some(SystemMenuAction::Quit)
        );
        assert_eq!(
            action_for_menu_id(&MenuId::new("bongocat.start-cultivation")),
            Some(SystemMenuAction::StartCultivation)
        );

        for removed in [
            "bongocat.open-source",
            "bongocat.restart",
            "bongocat.version",
            "bongocat.model-window",
        ] {
            assert_eq!(action_for_menu_id(&MenuId::new(removed)), None);
        }
    }
}

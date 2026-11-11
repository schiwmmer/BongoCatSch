//! The macOS app menu bar: its items, their labels and what each one does.
//!
//! The product runs as an accessory (menu-bar) app, so the menu bar is the only
//! native surface that can own a global Quit shortcut. `Cmd+Q` is bound to
//! [`QuitApp`] and the menu item carries the same accelerator, so the system
//! routes the keystroke to the action whether or not a GPUI window is focused.

use gpui_kit::{Action, App, KeyBinding, Menu, MenuItem};

/// Quits the product. The menu item and the `Cmd+Q` binding both dispatch this.
#[derive(Clone, Default, PartialEq, Debug, Action)]
#[action(no_register, no_json)]
pub(crate) struct QuitApp;

/// The label the menu bar shows for the app menu itself.
pub(crate) fn app_menu_title(locale: &str) -> &'static str {
    bongocat_i18n::text(locale, "system_menu.title")
}

/// The label the menu bar shows for the quit item.
pub(crate) fn quit_menu_label(locale: &str) -> &'static str {
    bongocat_i18n::text(locale, "system_menu.quit")
}

/// The menu bar: one menu, one item, one shortcut.
fn app_menu(locale: &str) -> Menu {
    Menu::new(app_menu_title(locale))
        .items(vec![MenuItem::action(quit_menu_label(locale), QuitApp)])
}

/// Installs the app menu bar and the `Cmd+Q` binding.
///
/// The binding is registered before the menu so the menu item can read the
/// keystroke back from the keymap and print `⌘Q` beside its label.
pub(crate) fn setup_app_menu(cx: &mut App, locale: &str) {
    cx.bind_keys(vec![KeyBinding::new("cmd-q", QuitApp, None)]);
    cx.set_menus(vec![app_menu(locale)]);
    cx.on_action(|_: &QuitApp, cx| {
        crate::product_shutdown::request_product_quit(cx);
    });
}

/// Refreshes the menu bar labels after a language change.
pub(crate) fn refresh_app_menu(cx: &App, locale: &str) {
    cx.set_menus(vec![app_menu(locale)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_english_menu_shows_bongocat_and_quit() {
        let menu = app_menu("en-US");
        assert_eq!(menu.name, "BongoCat");
        assert_eq!(menu.items.len(), 1);
        match &menu.items[0] {
            MenuItem::Action { name, action, .. } => {
                assert_eq!(name, "Quit BongoCat");
                assert_eq!(action.name(), "QuitApp");
            }
            _ => panic!("the app menu must hold exactly one action item"),
        }
    }

    #[test]
    fn the_chinese_menu_shows_bongocat_and_quit() {
        let menu = app_menu("zh-CN");
        assert_eq!(menu.name, "BongoCat");
        match &menu.items[0] {
            MenuItem::Action { name, .. } => assert_eq!(name, "退出 BongoCat"),
            _ => panic!("the app menu must hold exactly one action item"),
        }
    }

    #[test]
    fn the_quit_action_is_bound_to_command_q() {
        let binding = KeyBinding::new("cmd-q", QuitApp, None);
        assert_eq!(binding.keystrokes().len(), 1);
        assert_eq!(binding.action().name(), "QuitApp");
        #[cfg(target_os = "macos")]
        assert_eq!(binding.keystrokes()[0].to_string(), "⌘Q");
    }
}

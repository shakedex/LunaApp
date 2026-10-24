//! macOS: Luna's menu bar, laid out the native way. The app menu has About,
//! Settings… (⌘,), Services, Hide and Quit; the Window menu has Minimize,
//! Zoom and Full Screen. Slint's own default menu is turned off for this
//! (`ui::select_backend`): it has no place for Settings.

use std::cell::RefCell;

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

const SETTINGS: &str = "luna.settings";

thread_local! {
    /// What Settings… does, on the UI thread.
    static ON_SETTINGS: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Puts Luna's menu bar up. Keep the returned menu for as long as the app runs.
pub fn install(on_settings: impl Fn() + 'static) -> Menu {
    ON_SETTINGS.with(|f| *f.borrow_mut() = Some(Box::new(on_settings)));
    // muda calls back on the main thread, from AppKit; the handler must be
    // `Send`, so it goes through Slint's event loop to reach the window.
    MenuEvent::set_event_handler(Some(|event: MenuEvent| {
        if event.id().0 == SETTINGS {
            let _ = slint::invoke_from_event_loop(|| {
                ON_SETTINGS.with(|f| {
                    if let Some(f) = f.borrow().as_ref() {
                        f();
                    }
                });
            });
        }
    }));

    let settings = MenuItem::with_id(
        SETTINGS,
        "Settings…",
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)),
    );
    let app = Submenu::with_items(
        "Luna",
        true,
        &[
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ],
    )
    .expect("app menu");
    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(Some("Zoom")),
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::bring_all_to_front(None),
        ],
    )
    .expect("window menu");
    let menu = Menu::with_items(&[&app, &window]).expect("menu bar");
    menu.init_for_nsapp();
    window.set_as_windows_menu_for_nsapp();
    menu
}

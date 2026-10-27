//! Binds the Slint windows to the flow, the run backend and the settings store.
//!
//! All state transitions are decided in `flow` / `settings_edit`; this module
//! only forwards UI callbacks, opens native dialogs and copies view models
//! into window properties.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

use luna_core::run::RunRequest;
use luna_core::settings::Settings;
use luna_core::settings::{ReportTheme, ThumbnailColour};
use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::backdrop::{BackdropRenderer, Sky, place_moon};
use crate::backend::{RunBackend, RunControl, RunEvent};
use crate::flow::{Flow, FlowState, validate_source};
use crate::moon::{MoonFrame, MoonRenderer};
use crate::settings_edit::SettingsEditor;
use crate::settings_store::SettingsStore;
use crate::update_state::{Action, UpdateState};
use crate::view::{MainView, ViewPhase, main_view};

slint::include_modules!();

thread_local! {
    /// Where update events land on the UI thread (the library calls back on
    /// its own thread).
    static UPDATE_SINK: RefCell<Weak<App>> = const { RefCell::new(Weak::new()) };
}

/// Opens the main window and runs the event loop until it closes. With
/// `check_updates`, a packaged build also starts the update library. A
/// `preset` (from `Luna.exe report …`) starts that run as soon as the window
/// shows; an error in it opens the window idle, showing the error.
pub fn run(
    backend: Arc<dyn RunBackend>,
    store: Rc<dyn SettingsStore>,
    check_updates: bool,
    preset: Option<Result<RunRequest, String>>,
) -> Result<(), slint::PlatformError> {
    select_backend()?;

    let main = MainWindow::new()?;
    // Read when the window is created, at `show`.
    main.set_native_frame(cfg!(target_os = "macos"));
    let settings = store.load();
    let app = Rc::new(App {
        flow: Arc::new(Mutex::new(Flow::new())),
        control: RefCell::new(None),
        backend,
        editor: RefCell::new(SettingsEditor::new(settings.clone())),
        settings: RefCell::new(settings),
        store,
        dialog_open: Cell::new(false),
        moon: RefCell::new(MoonMotion::new()),
        header_fields: Rc::new(VecModel::default()),
        update: Cell::new(UpdateState::default()),
        updater: RefCell::new(None),
        preset: RefCell::new(None),
        main,
    });
    App::wire(&app);
    app.render();
    app.draw_moon();
    let moon_timer = slint::Timer::default();
    let weak = Rc::downgrade(&app);
    moon_timer.start(
        slint::TimerMode::Repeated,
        // 15 fps: the Moon turns once in 90 s, so this is smooth and cheap.
        std::time::Duration::from_millis(66),
        move || {
            if let Some(app) = weak.upgrade() {
                app.draw_moon();
            }
        },
    );
    #[cfg(target_os = "macos")]
    let _menu = crate::mac_menu::install({
        let weak = Rc::downgrade(&app);
        move || {
            if let Some(app) = weak.upgrade() {
                app.open_settings();
            }
        }
    });
    app.main.show()?;
    match preset {
        Some(Ok(request)) => app.start_preset(request),
        Some(Err(error)) => {
            app.flow.lock().expect("flow lock").pick_source(Err(error));
            app.render();
        }
        None => {}
    }
    if check_updates {
        app.start_updates();
    }
    slint::run_event_loop_until_quit()?;
    // Stop the update library before the window goes away.
    app.updater.borrow_mut().take();
    Ok(())
}

/// winit is required for folder drops. The software renderer is the default
/// because the OpenGL (femtovg) one painted a blank white window on the
/// Windows test machine; `SLINT_BACKEND` still overrides both.
fn select_backend() -> Result<(), slint::PlatformError> {
    let env_backend = std::env::var_os("SLINT_BACKEND").is_some();
    // macOS: Luna's own menu bar (`mac_menu`) replaces Slint's default one,
    // which can only be turned off on the backend itself.
    #[cfg(target_os = "macos")]
    if !env_backend {
        let backend = i_slint_backend_winit::Backend::builder()
            .with_renderer_name("software")
            .with_window_attributes_hook(frame)
            .with_default_menu_bar(false)
            .build()?;
        return slint::platform::set_platform(Box::new(backend))
            .map_err(|e| slint::PlatformError::Other(e.to_string()));
    }
    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(frame);
    let selector = if env_backend {
        selector
    } else {
        selector.renderer_name("software".into())
    };
    selector.select()
}

/// Luna draws its own title bar.
///
/// - Windows: the system still draws the shadow, the rounded corners and the
///   edge around the frameless window.
/// - macOS: the window keeps its system frame, with a transparent title bar
///   over Luna's own and no title text, so the traffic lights sit on Luna's bar.
///
/// Set as the window is created: it doesn't exist yet when `show` returns.
fn frame(
    attributes: slint::winit_030::winit::window::WindowAttributes,
) -> slint::winit_030::winit::window::WindowAttributes {
    #[cfg(windows)]
    {
        use slint::winit_030::winit::platform::windows::{
            CornerPreference, WindowAttributesExtWindows,
        };
        attributes
            .with_undecorated_shadow(true)
            .with_corner_preference(CornerPreference::Round)
            .with_border_color(Some(edge_colour(true)))
    }
    #[cfg(target_os = "macos")]
    {
        use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
        attributes
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    attributes
}

#[cfg(windows)]
fn edge_colour(focused: bool) -> slint::winit_030::winit::platform::windows::Color {
    use slint::winit_030::winit::platform::windows::Color;
    if focused {
        Color::from_rgb(0x79, 0xca, 0xea)
    } else {
        Color::from_rgb(0x2b, 0x33, 0x41)
    }
}

/// The window's edge: Luna blue while it has focus, quieter when it hasn't.
fn set_edge(main: &MainWindow, focused: bool) {
    #[cfg(windows)]
    {
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        let _ = main
            .window()
            .with_winit_window(|w| w.set_border_color(Some(edge_colour(focused))));
    }
    #[cfg(not(windows))]
    let _ = (main, focused);
}

struct App {
    main: MainWindow,
    /// Shared with the backend's event callback, which runs on a worker thread
    /// and hops back to the UI thread before touching it.
    flow: Arc<Mutex<Flow>>,
    control: RefCell<Option<Box<dyn RunControl>>>,
    backend: Arc<dyn RunBackend>,
    store: Rc<dyn SettingsStore>,
    /// The saved settings a run starts with.
    settings: RefCell<Settings>,
    editor: RefCell<SettingsEditor>,
    /// A native file dialog is open; ignore further requests until it closes.
    dialog_open: Cell<bool>,
    moon: RefCell<MoonMotion>,
    /// The custom fields in the main window's header panel. Kept, not
    /// replaced, so a value being typed keeps its field and focus.
    header_fields: Rc<VecModel<CustomField>>,
    update: Cell<UpdateState>,
    /// The update library, in packaged builds.
    updater: RefCell<Option<luna_update::Updater>>,
    /// The run handed over at launch; Retry on its source runs it again.
    preset: RefCell<Option<RunRequest>>,
}

/// The Moon's motion: it turns slowly, and its phase and dial ease toward the
/// run's progress rather than jumping clip by clip.
struct MoonMotion {
    renderer: MoonRenderer,
    sky: BackdropRenderer,
    started: std::time::Instant,
    lit: f32,
    dial: f32,
}

/// One turn of the Moon takes this long.
const MOON_TURN_SECONDS: f32 = 90.0;
/// The sky behind it turns this many degrees a second.
const SKY_DEGREES_PER_SECOND: f32 = 0.25;
/// A crescent, like the logo, while Luna waits.
const MOON_WAITING: f32 = 0.08;

impl MoonMotion {
    fn new() -> Self {
        Self {
            renderer: MoonRenderer::new(),
            sky: BackdropRenderer::new(),
            started: std::time::Instant::now(),
            lit: MOON_WAITING,
            dial: 0.0,
        }
    }

    /// Advances toward the target state and returns the frame to draw.
    fn step(&mut self, target_lit: f32, target_dial: f32) -> MoonFrame {
        self.lit += (target_lit - self.lit) * 0.15;
        self.dial += (target_dial - self.dial) * 0.22;
        let spin = self.started.elapsed().as_secs_f32() / MOON_TURN_SECONDS * std::f32::consts::TAU;
        MoonFrame {
            lit: self.lit,
            spin,
            progress: self.dial,
        }
    }
}

/// Wraps a callback so it only holds a weak reference to the app.
fn with_app<A: 'static>(app: &Rc<App>, f: impl Fn(&Rc<App>, A) + 'static) -> impl Fn(A) + 'static {
    let weak: Weak<App> = Rc::downgrade(app);
    move |args| {
        if let Some(app) = weak.upgrade() {
            f(&app, args);
        }
    }
}

impl App {
    fn wire(app: &Rc<App>) {
        let m = &app.main;
        let unit = |f: fn(&Rc<App>)| with_app(app, move |a, ()| f(a));
        m.on_pick_folder({
            let f = unit(App::pick_source);
            move || f(())
        });
        m.on_start({
            let f = unit(App::start);
            move || f(())
        });
        m.on_cancel({
            let f = unit(|a| a.cancel());
            move || f(())
        });
        m.on_back({
            let f = unit(|a| {
                a.flow.lock().expect("flow lock").back();
                a.render();
            });
            move || f(())
        });
        m.on_new_report({
            let f = unit(|a| {
                a.flow.lock().expect("flow lock").new_report();
                a.control.borrow_mut().take();
                a.preset.borrow_mut().take();
                a.main.set_open_error(SharedString::new());
                a.render();
            });
            move || f(())
        });
        m.on_open_pdf({
            let f = unit(|a| a.open_output(|o| o.pdf.clone()));
            move || f(())
        });
        m.on_open_html({
            let f = unit(|a| a.open_output(|o| o.html.clone()));
            move || f(())
        });
        m.on_open_csv({
            let f = unit(|a| a.open_output(|o| o.csv.clone()));
            move || f(())
        });
        m.on_show_folder({
            let f = unit(|a| a.open_output(|o| o.folder.clone()));
            move || f(())
        });
        m.on_open_settings({
            let f = unit(App::open_settings);
            move || f(())
        });
        app.wire_settings();
        app.wire_window();

        // Closing the main window quits.
        m.window().on_close_requested(|| {
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        });

        // Folders dragged onto the window.
        let on_drop = with_app(app, |a, event: WindowEvent| a.window_event(&event));
        let weak = m.as_weak();
        m.window().on_winit_window_event(move |_, event| {
            // Recolour after this event is handled, not from inside it.
            if let WindowEvent::Focused(focused) = *event {
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(main) = weak.upgrade() {
                        set_edge(&main, focused);
                    }
                });
            }
            if matches!(
                event,
                WindowEvent::HoveredFile(_)
                    | WindowEvent::HoveredFileCancelled
                    | WindowEvent::DroppedFile(_)
            ) {
                on_drop(event.clone());
            }
            EventResult::Propagate
        });
    }

    /// Draws the next Moon frame, unless the window can't be seen.
    fn draw_moon(&self) {
        let window = self.main.window();
        // Maximised by snapping or a shortcut, not only by our button.
        if self.main.get_is_maximized() != window.is_maximized() {
            self.main.set_is_maximized(window.is_maximized());
        }
        // macOS: full screen from the green button or the Window menu.
        if self.main.get_is_fullscreen() != window.is_fullscreen() {
            self.main.set_is_fullscreen(window.is_fullscreen());
        }
        if !window.is_visible() || window.is_minimized() {
            return;
        }
        let progress = if self.main.get_indeterminate() {
            0.0
        } else {
            self.main.get_progress()
        };
        let (lit, dial) = match self.main.get_phase() {
            Phase::Running => (MOON_WAITING + (1.0 - MOON_WAITING) * progress, progress),
            Phase::Done => (1.0, 1.0),
            _ => (MOON_WAITING, 0.0),
        };
        let scale = window.scale_factor();
        let size = (self.main.get_moon_box() * scale).round() as u32;
        let supersample = if scale >= 1.5 { 1 } else { 2 };
        let mut moon = self.moon.borrow_mut();
        let frame = moon.step(lit, dial);
        // Settings covers the sky and the Moon; don't draw what can't be seen.
        if self.main.get_settings_open() {
            return;
        }
        let disc = moon.renderer.render(size, supersample, frame);
        let window_size = window.size();
        let centre = [
            self.main.get_moon_x() * scale,
            self.main.get_moon_y() * scale,
        ];
        let sky = Sky {
            width: window_size.width,
            height: window_size.height,
            centre,
            dial: self.main.get_moon_box() * 0.42 * scale,
            scale,
        };
        let turn = moon.started.elapsed().as_secs_f32() * SKY_DEGREES_PER_SECOND;
        let mut pixels = moon.sky.render(sky, turn);
        place_moon(&mut pixels, &disc, centre);
        self.main
            .set_backdrop_image(slint::Image::from_rgb8(pixels));
    }

    fn render(&self) {
        let view = main_view(&self.flow.lock().expect("flow lock"));
        apply_view(&self.main, &view);
        let settings = self.settings.borrow();
        self.main
            .set_header_production(settings.production.as_str().into());
        sync_fields(&self.header_fields, &settings.custom);
    }

    /// A value typed on the folder screen is saved as it's typed.
    fn edit_header(&self, edit: impl FnOnce(&mut Settings)) {
        let mut settings = self.settings.borrow().clone();
        edit(&mut settings);
        match self.store.save(&settings) {
            Ok(()) => *self.settings.borrow_mut() = settings,
            Err(e) => tracing::error!("Couldn't save the report header: {e}"),
        }
        self.render();
    }

    /// The report theme switch saves at once; it has no draft to discard.
    fn set_report_dark(&self, dark: bool) {
        let mut settings = self.settings.borrow().clone();
        settings.report_theme = if dark {
            ReportTheme::Dark
        } else {
            ReportTheme::Light
        };
        match self.store.save(&settings) {
            Ok(()) => *self.settings.borrow_mut() = settings,
            Err(e) => tracing::error!("Couldn't save the report theme: {e}"),
        }
        self.settings_state()
            .set_report_dark(self.settings.borrow().report_theme == ReportTheme::Dark);
    }

    /// The source-drive copy saves at once, like the report theme.
    fn set_copy_to_source(&self, on: bool) {
        let mut settings = self.settings.borrow().clone();
        settings.copy_to_source = on;
        match self.store.save(&settings) {
            Ok(()) => *self.settings.borrow_mut() = settings,
            Err(e) => tracing::error!("Couldn't save the source-drive copy: {e}"),
        }
        self.settings_state()
            .set_copy_to_source(self.settings.borrow().copy_to_source);
    }

    /// The thumbnail colour saves at once, like the report theme.
    fn set_colour(&self, index: i32) {
        let colour = match index {
            1 => ThumbnailColour::AsRecorded,
            2 => ThumbnailColour::Lut,
            _ => ThumbnailColour::Rec709,
        };
        self.save_now("thumbnail colour", |s| s.colour = colour);
    }

    fn set_camera_look(&self, on: bool) {
        self.save_now("camera look", |s| s.camera_look = on);
    }

    fn pick_lut(self: &Rc<Self>) {
        let dialog = rfd::AsyncFileDialog::new().add_filter("LUT (.cube)", &["cube"]);
        self.run_dialog(self.main.window(), dialog, true, |app, path| {
            let mut candidate = app.settings.borrow().clone();
            match candidate.set_lut(&path) {
                Ok(()) => {
                    app.settings_state().set_lut_error(SharedString::new());
                    app.save_now("LUT", |s| {
                        s.lut = candidate.lut.clone();
                        s.recent_luts = candidate.recent_luts.clone();
                    });
                }
                Err(e) => app.settings_state().set_lut_error(e.into()),
            }
        });
    }

    /// Saves one setting at once and shows what was stored.
    fn save_now(&self, what: &str, change: impl FnOnce(&mut Settings)) {
        let mut settings = self.settings.borrow().clone();
        change(&mut settings);
        match self.store.save(&settings) {
            Ok(()) => *self.settings.borrow_mut() = settings,
            Err(e) => tracing::error!("Couldn't save the {what}: {e}"),
        }
        self.fill_colour();
    }

    fn fill_colour(&self) {
        let s = self.settings_state();
        let settings = self.settings.borrow();
        s.set_colour(match settings.colour {
            ThumbnailColour::Rec709 => 0,
            ThumbnailColour::AsRecorded => 1,
            ThumbnailColour::Lut => 2,
        });
        s.set_lut_name(
            settings
                .lut
                .as_deref()
                .map(file_name)
                .unwrap_or_default()
                .into(),
        );
        s.set_camera_look(settings.camera_look);
        let names: Vec<SharedString> = settings
            .recent_luts
            .iter()
            .map(|p| file_name(p).into())
            .collect();
        s.set_recent_luts(ModelRc::new(VecModel::from(names)));
        let current = settings
            .lut
            .as_ref()
            .and_then(|l| settings.recent_luts.iter().position(|p| p == l));
        s.set_recent_current(current.map_or(-1, |i| i as i32));
    }

    fn pick_recent_lut(&self, index: i32) {
        let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.borrow().recent_luts.get(i).cloned())
        else {
            return;
        };
        let mut candidate = self.settings.borrow().clone();
        match candidate.set_lut(&path) {
            Ok(()) => {
                self.settings_state().set_lut_error(SharedString::new());
                self.save_now("LUT", |s| {
                    s.lut = candidate.lut.clone();
                    s.recent_luts = candidate.recent_luts.clone();
                });
            }
            Err(e) => self.settings_state().set_lut_error(e.into()),
        }
    }

    /// Problems in reports save at once, like the source-drive copy.
    fn set_show_problems(&self, on: bool) {
        let mut settings = self.settings.borrow().clone();
        settings.show_problems = on;
        match self.store.save(&settings) {
            Ok(()) => *self.settings.borrow_mut() = settings,
            Err(e) => tracing::error!("Couldn't save the problems setting: {e}"),
        }
        self.settings_state()
            .set_show_problems(self.settings.borrow().show_problems);
    }

    fn window_event(&self, event: &WindowEvent) {
        let accepts = self.flow.lock().expect("flow lock").accepts_source();
        match event {
            WindowEvent::HoveredFile(_) => self.main.set_drag_hover(accepts),
            WindowEvent::HoveredFileCancelled => self.main.set_drag_hover(false),
            WindowEvent::DroppedFile(path) => {
                self.main.set_drag_hover(false);
                if accepts {
                    self.flow
                        .lock()
                        .expect("flow lock")
                        .pick_source(validate_source(path));
                    self.render();
                }
            }
            _ => {}
        }
    }

    fn pick_source(self: &Rc<Self>) {
        self.pick_folder(self.main.window(), None, |app, path| {
            app.flow
                .lock()
                .expect("flow lock")
                .pick_source(validate_source(&path));
            app.render();
        });
    }

    fn start(self: &Rc<Self>) {
        let Some((run, source)) = self.flow.lock().expect("flow lock").start() else {
            return;
        };
        let preset = self
            .preset
            .borrow()
            .clone()
            .filter(|p| p.sources.first() == Some(&source));
        let request =
            preset.unwrap_or_else(|| RunRequest::folder(source, self.settings.borrow().clone()));
        self.launch(run, request);
    }

    fn start_preset(self: &Rc<Self>, request: RunRequest) {
        let Some(source) = request.sources.first().cloned() else {
            return;
        };
        let Some(run) = self.flow.lock().expect("flow lock").start_preset(source) else {
            return;
        };
        *self.preset.borrow_mut() = Some(request.clone());
        self.launch(run, request);
    }

    fn launch(self: &Rc<Self>, run: u64, request: RunRequest) {
        self.render();
        let flow = self.flow.clone();
        let weak = self.main.as_weak();
        let events = Box::new(move |event: RunEvent| {
            let flow = flow.clone();
            let _ = weak.upgrade_in_event_loop(move |main| {
                let mut flow = flow.lock().expect("flow lock");
                flow.apply(run, event);
                apply_view(&main, &main_view(&flow));
            });
        });
        let control = self.backend.start(request, events);
        *self.control.borrow_mut() = Some(control);
    }

    fn start_updates(self: &Rc<Self>) {
        UPDATE_SINK.with(|s| *s.borrow_mut() = Rc::downgrade(self));
        let updater = luna_update::start(env!("CARGO_PKG_VERSION"), |event| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = UPDATE_SINK.with(|s| s.borrow().upgrade()) {
                    app.on_update(event);
                }
            });
        });
        if updater.is_none() {
            let why = luna_update::unavailable_reason().unwrap_or_default();
            tracing::error!("Updates are off: {why}");
        }
        self.settings_state()
            .set_can_check_updates(updater.is_some());
        *self.updater.borrow_mut() = updater;
    }

    fn on_update(&self, event: luna_update::Event) {
        let running = matches!(
            self.flow.lock().expect("flow lock").state(),
            FlowState::Running { .. }
        );
        let mut state = self.update.get();
        let action = state.apply(event, running);
        self.update.set(state);
        self.main.set_update_pending(state.pending);
        match action {
            Action::None => {}
            Action::Quit => {
                let _ = slint::quit_event_loop();
            }
            Action::CancelAndQuit => {
                self.cancel();
                let _ = slint::quit_event_loop();
            }
        }
    }

    /// The library's own window: the pending update, or "up to date".
    fn check_for_updates(&self) {
        if let Some(updater) = self.updater.borrow().as_ref() {
            updater.check_now();
        }
    }

    /// Opens the logs folder in the file manager.
    fn show_logs(&self) {
        let opened = ensure_dir(luna_log::logs_dir())
            .and_then(|dir| open::that_detached(dir).map_err(|e| e.to_string()));
        if let Err(e) = opened {
            let error = format!("Couldn't open the logs folder: {e}");
            self.settings_state().set_save_error(error.into());
        }
    }

    fn cancel(&self) {
        if self.flow.lock().expect("flow lock").request_cancel() {
            if let Some(control) = self.control.borrow().as_ref() {
                control.cancel();
            }
            self.render();
        }
    }

    fn open_output(&self, which: impl Fn(&crate::backend::RunOutcome) -> PathBuf) {
        let path = match self.flow.lock().expect("flow lock").state() {
            FlowState::Done { outcome, .. } => which(outcome),
            _ => return,
        };
        let error = if !path.exists() {
            format!(
                "{} isn't there any more. It may have been moved or deleted.",
                file_name(&path)
            )
        } else {
            match open::that_detached(&path) {
                Ok(()) => String::new(),
                Err(e) => format!("Couldn't open {}: {e}", file_name(&path)),
            }
        };
        self.main.set_open_error(error.into());
    }

    /// Shows a native folder dialog parented to `parent` and hands the choice
    /// to `done` on the UI thread.
    fn pick_folder(
        self: &Rc<Self>,
        parent: &slint::Window,
        start_in: Option<PathBuf>,
        done: impl FnOnce(&Rc<App>, PathBuf) + 'static,
    ) {
        let mut dialog = rfd::AsyncFileDialog::new();
        if let Some(dir) = start_in.filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        self.run_dialog(parent, dialog, false, done);
    }

    fn run_dialog(
        self: &Rc<Self>,
        parent: &slint::Window,
        dialog: rfd::AsyncFileDialog,
        file: bool,
        done: impl FnOnce(&Rc<App>, PathBuf) + 'static,
    ) {
        if self.dialog_open.replace(true) {
            return;
        }
        let dialog = parent
            .with_winit_window(|w| dialog.clone().set_parent(w))
            .unwrap_or(dialog);
        let weak = Rc::downgrade(self);
        let spawned = slint::spawn_local(async move {
            let picked = if file {
                dialog.pick_file().await
            } else {
                dialog.pick_folder().await
            };
            if let Some(app) = weak.upgrade() {
                app.dialog_open.set(false);
                if let Some(handle) = picked {
                    done(&app, handle.path().to_path_buf());
                }
            }
        });
        if spawned.is_err() {
            self.dialog_open.set(false);
        }
    }

    // ----- Settings -----

    fn settings_state(&self) -> SettingsState<'_> {
        self.main.global::<SettingsState>()
    }

    fn open_settings(self: &Rc<Self>) {
        if !self.main.get_settings_open() {
            *self.editor.borrow_mut() = SettingsEditor::new(self.settings.borrow().clone());
            let s = self.settings_state();
            s.set_logo_error(SharedString::new());
            s.set_save_error(SharedString::new());
            s.set_show_credits(false);
            self.fill_settings();
        }
        self.main.set_settings_open(true);
    }

    fn close_settings(&self) {
        self.editor.borrow_mut().revert();
        self.main.set_settings_open(false);
    }

    fn wire_settings(self: &Rc<Self>) {
        let s = self.settings_state();
        let unit = |f: fn(&Rc<App>)| with_app(self, move |app, ()| f(app));
        s.set_app_version(env!("CARGO_PKG_VERSION").into());
        s.on_check_for_updates({
            let f = unit(|app| app.check_for_updates());
            move || f(())
        });
        s.on_show_logs({
            let f = unit(|app| app.show_logs());
            move || f(())
        });
        s.on_text_edited({
            let f = unit(|app| {
                {
                    let s = app.settings_state();
                    let mut ed = app.editor.borrow_mut();
                    let d = ed.draft_mut();
                    d.production = s.get_production().into();
                    d.company = s.get_company().into();
                    d.author = s.get_author().into();
                }
                app.refresh_dirty();
            });
            move || f(())
        });
        s.on_set_report_dark({
            let f = with_app(self, |app, dark: bool| app.set_report_dark(dark));
            move |dark| f(dark)
        });
        s.on_set_copy_to_source({
            let f = with_app(self, |app, on: bool| app.set_copy_to_source(on));
            move |on| f(on)
        });
        s.on_set_show_problems({
            let f = with_app(self, |app, on: bool| app.set_show_problems(on));
            move |on| f(on)
        });
        s.on_set_colour({
            let f = with_app(self, |app, i: i32| app.set_colour(i));
            move |i| f(i)
        });
        s.on_set_camera_look({
            let f = with_app(self, |app, on: bool| app.set_camera_look(on));
            move |on| f(on)
        });
        s.on_pick_lut({
            let f = unit(App::pick_lut);
            move || f(())
        });
        s.on_pick_recent_lut({
            let f = with_app(self, |app, i: i32| app.pick_recent_lut(i));
            move |i| f(i)
        });
        s.on_label_edited({
            let f = with_app(self, |app, (i, t): (i32, SharedString)| {
                app.editor.borrow_mut().set_label(i as usize, &t);
                app.refresh_dirty();
            });
            move |i, t| f((i, t))
        });
        s.on_value_edited({
            let f = with_app(self, |app, (i, t): (i32, SharedString)| {
                app.editor.borrow_mut().set_value(i as usize, &t);
                app.refresh_dirty();
            });
            move |i, t| f((i, t))
        });
        s.on_add_field({
            let f = unit(|app| {
                app.editor.borrow_mut().add_field();
                app.fill_fields();
            });
            move || f(())
        });
        let row_op = |op: fn(&mut SettingsEditor, usize)| {
            with_app(self, move |app, i: i32| {
                op(&mut app.editor.borrow_mut(), i as usize);
                app.fill_fields();
            })
        };
        s.on_remove_field(row_op(SettingsEditor::remove_field));
        s.on_move_up(row_op(SettingsEditor::move_up));
        s.on_move_down(row_op(SettingsEditor::move_down));
        s.on_pick_logo({
            let f = unit(App::pick_logo);
            move || f(())
        });
        s.on_remove_logo({
            let f = unit(|app| {
                app.store.clear_logo(app.editor.borrow_mut().draft_mut());
                app.settings_state().set_logo_error(SharedString::new());
                app.fill_logo();
                app.refresh_dirty();
            });
            move || f(())
        });
        s.on_pick_output({
            let f = unit(|app| {
                let start = Some(app.editor.borrow().draft().output_dir.clone());
                app.pick_folder(app.main.window(), start, |app, path| {
                    app.editor.borrow_mut().draft_mut().output_dir = path;
                    app.settings_state()
                        .set_output_dir(output_text(app.editor.borrow().draft()).into());
                    app.refresh_dirty();
                });
            });
            move || f(())
        });
        s.on_save({
            let f = unit(|a| a.save_settings());
            move || f(())
        });
        s.on_cancel({
            let f = unit(|a| a.close_settings());
            move || f(())
        });
    }

    fn fill_settings(&self) {
        let s = self.settings_state();
        {
            let ed = self.editor.borrow();
            let d = ed.draft();
            s.set_production(d.production.clone().into());
            s.set_company(d.company.clone().into());
            s.set_author(d.author.clone().into());
            s.set_output_dir(output_text(d).into());
        }
        s.set_report_dark(self.settings.borrow().report_theme == ReportTheme::Dark);
        s.set_copy_to_source(self.settings.borrow().copy_to_source);
        s.set_show_problems(self.settings.borrow().show_problems);
        s.set_lut_error(SharedString::new());
        self.fill_colour();
        self.fill_logo();
        self.fill_fields();
        self.refresh_dirty();
    }

    fn fill_fields(&self) {
        // A fresh model each time: rows are re-created, so reordered text
        // can't be left behind in a recycled field.
        let rows: Vec<CustomField> = self
            .editor
            .borrow()
            .draft()
            .custom
            .iter()
            .map(|(l, v)| CustomField {
                label: l.into(),
                value: v.into(),
            })
            .collect();
        self.settings_state()
            .set_fields(ModelRc::new(VecModel::from(rows)));
        self.refresh_dirty();
    }

    fn fill_logo(&self) {
        let s = self.settings_state();
        let logo = self.editor.borrow().draft().logo.clone();
        match logo
            .as_deref()
            .and_then(|p| Some((p, slint::Image::load_from_path(p).ok()?)))
        {
            Some((path, image)) => {
                s.set_logo(image);
                s.set_logo_name(file_name(path).into());
                s.set_has_logo(true);
            }
            None => {
                s.set_logo(slint::Image::default());
                s.set_logo_name(SharedString::new());
                s.set_has_logo(false);
            }
        }
    }

    fn refresh_dirty(&self) {
        let s = self.settings_state();
        s.set_can_save(self.editor.borrow().is_dirty());
        s.set_save_error(SharedString::new());
    }

    fn pick_logo(self: &Rc<Self>) {
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter("Logo (PNG, JPEG, SVG)", &["png", "jpg", "jpeg", "svg"]);
        self.run_dialog(self.main.window(), dialog, true, |app, path| {
            let mut candidate = app.editor.borrow().draft().clone();
            let result = app.store.set_logo(&mut candidate, &path).and_then(|()| {
                let shown = candidate.logo.as_deref().unwrap_or(&path);
                slint::Image::load_from_path(shown)
                    .map(|_| ())
                    .map_err(|_| {
                        "Luna couldn't read that image. Use a PNG, JPEG or SVG file.".to_string()
                    })
            });
            let s = app.settings_state();
            match result {
                Ok(()) => {
                    *app.editor.borrow_mut().draft_mut() = candidate;
                    s.set_logo_error(SharedString::new());
                    app.fill_logo();
                    app.refresh_dirty();
                }
                Err(e) => s.set_logo_error(format!("{}: {e}", file_name(&path)).into()),
            }
        });
    }

    fn save_settings(&self) {
        let validated = self.editor.borrow().validated_over(&self.settings.borrow());
        let result = validated.and_then(|s| self.store.save(&s).map(|()| s));
        match result {
            Ok(saved) => {
                self.editor.borrow_mut().mark_saved(saved.clone());
                *self.settings.borrow_mut() = saved;
                self.render();
                self.main.set_settings_open(false);
            }
            Err(e) => self.settings_state().set_save_error(e.into()),
        }
    }

    // ----- Window -----

    fn wire_window(self: &Rc<Self>) {
        let m = &self.main;
        let unit = |f: fn(&Rc<App>)| with_app(self, move |app, ()| f(app));
        m.on_drag_window({
            let f = unit(|app| {
                let _ = app.main.window().with_winit_window(|w| w.drag_window());
            });
            move || f(())
        });
        m.on_toggle_maximized({
            let f = unit(|app| {
                let w = app.main.window();
                let maximize = !w.is_maximized();
                w.set_maximized(maximize);
                app.main.set_is_maximized(maximize);
            });
            move || f(())
        });
        m.on_minimize({
            let f = unit(|app| app.main.window().set_minimized(true));
            move || f(())
        });
        m.on_close_window(|| {
            let _ = slint::quit_event_loop();
        });
        m.on_show_update({
            let f = unit(|app| app.check_for_updates());
            move || f(())
        });
        m.on_resize_window({
            let f = with_app(self, |app, dir: i32| {
                use slint::winit_030::winit::window::ResizeDirection as D;
                let direction = match dir {
                    0 => D::North,
                    1 => D::South,
                    2 => D::East,
                    3 => D::West,
                    4 => D::NorthEast,
                    5 => D::NorthWest,
                    6 => D::SouthEast,
                    _ => D::SouthWest,
                };
                let _ = app
                    .main
                    .window()
                    .with_winit_window(|w| w.drag_resize_window(direction));
            });
            move |dir| f(dir)
        });
        m.set_header_fields(ModelRc::from(self.header_fields.clone()));
        m.on_header_production_edited({
            let f = with_app(self, |app, t: SharedString| {
                app.edit_header(|s| s.production = t.into());
            });
            move |t| f(t)
        });
        m.on_header_field_edited({
            let f = with_app(self, |app, (i, t): (i32, SharedString)| {
                app.edit_header(|s| {
                    if let Some(field) = s.custom.get_mut(i as usize) {
                        field.1 = t.into();
                    }
                });
            });
            move |i, t| f((i, t))
        });
        self.settings_state()
            .set_credits(ModelRc::new(VecModel::from(credits())));
    }
}

/// Brings `model` in line with `custom`, touching only the rows that differ.
fn sync_fields(model: &VecModel<CustomField>, custom: &[(String, String)]) {
    use slint::Model;
    if model.row_count() != custom.len() {
        model.set_vec(
            custom
                .iter()
                .map(|(l, v)| CustomField {
                    label: l.into(),
                    value: v.into(),
                })
                .collect::<Vec<_>>(),
        );
        return;
    }
    for (i, (l, v)) in custom.iter().enumerate() {
        let row = model.row_data(i).unwrap_or_default();
        if row.label != l.as_str() || row.value != v.as_str() {
            model.set_row_data(
                i,
                CustomField {
                    label: l.into(),
                    value: v.into(),
                },
            );
        }
    }
}

fn credits() -> Vec<Component> {
    [
        ("Slint", "Royalty-free 2.0", "User interface toolkit"),
        ("Typst", "Apache-2.0", "Typesetting for the PDF report"),
        (
            "askama",
            "MIT or Apache-2.0",
            "Templates for the HTML report",
        ),
        ("rfd", "MIT", "Native file and folder dialogs"),
        ("open", "MIT", "Opens reports in their default app"),
        (
            "IBM Plex Sans",
            "OFL-1.1",
            "Typeface of the app and the report",
        ),
        ("Phosphor Icons", "MIT", "Icons"),
        (
            "NASA CGI Moon Kit",
            "NASA media",
            "Lunar surface map, by NASA's Scientific Visualization Studio",
        ),
    ]
    .into_iter()
    .map(|(name, licence, role)| Component {
        name: name.into(),
        licence: licence.into(),
        role: role.into(),
    })
    .collect()
}

fn apply_view(main: &MainWindow, v: &MainView) {
    main.set_phase(match v.phase {
        ViewPhase::Idle => Phase::Idle,
        ViewPhase::Picked => Phase::Picked,
        ViewPhase::Running => Phase::Running,
        ViewPhase::Done => Phase::Done,
        ViewPhase::Failed => Phase::Failed,
        ViewPhase::Cancelled => Phase::Cancelled,
    });
    main.set_source(v.source.as_str().into());
    main.set_source_name(v.source_name.as_str().into());
    main.set_source_error(v.source_error.as_str().into());
    main.set_files_found(v.files_found.as_str().into());
    main.set_clips_done(v.clips_done.as_str().into());
    main.set_clips_total(v.clips_total.as_str().into());
    main.set_problems(v.problems.as_str().into());
    main.set_has_problems(v.has_problems);
    main.set_progress(v.progress);
    main.set_indeterminate(v.indeterminate);
    main.set_cancelling(v.cancelling);
    main.set_summary(v.summary.as_str().into());
    main.set_pdf_name(v.pdf_name.as_str().into());
    main.set_html_name(v.html_name.as_str().into());
    main.set_csv_name(v.csv_name.as_str().into());
    main.set_folder(v.folder.as_str().into());
    main.set_failure(v.failure.as_str().into());
    main.set_copy_error(v.copy_error.as_str().into());
}

fn output_text(s: &Settings) -> String {
    s.output_dir.display().to_string()
}

/// Creates the logs folder if it's missing so the file manager has
/// something to open.
fn ensure_dir(dir: Option<PathBuf>) -> Result<PathBuf, String> {
    let dir = dir.ok_or("Luna has no local data folder on this system")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_logs_creates_the_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("not").join("yet").join("there");
        let got = ensure_dir(Some(dir.clone())).unwrap();
        assert_eq!(got, dir);
        assert!(dir.is_dir());
    }

    #[test]
    fn show_logs_without_a_data_folder_is_an_error() {
        assert_eq!(
            ensure_dir(None).unwrap_err(),
            "Luna has no local data folder on this system"
        );
    }
}

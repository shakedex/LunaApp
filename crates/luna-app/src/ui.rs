//! Binds the Slint windows to the flow, the run backend and the settings store.
//!
//! All state transitions are decided in `flow` / `settings_edit`; this module
//! only forwards UI callbacks, opens native dialogs and copies view models
//! into window properties.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

use luna_core::settings::ReportTheme;
use luna_core::settings::Settings;
use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::backend::{RunBackend, RunControl, RunEvent};
use crate::flow::{Flow, FlowState, validate_source};
use crate::moon::{MoonFrame, MoonRenderer};
use crate::settings_edit::SettingsEditor;
use crate::settings_store::SettingsStore;
use crate::view::{MainView, ViewPhase, main_view};

slint::include_modules!();

/// Opens the main window and runs the event loop until it closes.
pub fn run(
    backend: Arc<dyn RunBackend>,
    store: Rc<dyn SettingsStore>,
) -> Result<(), slint::PlatformError> {
    // winit is required for folder drops. The software renderer is the
    // default because the OpenGL (femtovg) one painted a blank white window on
    // the Windows test machine; `SLINT_BACKEND` still overrides both.
    let selector = slint::BackendSelector::new().backend_name("winit".into());
    let selector = if std::env::var_os("SLINT_BACKEND").is_none() {
        selector.renderer_name("software".into())
    } else {
        selector
    };
    selector.select()?;

    let main = MainWindow::new()?;
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
    app.main.show()?;
    // Luna draws its own title bar; keep the system's shadow and rounded
    // corners around the frameless window.
    #[cfg(windows)]
    {
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        let _ = app
            .main
            .window()
            .with_winit_window(|w| w.set_undecorated_shadow(true));
    }
    slint::run_event_loop_until_quit()?;
    Ok(())
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
}

/// The Moon's motion: it turns slowly, and its phase and dial ease toward the
/// run's progress rather than jumping clip by clip.
struct MoonMotion {
    renderer: MoonRenderer,
    started: std::time::Instant,
    lit: f32,
    dial: f32,
}

/// One turn of the Moon takes this long.
const MOON_TURN_SECONDS: f32 = 90.0;
/// A crescent, like the logo, while Luna waits.
const MOON_WAITING: f32 = 0.08;

impl MoonMotion {
    fn new() -> Self {
        Self {
            renderer: MoonRenderer::new(),
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
        m.window().on_winit_window_event(move |_, event| {
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
        let pixels = moon.renderer.render(size, supersample, frame);
        self.main.set_moon_image(slint::Image::from_rgb8(pixels));
    }

    fn render(&self) {
        let view = main_view(&self.flow.lock().expect("flow lock"));
        apply_view(&self.main, &view);
        let settings = self.settings.borrow();
        self.main
            .set_output_dir(settings.output_dir.display().to_string().into());
        self.main
            .set_header_production(settings.production.as_str().into());
        self.main
            .set_header_company(settings.company.as_str().into());
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
            Err(e) => crate::log::error(&format!("Couldn't save the report theme: {e}")),
        }
        self.settings_state()
            .set_report_dark(self.settings.borrow().report_theme == ReportTheme::Dark);
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
        let control = self
            .backend
            .start(source, self.settings.borrow().clone(), events);
        *self.control.borrow_mut() = Some(control);
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
        // The theme is saved the moment it's switched, not with this draft.
        let theme = self.settings.borrow().report_theme;
        let validated = self.editor.borrow().validated().map(|mut s| {
            s.report_theme = theme;
            s
        });
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

    /// The bottom bar's destination: choosing a folder saves it at once.
    fn change_output(self: &Rc<Self>) {
        let start = Some(self.settings.borrow().output_dir.clone());
        self.pick_folder(self.main.window(), start, |app, path| {
            let mut settings = app.settings.borrow().clone();
            settings.output_dir = path;
            match app.store.save(&settings) {
                Ok(()) => *app.settings.borrow_mut() = settings,
                Err(e) => crate::log::error(&format!("Couldn't save the destination: {e}")),
            }
            app.render();
        });
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
        m.on_change_output({
            let f = unit(App::change_output);
            move || f(())
        });
        self.settings_state()
            .set_credits(ModelRc::new(VecModel::from(credits())));
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
}

fn output_text(s: &Settings) -> String {
    s.output_dir.display().to_string()
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

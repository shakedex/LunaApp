//! Renders each screen of the app to an image without opening a window, for
//! design review: `cargo run -p luna-app --example snapshot -- <output dir>`.
//! Writes one binary PPM per screen.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use luna_app::backdrop::{BackdropRenderer, Sky, place_moon};
use luna_app::moon::{MoonFrame, MoonRenderer};
use luna_app::ui::{Component, CustomField, MainWindow, Phase, SettingsState};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, PhysicalSize, PlatformError, VecModel};

/// Hands out a fresh software window for each component; the last one made is
/// the one drawn.
struct Headless {
    last: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
}

impl Platform for &'static Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        *self.last.borrow_mut() = Some(w.clone());
        Ok(w)
    }
}

impl Headless {
    fn last_window(&self) -> Rc<MinimalSoftwareWindow> {
        self.last.borrow().clone().expect("a component was shown")
    }
}

/// Sets up one screen of the main window.
type Screen = fn(&MainWindow);

fn snap(window: &Rc<MinimalSoftwareWindow>, size: (u32, u32), path: &Path) {
    window.set_size(PhysicalSize::new(size.0, size.1));
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let (w, h) = (size.0 as usize, size.1 as usize);
    let mut buf = vec![PremultipliedRgbaColor::default(); w * h];
    window.draw_if_needed(|r| {
        r.render(&mut buf, w);
    });
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in &buf {
        out.extend([p.red, p.green, p.blue]);
    }
    std::fs::write(path, out).expect("write snapshot");
    eprintln!("wrote {}", path.display());
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let platform: &'static Headless = Box::leak(Box::new(Headless {
        last: RefCell::new(None),
    }));
    slint::platform::set_platform(Box::new(platform)).unwrap();

    let size = (880, 640);
    let mut moon = MoonRenderer::new();
    let mut sky = BackdropRenderer::new();
    let screens: [(&str, Screen); 9] = [
        ("1-idle", |_| {}),
        ("1d-update-pending", |m| m.set_update_pending(true)),
        ("1c-header-open", |m| {
            m.set_header_open(true);
        }),
        ("1b-drop-hover", |m| {
            m.set_drag_hover(true);
        }),
        ("2-picked", |m| {
            m.set_phase(Phase::Picked);
        }),
        ("3-running", |m| {
            m.set_phase(Phase::Running);
            m.set_files_found("214".into());
            m.set_clips_done("17".into());
            m.set_clips_total("25".into());
            m.set_problems("2".into());
            m.set_has_problems(true);
            m.set_progress(0.68);
        }),
        ("4-done", |m| {
            m.set_phase(Phase::Done);
            m.set_summary("6 of 25 clips need attention".into());
            m.set_has_problems(true);
        }),
        ("5-failed", |m| {
            m.set_phase(Phase::Failed);
            m.set_failure(
                "The output folder D:\\Reports is read-only. Choose another folder in Settings."
                    .into(),
            );
        }),
        ("6-cancelled", |m| {
            m.set_phase(Phase::Cancelled);
        }),
    ];
    for (name, setup) in screens {
        let m = MainWindow::new().unwrap();
        m.set_output_dir("C:\\Users\\dana\\Documents\\Luna Reports".into());
        m.set_source("D:\\SHOOT_DAY_12\\CAMERA".into());
        m.set_source_name("SHOOT_DAY_12".into());
        m.set_pdf_name("Luna Report 2026-10-16 1334.pdf".into());
        m.set_html_name("Luna Report 2026-10-16 1334.html".into());
        m.set_csv_name("Luna Report 2026-10-16 1334.csv".into());
        m.set_folder(
            "C:\\Users\\dana\\Documents\\Luna Reports\\Luna Report 2026-10-16 1334".into(),
        );
        m.set_header_production("Night Shift".into());
        m.set_header_company("Lumen Rentals".into());
        m.set_header_summary("Night Shift · Shoot day 12 · Unit Main".into());
        m.set_header_fields(ModelRc::new(VecModel::from(vec![
            CustomField {
                label: "Shoot day".into(),
                value: "12".into(),
            },
            CustomField {
                label: "Unit".into(),
                value: "Main".into(),
            },
        ])));
        setup(&m);
        let spin = 0.0;
        m.show().unwrap();
        platform
            .last_window()
            .set_size(PhysicalSize::new(size.0, size.1));
        draw(&m, &mut moon, &mut sky, size, spin);
        snap(
            &platform.last_window(),
            size,
            &dir.join(format!("{name}.ppm")),
        );
        m.hide().unwrap();
    }

    // A run from start to finish, the Moon turning as it goes. LUNA_MOTION
    // sets the frame count; the frames make a video of the real window.
    let frames: usize = std::env::var("LUNA_MOTION")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let idle = std::env::var_os("LUNA_MOTION_IDLE").is_some();
    for i in 0..frames {
        let t = i as f32 / (frames.max(2) - 1) as f32;
        let m = MainWindow::new().unwrap();
        m.set_output_dir(r"C:\Users\dana\Documents\Luna Reports".into());
        m.set_source(r"D:\SHOOT_DAY_12\CAMERA".into());
        m.set_source_name("SHOOT_DAY_12".into());
        m.set_header_production("Night Shift".into());
        // LUNA_MOTION_IDLE: Luna waiting, at the app's own speed and 15 fps.
        let spin = if idle {
            i as f32 / 15.0 / 90.0 * std::f32::consts::TAU
        } else {
            m.set_phase(Phase::Running);
            let done = (t * 25.0).round() as u32;
            m.set_clips_done(done.to_string().into());
            m.set_clips_total("25".into());
            m.set_progress(t);
            // Several turns' worth of spin over the clip, so the motion shows.
            t * 1.4
        };
        m.show().unwrap();
        platform
            .last_window()
            .set_size(PhysicalSize::new(size.0, size.1));
        draw(&m, &mut moon, &mut sky, size, spin);
        snap(
            &platform.last_window(),
            size,
            &dir.join(format!("motion-{i:03}.ppm")),
        );
        m.hide().unwrap();
    }

    // Settings replaces the moon; Credits sits behind a link in it.
    for (name, credits) in [("7-settings", false), ("8-credits", true)] {
        let m = MainWindow::new().unwrap();
        m.set_output_dir(r"C:\Users\dana\Documents\Luna Reports".into());
        m.set_header_production("Night Shift".into());
        let spin = 0.0;
        let state = m.global::<SettingsState>();
        state.set_production("Night Shift".into());
        state.set_app_version("2026.10.0".into());
        state.set_can_check_updates(true);
        m.set_update_pending(true);
        state.set_company("Lumen Rentals".into());
        state.set_author("Dana Levi".into());
        state.set_fields(ModelRc::new(VecModel::from(vec![CustomField {
            label: "Shoot day".into(),
            value: "12".into(),
        }])));
        state.set_output_dir(r"C:\Users\dana\Documents\Luna Reports".into());
        state.set_can_save(true);
        state.set_show_credits(credits);
        state.set_credits(ModelRc::new(VecModel::from(vec![
            Component {
                name: "Slint".into(),
                licence: "Royalty-free 2.0".into(),
                role: "Window and controls".into(),
            },
            Component {
                name: "Typst".into(),
                licence: "Apache-2.0".into(),
                role: "PDF typesetting".into(),
            },
        ])));
        m.set_settings_open(true);
        m.show().unwrap();
        platform
            .last_window()
            .set_size(PhysicalSize::new(size.0, size.1));
        draw(&m, &mut moon, &mut sky, size, spin);
        snap(
            &platform.last_window(),
            size,
            &dir.join(format!("{name}.ppm")),
        );
        m.hide().unwrap();
    }
}

/// The sky and the Moon as the app would draw them for this window's state,
/// once it's laid out. The sky turns with the Moon, much more slowly.
fn draw(
    m: &MainWindow,
    moon: &mut MoonRenderer,
    sky: &mut BackdropRenderer,
    size: (u32, u32),
    spin: f32,
) {
    let progress = m.get_progress();
    let (lit, dial) = match m.get_phase() {
        Phase::Running => (0.08 + 0.92 * progress, progress),
        Phase::Done => (1.0, 1.0),
        _ => (0.08, 0.0),
    };
    let disc = moon.render(
        m.get_moon_box() as u32,
        2,
        MoonFrame {
            lit,
            spin,
            progress: dial,
        },
    );
    let centre = [m.get_moon_x(), m.get_moon_y()];
    let mut pixels = sky.render(
        Sky {
            width: size.0,
            height: size.1,
            centre,
            dial: m.get_moon_box() * 0.42,
            scale: 1.0,
        },
        spin.to_degrees() * 0.25 * 90.0 / 360.0,
    );
    place_moon(&mut pixels, &disc, centre);
    m.set_backdrop_image(slint::Image::from_rgb8(pixels));
}

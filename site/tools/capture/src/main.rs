//! Pictures for the marketing site, drawn by Luna's own renderers.
//!
//! `capture report <frames dir> <logo png> <out dir>`: the cat sample report
//! (real GH5 frames, faux pro metadata) as PDF, HTML and CSV, light and dark.
//! `capture screens <out dir>`: the main window at 2x in three phases, each
//! with and without the Moon, plus `layout.json` locating the Moon.
//! `capture moon <out dir>`: the full Moon with its dial, alpha outside.
//! Images are written as PPM/PAM; `build-assets.sh` converts them.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use luna_app::backdrop::{BackdropRenderer, Sky, place_moon};
use luna_app::moon::{MoonFrame, MoonRenderer};
use luna_app::ui::{CustomField, MainWindow, Phase};
use luna_report::{
    Camera, Card, Clip, ClipFields, Header, IdentitySource, Logo, Report, ReportTheme, Thumbnail,
};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, ModelRc, PhysicalSize, PlatformError, VecModel};

const LOGICAL: (u32, u32) = (880, 640);
const SCALE: f32 = 2.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("report") if args.len() == 4 => report(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
        ),
        Some("screens") if args.len() == 2 => screens(Path::new(&args[1])),
        Some("moon") if args.len() == 2 => moon(Path::new(&args[1])),
        _ => {
            eprintln!(
                "usage: capture report <frames> <logo.png> <out> | screens <out> | moon <out>"
            );
            std::process::exit(2);
        }
    }
}

// ---------------------------------------------------------------- report

struct Cam {
    model: &'static str,
    serial: &'static str,
    resolution: &'static str,
    codec: &'static str,
    iso: &'static str,
    wb: &'static str,
    bytes_per_frame: u64,
}

struct Shot {
    source: &'static str,
    name: &'static str,
    tc_in: &'static str,
    frames: u32,
    lens: &'static str,
    focal: &'static str,
    stop: &'static str,
    nd: &'static str,
    sensor_fps: Option<&'static str>,
}

const FPS: u32 = 25;

fn tc(frames: u32) -> String {
    let s = frames / FPS;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        s / 3600,
        s / 60 % 60,
        s % 60,
        frames % FPS
    )
}

fn parse_tc(t: &str) -> u32 {
    let p: Vec<u32> = t.split(':').map(|x| x.parse().expect("timecode")).collect();
    ((p[0] * 60 + p[1]) * 60 + p[2]) * FPS + p[3]
}

fn clip(frames_dir: &Path, s: &Shot, cam: &Cam) -> Clip {
    let start = parse_tc(s.tc_in);
    let thumbnails = (0..3)
        .map(|i| {
            let path = frames_dir.join(format!("{}_{i}.jpg", s.source));
            Thumbnail::from_jpeg(std::fs::read(&path).unwrap_or_else(|e| {
                panic!("{}: {e}", path.display());
            }))
        })
        .collect();
    Clip {
        fields: ClipFields {
            file_name: s.name.into(),
            recording_date: Some("2026-10-21".into()),
            tc_in: Some(tc(start)),
            tc_out: Some(tc(start + s.frames)),
            duration: Some(tc(s.frames)),
            fps: Some(FPS.to_string()),
            sensor_fps: s.sensor_fps.map(Into::into),
            resolution: Some(cam.resolution.into()),
            codec: Some(cam.codec.into()),
            camera_model: Some(cam.model.into()),
            camera_serial: Some(cam.serial.into()),
            lens: Some(s.lens.into()),
            focal_length: Some(s.focal.into()),
            stop: Some(s.stop.into()),
            iso: Some(cam.iso.into()),
            shutter: Some("180°".into()),
            white_balance: Some(cam.wb.into()),
            nd: Some(s.nd.into()),
            audio: Some("4 ch, 48 kHz, 24-bit".into()),
            // Organic sizes: a per-frame rate plus a little per-frame noise.
            file_size: Some(
                cam.bytes_per_frame * u64::from(s.frames) + u64::from(s.frames) * 7_919,
            ),
        },
        camera_source: IdentitySource::Metadata,
        reel_source: IdentitySource::Metadata,
        thumbnails,
        warnings: vec![],
    }
}

#[allow(clippy::too_many_arguments)]
const fn shot(
    source: &'static str,
    name: &'static str,
    tc_in: &'static str,
    frames: u32,
    lens: &'static str,
    focal: &'static str,
    stop: &'static str,
    nd: &'static str,
    sensor_fps: Option<&'static str>,
) -> Shot {
    Shot {
        source,
        name,
        tc_in,
        frames,
        lens,
        focal,
        stop,
        nd,
        sensor_fps,
    }
}

/// Camera A: ALEXA 35. Camera B: VENICE 2. Timecodes and lengths are the
/// GH5 clips' own; everything else is made up to read like a real day.
fn sample_report(frames_dir: &Path, logo: Vec<u8>) -> Report {
    let a = Cam {
        model: "ALEXA 35",
        serial: "60417",
        resolution: "4608×2592",
        codec: "ProRes 4444 XQ",
        iso: "800",
        wb: "4300 K",
        bytes_per_frame: 11_284_113,
    };
    let b = Cam {
        model: "VENICE 2",
        serial: "0012684",
        resolution: "8640×4556",
        codec: "X-OCN ST",
        iso: "3200",
        wb: "3200 K",
        bytes_per_frame: 14_706_381,
    };
    #[rustfmt::skip]
    let a003 = [
        shot("P1066462", "A003C001_261021_R2KB.mov", "16:41:09:08", 216, "Signature Prime 47", "47 mm", "T2.8", "0.3", None),
        shot("P1066463", "A003C002_261021_R2KB.mov", "16:41:57:17", 384, "Signature Prime 47", "47 mm", "T2.8", "0.3", None),
        shot("P1066464", "A003C003_261021_R2KB.mov", "16:42:47:02", 444, "Signature Prime 35", "35 mm", "T2.8", "0.3", None),
        shot("P1066465", "A003C004_261021_R2KB.mov", "16:44:43:19", 156, "Signature Prime 18", "18 mm", "T4", "0.9", None),
    ];
    #[rustfmt::skip]
    let a004 = [
        shot("P1066466", "A004C001_261021_R2KB.mov", "16:46:13:22", 348, "Signature Prime 29", "29 mm", "T2", "0.0", None),
        shot("P1066467", "A004C002_261021_R2KB.mov", "16:48:01:11", 348, "Signature Prime 29", "29 mm", "T2", "0.0", None),
        shot("P1066468", "A004C003_261021_R2KB.mov", "16:50:13:12", 324, "Signature Prime 125", "125 mm", "T2.8", "0.0", Some("50")),
        shot("P1066469", "A004C004_261021_R2KB.mov", "16:50:33:20", 588, "Signature Prime 125", "125 mm", "T1.8", "0.0", Some("50")),
    ];
    #[rustfmt::skip]
    let b002 = [
        shot("P1066470", "B002C001_261021_V2RX.mxf", "10:04:15:17", 372, "Supreme Prime 21", "21 mm", "T2.8", "0.6", None),
        shot("P1066471", "B002C002_261021_V2RX.mxf", "10:04:45:00", 588, "Supreme Prime 65", "65 mm", "T2", "0.6", None),
        shot("P1066473", "B002C003_261021_V2RX.mxf", "10:06:22:00", 408, "Supreme Prime 65", "65 mm", "T2", "0.6", None),
    ];
    let card = |reel: &str, shots: &[Shot], cam: &Cam| Card {
        reel: reel.into(),
        clips: shots.iter().map(|s| clip(frames_dir, s, cam)).collect(),
    };
    Report {
        header: Header {
            production: Some("Luna".into()),
            company: Some("Moonlight Pictures".into()),
            author: Some("Shaked L.".into()),
            logo: Some(Logo::from_bytes(logo).expect("logo decodes")),
            custom: vec![luna_report::CustomField {
                label: "Shoot day".into(),
                value: "12".into(),
            }],
        },
        theme: ReportTheme::Light,
        sources: vec!["E:\\LUNA_DAY12".into()],
        generated_at: "2026-10-21 19:42".into(),
        luna_version: "2026.10.3".into(),
        cameras: vec![
            Camera {
                name: "A".into(),
                cards: vec![card("A003", &a003, &a), card("A004", &a004, &a)],
            },
            Camera {
                name: "B".into(),
                cards: vec![card("B002", &b002, &b)],
            },
        ],
        not_included: vec![],
        unreadable: vec![],
    }
}

fn report(frames_dir: &Path, logo: &Path, out: &Path) {
    std::fs::create_dir_all(out).expect("out dir");
    let light = sample_report(frames_dir, std::fs::read(logo).expect("logo"));
    let mut dark = light.clone();
    dark.theme = ReportTheme::Dark;
    for (r, stem) in [(&light, "luna-sample"), (&dark, "luna-sample-dark")] {
        write(&out.join(format!("{stem}.pdf")), &luna_report::write_pdf(r).expect("pdf"));
        write(&out.join(format!("{stem}.html")), &luna_report::write_html(r).expect("html"));
    }
    write(&out.join("luna-sample.csv"), &luna_report::write_csv(&light));
}

// --------------------------------------------------------------- screens

/// Hands out a fresh software window for each component; the last one made
/// is the one drawn.
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

fn screens(out: &Path) {
    std::fs::create_dir_all(out).expect("out dir");
    let platform: &'static Headless = Box::leak(Box::new(Headless {
        last: RefCell::new(None),
    }));
    slint::platform::set_platform(Box::new(platform)).expect("platform");
    let size = (
        (LOGICAL.0 as f32 * SCALE) as u32,
        (LOGICAL.1 as f32 * SCALE) as u32,
    );
    let mut moon = MoonRenderer::new();
    let mut sky = BackdropRenderer::new();
    type Setup = fn(&MainWindow);
    let phases: [(&str, Setup); 3] = [
        ("idle", |_| {}),
        ("running", |m| {
            m.set_phase(Phase::Running);
            m.set_files_found("214".into());
            m.set_clips_done("17".into());
            m.set_clips_total("25".into());
            m.set_problems("2".into());
            m.set_has_problems(true);
            m.set_progress(0.68);
        }),
        ("done", |m| {
            m.set_phase(Phase::Done);
            m.set_summary("6 of 25 clips need attention".into());
            m.set_has_problems(true);
        }),
    ];
    let mut layout = String::new();
    for (name, setup) in phases {
        for with_moon in [true, false] {
            let m = MainWindow::new().expect("window");
            m.set_source(r"E:\LUNA_DAY12".into());
            m.set_source_name("LUNA_DAY12".into());
            m.set_pdf_name("Luna Report 2026-10-21 1942.pdf".into());
            m.set_html_name("Luna Report 2026-10-21 1942.html".into());
            m.set_csv_name("Luna Report 2026-10-21 1942.csv".into());
            m.set_folder(r"C:\Users\shaked\Documents\Luna Reports".into());
            m.set_header_production("Luna".into());
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
            m.show().expect("show");
            let window = platform.last_window();
            window.dispatch_event(WindowEvent::ScaleFactorChanged {
                scale_factor: SCALE,
            });
            window.set_size(PhysicalSize::new(size.0, size.1));
            let centre = draw(&m, &mut moon, &mut sky, size, with_moon);
            let suffix = if with_moon { "" } else { "-nomoon" };
            snap(&window, size, &out.join(format!("app-{name}{suffix}@2x.ppm")));
            if layout.is_empty() {
                layout = format!(
                    "{{ \"moonCx\": {:.1}, \"moonCy\": {:.1}, \"moonBox\": {:.1}, \"w\": {}, \"h\": {} }}\n",
                    centre[0],
                    centre[1],
                    m.get_moon_box() * SCALE,
                    size.0,
                    size.1
                );
            }
            m.hide().expect("hide");
        }
    }
    write(&out.join("layout.json"), layout.as_bytes());
}

/// The sky, and the Moon on it when `with_moon`, as the app draws them for
/// this window's state (see `ui.rs`). Returns the Moon's centre in pixels.
fn draw(
    m: &MainWindow,
    moon: &mut MoonRenderer,
    sky: &mut BackdropRenderer,
    size: (u32, u32),
    with_moon: bool,
) -> [f32; 2] {
    slint::platform::update_timers_and_animations();
    let progress = m.get_progress();
    let (lit, dial) = match m.get_phase() {
        Phase::Running => (0.08 + 0.92 * progress, progress),
        Phase::Done => (1.0, 1.0),
        _ => (0.08, 0.0),
    };
    let centre = [m.get_moon_x() * SCALE, m.get_moon_y() * SCALE];
    let mut pixels = sky.render(
        Sky {
            width: size.0,
            height: size.1,
            centre,
            dial: m.get_moon_box() * 0.42 * SCALE,
            scale: SCALE,
        },
        0.0,
    );
    if with_moon {
        let disc = moon.render(
            (m.get_moon_box() * SCALE).round() as u32,
            1,
            MoonFrame {
                lit,
                spin: 0.0,
                progress: dial,
            },
        );
        place_moon(&mut pixels, &disc, centre);
    }
    m.set_backdrop_image(slint::Image::from_rgb8(pixels));
    centre
}

fn snap(window: &Rc<MinimalSoftwareWindow>, size: (u32, u32), path: &Path) {
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
    write(path, &out);
}

// ------------------------------------------------------------------ moon

fn moon(out: &Path) {
    std::fs::create_dir_all(out).expect("out dir");
    let size = 1200u32;
    let img = MoonRenderer::new().render(
        size,
        2,
        MoonFrame {
            lit: 1.0,
            spin: 0.0,
            progress: 1.0,
        },
    );
    // PAM keeps the alpha. The renderer's pixels are premultiplied: undo it.
    let mut bytes = format!(
        "P7\nWIDTH {size}\nHEIGHT {size}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
    )
    .into_bytes();
    for p in img.as_slice() {
        let a = p.a as u32;
        let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
        bytes.extend([un(p.r), un(p.g), un(p.b), p.a]);
    }
    write(&out.join("moon-still.pam"), &bytes);
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    eprintln!("wrote {}", path.display());
}


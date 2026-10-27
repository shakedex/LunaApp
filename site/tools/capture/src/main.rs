//! Pictures for the marketing site, drawn by Luna's own renderers, so every
//! image on the site is the real app and the real report.
//!
//! `capture screens <out>`: the main window at 2x in its phases, plus
//! `layout.json` locating the Moon and the text lines inside the window.
//! `capture sky <out>`: the app's star chart, page sized, for the backdrop.
//! `capture albedo <out>`: the lunar albedo the Moon shader samples.
//! `capture report <frames dir> <logo png> <out>`: the sample report (real
//! frames from Shaked's GH5 clips of Luna, faux production metadata) as PDF,
//! HTML and CSV, light and dark.
//!
//! Images come out as PPM/PGM; `build-assets.sh` converts them.

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

/// The app's default window, in logical pixels, captured at 2x.
const LOGICAL: (u32, u32) = (880, 640);
const SCALE: f32 = 2.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| Path::new(&args[i]);
    match (args.first().map(String::as_str), args.len()) {
        (Some("screens"), 2) => screens(arg(1)),
        (Some("sky"), 2) => sky(arg(1)),
        (Some("albedo"), 2) => albedo(arg(1)),
        (Some("report"), 4) => report(arg(1), arg(2), arg(3)),
        _ => {
            eprintln!(
                "usage: capture screens <out> | sky <out> | albedo <out> | report <frames> <logo.png> <out>"
            );
            std::process::exit(2);
        }
    }
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

type Setup = fn(&MainWindow);

/// One capture of the window.
struct Shot {
    name: &'static str,
    setup: Setup,
    /// Draw the Moon into the sky. The hero shots leave it out, and lose
    /// the two text lines under it: the site draws those live.
    moon: bool,
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

    let running: Setup = |m| {
        m.set_phase(Phase::Running);
        m.set_files_found("214".into());
        m.set_clips_done("17".into());
        m.set_clips_total("25".into());
        m.set_progress(0.68);
    };
    let done: Setup = |m| {
        m.set_phase(Phase::Done);
        m.set_summary("25 clips, no problems".into());
    };
    let shots = [
        Shot {
            name: "idle",
            setup: |_| {},
            moon: true,
        },
        Shot {
            name: "running",
            setup: running,
            moon: true,
        },
        Shot {
            name: "done",
            setup: done,
            moon: true,
        },
        Shot {
            name: "hero-idle",
            setup: |_| {},
            moon: false,
        },
        Shot {
            name: "hero-running",
            setup: running,
            moon: false,
        },
        Shot {
            name: "hero-done",
            setup: done,
            moon: false,
        },
    ];

    let mut layout = String::new();
    for shot in shots {
        let m = MainWindow::new().expect("window");
        m.set_source(r"E:\NIGHT_SHIFT_DAY12".into());
        m.set_source_name("NIGHT_SHIFT_DAY12".into());
        m.set_pdf_name("Luna Report 2026-10-21 1942.pdf".into());
        m.set_html_name("Luna Report 2026-10-21 1942.html".into());
        m.set_csv_name("Luna Report 2026-10-21 1942.csv".into());
        m.set_folder(r"C:\Users\shaked\Documents\Luna Reports".into());
        m.set_header_production("Night Shift".into());
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
        (shot.setup)(&m);
        m.show().expect("show");
        let window = platform.last_window();
        window.dispatch_event(WindowEvent::ScaleFactorChanged {
            scale_factor: SCALE,
        });
        window.set_size(PhysicalSize::new(size.0, size.1));
        let (centre, sky_rgb) = draw(&m, &mut moon, &mut sky, size, shot.moon);
        let mut rgb = render(&window, size);
        let moon_box = m.get_moon_box() * SCALE;
        if layout.is_empty() {
            layout = format!(
                "{{\n  \"width\": {},\n  \"height\": {},\n  \"scale\": {},\n  \"moon\": {{ \"cx\": {:.1}, \"cy\": {:.1}, \"box\": {:.1} }}",
                size.0, size.1, SCALE, centre[0], centre[1], moon_box,
            );
        }
        if !shot.moon {
            let rows = blank_text(&mut rgb, &sky_rgb, size, centre, moon_box);
            // The done layout sits higher than the running one, so each
            // hero shot records its own Moon centre and text rows.
            layout += &format!(
                ",\n  \"{}\": {{ \"moon\": {{ \"cx\": {:.1}, \"cy\": {:.1}, \"box\": {:.1} }}, \"titleTop\": {}, \"titleBottom\": {}, \"subTop\": {}, \"subBottom\": {}, \"buttonTop\": {} }}",
                shot.name,
                centre[0],
                centre[1],
                moon_box,
                rows.title.0,
                rows.title.1,
                rows.sub.0,
                rows.sub.1,
                rows.button
            );
        }
        write_ppm(&out.join(format!("app-{}@2x.ppm", shot.name)), size, &rgb);
        m.hide().expect("hide");
    }
    layout += "\n}\n";
    write(&out.join("layout.json"), layout.as_bytes());
}

/// Where the two text lines under the Moon were, in pixel rows.
struct TextRows {
    title: (u32, u32),
    sub: (u32, u32),
    button: u32,
}

/// Paints the sky back over the two text lines under the Moon, so the site
/// can set its own live text there, and reports where they were. The button
/// row below them is the first row that differs from the sky right across
/// the centre band; text never does.
fn blank_text(
    rgb: &mut [u8],
    sky: &[u8],
    size: (u32, u32),
    centre: [f32; 2],
    moon_box: f32,
) -> TextRows {
    let w = size.0 as usize;
    let cx = centre[0].round() as usize;
    let moon_bottom = (centre[1] + moon_box / 2.0).round() as usize + 2;
    let differs = |row: usize, x: usize| {
        let i = (row * w + x) * 3;
        (0..3).any(|k| rgb[i + k].abs_diff(sky[i + k]) > 8)
    };
    // A button is a solid run of pixels unlike the sky; a glyph never is.
    let solid_run = 50usize;
    let mut text_rows = Vec::new();
    let mut button = size.1 as usize;
    'rows: for y in moon_bottom..size.1 as usize {
        let mut run = 0;
        let mut any = false;
        for x in w / 4..3 * w / 4 {
            if differs(y, x) {
                any = true;
                run += 1;
                if run >= solid_run {
                    button = y;
                    break 'rows;
                }
            } else {
                run = 0;
            }
        }
        if any {
            text_rows.push(y);
        }
    }
    let _ = cx;
    // Two lines: split the rows at the widest gap.
    let mut split = text_rows.len();
    let mut widest = 0;
    for i in 1..text_rows.len() {
        let gap = text_rows[i] - text_rows[i - 1];
        if gap > widest {
            widest = gap;
            split = i;
        }
    }
    let (a, b) = text_rows.split_at(split);
    let span = |r: &[usize]| {
        (
            r.first().copied().unwrap_or(0) as u32,
            r.last().copied().unwrap_or(0) as u32,
        )
    };
    for y in moon_bottom..button.saturating_sub(2) {
        let i = y * w * 3;
        rgb[i..i + w * 3].copy_from_slice(&sky[i..i + w * 3]);
    }
    TextRows {
        title: span(a),
        sub: span(b),
        button: button as u32,
    }
}

/// The sky, and the Moon on it when `with_moon`, as the app draws them for
/// this window's state (see `ui.rs`). Returns the Moon's centre in pixels
/// and the bare sky, RGB.
fn draw(
    m: &MainWindow,
    moon: &mut MoonRenderer,
    sky: &mut BackdropRenderer,
    size: (u32, u32),
    with_moon: bool,
) -> ([f32; 2], Vec<u8>) {
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
    let bare: Vec<u8> = pixels
        .as_slice()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b])
        .collect();
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
    (centre, bare)
}

/// Draws the window as it stands, RGB.
fn render(window: &Rc<MinimalSoftwareWindow>, size: (u32, u32)) -> Vec<u8> {
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let (w, h) = (size.0 as usize, size.1 as usize);
    let mut buf = vec![PremultipliedRgbaColor::default(); w * h];
    window.draw_if_needed(|r| {
        r.render(&mut buf, w);
    });
    buf.iter().flat_map(|p| [p.red, p.green, p.blue]).collect()
}

fn write_ppm(path: &Path, size: (u32, u32), rgb: &[u8]) {
    let mut out = format!("P6\n{} {}\n255\n", size.0, size.1).into_bytes();
    out.extend_from_slice(rgb);
    write(path, &out);
}

// ------------------------------------------------------------------- sky

/// The app's star chart at page size. The Moon's place is where the hero
/// window sits, right of centre, so the graticule calms there.
fn sky(out: &Path) {
    std::fs::create_dir_all(out).expect("out dir");
    let (w, h) = (3200u32, 2000u32);
    let pixels = BackdropRenderer::new().render(
        Sky {
            width: w,
            height: h,
            centre: [w as f32 * 0.68, h as f32 * 0.46],
            dial: h as f32 * 0.30,
            scale: 2.0,
        },
        6.0,
    );
    let mut bytes = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in pixels.as_slice() {
        bytes.extend([p.r, p.g, p.b]);
    }
    write(&out.join("sky.ppm"), &bytes);
}

// ---------------------------------------------------------------- albedo

/// The 16-bit lunar albedo as an 8-bit grey map, for the site's shader.
fn albedo(out: &Path) {
    std::fs::create_dir_all(out).expect("out dir");
    let raw = include_bytes!("../../../../crates/luna-app/assets/moon-albedo-1024x512.u16");
    let (w, h) = (1024usize, 512usize);
    assert_eq!(raw.len(), w * h * 2, "albedo size");
    let mut bytes = format!("P5\n{w} {h}\n255\n").into_bytes();
    for px in raw.chunks_exact(2) {
        let v = u16::from_le_bytes([px[0], px[1]]);
        bytes.push((v >> 8) as u8);
    }
    write(&out.join("moon-albedo.pgm"), &bytes);
}

// ---------------------------------------------------------------- report

/// A camera's fixed facts for the day.
struct Cam {
    model: &'static str,
    serial: &'static str,
    resolution: &'static str,
    codec: &'static str,
    iso: &'static str,
    wb: &'static str,
    bytes_per_frame: u64,
}

/// One take. `source` is the GH5 clip whose frames stand in for it; its
/// length in frames is the clip's own.
struct Take {
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

fn clip(frames_dir: &Path, t: &Take, cam: &Cam) -> Clip {
    let start = parse_tc(t.tc_in);
    let thumbnails = (0..3)
        .map(|i| {
            let path = frames_dir.join(format!("{}_{i}.jpg", t.source));
            Thumbnail::from_jpeg(
                std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
            )
        })
        .collect();
    Clip {
        fields: ClipFields {
            file_name: t.name.into(),
            recording_date: Some("2026-10-21".into()),
            tc_in: Some(tc(start)),
            tc_out: Some(tc(start + t.frames)),
            duration: Some(tc(t.frames)),
            fps: Some(FPS.to_string()),
            sensor_fps: t.sensor_fps.map(Into::into),
            resolution: Some(cam.resolution.into()),
            codec: Some(cam.codec.into()),
            camera_model: Some(cam.model.into()),
            camera_serial: Some(cam.serial.into()),
            lens: Some(t.lens.into()),
            focal_length: Some(t.focal.into()),
            stop: Some(t.stop.into()),
            iso: Some(cam.iso.into()),
            shutter: Some("180°".into()),
            white_balance: Some(cam.wb.into()),
            nd: Some(t.nd.into()),
            audio: Some("4 ch, 48 kHz, 24-bit".into()),
            // A per-frame rate plus a little per-frame noise, so sizes read as real.
            file_size: Some(
                cam.bytes_per_frame * u64::from(t.frames) + u64::from(t.frames) * 7_919,
            ),
        },
        camera_source: IdentitySource::Metadata,
        reel_source: IdentitySource::Metadata,
        thumbnails,
        warnings: vec![],
        colour: None,
    }
}

#[allow(clippy::too_many_arguments)]
const fn take(
    source: &'static str,
    name: &'static str,
    tc_in: &'static str,
    frames: u32,
    lens: &'static str,
    focal: &'static str,
    stop: &'static str,
    nd: &'static str,
    sensor_fps: Option<&'static str>,
) -> Take {
    Take {
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

/// Camera A: ALEXA 35 recording ARRIRAW. Camera B: VENICE 2 recording X-OCN.
/// Clip lengths and the frames are the GH5 clips' own; the rest is made up
/// to read like one shoot day.
fn sample_report(frames_dir: &Path, logo: Vec<u8>) -> Report {
    let a = Cam {
        model: "ALEXA 35",
        serial: "60417",
        resolution: "4608×2592",
        codec: "ARRIRAW",
        iso: "800",
        wb: "4300 K",
        bytes_per_frame: 17_915_904,
    };
    let b = Cam {
        model: "VENICE 2",
        serial: "0012684",
        resolution: "8640×4556",
        codec: "X-OCN ST",
        iso: "800",
        wb: "3200 K",
        bytes_per_frame: 8_306_688,
    };
    #[rustfmt::skip]
    let a003 = [
        take("P1066462", "A003C001_261021_R2KB.ari", "16:41:09:08", 216, "Signature Prime 47", "47 mm", "T2.8", "0.3", None),
        take("P1066463", "A003C002_261021_R2KB.ari", "16:41:57:17", 384, "Signature Prime 47", "47 mm", "T2.8", "0.3", None),
        take("P1066464", "A003C003_261021_R2KB.ari", "16:42:47:02", 444, "Signature Prime 35", "35 mm", "T2.8", "0.3", None),
        take("P1066465", "A003C004_261021_R2KB.ari", "16:44:43:19", 156, "Signature Prime 18", "18 mm", "T4", "0.9", None),
    ];
    #[rustfmt::skip]
    let a004 = [
        take("P1066466", "A004C001_261021_R2KB.ari", "16:46:13:22", 348, "Signature Prime 29", "29 mm", "T2", "0.0", None),
        take("P1066467", "A004C002_261021_R2KB.ari", "16:48:01:11", 348, "Signature Prime 29", "29 mm", "T2", "0.0", None),
        take("P1066468", "A004C003_261021_R2KB.ari", "16:50:13:12", 324, "Signature Prime 125", "125 mm", "T2.8", "0.0", Some("50")),
        take("P1066469", "A004C004_261021_R2KB.ari", "16:50:33:20", 588, "Signature Prime 125", "125 mm", "T1.8", "0.0", Some("50")),
    ];
    #[rustfmt::skip]
    let b002 = [
        take("P1066470", "B002C001_261021_V2RX.mxf", "10:04:15:17", 372, "Supreme Prime 21", "21 mm", "T2.8", "0.6", None),
        take("P1066471", "B002C002_261021_V2RX.mxf", "10:04:45:00", 588, "Supreme Prime 65", "65 mm", "T2", "0.6", None),
        take("P1066473", "B002C003_261021_V2RX.mxf", "10:06:22:00", 408, "Supreme Prime 65", "65 mm", "T2", "0.6", None),
    ];
    let card = |reel: &str, takes: &[Take], cam: &Cam| Card {
        reel: reel.into(),
        clips: takes.iter().map(|t| clip(frames_dir, t, cam)).collect(),
    };
    Report {
        header: Header {
            production: Some("Night Shift".into()),
            company: Some("Moonlight Pictures".into()),
            author: Some("Shaked L.".into()),
            logo: Some(Logo::from_bytes(logo).expect("logo decodes")),
            custom: vec![luna_report::CustomField {
                label: "Shoot day".into(),
                value: "12".into(),
            }],
        },
        theme: ReportTheme::Light,
        sources: vec![r"E:\NIGHT_SHIFT_DAY12".into()],
        generated_at: "2026-10-21 19:42".into(),
        luna_version: "2026.10.0".into(),
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
        show_problems: false,
    }
}

fn report(frames_dir: &Path, logo: &Path, out: &Path) {
    std::fs::create_dir_all(out).expect("out dir");
    let light = sample_report(frames_dir, std::fs::read(logo).expect("logo"));
    let mut dark = light.clone();
    dark.theme = ReportTheme::Dark;
    for (r, stem) in [(&light, "sample-light"), (&dark, "sample-dark")] {
        write(
            &out.join(format!("{stem}.pdf")),
            &luna_report::write_pdf(r).expect("pdf"),
        );
        write(
            &out.join(format!("{stem}.html")),
            &luna_report::write_html(r).expect("html"),
        );
    }
    write(&out.join("sample.csv"), &luna_report::write_csv(&light));
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    eprintln!("wrote {}", path.display());
}

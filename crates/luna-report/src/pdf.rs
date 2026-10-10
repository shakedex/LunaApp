//! PDF: the report template compiled by an embedded Typst, entirely in memory.
//!
//! Data reaches the template as JSON, so text from files and Settings is always
//! printed literally, never interpreted as Typst markup.

use std::collections::HashMap;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration, Smart};
use typst::layout::{Frame, FrameItem, Point};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;

use crate::error::ReportError;
use crate::model::Report;
use crate::view::{self, ImageRef};

const TEMPLATE: &str = include_str!("../templates/report.typ");

pub fn write_pdf(report: &Report) -> Result<Vec<u8>, ReportError> {
    let doc = compile(report)?;
    let options = PdfOptions {
        creator: Smart::Custom(Some(format!("Luna {}", report.luna_version))),
        ..PdfOptions::default()
    };
    typst_pdf::pdf(&doc, &options).map_err(|d| ReportError::Pdf(messages(&d)))
}

/// What the compiled PDF contains. For tests; not part of the report.
#[derive(Debug)]
pub struct PdfInspection {
    pub pages: usize,
    /// Every shaped text run, one per line.
    pub text: String,
    pub images: usize,
    /// Text runs that contain a glyph no embedded font covers.
    pub missing_glyphs: Vec<String>,
    /// Where each text run sits, for layout checks.
    pub runs: Vec<TextRun>,
}

/// One shaped text run, positioned in points from the page's top-left corner.
#[derive(Debug, Clone)]
pub struct TextRun {
    pub page: usize,
    pub x: f64,
    /// The baseline.
    pub y: f64,
    pub width: f64,
    pub text: String,
}

#[doc(hidden)]
pub fn inspect_pdf(report: &Report) -> Result<PdfInspection, ReportError> {
    let doc = compile(report)?;
    let mut inspection = PdfInspection {
        pages: doc.pages().len(),
        text: String::new(),
        images: 0,
        missing_glyphs: Vec::new(),
        runs: Vec::new(),
    };
    for (page, p) in doc.pages().iter().enumerate() {
        walk(&p.frame, page, Point::zero(), &mut inspection);
    }
    Ok(inspection)
}

/// The report template only translates groups, so offsets add up.
fn walk(frame: &Frame, page: usize, origin: Point, out: &mut PdfInspection) {
    for (pos, item) in frame.items() {
        let at = origin + *pos;
        match item {
            FrameItem::Group(group) => {
                let shift = Point::new(group.transform.tx, group.transform.ty);
                walk(&group.frame, page, at + shift, out);
            }
            FrameItem::Text(text) => {
                out.text.push_str(&text.text);
                out.text.push('\n');
                if text.glyphs.iter().any(|g| g.id == 0) {
                    out.missing_glyphs.push(text.text.to_string());
                }
                out.runs.push(TextRun {
                    page,
                    x: at.x.to_pt(),
                    y: at.y.to_pt(),
                    width: text.width().to_pt(),
                    text: text.text.to_string(),
                });
            }
            FrameItem::Image(..) => out.images += 1,
            _ => {}
        }
    }
}

fn compile(report: &Report) -> Result<PagedDocument, ReportError> {
    let mut files: HashMap<FileId, Bytes> = HashMap::new();
    let mut thumb_count = 0usize;
    let view = view::build(report, &mut |image| {
        let (path, bytes) = match image {
            ImageRef::Logo(format, bytes) => (format!("/logo.{}", format.extension()), bytes),
            ImageRef::Thumb(bytes) => {
                thumb_count += 1;
                (format!("/thumbs/{thumb_count}.jpg"), bytes)
            }
        };
        files.insert(file_id(&path), Bytes::new(bytes.to_vec()));
        path
    });
    let json = serde_json::to_vec(&view).map_err(|e| ReportError::Pdf(e.to_string()))?;
    files.insert(file_id("/data.json"), Bytes::new(json));
    let main = file_id("/report.typ");
    files.insert(main, Bytes::from_string(TEMPLATE.to_owned()));
    render(main, files).map_err(ReportError::Pdf)
}

/// Places an image exactly as the report does, so a check here holds for the report.
pub(crate) fn check_image(path: &str, bytes: &[u8]) -> Result<(), String> {
    let main = file_id("/check.typ");
    let files = HashMap::from([
        (file_id(path), Bytes::new(bytes.to_vec())),
        (main, Bytes::from_string(format!("#image({path:?})"))),
    ]);
    let doc = render(main, files)?;
    typst_pdf::pdf(&doc, &PdfOptions::default())
        .map(drop)
        .map_err(|d| messages(&d))
}

fn render(main: FileId, files: HashMap<FileId, Bytes>) -> Result<PagedDocument, String> {
    let mut fonts = FontStore::new();
    fonts.extend(typst_kit::fonts::embedded());
    let world = MemoryWorld {
        library: LazyHash::new(Library::builder().build()),
        fonts,
        main,
        files,
    };
    typst::compile::<PagedDocument>(&world)
        .output
        .map_err(|d| messages(&d))
}

fn file_id(path: &str) -> FileId {
    let vpath = VirtualPath::new(path).expect("report paths are static and valid");
    RootedPath::new(VirtualRoot::Project, vpath).intern()
}

fn messages(diagnostics: &[SourceDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| d.message.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Typst's view of the files: the template, the data and the images, all in memory.
struct MemoryWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    files: HashMap<FileId, Bytes>,
}

impl World for MemoryWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let bytes = self.file(id)?;
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text.to_owned()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(id.get().vpath().get_with_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    /// The report carries its own formatted date; the template never asks Typst for one.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

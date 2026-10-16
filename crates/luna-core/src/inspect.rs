//! What is in a set of folders, from metadata only: cameras, cards, clip
//! counts. No frame is decoded, so it is quick enough to plan reports with.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use luna_worker::pool::JobOutcome;
use serde::Serialize;

use crate::assemble::clip_fields;
use crate::identity::resolve_identity;
use crate::run::{Progress, RunError, WorkerSpec, probe_all, source_roots};
use crate::scan::{Scan, scan_roots};
use crate::select::camera_letter;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inspection {
    pub sources: Vec<String>,
    pub cameras: Vec<InspectedCamera>,
    pub not_included: usize,
    pub unreadable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InspectedCamera {
    /// The camera as the report names it; what a camera filter accepts.
    pub id: String,
    pub letter: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub cards: Vec<InspectedCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InspectedCard {
    pub reel: String,
    pub folder: String,
    pub clips: usize,
    pub formats: Vec<String>,
    pub first_recorded: Option<String>,
    pub last_recorded: Option<String>,
    pub bytes: u64,
}

/// Scans `roots` and reads every clip's metadata in worker processes.
pub fn inspect(
    roots: &[PathBuf],
    workers: &WorkerSpec,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &AtomicBool,
) -> Result<Inspection, RunError> {
    let roots = source_roots(roots)?;
    let scanned = scan_roots(&roots);
    let outcomes = probe_all(&scanned, workers, progress, cancel)?;
    Ok(inspection(&roots, &scanned, &outcomes))
}

/// The inspection of a scan, given one probe outcome per clip (same order).
pub(crate) fn inspection(roots: &[PathBuf], scanned: &Scan, outcomes: &[JobOutcome]) -> Inspection {
    let results: Vec<_> = outcomes
        .iter()
        .map(|o| match o {
            JobOutcome::Done(r) => Some(&**r),
            _ => None,
        })
        .collect();
    let input: Vec<_> = scanned
        .clips
        .iter()
        .zip(&results)
        .map(|(f, r)| (f.clone(), r.and_then(|r| r.meta.as_ref())))
        .collect();
    let cameras = resolve_identity(roots, &input)
        .into_iter()
        .map(|cam| {
            let metas = || {
                cam.cards
                    .iter()
                    .flat_map(|c| &c.clips)
                    .filter_map(|id| results[id.index].and_then(|r| r.meta.as_ref()))
            };
            InspectedCamera {
                letter: camera_letter(&cam.name),
                model: most_common(metas().filter_map(|m| m.camera_model.clone())),
                serial: most_common(metas().filter_map(|m| m.camera_serial.clone())),
                cards: cam
                    .cards
                    .iter()
                    .map(|card| {
                        let mut formats: Vec<String> = Vec::new();
                        let mut dates: Vec<String> = Vec::new();
                        let mut bytes = 0;
                        for id in &card.clips {
                            let r = results[id.index];
                            let fields = clip_fields(
                                "",
                                r.and_then(|r| r.meta.as_ref()),
                                r.and_then(|r| r.codec.as_ref()),
                                r.map_or(0, |r| r.bytes),
                            );
                            bytes += r.map_or(0, |r| r.bytes);
                            if let Some(c) = fields.codec
                                && !formats.contains(&c)
                            {
                                formats.push(c);
                            }
                            dates.extend(fields.recording_date);
                        }
                        dates.sort();
                        InspectedCard {
                            reel: card.reel.clone(),
                            folder: card.folder.clone(),
                            clips: card.clips.len(),
                            formats,
                            first_recorded: dates.first().cloned(),
                            last_recorded: dates.last().cloned(),
                            bytes,
                        }
                    })
                    .collect(),
                id: cam.name,
            }
        })
        .collect();
    Inspection {
        sources: roots
            .iter()
            .map(|r| r.to_string_lossy().into_owned())
            .collect(),
        cameras,
        not_included: scanned.not_included.len(),
        unreadable: scanned.unreadable.iter().map(|u| u.path.clone()).collect(),
    }
}

fn most_common(values: impl Iterator<Item = String>) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for v in values {
        match counts.iter_mut().find(|(c, _)| *c == v) {
            Some((_, n)) => *n += 1,
            None => counts.push((v, 1)),
        }
    }
    // First seen wins a tie.
    counts
        .into_iter()
        .rev()
        .max_by_key(|(_, n)| *n)
        .map(|(v, _)| v)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use luna_formats::{ClipFiles, ClipMeta, Container};
    use luna_worker::ClipResult;
    use luna_worker::pool::JobOutcome;

    use super::*;
    use crate::run::build_report;
    use crate::scan::Scan;
    use crate::settings::Settings;

    fn clip(path: &str) -> ClipFiles {
        ClipFiles {
            container: Container::Mxf,
            main: Path::new("D:/Shoot").join(path),
            segments: vec![],
            sidecars: vec![],
        }
    }

    fn probed(id: u64, date: &str) -> JobOutcome {
        JobOutcome::Done(Box::new(ClipResult {
            id,
            meta: Some(ClipMeta {
                camera_model: Some("ALEXA 35".into()),
                camera_serial: Some("12345".into()),
                recording_date: Some(date.into()),
                ..ClipMeta::default()
            }),
            codec: None,
            bytes: 1000,
            thumbnails: vec![],
            warnings: vec![],
        }))
    }

    fn fixture() -> (Vec<PathBuf>, Scan, Vec<JobOutcome>) {
        let scan = Scan {
            clips: vec![
                clip("A_CAM/A001/A001C001_251020_R1AB.mxf"),
                clip("A_CAM/A001/A001C002_251020_R1AB.mxf"),
                clip("B_CAM/B001/B001C001_251020_R2CD.mxf"),
            ],
            not_included: vec!["notes.txt".into()],
            unreadable: vec![],
        };
        let outcomes = vec![
            probed(0, "2025-10-20 09:00:00"),
            probed(1, "2025-10-20 17:30:00"),
            probed(2, "2025-10-20 10:00:00"),
        ];
        (vec![PathBuf::from("D:/Shoot")], scan, outcomes)
    }

    #[test]
    fn inspection_ids_match_report_cameras() {
        let (roots, scan, outcomes) = fixture();
        let i = inspection(&roots, &scan, &outcomes);
        let r = build_report(&roots, &Settings::default(), scan, outcomes, "now", None);
        let ids: Vec<_> = i.cameras.iter().map(|c| c.id.clone()).collect();
        let names: Vec<_> = r.cameras.iter().map(|c| c.name.clone()).collect();
        assert_eq!(ids, names);
    }

    #[test]
    fn inspection_counts_and_bytes() {
        let (roots, scan, outcomes) = fixture();
        let i = inspection(&roots, &scan, &outcomes);
        let a = &i.cameras[0];
        assert_eq!(a.letter.as_deref(), Some("A"));
        assert_eq!(a.model.as_deref(), Some("ALEXA 35"));
        assert_eq!(a.serial.as_deref(), Some("12345"));
        let card = &a.cards[0];
        assert_eq!(card.reel, "A001");
        assert_eq!(card.clips, 2);
        assert_eq!(card.bytes, 2000);
        assert!(card.first_recorded.as_deref().unwrap().contains("09:00"));
        assert!(card.last_recorded.as_deref().unwrap().contains("17:30"));
        assert_eq!(i.not_included, 1);
    }

    #[test]
    fn inspection_serialises() {
        let (roots, scan, outcomes) = fixture();
        let v = serde_json::to_value(inspection(&roots, &scan, &outcomes)).unwrap();
        assert_eq!(v["cameras"][0]["cards"][0]["reel"], "A001");
        assert_eq!(v["sources"][0], "D:/Shoot");
    }
}

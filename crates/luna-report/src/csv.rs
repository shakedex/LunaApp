//! CSV: one row per clip, for spreadsheets and other tools.

use crate::model::{FIELD_LABELS, Report};

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// UTF-8 with a byte-order mark and CRLF line endings, so Excel reads it correctly.
/// Missing values are empty cells; the file size is an exact byte count.
pub fn write_csv(report: &Report) -> Vec<u8> {
    let mut out = BOM.to_vec();

    let mut header = vec!["Camera", "Reel"];
    // Every column but the formatted file size, which becomes an exact byte count.
    let text_columns = FIELD_LABELS.len() - 1;
    header.extend(&FIELD_LABELS[..text_columns]);
    header.push("File size (bytes)");
    push_row(&mut out, header.iter().map(|s| s.to_string()));

    for camera in &report.cameras {
        for card in &camera.cards {
            for clip in &card.clips {
                let values = clip.fields.display_values();
                let mut row = vec![camera.name.clone(), card.reel.clone()];
                row.extend(
                    values[..text_columns]
                        .iter()
                        .map(|v| v.clone().unwrap_or_default()),
                );
                row.push(
                    clip.fields
                        .file_size
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                );
                push_row(&mut out, row.into_iter());
            }
        }
    }
    out
}

fn push_row(out: &mut Vec<u8>, cells: impl Iterator<Item = String>) {
    let line: Vec<String> = cells.map(|c| quote(&c)).collect();
    out.extend(line.join(",").as_bytes());
    out.extend(b"\r\n");
}

/// RFC 4180: quote a cell that holds a comma, quote or line break; double inner quotes.
fn quote(cell: &str) -> String {
    if cell.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_owned()
    }
}

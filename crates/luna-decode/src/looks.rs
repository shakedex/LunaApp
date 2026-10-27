//! The look a Sony or ARRI camera embedded in an MXF clip, for "camera look":
//! Sony's `SONYBCUB` table (VENICE 2, BURANO) through our `sony-look` crate,
//! ARRI's look tables (ALEXA 35, ALEXA Mini LF) through `arri-look`. Both
//! crates were checked against the makers' own software.

use luna_formats::{ClipFiles, ColourHint, Container, EmbeddedLut, Gamut, Probe, Transfer};

use crate::util::{self, failed};

/// The clip's embedded look, or `None` when it has none or isn't MXF. A look
/// that can't be read is logged and left out. `probe` gives the exposure index
/// an ARRI LogC3 table is fed at. Never panics.
pub fn embedded_look(clip: &ClipFiles, probe: &Probe) -> Option<EmbeddedLut> {
    if clip.container != Container::Mxf {
        return None;
    }
    let path = &clip.main;
    let read = util::guard("look", || {
        if let Some(look) = sony_look::read(path).map_err(failed)? {
            return Ok(from_sony(look));
        }
        let ei = probe
            .meta
            .iso
            .as_deref()
            .and_then(|s| s.trim().parse().ok());
        Ok(arri_look::read(path)
            .map_err(failed)?
            .map(|look| from_arri(look, ei)))
    });
    read.unwrap_or_else(|e| {
        tracing::warn!("embedded look ignored: {e}");
        None
    })
}

fn from_sony(look: sony_look::Look) -> Option<EmbeddedLut> {
    // Every table seen so far takes 0–1; others would need rescaling first.
    if look.domain_min != [0.0; 3] || look.domain_max != [1.0; 3] {
        tracing::warn!(
            min = ?look.domain_min,
            max = ?look.domain_max,
            "Sony look {} has another input range, ignored",
            look.name
        );
        return None;
    }
    Some(EmbeddedLut {
        name: look.name,
        size: look.size,
        data: le_bytes(&look.table),
        input: ColourHint {
            transfer: Some(Transfer::SLog3),
            gamut: Some(Gamut::SGamut3Cine),
        },
        exposure_index: None,
    })
}

/// `ei`: the clip's exposure index. Mini LF tables take LogC3 at that EI.
fn from_arri(look: arri_look::Look, ei: Option<u32>) -> EmbeddedLut {
    let (input, exposure_index) = match look.input {
        arri_look::Input::LogC4 => ((Transfer::LogC4, Gamut::ArriWideGamut4), None),
        arri_look::Input::LogC3 => ((Transfer::LogC3, Gamut::ArriWideGamut3), ei),
    };
    EmbeddedLut {
        // The camera's own display table (what its monitor output showed).
        name: look.name.unwrap_or_else(|| "Embedded LUT".into()),
        size: look.display.size,
        data: le_bytes(&look.display.rows),
        input: ColourHint {
            transfer: Some(input.0),
            gamut: Some(input.1),
        },
        exposure_index,
    }
}

fn le_bytes(rows: &[[f32; 3]]) -> Vec<u8> {
    rows.iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

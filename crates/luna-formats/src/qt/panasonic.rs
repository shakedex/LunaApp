//! Panasonic (Lumix S/GH): the `com.panasonic.Semi-Pro.metadata.xml` key holds
//! a ClipMain XML (device, dates, ISO, capture gamma/gamut); the `PANA` udta
//! atom carries a preview JPEG whose Exif has exposure and serial.

use super::display;
use super::exif;
use super::meta::Items;
use crate::types::{ClipMeta, ColourHint, Gamut, Transfer};

pub(crate) const XML_KEY: &str = "com.panasonic.Semi-Pro.metadata.xml";

/// Reads the ClipMain XML. Elements are matched by local name, so the P2 and
/// Semi-Pro namespaces both work.
pub(crate) fn read_xml(xml: &str) -> Option<ClipMeta> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let find = |name: &str| {
        doc.descendants()
            .find(|n| n.is_element() && n.tag_name().name() == name)
            .and_then(|n| n.text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let mut m = ClipMeta {
        camera_make: find("Manufacturer"),
        camera_model: find("ModelName"),
        camera_serial: find("SerialNo."),
        iso: find("ISOSensitivity"),
        recording_date: find("StartDate")
            .or_else(|| find("CreationDate"))
            .and_then(|s| display::iso_local(&s)),
        ..ClipMeta::default()
    };
    m.colour = ColourHint {
        transfer: find("CaptureGamma").and_then(|g| transfer(&g)),
        gamut: find("CaptureGamut").and_then(|g| gamut(&g)),
    };
    Some(m)
}

fn norm(s: &str) -> String {
    s.to_ascii_uppercase().replace(['-', ' ', '_', '.'], "")
}

fn transfer(g: &str) -> Option<Transfer> {
    match norm(g).as_str() {
        "VLOG" => Some(Transfer::VLog),
        "HLG" => Some(Transfer::Hlg),
        "PQ" => Some(Transfer::Pq),
        "BT709" | "REC709" | "ITUR709" => Some(Transfer::Rec709),
        _ => None,
    }
}

fn gamut(g: &str) -> Option<Gamut> {
    match norm(g).as_str() {
        "VGAMUT" => Some(Gamut::VGamut),
        "BT2020" | "REC2020" => Some(Gamut::Rec2020),
        "BT709" | "REC709" => Some(Gamut::Rec709),
        _ => None,
    }
}

pub(crate) fn read(items: &Items, pana: Option<&[u8]>) -> Option<ClipMeta> {
    let xml = items.text(XML_KEY).and_then(|x| read_xml(&x));
    let exif = pana.and_then(exif::find_tiff).map(exif::read);
    match (xml, exif) {
        (None, None) => None,
        (Some(x), None) => Some(x),
        (None, Some(e)) => Some(e),
        (Some(mut x), Some(e)) => {
            super::fill(&mut x, e);
            Some(x)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<ClipMain xmlns="urn:schemas-Professional-Plug-in:Semi-Pro:ClipMetadata:v1.0">
  <ClipContent><ClipMetadata>
    <Access><CreationDate>2022-04-19T22:47:20-05:00</CreationDate></Access>
    <Device><Manufacturer>Panasonic</Manufacturer><ModelName>DC-S5</ModelName></Device>
    <Shoot><StartDate>2022-04-19T22:47:20-05:00</StartDate></Shoot>
  </ClipMetadata></ClipContent>
  <UserArea><AcquisitionMetadata xmlns="urn:schemas-Professional-Plug-in:P2:CameraMetadata:v1.2">
    <CameraUnitMetadata><ISOSensitivity>640</ISOSensitivity>
      <Gamma><CaptureGamma>V-Log</CaptureGamma></Gamma>
      <Gamut><CaptureGamut>V-Gamut</CaptureGamut></Gamut>
    </CameraUnitMetadata></AcquisitionMetadata></UserArea>
</ClipMain>"#;

    #[test]
    fn clip_main_xml() {
        let m = read_xml(XML).unwrap();
        assert_eq!(m.camera_make.as_deref(), Some("Panasonic"));
        assert_eq!(m.camera_model.as_deref(), Some("DC-S5"));
        assert_eq!(m.iso.as_deref(), Some("640"));
        assert_eq!(m.recording_date.as_deref(), Some("2022-04-19 22:47:20"));
        assert_eq!(m.colour.transfer, Some(Transfer::VLog));
        assert_eq!(m.colour.gamut, Some(Gamut::VGamut));
    }

    #[test]
    fn broken_xml_is_none() {
        assert!(read_xml("<ClipMain><unclosed>").is_none());
    }
}

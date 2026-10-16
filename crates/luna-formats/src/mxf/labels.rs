//! What the descriptor's universal labels mean: codec, transfer, primaries.

use super::klv::Ul;
use crate::types::{Codec, Gamut, ProResProfile, Transfer};

const SMPTE: [u8; 4] = [0x06, 0x0e, 0x2b, 0x34];

fn at(u: &Ul, from: usize, bytes: &[u8]) -> bool {
    u.0.get(from..from + bytes.len()) == Some(bytes)
}

/// Codec from the picture essence coding label, the essence container label
/// and the picture element key, in that order of trust.
pub(crate) fn codec(coding: Option<&Ul>, container: Option<&Ul>, element: Option<&Ul>) -> Codec {
    if let Some(c) = coding.filter(|c| c.0[..4] == SMPTE) {
        // RDD 36/44 ProRes: 04 01 02 02 03 06 pp
        if at(c, 8, &[0x04, 0x01, 0x02, 0x02, 0x03, 0x06]) {
            let p = match c.0[14] {
                0x01 => Some(ProResProfile::Proxy),
                0x02 => Some(ProResProfile::Lt),
                0x03 => Some(ProResProfile::Standard),
                0x04 => Some(ProResProfile::Hq),
                0x05 => Some(ProResProfile::P4444),
                0x06 => Some(ProResProfile::P4444Xq),
                _ => None,
            };
            if let Some(p) = p {
                return Codec::ProRes(p);
            }
            return Codec::Other("ProRes".into());
        }
        // ARRICORE (ARRI-registered, …04 01 02 02 03 0a …)
        if at(c, 8, &[0x04, 0x01, 0x02, 0x02, 0x03, 0x0a]) {
            return Codec::ArriCore;
        }
        // Sony X-OCN: 0e 06 04 01 02 06 …
        if at(c, 8, &[0x0e, 0x06, 0x04, 0x01, 0x02, 0x06]) {
            return Codec::XOcn;
        }
        // ST 381-3 MPEG-4 AVC: 04 01 02 02 01 31 (long GOP) / 32 (intra)
        if at(c, 8, &[0x04, 0x01, 0x02, 0x02, 0x01]) && matches!(c.0[13], 0x31 | 0x32) {
            return Codec::H264;
        }
    }
    if let Some(c) = container.filter(|c| c.0[..4] == SMPTE) {
        // ST 2071-2/RDD 54 ARRIRAW and ARRICORE generic-container mappings.
        if at(c, 8, &[0x0d, 0x01, 0x03, 0x01, 0x02, 0x22]) {
            return Codec::ArriRaw;
        }
        if at(c, 8, &[0x0d, 0x01, 0x03, 0x01, 0x02, 0x27]) {
            return Codec::ArriCore;
        }
        if at(c, 8, &[0x0d, 0x01, 0x03, 0x01, 0x02, 0x1c]) {
            return Codec::Other("ProRes".into());
        }
        if at(c, 8, &[0x0e, 0x06, 0x0d, 0x03, 0x02]) {
            return Codec::XOcn;
        }
    }
    if let Some(k) = element {
        if at(k, 8, &[0x0d, 0x01, 0x03, 0x01, 0x15, 0x01, 0x1c])
            || at(k, 8, &[0x0f, 0x01, 0x03, 0x01, 0x01, 0x01, 0x01])
        {
            return Codec::ArriRaw;
        }
        if at(k, 8, &[0x0d, 0x01, 0x03, 0x01, 0x15, 0x01, 0x20]) {
            return Codec::ArriCore;
        }
    }
    let name = coding
        .or(container)
        .map(|u| format!("MXF {}", u.hex()))
        .unwrap_or_else(|| "MXF (unknown picture coding)".into());
    Codec::Other(name)
}

/// TransferCharacteristic label → transfer function.
pub(crate) fn transfer(u: &Ul) -> Option<Transfer> {
    if u.0[..4] != SMPTE {
        return None;
    }
    // SMPTE: 04 01 01 01 01 xx
    if at(u, 8, &[0x04, 0x01, 0x01, 0x01, 0x01]) {
        return match u.0[13] {
            0x02 => Some(Transfer::Rec709),
            0x06 => Some(Transfer::Linear),
            0x0a => Some(Transfer::Pq),
            0x0b => Some(Transfer::Hlg),
            _ => None,
        };
    }
    // Sony private: 0e 06 04 01 01 01 06 xx (S-Log3 = 05, as the XML pairs it).
    if at(u, 8, &[0x0e, 0x06, 0x04, 0x01, 0x01, 0x01, 0x06, 0x05]) {
        return Some(Transfer::SLog3);
    }
    // ARRI private, as written by ALEXA 35 (LogC4) and ALEXA Mini LF (LogC3).
    if at(u, 8, &[0x0e, 0x17, 0x01, 0x02, 0x04, 0x02]) {
        return Some(Transfer::LogC4);
    }
    if at(u, 8, &[0x0e, 0x17, 0x04, 0x01, 0x03, 0x01, 0x02, 0x01]) {
        return Some(Transfer::LogC3);
    }
    // Canon private: 0e 15 00 01 xx. Premiere's MXF importer lists Canon's
    // transfers 01, 02, 07 against Canon Log, Canon Log 2, Canon Log 3; the C50
    // MP4's own Canon box agrees (CNLG = 2 with label 02).
    if at(u, 8, &[0x0e, 0x15, 0x00, 0x01]) {
        return match u.0[12] {
            0x02 => Some(Transfer::CanonLog2),
            0x07 => Some(Transfer::CanonLog3),
            _ => None,
        };
    }
    None
}

/// ColorPrimaries label → gamut.
pub(crate) fn gamut(u: &Ul) -> Option<Gamut> {
    if u.0[..4] != SMPTE {
        return None;
    }
    // SMPTE: 04 01 01 01 03 xx
    if at(u, 8, &[0x04, 0x01, 0x01, 0x01, 0x03]) {
        return match u.0[13] {
            0x03 => Some(Gamut::Rec709),
            0x04 => Some(Gamut::Rec2020),
            _ => None,
        };
    }
    // Sony private: 0e 06 04 01 01 03 01 xx (S-Gamut3.Cine = 05, S-Gamut3 = 04).
    if at(u, 8, &[0x0e, 0x06, 0x04, 0x01, 0x01, 0x03, 0x01]) {
        return match u.0[15] {
            0x05 => Some(Gamut::SGamut3Cine),
            0x04 => Some(Gamut::SGamut3),
            _ => None,
        };
    }
    if at(u, 8, &[0x0e, 0x17, 0x01, 0x02, 0x03, 0x02]) {
        return Some(Gamut::ArriWideGamut4);
    }
    if at(u, 8, &[0x0e, 0x17, 0x04, 0x01, 0x03, 0x01, 0x03, 0x01]) {
        return Some(Gamut::ArriWideGamut3);
    }
    // Canon private: 0e 15 00 02 xx; 02 sits with the camera-native gamuts in
    // Premiere's MXF importer and is what Cinema EOS writes beside Canon Log.
    if at(u, 8, &[0x0e, 0x15, 0x00, 0x02, 0x02]) {
        return Some(Gamut::CinemaGamut);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mxf::klv::ul;

    #[test]
    fn prores_profiles_from_coding_label() {
        let c = ul("060e2b340401010d0401020203060500");
        assert_eq!(
            codec(Some(&c), None, None),
            Codec::ProRes(ProResProfile::P4444)
        );
        let c = ul("060e2b340401010d0401020203060300");
        assert_eq!(
            codec(Some(&c), None, None),
            Codec::ProRes(ProResProfile::Standard)
        );
    }

    #[test]
    fn vendor_codecs() {
        let xocn = ul("060e2b34040101060e06040102060601");
        assert_eq!(codec(Some(&xocn), None, None), Codec::XOcn);
        let avc = ul("060e2b340401010a0401020201323001");
        assert_eq!(codec(Some(&avc), None, None), Codec::H264);
        let raw = ul("060e2b340401010d0401020102010103");
        let cont = ul("060e2b340401010d0d01030102220100");
        assert_eq!(codec(Some(&raw), Some(&cont), None), Codec::ArriRaw);
        let core = ul("060e2b340401010d04010202030a0101");
        assert_eq!(codec(Some(&core), None, None), Codec::ArriCore);
    }

    #[test]
    fn colour_labels() {
        assert_eq!(
            transfer(&ul("060e2b34040101010401010101020000")),
            Some(Transfer::Rec709)
        );
        assert_eq!(
            transfer(&ul("060e2b34040101060e06040101010605")),
            Some(Transfer::SLog3)
        );
        assert_eq!(
            gamut(&ul("060e2b34040101060e06040101030105")),
            Some(Gamut::SGamut3Cine)
        );
        assert_eq!(
            transfer(&ul("060e2b340401010d0e17010204020000")),
            Some(Transfer::LogC4)
        );
        assert_eq!(
            gamut(&ul("060e2b340401010d0e17040103010301")),
            Some(Gamut::ArriWideGamut3)
        );
        assert_eq!(
            transfer(&ul("060e2b340401010d0e15000102000000")),
            Some(Transfer::CanonLog2)
        );
        assert_eq!(
            transfer(&ul("060e2b340401010d0e15000107000000")),
            Some(Transfer::CanonLog3)
        );
        // Original Canon Log: no curve to render it with.
        assert_eq!(transfer(&ul("060e2b340401010d0e15000101000000")), None);
        assert_eq!(
            gamut(&ul("060e2b340401010d0e15000202000000")),
            Some(Gamut::CinemaGamut)
        );
    }
}

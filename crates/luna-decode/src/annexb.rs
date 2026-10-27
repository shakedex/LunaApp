//! HEVC access units in Annex B form (start-code delimited NAL units), as
//! XAVC H stores each frame in MXF. Shared by the platform decoders.

/// The NAL units of an access unit, start codes removed. Trailing zero bytes
/// belong to the next start code (`00 00 00 01`), so they are trimmed.
pub(crate) fn nal_units(au: &[u8]) -> Vec<&[u8]> {
    let starts: Vec<usize> = au
        .windows(3)
        .enumerate()
        .filter(|(_, w)| *w == [0, 0, 1])
        .map(|(i, _)| i + 3)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| {
            let end = starts.get(n + 1).map_or(au.len(), |&next| next - 3);
            let mut nal = &au[s..end.max(s)];
            while let [rest @ .., 0] = nal {
                nal = rest;
            }
            nal
        })
        .filter(|nal| nal.len() >= 2)
        .collect()
}

/// The HEVC NAL unit type (H.265 7.4.2.2).
pub(crate) fn nal_type(nal: &[u8]) -> u8 {
    (nal[0] >> 1) & 0x3f
}

/// NAL unit types in an Annex B access unit (start codes `00 00 01`).
fn nal_types(au: &[u8]) -> impl Iterator<Item = u8> + '_ {
    au.windows(4)
        .filter(|w| w[..3] == [0, 0, 1])
        .map(|w| (w[3] >> 1) & 0x3f)
}

/// Whether the access unit holds an IRAP (IDR, CRA or BLA) picture.
pub(crate) fn has_irap(au: &[u8]) -> bool {
    nal_types(au).any(|t| (16..=23).contains(&t))
}

/// "HEVC 4:2:2 (Range Extensions)" etc. from the SPS profile, for messages.
pub(crate) fn profile_label(au: &[u8]) -> Option<String> {
    let at = au
        .windows(4)
        .position(|w| w[..3] == [0, 0, 1] && (w[3] >> 1) & 0x3f == 33)?;
    // NAL header (2 bytes), then vps id / sub-layers / nesting (1 byte), then
    // profile_space(2) tier(1) profile_idc(5).
    let p = *au.get(at + 3 + 3)? & 0x1f;
    let name = match p {
        1 => "Main",
        2 => "Main 10",
        4 => "4:2:2 (Range Extensions)",
        _ => return Some("This HEVC profile".into()),
    };
    Some(format!("HEVC {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // AUD, VPS, SPS (profile byte 0x24: high tier, RExt), IDR_W_RADL
    const AU: [u8; 29] = [
        0, 0, 0, 1, 0x46, 0x01, 0x10, 0, 0, 0, 1, 0x40, 0x01, 0x0c, 0, 0, 0, 1, 0x42, 0x01, 0x01,
        0x24, 0, 0, 0, 1, 0x26, 0x01, 0xaf,
    ];

    #[test]
    fn finds_irap_and_profile() {
        assert!(has_irap(&AU));
        assert_eq!(
            profile_label(&AU).as_deref(),
            Some("HEVC 4:2:2 (Range Extensions)")
        );
        // a trailing (TRAIL_R) picture only
        assert!(!has_irap(&[0, 0, 1, 0x02, 0x01, 0xaf]));
    }

    #[test]
    fn splits_nal_units() {
        let nals = nal_units(&AU);
        assert_eq!(
            nals.iter().map(|n| nal_type(n)).collect::<Vec<_>>(),
            [35, 32, 33, 19]
        );
        assert_eq!(nals[0], [0x46, 0x01, 0x10]);
        assert_eq!(nals[2], [0x42, 0x01, 0x01, 0x24]);
        assert_eq!(nals[3], [0x26, 0x01, 0xaf]);
        assert!(nal_units(&[1, 2, 3]).is_empty());
    }
}

//! Frame header and picture header parsing (RDD 36 frame/picture syntax).

use crate::Error;

fn corrupt(msg: &str) -> Error {
    Error::Corrupt(msg.into())
}

fn be16(b: &[u8], at: usize) -> usize {
    u16::from_be_bytes([b[at], b[at + 1]]) as usize
}

fn be32(b: &[u8], at: usize) -> usize {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize
}

/// How the frame's pictures map onto lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Interlace {
    Progressive,
    /// Two field pictures; the first holds the top (even) lines.
    TopFieldFirst,
    /// Two field pictures; the first holds the bottom (odd) lines.
    BottomFieldFirst,
}

#[derive(Debug, Clone)]
pub(crate) struct FrameHeader {
    pub width: usize,
    pub height: usize,
    /// 2 = 4:2:2, 3 = 4:4:4.
    pub chroma_format: u8,
    pub interlace: Interlace,
    pub color_primaries: u8,
    pub transfer_characteristic: u8,
    pub matrix_coefficients: u8,
    /// 0 = none, otherwise alpha is coded after Cr in every slice.
    pub alpha_type: u8,
    pub qmat_luma: [u8; 64],
    pub qmat_chroma: [u8; 64],
}

/// Parses the frame container and header. Returns the header and the bytes
/// holding the picture(s).
pub(crate) fn parse_frame(frame: &[u8]) -> Result<(FrameHeader, &[u8]), Error> {
    if frame.len() < 8 {
        return Err(corrupt("shorter than the frame container"));
    }
    if &frame[4..8] != b"icpf" {
        return Err(corrupt("missing icpf frame identifier"));
    }
    let frame_size = be32(frame, 0);
    if frame_size < 8 || frame_size > frame.len() {
        return Err(corrupt("frame size exceeds the data"));
    }
    let frame = &frame[..frame_size];
    let hdr = &frame[8..];
    if hdr.len() < 20 {
        return Err(corrupt("frame header truncated"));
    }
    let header_size = be16(hdr, 0);
    if header_size < 20 || header_size > hdr.len() {
        return Err(corrupt("bad frame header size"));
    }
    let version = hdr[3];
    if version > 1 {
        return Err(Error::Unsupported(format!("bitstream version {version}")));
    }
    let width = be16(hdr, 8);
    let height = be16(hdr, 10);
    if width == 0 || height == 0 {
        return Err(corrupt("zero picture dimension"));
    }
    let chroma_format = hdr[12] >> 6;
    let interlace = match (hdr[12] >> 2) & 3 {
        0 => Interlace::Progressive,
        1 => Interlace::TopFieldFirst,
        2 => Interlace::BottomFieldFirst,
        _ => return Err(corrupt("reserved interlace mode")),
    };
    if chroma_format != 2 && chroma_format != 3 {
        return Err(Error::Unsupported(format!(
            "chroma format code {chroma_format}"
        )));
    }
    let alpha_type = hdr[17] & 0xf;
    if alpha_type > 2 {
        return Err(Error::Unsupported(format!(
            "alpha channel type {alpha_type}"
        )));
    }
    let load_luma = hdr[19] & 2 != 0;
    let load_chroma = hdr[19] & 1 != 0;
    let mut at = 20;
    let read_matrix = |at: &mut usize| -> Result<[u8; 64], Error> {
        let m: [u8; 64] = hdr
            .get(*at..*at + 64)
            .filter(|_| *at + 64 <= header_size)
            .ok_or_else(|| corrupt("quantisation matrix truncated"))?
            .try_into()
            .unwrap();
        if m.contains(&0) {
            return Err(corrupt("zero quantisation matrix entry"));
        }
        *at += 64;
        Ok(m)
    };
    let qmat_luma = if load_luma {
        read_matrix(&mut at)?
    } else {
        [4; 64]
    };
    let qmat_chroma = if load_chroma {
        read_matrix(&mut at)?
    } else {
        qmat_luma
    };
    Ok((
        FrameHeader {
            width,
            height,
            chroma_format,
            interlace,
            color_primaries: hdr[14],
            transfer_characteristic: hdr[15],
            matrix_coefficients: hdr[16],
            alpha_type,
            qmat_luma,
            qmat_chroma,
        },
        &hdr[header_size..],
    ))
}

/// One picture's slice layout: where every slice's bytes are and how many
/// macroblocks wide each is.
#[derive(Debug)]
pub(crate) struct PictureLayout<'a> {
    /// Total bytes of this picture (header + table + slices).
    pub size: usize,
    pub mb_height: usize,
    /// Slice widths in macroblocks for one row, left to right.
    pub row_slices: Vec<usize>,
    /// Coded bytes of each slice, in raster order.
    pub slices: Vec<&'a [u8]>,
}

/// Slice widths for one macroblock row: as many full-size slices as fit,
/// then successively halved slices for the remainder.
pub(crate) fn row_slice_widths(mb_width: usize, log2_slice: u32) -> Vec<usize> {
    let mut widths = Vec::new();
    let mut size = 1usize << log2_slice;
    let mut left = mb_width;
    while left > 0 {
        while size > left {
            size >>= 1;
        }
        widths.push(size);
        left -= size;
    }
    widths
}

/// Parses one picture header and slice table from the start of `data`.
pub(crate) fn parse_picture(
    data: &[u8],
    width: usize,
    mb_height: usize,
) -> Result<PictureLayout<'_>, Error> {
    if data.len() < 8 {
        return Err(corrupt("picture header truncated"));
    }
    let header_size = (data[0] >> 3) as usize;
    if header_size < 8 {
        return Err(corrupt("bad picture header size"));
    }
    let size = be32(data, 1);
    if size < header_size || size > data.len() {
        return Err(corrupt("picture size exceeds the frame"));
    }
    let data = &data[..size];
    let log2_slice = ((data[7] >> 4) & 3) as u32;
    let mb_width = width.div_ceil(16);
    let row_slices = row_slice_widths(mb_width, log2_slice);
    let count = row_slices.len() * mb_height;
    let table_end = header_size + 2 * count;
    if table_end > size {
        return Err(corrupt("slice table exceeds the picture"));
    }
    let mut slices = Vec::with_capacity(count);
    let mut at = table_end;
    for i in 0..count {
        let len = be16(data, header_size + 2 * i);
        let slice = data
            .get(at..at + len)
            .ok_or_else(|| corrupt("slice exceeds the picture"))?;
        slices.push(slice);
        at += len;
    }
    Ok(PictureLayout {
        size,

        mb_height,
        row_slices,
        slices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_slices_split_the_remainder_in_powers_of_two() {
        assert_eq!(row_slice_widths(13, 2), [4, 4, 4, 1]);
        assert_eq!(row_slice_widths(13, 3), [8, 4, 1]);
        assert_eq!(row_slice_widths(120, 3).len(), 15);
        assert_eq!(row_slice_widths(7, 0), [1; 7]);
        assert_eq!(row_slice_widths(15, 3), [8, 4, 2, 1]);
    }
}

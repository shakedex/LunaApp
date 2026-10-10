//! Sequence and picture parameter sets (7.3.2.1, 7.3.2.2) and the scaling
//! matrices they carry (7.4.2.1.1, Table 7-2 fall-back rules).

use crate::Error;
use crate::bits::BitReader;
use crate::tables::{DEFAULT_4X4_INTER, DEFAULT_4X4_INTRA, DEFAULT_8X8_INTER, DEFAULT_8X8_INTRA};

/// Largest picture we accept, in macroblocks (level 6.2's MaxFS).
const MAX_MBS: u32 = 139_264;

/// One scaling list as signalled.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ListSpec {
    NotPresent,
    UseDefault,
    /// Values in zig-zag order (16 or 64 of them).
    Explicit(Vec<u8>),
}

/// The scaling matrices in force for a picture, in zig-zag order.
/// `l4`: Intra Y, Intra Cb, Intra Cr, Inter Y, Inter Cb, Inter Cr.
/// `l8`: Intra Y, Inter Y, Intra Cb, Inter Cb, Intra Cr, Inter Cr.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScalingLists {
    pub l4: [[u8; 16]; 6],
    pub l8: [[u8; 64]; 6],
}

impl ScalingLists {
    pub(crate) fn flat() -> ScalingLists {
        ScalingLists {
            l4: [[16; 16]; 6],
            l8: [[16; 64]; 6],
        }
    }
}

fn parse_list(r: &mut BitReader, size: usize) -> Result<ListSpec, Error> {
    let mut list = vec![0u8; size];
    let mut last = 8i32;
    let mut next = 8i32;
    for j in 0..size {
        if next != 0 {
            let delta = r.se_range(-128, 127, "delta_scale")?;
            next = (last + delta + 256) % 256;
            if j == 0 && next == 0 {
                return Ok(ListSpec::UseDefault);
            }
        }
        let v = if next == 0 { last } else { next };
        list[j] = v as u8;
        last = v;
    }
    Ok(ListSpec::Explicit(list))
}

fn parse_lists(r: &mut BitReader, count: usize) -> Result<Vec<ListSpec>, Error> {
    let mut v = Vec::with_capacity(count);
    for i in 0..count {
        if r.flag() {
            v.push(parse_list(r, if i < 6 { 16 } else { 64 })?);
        } else {
            v.push(ListSpec::NotPresent);
        }
    }
    Ok(v)
}

/// Applies a set of signalled lists over `fallback` (rule A when `fallback`
/// holds the defaults, rule B when it holds the sequence-level lists).
fn resolve(specs: &[ListSpec], fallback: &ScalingLists) -> ScalingLists {
    let mut out = ScalingLists::flat();
    for i in 0..6 {
        out.l4[i] = match specs.get(i).unwrap_or(&ListSpec::NotPresent) {
            ListSpec::Explicit(v) => v.as_slice().try_into().unwrap_or([16; 16]),
            ListSpec::UseDefault => {
                if i < 3 {
                    DEFAULT_4X4_INTRA
                } else {
                    DEFAULT_4X4_INTER
                }
            }
            ListSpec::NotPresent => {
                if i == 0 || i == 3 {
                    fallback.l4[i]
                } else {
                    out.l4[i - 1]
                }
            }
        };
    }
    for k in 0..6 {
        out.l8[k] = match specs.get(6 + k).unwrap_or(&ListSpec::NotPresent) {
            ListSpec::Explicit(v) => v.as_slice().try_into().unwrap_or([16; 64]),
            ListSpec::UseDefault => {
                if k % 2 == 0 {
                    DEFAULT_8X8_INTRA
                } else {
                    DEFAULT_8X8_INTER
                }
            }
            ListSpec::NotPresent => {
                if k < 2 {
                    fallback.l8[k]
                } else {
                    out.l8[k - 2]
                }
            }
        };
    }
    out
}

fn defaults() -> ScalingLists {
    let mut d = ScalingLists::flat();
    d.l4[0] = DEFAULT_4X4_INTRA;
    d.l4[3] = DEFAULT_4X4_INTER;
    d.l8[0] = DEFAULT_8X8_INTRA;
    d.l8[1] = DEFAULT_8X8_INTER;
    d
}

/// Colour description from the VUI (ITU-T H.273 codes; 2 = unspecified).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour {
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    pub full_range: bool,
}

impl Default for Colour {
    fn default() -> Self {
        Colour {
            primaries: 2,
            transfer: 2,
            matrix: 2,
            full_range: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Sps {
    pub profile_idc: u8,
    pub id: u32,
    pub chroma_format_idc: u32,
    pub bit_depth_luma: u32,
    pub bit_depth_chroma: u32,
    pub transform_bypass: bool,
    /// Sequence-level matrices (Flat_16 when not signalled).
    pub scaling: ScalingLists,
    /// seq_scaling_matrix_present_flag (selects the PPS fall-back rule).
    pub scaling_present: bool,
    pub log2_max_frame_num: u32,
    pub poc_type: u32,
    pub log2_max_poc_lsb: u32,
    pub delta_pic_order_always_zero: bool,
    pub width_mbs: u32,
    pub height_map_units: u32,
    pub frame_mbs_only: bool,
    pub mbaff: bool,
    /// frame_crop_left/right/top/bottom_offset.
    pub crop: [u32; 4],
    pub colour: Colour,
}

impl Sps {
    pub(crate) fn height_mbs(&self) -> u32 {
        self.height_map_units * if self.frame_mbs_only { 1 } else { 2 }
    }

    /// (SubWidthC, SubHeightC).
    pub(crate) fn chroma_sub(&self) -> (u32, u32) {
        match self.chroma_format_idc {
            1 => (2, 2),
            2 => (2, 1),
            _ => (1, 1),
        }
    }

    pub(crate) fn parse(r: &mut BitReader) -> Result<Sps, Error> {
        let profile_idc = r.u(8) as u8;
        r.skip(16); // constraint flags, reserved, level_idc
        let id = r.ue_max(31, "seq_parameter_set_id")?;
        let mut chroma_format_idc = 1;
        let mut bit_depth_luma = 8;
        let mut bit_depth_chroma = 8;
        let mut transform_bypass = false;
        let mut scaling = ScalingLists::flat();
        let mut scaling_present = false;
        if matches!(
            profile_idc,
            100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
        ) {
            chroma_format_idc = r.ue_max(3, "chroma_format_idc")?;
            if chroma_format_idc == 3 && r.flag() {
                return Err(Error::Unsupported("separate colour planes (4:4:4)".into()));
            }
            bit_depth_luma = 8 + r.ue_max(6, "bit_depth_luma_minus8")?;
            bit_depth_chroma = 8 + r.ue_max(6, "bit_depth_chroma_minus8")?;
            transform_bypass = r.flag();
            if r.flag() {
                let n = if chroma_format_idc != 3 { 8 } else { 12 };
                let specs = parse_lists(r, n)?;
                scaling = resolve(&specs, &defaults());
                scaling_present = true;
            }
        }
        let log2_max_frame_num = 4 + r.ue_max(12, "log2_max_frame_num_minus4")?;
        let poc_type = r.ue_max(2, "pic_order_cnt_type")?;
        let mut log2_max_poc_lsb = 0;
        let mut delta_pic_order_always_zero = false;
        if poc_type == 0 {
            log2_max_poc_lsb = 4 + r.ue_max(12, "log2_max_pic_order_cnt_lsb_minus4")?;
        } else if poc_type == 1 {
            delta_pic_order_always_zero = r.flag();
            r.se()?;
            r.se()?;
            let n = r.ue_max(255, "num_ref_frames_in_pic_order_cnt_cycle")?;
            for _ in 0..n {
                r.se()?;
            }
        }
        r.ue()?; // max_num_ref_frames
        r.skip(1); // gaps_in_frame_num_value_allowed_flag
        let width_mbs = r.ue_max(MAX_MBS, "pic_width_in_mbs_minus1")? + 1;
        let height_map_units = r.ue_max(MAX_MBS, "pic_height_in_map_units_minus1")? + 1;
        let frame_mbs_only = r.flag();
        let mbaff = if frame_mbs_only { false } else { r.flag() };
        r.skip(1); // direct_8x8_inference_flag
        let mut crop = [0; 4];
        if r.flag() {
            for c in &mut crop {
                *c = r.ue_max(1 << 16, "frame_crop_offset")?;
            }
        }
        let mut colour = Colour::default();
        if r.flag() {
            if r.flag() && r.u(8) == 255 {
                r.skip(32); // sar_width, sar_height
            }
            if r.flag() {
                r.skip(1); // overscan_appropriate_flag
            }
            if r.flag() {
                r.skip(3); // video_format
                colour.full_range = r.flag();
                if r.flag() {
                    colour.primaries = r.u(8) as u8;
                    colour.transfer = r.u(8) as u8;
                    colour.matrix = r.u(8) as u8;
                }
            }
        }
        r.check()?;
        let sps = Sps {
            profile_idc,
            id,
            chroma_format_idc,
            bit_depth_luma,
            bit_depth_chroma,
            transform_bypass,
            scaling,
            scaling_present,
            log2_max_frame_num,
            poc_type,
            log2_max_poc_lsb,
            delta_pic_order_always_zero,
            width_mbs,
            height_map_units,
            frame_mbs_only,
            mbaff,
            crop,
            colour,
        };
        if sps.width_mbs as u64 * sps.height_mbs() as u64 > MAX_MBS as u64 {
            return Err(Error::Unsupported(format!(
                "a {}×{} macroblock picture is larger than any level allows",
                sps.width_mbs,
                sps.height_mbs()
            )));
        }
        let (sw, sh) = sps.chroma_sub();
        let crop_y = sh * if frame_mbs_only { 1 } else { 2 };
        if (crop[0] + crop[1]) * sw >= width_mbs * 16
            || (crop[2] + crop[3]) * crop_y >= sps.height_mbs() * 16
        {
            return Err(Error::corrupt(
                "the frame cropping removes the whole picture",
            ));
        }
        Ok(sps)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Pps {
    pub id: u32,
    pub sps_id: u32,
    pub cabac: bool,
    pub bottom_field_pic_order_in_frame_present: bool,
    pub num_ref_idx_l0_default: u32,
    pub num_ref_idx_l1_default: u32,
    pub weighted_pred: bool,
    pub weighted_bipred_idc: u32,
    pub pic_init_qp: i32,
    pub chroma_qp_offset: [i32; 2],
    pub deblocking_filter_control_present: bool,
    pub constrained_intra_pred: bool,
    pub redundant_pic_cnt_present: bool,
    pub transform_8x8_mode: bool,
    /// Matrices in force (picture-level over sequence-level).
    pub scaling: ScalingLists,
}

impl Pps {
    /// The PPS id and the SPS id it refers to, without parsing the rest.
    pub(crate) fn ids(r: &mut BitReader) -> Result<(u32, u32), Error> {
        let id = r.ue_max(255, "pic_parameter_set_id")?;
        let sps_id = r.ue_max(31, "seq_parameter_set_id")?;
        Ok((id, sps_id))
    }

    pub(crate) fn parse(r: &mut BitReader, sps: &Sps) -> Result<Pps, Error> {
        let (id, sps_id) = Pps::ids(r)?;
        let cabac = r.flag();
        let bottom_field_pic_order_in_frame_present = r.flag();
        if r.ue()? != 0 {
            return Err(Error::Unsupported(
                "slice groups (flexible macroblock ordering)".into(),
            ));
        }
        let num_ref_idx_l0_default = r.ue_max(31, "num_ref_idx_l0_default_active_minus1")? + 1;
        let num_ref_idx_l1_default = r.ue_max(31, "num_ref_idx_l1_default_active_minus1")? + 1;
        let weighted_pred = r.flag();
        let weighted_bipred_idc = r.u(2);
        let qp_bd = 6 * (sps.bit_depth_luma as i32 - 8);
        let pic_init_qp = 26 + r.se_range(-26 - qp_bd, 25, "pic_init_qp_minus26")?;
        r.se()?; // pic_init_qs_minus26
        let c0 = r.se_range(-12, 12, "chroma_qp_index_offset")?;
        let deblocking_filter_control_present = r.flag();
        let constrained_intra_pred = r.flag();
        let redundant_pic_cnt_present = r.flag();
        let mut transform_8x8_mode = false;
        let mut scaling = sps.scaling.clone();
        let mut c1 = c0;
        if r.more_rbsp_data() {
            transform_8x8_mode = r.flag();
            if r.flag() {
                let per_8x8 = if sps.chroma_format_idc != 3 { 2 } else { 6 };
                let n = 6 + per_8x8 * transform_8x8_mode as usize;
                let specs = parse_lists(r, n)?;
                // Rule A over the defaults without sequence matrices,
                // rule B over the sequence matrices with them (Table 7-2).
                let fallback = if sps.scaling_present {
                    sps.scaling.clone()
                } else {
                    defaults()
                };
                scaling = resolve(&specs, &fallback);
            }
            c1 = r.se_range(-12, 12, "second_chroma_qp_index_offset")?;
        }
        r.check()?;
        Ok(Pps {
            id,
            sps_id,
            cabac,
            bottom_field_pic_order_in_frame_present,
            num_ref_idx_l0_default,
            num_ref_idx_l1_default,
            weighted_pred,
            weighted_bipred_idc,
            pic_init_qp,
            chroma_qp_offset: [c0, c1],
            deblocking_filter_control_present,
            constrained_intra_pred,
            redundant_pic_cnt_present,
            transform_8x8_mode,
            scaling,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fall_back_rule_a_and_b() {
        // SPS: only list 1 explicit (rule A): list 0 default intra, list 2 copies 1.
        let mut specs = vec![ListSpec::NotPresent; 8];
        specs[1] = ListSpec::Explicit(vec![20; 16]);
        specs[7] = ListSpec::UseDefault;
        let a = resolve(&specs, &defaults());
        assert_eq!(a.l4[0], DEFAULT_4X4_INTRA);
        assert_eq!(a.l4[1], [20; 16]);
        assert_eq!(a.l4[2], [20; 16]);
        assert_eq!(a.l4[3], DEFAULT_4X4_INTER);
        assert_eq!(a.l4[5], DEFAULT_4X4_INTER);
        assert_eq!(a.l8[0], DEFAULT_8X8_INTRA);
        assert_eq!(a.l8[1], DEFAULT_8X8_INTER);
        // PPS (rule B): list 0 absent falls back to the sequence list 0.
        let mut seq = ScalingLists::flat();
        seq.l4[0] = [7; 16];
        seq.l8[0] = [9; 64];
        let b = resolve(&vec![ListSpec::NotPresent; 8], &seq);
        assert_eq!(b.l4[0], [7; 16]);
        assert_eq!(b.l4[2], [7; 16]);
        assert_eq!(b.l4[3], [16; 16]);
        assert_eq!(b.l8[0], [9; 64]);
    }
}

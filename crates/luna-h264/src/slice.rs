//! Slice headers (7.3.3) of intra slices.

use crate::Error;
use crate::bits::BitReader;
use crate::params::{Pps, Sps};

/// slice_type % 5.
pub(crate) const SLICE_P: u32 = 0;
pub(crate) const SLICE_B: u32 = 1;
pub(crate) const SLICE_I: u32 = 2;
pub(crate) const SLICE_SP: u32 = 3;
pub(crate) const SLICE_SI: u32 = 4;

/// The first three fields, enough to classify a slice.
pub(crate) fn peek(r: &mut BitReader) -> Result<(u32, u32, u32), Error> {
    let first_mb = r.ue()?;
    let slice_type = r.ue_max(9, "slice_type")? % 5;
    let pps_id = r.ue_max(255, "pic_parameter_set_id")?;
    Ok((first_mb, slice_type, pps_id))
}

#[derive(Debug, Clone)]
pub(crate) struct SliceHeader {
    pub first_mb: u32,
    pub slice_type: u32,
    pub frame_num: u32,
    pub idr_pic_id: Option<u32>,
    /// SliceQPY.
    pub qp: i32,
    pub disable_deblocking: u32,
    /// FilterOffsetA / FilterOffsetB.
    pub alpha_offset: i32,
    pub beta_offset: i32,
}

/// Parses the rest of an I or SI slice header after [`peek`].
pub(crate) fn parse_intra(
    r: &mut BitReader,
    first_mb: u32,
    slice_type: u32,
    idr: bool,
    ref_idc: u8,
    sps: &Sps,
    pps: &Pps,
) -> Result<SliceHeader, Error> {
    let frame_num = r.u(sps.log2_max_frame_num);
    if !sps.frame_mbs_only && r.flag() {
        return Err(Error::Unsupported("field pictures".into()));
    }
    let idr_pic_id = if idr { Some(r.ue()?) } else { None };
    if sps.poc_type == 0 {
        r.skip(sps.log2_max_poc_lsb);
        if pps.bottom_field_pic_order_in_frame_present {
            r.se()?;
        }
    } else if sps.poc_type == 1 && !sps.delta_pic_order_always_zero {
        r.se()?;
        if pps.bottom_field_pic_order_in_frame_present {
            r.se()?;
        }
    }
    if pps.redundant_pic_cnt_present && r.ue()? != 0 {
        return Err(Error::Unsupported("redundant pictures".into()));
    }
    // I/SI slices have no reference list modification or weights.
    if ref_idc != 0 {
        if idr {
            r.skip(2); // no_output_of_prior_pics_flag, long_term_reference_flag
        } else if r.flag() {
            // adaptive_ref_pic_marking_mode_flag
            for _ in 0..64 {
                let op = r.ue_max(6, "memory_management_control_operation")?;
                match op {
                    0 => break,
                    1 | 2 | 6 | 4 => {
                        r.ue()?;
                    }
                    3 => {
                        r.ue()?;
                        r.ue()?;
                    }
                    _ => {}
                }
            }
        }
    }
    let qp_bd = 6 * (sps.bit_depth_luma as i32 - 8);
    let qp = pps.pic_init_qp + r.se()?;
    if qp < -qp_bd || qp > 51 {
        return Err(Error::corrupt(format!("slice QP {qp} out of range")));
    }
    if slice_type == SLICE_SI {
        r.se()?; // slice_qs_delta
    }
    let (mut disable_deblocking, mut alpha_offset, mut beta_offset) = (0, 0, 0);
    if pps.deblocking_filter_control_present {
        disable_deblocking = r.ue_max(2, "disable_deblocking_filter_idc")?;
        if disable_deblocking != 1 {
            alpha_offset = 2 * r.se_range(-6, 6, "slice_alpha_c0_offset_div2")?;
            beta_offset = 2 * r.se_range(-6, 6, "slice_beta_offset_div2")?;
        }
    }
    r.check()?;
    Ok(SliceHeader {
        first_mb,
        slice_type,
        frame_num,
        idr_pic_id,
        qp,
        disable_deblocking,
        alpha_offset,
        beta_offset,
    })
}

//! One access unit → one picture: parameter sets, slices (decoded in
//! parallel when there are several), deblocking, cropping.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::bits::Rbsp;
use crate::cabac::{Engine, init_contexts};
use crate::deblock::{SliceFilter, deblock};
use crate::mb::{CabacState, Canvas, Entropy, MbInfo, PicParams, SliceDecoder};
use crate::nal::{NAL_IDR, NAL_PPS, NAL_SLICE, NAL_SPS, Nal};
use crate::params::{Pps, Sps};
use crate::slice::{self, SLICE_I, SliceHeader};
use crate::tables::{ZIGZAG4, ZIGZAG8, norm4, norm8};
use crate::{DecoderConfig, Error, Picture};

/// Pictures smaller than this many macroblocks decode on the calling thread.
const PARALLEL_MIN_MBS: usize = 1024;

/// A decoded slice: its macroblock rows and per-macroblock info.
type SliceResult = Result<(Canvas, Vec<MbInfo>), Error>;

struct SliceJob {
    hdr: SliceHeader,
    rbsp: Rbsp,
    /// Bit position of slice_data().
    data_pos: usize,
    end_mb: usize,
}

pub(crate) fn decode(nals: &[Nal], config: &DecoderConfig) -> Result<Picture, Error> {
    let mut sps_set = config.sps.clone();
    let mut pps_set = config.pps.clone();
    // Slices of the first picture, with parameter sets seen before them.
    let mut slices: Vec<(Nal, Rbsp, u32, u32, u32)> = Vec::new();
    for n in nals {
        match n.kind {
            NAL_SPS if slices.is_empty() => {
                let r = Rbsp::new(n.payload);
                let mut br = r.reader();
                br.skip(8 + 16);
                if let Ok(id) = br.ue_max(31, "seq_parameter_set_id") {
                    sps_set.retain(|(i, _)| *i != id);
                    sps_set.push((id, n.payload.to_vec()));
                }
            }
            NAL_PPS if slices.is_empty() => {
                let r = Rbsp::new(n.payload);
                if let Ok((id, _)) = Pps::ids(&mut r.reader()) {
                    pps_set.retain(|(i, _)| *i != id);
                    pps_set.push((id, n.payload.to_vec()));
                }
            }
            NAL_SLICE | NAL_IDR => {
                let r = Rbsp::new(n.payload);
                let (first_mb, st, pps_id) = slice::peek(&mut r.reader())?;
                if first_mb == 0 && !slices.is_empty() {
                    break; // the next picture
                }
                slices.push((*n, r, first_mb, st, pps_id));
            }
            _ => {}
        }
    }
    let Some(&(_, _, _, _, pps_id)) = slices.first() else {
        return Err(Error::corrupt("the access unit holds no picture"));
    };
    for s in &slices {
        if s.3 != SLICE_I {
            return Err(Error::Unsupported(
                "only intra pictures are decoded (this one predicts from other frames)".into(),
            ));
        }
        if s.4 != pps_id {
            return Err(Error::Unsupported("slices with different PPS".into()));
        }
    }
    let pps_raw = pps_set
        .iter()
        .find(|(i, _)| *i == pps_id)
        .map(|(_, b)| Rbsp::new(b))
        .ok_or_else(|| Error::corrupt("missing picture parameter set"))?;
    let (_, sps_id) = Pps::ids(&mut pps_raw.reader())?;
    let sps_raw = sps_set
        .iter()
        .find(|(i, _)| *i == sps_id)
        .map(|(_, b)| Rbsp::new(b))
        .ok_or_else(|| Error::corrupt("missing sequence parameter set"))?;
    let sps = Sps::parse(&mut sps_raw.reader())?;
    let pps = Pps::parse(&mut pps_raw.reader(), &sps)?;
    check_supported(&sps)?;
    let p = pic_params(&sps, &pps);
    let total = p.w_mbs * p.h_mbs;

    // Slice headers and extents.
    let mut jobs: Vec<SliceJob> = Vec::with_capacity(slices.len());
    for (n, r, first_mb, st, _) in slices {
        let mut br = r.reader();
        slice::peek(&mut br)?;
        let hdr = slice::parse_intra(
            &mut br,
            first_mb,
            st,
            n.kind == NAL_IDR,
            n.ref_idc,
            &sps,
            &pps,
        )?;
        let data_pos = br.pos();
        jobs.push(SliceJob {
            hdr,
            rbsp: r,
            data_pos,
            end_mb: 0,
        });
    }
    for k in 0..jobs.len() {
        let end = jobs.get(k + 1).map_or(total, |j| j.hdr.first_mb as usize);
        let first = jobs[k].hdr.first_mb as usize;
        if first >= end || end > total {
            return Err(Error::corrupt("slices out of order or outside the picture"));
        }
        jobs[k].end_mb = end;
    }

    if jobs.len() > u16::MAX as usize {
        return Err(Error::Unsupported("more than 65535 slices".into()));
    }

    // Decode, slices in parallel when the picture is large enough.
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads = match config.max_threads {
        0 => cores,
        n => n.min(cores),
    }
    .min(jobs.len());
    let results: Vec<SliceResult> = if threads > 1 && total >= PARALLEL_MIN_MBS {
        let next = AtomicUsize::new(0);
        let slots: Vec<Mutex<Option<SliceResult>>> =
            (0..jobs.len()).map(|_| Mutex::new(None)).collect();
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(job) = jobs.get(k) else { break };
                        let r = decode_slice(&p, &pps, job, k as u16 + 1);
                        if let Ok(mut slot) = slots[k].lock() {
                            *slot = Some(r);
                        }
                    }
                });
            }
        });
        slots
            .into_iter()
            .map(|m| {
                m.into_inner()
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| Err(Error::corrupt("a slice decoder stopped")))
            })
            .collect()
    } else {
        jobs.iter()
            .enumerate()
            .map(|(k, j)| decode_slice(&p, &pps, j, k as u16 + 1))
            .collect()
    };

    // Assemble.
    let sy = p.w_mbs * 16;
    let sc = p.w_mbs * 8;
    let mut y = vec![0u16; sy * p.h_mbs * 16];
    let mut c = [
        vec![0u16; sc * p.h_mbs * p.mbh_c],
        vec![0u16; sc * p.h_mbs * p.mbh_c],
    ];
    let mut mbs = vec![MbInfo::default(); total];
    for (job, r) in jobs.iter().zip(results) {
        let (cv, infos) = r?;
        let first = job.hdr.first_mb as usize;
        for addr in first..job.end_mb {
            mbs[addr] = infos[addr - first];
            let (mx, my) = (addr % p.w_mbs, addr / p.w_mbs);
            let ry = my - cv.row0;
            for row in 0..16 {
                let src = (ry * 16 + row) * sy + mx * 16;
                let dst = (my * 16 + row) * sy + mx * 16;
                y[dst..dst + 16].copy_from_slice(&cv.y[src..src + 16]);
            }
            for k in 0..2 {
                for row in 0..p.mbh_c {
                    let src = (ry * p.mbh_c + row) * sc + mx * 8;
                    let dst = (my * p.mbh_c + row) * sc + mx * 8;
                    c[k][dst..dst + 8].copy_from_slice(&cv.c[k][src..src + 8]);
                }
            }
        }
    }
    if mbs.iter().any(|m| m.slice == 0) {
        return Err(Error::corrupt("the picture is missing macroblocks"));
    }
    let filters: Vec<SliceFilter> = jobs
        .iter()
        .map(|j| SliceFilter {
            disable: j.hdr.disable_deblocking,
            alpha_offset: j.hdr.alpha_offset,
            beta_offset: j.hdr.beta_offset,
        })
        .collect();
    deblock(&p, &mbs, &filters, &mut y, &mut c);
    Ok(crop(&sps, &p, y, c))
}

fn check_supported(sps: &Sps) -> Result<(), Error> {
    if !matches!(sps.chroma_format_idc, 1 | 2) {
        return Err(Error::Unsupported(format!(
            "chroma format {} (only 4:2:0 and 4:2:2 are decoded)",
            match sps.chroma_format_idc {
                0 => "4:0:0",
                _ => "4:4:4",
            }
        )));
    }
    if !sps.frame_mbs_only {
        return Err(Error::Unsupported(
            "interlaced (field or MBAFF) coding".into(),
        ));
    }
    if sps.transform_bypass {
        return Err(Error::Unsupported(
            "lossless (transform bypass) coding".into(),
        ));
    }
    Ok(())
}

fn pic_params(sps: &Sps, pps: &Pps) -> PicParams {
    let mut ls4 = [[[0i32; 16]; 6]; 3];
    for (list, l) in ls4.iter_mut().enumerate() {
        let mut weight = [0i32; 16];
        for k in 0..16 {
            weight[ZIGZAG4[k] as usize] = pps.scaling.l4[list][k] as i32;
        }
        for (m, lm) in l.iter_mut().enumerate() {
            for pos in 0..16 {
                lm[pos] = weight[pos] * norm4(m, pos / 4, pos % 4);
            }
        }
    }
    let mut ls8 = [[0i32; 64]; 6];
    let mut weight = [0i32; 64];
    for k in 0..64 {
        weight[ZIGZAG8[k] as usize] = pps.scaling.l8[0][k] as i32;
    }
    for (m, lm) in ls8.iter_mut().enumerate() {
        for pos in 0..64 {
            lm[pos] = weight[pos] * norm8(m, pos / 8, pos % 8);
        }
    }
    PicParams {
        w_mbs: sps.width_mbs as usize,
        h_mbs: sps.height_mbs() as usize,
        mbh_c: if sps.chroma_format_idc == 2 { 16 } else { 8 },
        num_c8x8: if sps.chroma_format_idc == 2 { 2 } else { 1 },
        bd_y: sps.bit_depth_luma,
        bd_c: sps.bit_depth_chroma,
        qp_bd_y: 6 * (sps.bit_depth_luma as i32 - 8),
        qp_bd_c: 6 * (sps.bit_depth_chroma as i32 - 8),
        cqp_offset: pps.chroma_qp_offset,
        transform8x8_mode: pps.transform_8x8_mode,
        ls4,
        ls8,
    }
}

fn decode_slice(
    p: &PicParams,
    pps: &Pps,
    job: &SliceJob,
    slice_num: u16,
) -> Result<(Canvas, Vec<MbInfo>), Error> {
    let first = job.hdr.first_mb as usize;
    let row0 = first / p.w_mbs;
    let rows = (job.end_mb - 1) / p.w_mbs - row0 + 1;
    let mut cv = Canvas::new(p, row0, rows);
    let mut mbs = vec![MbInfo::default(); job.end_mb - first];
    let mut r = job.rbsp.reader();
    r.skip(job.data_pos as u32);
    let mut ent = if pps.cabac {
        r.align();
        let e = Engine::new(r)?;
        Entropy::Cabac(Box::new(CabacState {
            e,
            ctx: init_contexts(job.hdr.qp),
        }))
    } else {
        Entropy::Cavlc(r)
    };
    let mut dec = SliceDecoder::new(p, first, &mut mbs, &mut cv, slice_num, job.hdr.qp);
    let mut addr = first;
    loop {
        dec.macroblock(&mut ent, addr)?;
        let more = match &mut ent {
            Entropy::Cavlc(r) => r.more_rbsp_data(),
            Entropy::Cabac(s) => s.e.terminate() == 0,
        };
        if !more {
            break;
        }
        addr += 1;
        if addr >= job.end_mb {
            return Err(Error::corrupt("a slice runs into the next one"));
        }
    }
    Ok((cv, mbs))
}

fn crop(sps: &Sps, p: &PicParams, y: Vec<u16>, c: [Vec<u16>; 2]) -> Picture {
    let (sw, sh) = sps.chroma_sub();
    let full_w = p.w_mbs * 16;
    let full_h = p.h_mbs * 16;
    let [l, r, t, b] = sps.crop.map(|v| v as usize);
    let (x0, x1) = (l * sw as usize, full_w - r * sw as usize);
    let (y0, y1) = (t * sh as usize, full_h - b * sh as usize);
    let (w, h) = (x1 - x0, y1 - y0);
    let cut = |plane: &[u16], stride: usize, x0: usize, y0: usize, w: usize, h: usize| {
        if x0 == 0 && w == stride && y0 == 0 && plane.len() == stride * h {
            return plane.to_vec();
        }
        let mut out = Vec::with_capacity(w * h);
        for row in y0..y0 + h {
            out.extend_from_slice(&plane[row * stride + x0..row * stride + x0 + w]);
        }
        out
    };
    let cw = w / sw as usize;
    let ch = h / sh as usize;
    let cx0 = x0 / sw as usize;
    let cy0 = y0 / sh as usize;
    let sc = p.w_mbs * 8;
    let [cb, cr] = c;
    Picture {
        width: w as u32,
        height: h as u32,
        chroma_format: sps.chroma_format_idc as u8,
        bit_depth: sps.bit_depth_luma as u8,
        bit_depth_chroma: sps.bit_depth_chroma as u8,
        y: cut(&y, full_w, x0, y0, w, h),
        cb: cut(&cb, sc, cx0, cy0, cw, ch),
        cr: cut(&cr, sc, cx0, cy0, cw, ch),
        colour: sps.colour,
    }
}

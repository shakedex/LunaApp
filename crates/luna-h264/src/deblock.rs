//! Deblocking filter (8.7) for intra frame pictures, where every macroblock
//! edge has bS = 4 and every internal transform edge bS = 3.

use crate::mb::{KIND_I8, KIND_PCM, MbInfo, PicParams};
use crate::tables::{ALPHA, BETA, TC0, chroma_qp};

/// Per-slice filter controls.
#[derive(Clone, Copy)]
pub(crate) struct SliceFilter {
    pub disable: u32,
    pub alpha_offset: i32,
    pub beta_offset: i32,
}

struct Edge {
    alpha: i32,
    beta: i32,
    tc0: i32,
    bs4: bool,
    chroma: bool,
    max: i32,
}

/// Filters `n` lines across one edge. `q0` is the index of the first q0
/// sample, `step` moves across the edge (p side is −step), `line` moves
/// along it.
fn filter_lines(buf: &mut [u16], q0: usize, step: usize, line: usize, n: usize, e: &Edge) {
    for k in 0..n {
        let i = q0 + k * line;
        let at = |o: isize| (i as isize + o * step as isize) as usize;
        let (p0, p1, q0v, q1) = (
            buf[at(-1)] as i32,
            buf[at(-2)] as i32,
            buf[at(0)] as i32,
            buf[at(1)] as i32,
        );
        if (p0 - q0v).abs() >= e.alpha || (p1 - p0).abs() >= e.beta || (q1 - q0v).abs() >= e.beta {
            continue;
        }
        if e.chroma {
            if e.bs4 {
                buf[at(-1)] = ((2 * p1 + p0 + q1 + 2) >> 2) as u16;
                buf[at(0)] = ((2 * q1 + q0v + p1 + 2) >> 2) as u16;
            } else {
                let tc = e.tc0 + 1;
                let delta = ((((q0v - p0) << 2) + (p1 - q1) + 4) >> 3).clamp(-tc, tc);
                buf[at(-1)] = (p0 + delta).clamp(0, e.max) as u16;
                buf[at(0)] = (q0v - delta).clamp(0, e.max) as u16;
            }
            continue;
        }
        let (p2, q2) = (buf[at(-3)] as i32, buf[at(2)] as i32);
        let ap = (p2 - p0).abs() < e.beta;
        let aq = (q2 - q0v).abs() < e.beta;
        if e.bs4 {
            let strong = (p0 - q0v).abs() < (e.alpha >> 2) + 2;
            if ap && strong {
                let p3 = buf[at(-4)] as i32;
                buf[at(-1)] = ((p2 + 2 * p1 + 2 * p0 + 2 * q0v + q1 + 4) >> 3) as u16;
                buf[at(-2)] = ((p2 + p1 + p0 + q0v + 2) >> 2) as u16;
                buf[at(-3)] = ((2 * p3 + 3 * p2 + p1 + p0 + q0v + 4) >> 3) as u16;
            } else {
                buf[at(-1)] = ((2 * p1 + p0 + q1 + 2) >> 2) as u16;
            }
            if aq && strong {
                let q3 = buf[at(3)] as i32;
                buf[at(0)] = ((p1 + 2 * p0 + 2 * q0v + 2 * q1 + q2 + 4) >> 3) as u16;
                buf[at(1)] = ((p0 + q0v + q1 + q2 + 2) >> 2) as u16;
                buf[at(2)] = ((2 * q3 + 3 * q2 + q1 + q0v + p0 + 4) >> 3) as u16;
            } else {
                buf[at(0)] = ((2 * q1 + q0v + p1 + 2) >> 2) as u16;
            }
        } else {
            let tc = e.tc0 + ap as i32 + aq as i32;
            let delta = ((((q0v - p0) << 2) + (p1 - q1) + 4) >> 3).clamp(-tc, tc);
            buf[at(-1)] = (p0 + delta).clamp(0, e.max) as u16;
            buf[at(0)] = (q0v - delta).clamp(0, e.max) as u16;
            if ap {
                buf[at(-2)] = (p1
                    + ((p2 + ((p0 + q0v + 1) >> 1) - (p1 << 1)) >> 1).clamp(-e.tc0, e.tc0))
                    as u16;
            }
            if aq {
                buf[at(1)] = (q1
                    + ((q2 + ((p0 + q0v + 1) >> 1) - (q1 << 1)) >> 1).clamp(-e.tc0, e.tc0))
                    as u16;
            }
        }
    }
}

fn edge(qpav: i32, f: &SliceFilter, bs4: bool, chroma: bool, bd: u32) -> Edge {
    let ia = (qpav + f.alpha_offset).clamp(0, 51) as usize;
    let ib = (qpav + f.beta_offset).clamp(0, 51) as usize;
    let sc = 1 << (bd - 8);
    Edge {
        alpha: ALPHA[ia] as i32 * sc,
        beta: BETA[ib] as i32 * sc,
        tc0: TC0[ia][2] as i32 * sc,
        bs4,
        chroma,
        max: (1 << bd) - 1,
    }
}

/// Filters a whole decoded picture in place.
pub(crate) fn deblock(
    p: &PicParams,
    mbs: &[MbInfo],
    filters: &[SliceFilter],
    y: &mut [u16],
    c: &mut [Vec<u16>; 2],
) {
    let w = p.w_mbs;
    let sy = w * 16;
    let sc = w * 8;
    let mbh_c = p.mbh_c;
    let qp_of = |m: &MbInfo| if m.kind == KIND_PCM { 0 } else { m.qp as i32 };
    for addr in 0..w * p.h_mbs {
        let m = &mbs[addr];
        if m.slice == 0 {
            continue;
        }
        let Some(f) = filters.get(m.slice as usize - 1) else {
            continue;
        };
        if f.disable == 1 {
            continue;
        }
        let (mx, my) = (addr % w, addr / w);
        let left = (mx > 0)
            .then(|| &mbs[addr - 1])
            .filter(|n| n.slice != 0 && (f.disable != 2 || n.slice == m.slice));
        let top = (my > 0)
            .then(|| &mbs[addr - w])
            .filter(|n| n.slice != 0 && (f.disable != 2 || n.slice == m.slice));
        let qq = qp_of(m);
        let internal: &[usize] = if m.kind == KIND_I8 { &[8] } else { &[4, 8, 12] };
        // Luma: vertical edges, then horizontal.
        let base = my * 16 * sy + mx * 16;
        if let Some(n) = left {
            let e = edge((qp_of(n) + qq + 1) >> 1, f, true, false, p.bd_y);
            filter_lines(y, base, 1, sy, 16, &e);
        }
        let ei = edge(qq, f, false, false, p.bd_y);
        for &x in internal {
            filter_lines(y, base + x, 1, sy, 16, &ei);
        }
        if let Some(n) = top {
            let e = edge((qp_of(n) + qq + 1) >> 1, f, true, false, p.bd_y);
            filter_lines(y, base, sy, 1, 16, &e);
        }
        for &yy in internal {
            filter_lines(y, base + yy * sy, sy, 1, 16, &ei);
        }
        // Chroma.
        let cbase = my * mbh_c * sc + mx * 8;
        for (ci, plane) in c.iter_mut().enumerate() {
            let cq = |qpy: i32| chroma_qp((qpy + p.cqp_offset[ci]).clamp(-p.qp_bd_c, 51));
            let cqq = cq(qq);
            if let Some(n) = left {
                let e = edge((cq(qp_of(n)) + cqq + 1) >> 1, f, true, true, p.bd_c);
                filter_lines(plane, cbase, 1, sc, mbh_c, &e);
            }
            let ei = edge(cqq, f, false, true, p.bd_c);
            filter_lines(plane, cbase + 4, 1, sc, mbh_c, &ei);
            if let Some(n) = top {
                let e = edge((cq(qp_of(n)) + cqq + 1) >> 1, f, true, true, p.bd_c);
                filter_lines(plane, cbase, sc, 1, 8, &e);
            }
            let mut yy = 4;
            while yy < mbh_c {
                filter_lines(plane, cbase + yy * sc, sc, 1, 8, &ei);
                yy += 4;
            }
        }
    }
}

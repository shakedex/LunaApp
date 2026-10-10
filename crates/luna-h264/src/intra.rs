//! Intra sample prediction (8.3.1.2, 8.3.2.2, 8.3.3, 8.3.4).
//!
//! Reference samples come in as arrays: `top[0]` is p[−1,−1] and `top[1 + x]`
//! is p[x,−1]; `left[y]` is p[−1,y]. Predictions are written row-major.

use crate::Error;

#[derive(Clone, Copy, Default)]
pub(crate) struct Avail {
    pub left: bool,
    pub top: bool,
    pub top_left: bool,
    pub top_right: bool,
}

fn need(ok: bool) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::corrupt("intra prediction from unavailable samples"))
    }
}

/// Intra_4x4 (8.3.1.2). `top` holds p[−1,−1] and p[0..7,−1] (top-right
/// already substituted when unavailable).
pub(crate) fn pred4x4(
    mode: u8,
    top: &[i32; 9],
    left: &[i32; 4],
    a: Avail,
    bd: u32,
    out: &mut [i32; 16],
) -> Result<(), Error> {
    let t = |x: i32| top[(x + 1) as usize]; // p[x,-1], x in -1..=7
    let l = |y: i32| if y < 0 { top[0] } else { left[y as usize] }; // p[-1,y]
    match mode {
        0 => {
            need(a.top)?;
            for y in 0..4 {
                for x in 0..4 {
                    out[y * 4 + x] = top[1 + x];
                }
            }
        }
        1 => {
            need(a.left)?;
            for y in 0..4 {
                for x in 0..4 {
                    out[y * 4 + x] = left[y];
                }
            }
        }
        2 => {
            let st: i32 = top[1..5].iter().sum();
            let sl: i32 = left.iter().sum();
            let dc = match (a.top, a.left) {
                (true, true) => (st + sl + 4) >> 3,
                (false, true) => (sl + 2) >> 2,
                (true, false) => (st + 2) >> 2,
                (false, false) => 1 << (bd - 1),
            };
            out.fill(dc);
        }
        3 => {
            need(a.top)?;
            for y in 0..4i32 {
                for x in 0..4i32 {
                    out[(y * 4 + x) as usize] = if x == 3 && y == 3 {
                        (t(6) + 3 * t(7) + 2) >> 2
                    } else {
                        (t(x + y) + 2 * t(x + y + 1) + t(x + y + 2) + 2) >> 2
                    };
                }
            }
        }
        4..=6 => {
            need(a.top && a.left && a.top_left)?;
            for y in 0..4i32 {
                for x in 0..4i32 {
                    out[(y * 4 + x) as usize] = match mode {
                        4 => {
                            if x > y {
                                (t(x - y - 2) + 2 * t(x - y - 1) + t(x - y) + 2) >> 2
                            } else if x < y {
                                (l(y - x - 2) + 2 * l(y - x - 1) + l(y - x) + 2) >> 2
                            } else {
                                (t(0) + 2 * t(-1) + l(0) + 2) >> 2
                            }
                        }
                        5 => {
                            let z = 2 * x - y;
                            if z >= 0 && z & 1 == 0 {
                                (t(x - (y >> 1) - 1) + t(x - (y >> 1)) + 1) >> 1
                            } else if z >= 0 {
                                (t(x - (y >> 1) - 2)
                                    + 2 * t(x - (y >> 1) - 1)
                                    + t(x - (y >> 1))
                                    + 2)
                                    >> 2
                            } else if z == -1 {
                                (l(0) + 2 * l(-1) + t(0) + 2) >> 2
                            } else {
                                (l(y - 1) + 2 * l(y - 2) + l(y - 3) + 2) >> 2
                            }
                        }
                        _ => {
                            let z = 2 * y - x;
                            if z >= 0 && z & 1 == 0 {
                                (l(y - (x >> 1) - 1) + l(y - (x >> 1)) + 1) >> 1
                            } else if z >= 0 {
                                (l(y - (x >> 1) - 2)
                                    + 2 * l(y - (x >> 1) - 1)
                                    + l(y - (x >> 1))
                                    + 2)
                                    >> 2
                            } else if z == -1 {
                                (l(0) + 2 * l(-1) + t(0) + 2) >> 2
                            } else {
                                (t(x - 1) + 2 * t(x - 2) + t(x - 3) + 2) >> 2
                            }
                        }
                    };
                }
            }
        }
        7 => {
            need(a.top)?;
            for y in 0..4i32 {
                for x in 0..4i32 {
                    let i = x + (y >> 1);
                    out[(y * 4 + x) as usize] = if y & 1 == 0 {
                        (t(i) + t(i + 1) + 1) >> 1
                    } else {
                        (t(i) + 2 * t(i + 1) + t(i + 2) + 2) >> 2
                    };
                }
            }
        }
        8 => {
            need(a.left)?;
            for y in 0..4i32 {
                for x in 0..4i32 {
                    let z = x + 2 * y;
                    let i = y + (x >> 1);
                    out[(y * 4 + x) as usize] = if z > 5 {
                        l(3)
                    } else if z == 5 {
                        (l(2) + 3 * l(3) + 2) >> 2
                    } else if z & 1 == 0 {
                        (l(i) + l(i + 1) + 1) >> 1
                    } else {
                        (l(i) + 2 * l(i + 1) + l(i + 2) + 2) >> 2
                    };
                }
            }
        }
        _ => return Err(Error::corrupt("invalid Intra4x4 mode")),
    }
    Ok(())
}

/// Reference sample filtering for Intra_8x8 (8.3.2.2.1). `top` holds
/// p[−1,−1] and p[0..15,−1] (top-right substituted when unavailable).
fn filter8(top: &[i32; 17], left: &[i32; 8], a: Avail) -> ([i32; 17], [i32; 8]) {
    let mut ft = *top;
    let mut fl = *left;
    let t = |x: usize| top[x + 1];
    if a.top {
        ft[1] = if a.top_left {
            (top[0] + 2 * t(0) + t(1) + 2) >> 2
        } else {
            (3 * t(0) + t(1) + 2) >> 2
        };
        for x in 1..15 {
            ft[x + 1] = (t(x - 1) + 2 * t(x) + t(x + 1) + 2) >> 2;
        }
        ft[16] = (t(14) + 3 * t(15) + 2) >> 2;
    }
    if a.top_left {
        ft[0] = match (a.top, a.left) {
            (true, true) => (t(0) + 2 * top[0] + left[0] + 2) >> 2,
            (true, false) => (3 * top[0] + t(0) + 2) >> 2,
            (false, true) => (3 * top[0] + left[0] + 2) >> 2,
            (false, false) => top[0],
        };
    }
    if a.left {
        fl[0] = if a.top_left {
            (top[0] + 2 * left[0] + left[1] + 2) >> 2
        } else {
            (3 * left[0] + left[1] + 2) >> 2
        };
        for y in 1..7 {
            fl[y] = (left[y - 1] + 2 * left[y] + left[y + 1] + 2) >> 2;
        }
        fl[7] = (left[6] + 3 * left[7] + 2) >> 2;
    }
    (ft, fl)
}

/// Intra_8x8 (8.3.2.2).
pub(crate) fn pred8x8(
    mode: u8,
    top: &[i32; 17],
    left: &[i32; 8],
    a: Avail,
    bd: u32,
    out: &mut [i32; 64],
) -> Result<(), Error> {
    let (top, left) = filter8(top, left, a);
    let t = |x: i32| top[(x + 1) as usize];
    let l = |y: i32| if y < 0 { top[0] } else { left[y as usize] };
    match mode {
        0 => {
            need(a.top)?;
            for y in 0..8 {
                out[y * 8..y * 8 + 8].copy_from_slice(&top[1..9]);
            }
        }
        1 => {
            need(a.left)?;
            for y in 0..8 {
                out[y * 8..y * 8 + 8].fill(left[y]);
            }
        }
        2 => {
            let st: i32 = top[1..9].iter().sum();
            let sl: i32 = left.iter().sum();
            let dc = match (a.top, a.left) {
                (true, true) => (st + sl + 8) >> 4,
                (false, true) => (sl + 4) >> 3,
                (true, false) => (st + 4) >> 3,
                (false, false) => 1 << (bd - 1),
            };
            out.fill(dc);
        }
        3 => {
            need(a.top)?;
            for y in 0..8i32 {
                for x in 0..8i32 {
                    out[(y * 8 + x) as usize] = if x == 7 && y == 7 {
                        (t(14) + 3 * t(15) + 2) >> 2
                    } else {
                        (t(x + y) + 2 * t(x + y + 1) + t(x + y + 2) + 2) >> 2
                    };
                }
            }
        }
        4..=6 => {
            need(a.top && a.left && a.top_left)?;
            for y in 0..8i32 {
                for x in 0..8i32 {
                    out[(y * 8 + x) as usize] = match mode {
                        4 => {
                            if x > y {
                                (t(x - y - 2) + 2 * t(x - y - 1) + t(x - y) + 2) >> 2
                            } else if x < y {
                                (l(y - x - 2) + 2 * l(y - x - 1) + l(y - x) + 2) >> 2
                            } else {
                                (t(0) + 2 * t(-1) + l(0) + 2) >> 2
                            }
                        }
                        5 => {
                            let z = 2 * x - y;
                            if z >= 0 && z & 1 == 0 {
                                (t(x - (y >> 1) - 1) + t(x - (y >> 1)) + 1) >> 1
                            } else if z >= 0 {
                                (t(x - (y >> 1) - 2)
                                    + 2 * t(x - (y >> 1) - 1)
                                    + t(x - (y >> 1))
                                    + 2)
                                    >> 2
                            } else if z == -1 {
                                (l(0) + 2 * l(-1) + t(0) + 2) >> 2
                            } else {
                                (l(y - 2 * x - 1) + 2 * l(y - 2 * x - 2) + l(y - 2 * x - 3) + 2)
                                    >> 2
                            }
                        }
                        _ => {
                            let z = 2 * y - x;
                            if z >= 0 && z & 1 == 0 {
                                (l(y - (x >> 1) - 1) + l(y - (x >> 1)) + 1) >> 1
                            } else if z >= 0 {
                                (l(y - (x >> 1) - 2)
                                    + 2 * l(y - (x >> 1) - 1)
                                    + l(y - (x >> 1))
                                    + 2)
                                    >> 2
                            } else if z == -1 {
                                (l(0) + 2 * l(-1) + t(0) + 2) >> 2
                            } else {
                                (t(x - 2 * y - 1) + 2 * t(x - 2 * y - 2) + t(x - 2 * y - 3) + 2)
                                    >> 2
                            }
                        }
                    };
                }
            }
        }
        7 => {
            need(a.top)?;
            for y in 0..8i32 {
                for x in 0..8i32 {
                    let i = x + (y >> 1);
                    out[(y * 8 + x) as usize] = if y & 1 == 0 {
                        (t(i) + t(i + 1) + 1) >> 1
                    } else {
                        (t(i) + 2 * t(i + 1) + t(i + 2) + 2) >> 2
                    };
                }
            }
        }
        8 => {
            need(a.left)?;
            for y in 0..8i32 {
                for x in 0..8i32 {
                    let z = x + 2 * y;
                    let i = y + (x >> 1);
                    out[(y * 8 + x) as usize] = if z > 13 {
                        l(7)
                    } else if z == 13 {
                        (l(6) + 3 * l(7) + 2) >> 2
                    } else if z & 1 == 0 {
                        (l(i) + l(i + 1) + 1) >> 1
                    } else {
                        (l(i) + 2 * l(i + 1) + l(i + 2) + 2) >> 2
                    };
                }
            }
        }
        _ => return Err(Error::corrupt("invalid Intra8x8 mode")),
    }
    Ok(())
}

/// Intra_16x16 (8.3.3). `top` holds p[−1,−1] and p[0..15,−1].
pub(crate) fn pred16x16(
    mode: u8,
    top: &[i32; 17],
    left: &[i32; 16],
    a: Avail,
    bd: u32,
    out: &mut [i32; 256],
) -> Result<(), Error> {
    match mode {
        0 => {
            need(a.top)?;
            for y in 0..16 {
                out[y * 16..y * 16 + 16].copy_from_slice(&top[1..17]);
            }
        }
        1 => {
            need(a.left)?;
            for y in 0..16 {
                out[y * 16..y * 16 + 16].fill(left[y]);
            }
        }
        2 => {
            let st: i32 = top[1..17].iter().sum();
            let sl: i32 = left.iter().sum();
            let dc = match (a.top, a.left) {
                (true, true) => (st + sl + 16) >> 5,
                (false, true) => (sl + 8) >> 4,
                (true, false) => (st + 8) >> 4,
                (false, false) => 1 << (bd - 1),
            };
            out.fill(dc);
        }
        3 => {
            need(a.top && a.left && a.top_left)?;
            plane(top, left, 16, 16, 0, 0, bd, out);
        }
        _ => return Err(Error::corrupt("invalid Intra16x16 mode")),
    }
    Ok(())
}

/// Plane prediction shared by luma (8-140) and chroma (8-145). `xcf`/`ycf`
/// are 0 or 4 (chroma 4:2:2 has ycf = 4); luma uses w = h = 16 and its own
/// constants via `xcf = ycf = 4` with the 5/5 multipliers.
#[allow(clippy::too_many_arguments)]
fn plane(
    top: &[i32],
    left: &[i32],
    w: usize,
    h: usize,
    xcf: i32,
    ycf: i32,
    bd: u32,
    out: &mut [i32],
) {
    let t = |x: i32| top[(x + 1) as usize];
    let l = |y: i32| if y < 0 { top[0] } else { left[y as usize] };
    let max = (1 << bd) - 1;
    let (a, b, c, cx, cy);
    if w == 16 {
        let mut hh = 0;
        let mut vv = 0;
        for i in 0..8 {
            hh += (i + 1) * (t(8 + i) - t(6 - i));
            vv += (i + 1) * (l(8 + i) - l(6 - i));
        }
        a = 16 * (l(15) + t(15));
        b = (5 * hh + 32) >> 6;
        c = (5 * vv + 32) >> 6;
        cx = 7;
        cy = 7;
    } else {
        let mut hh = 0;
        let mut vv = 0;
        for i in 0..=(3 + xcf) {
            hh += (i + 1) * (t(4 + xcf + i) - t(2 + xcf - i));
        }
        for i in 0..=(3 + ycf) {
            vv += (i + 1) * (l(4 + ycf + i) - l(2 + ycf - i));
        }
        a = 16 * (l(h as i32 - 1) + t(w as i32 - 1));
        b = ((34 - 29 * (xcf == 4) as i32) * hh + 32) >> 6;
        c = ((34 - 29 * (ycf == 4) as i32) * vv + 32) >> 6;
        cx = 3 + xcf;
        cy = 3 + ycf;
    }
    for y in 0..h {
        for x in 0..w {
            let v = (a + b * (x as i32 - cx) + c * (y as i32 - cy) + 16) >> 5;
            out[y * w + x] = v.clamp(0, max);
        }
    }
}

/// Chroma intra prediction (8.3.4) for one component: 8 wide, `h` (8 or 16)
/// high. `top` holds p[−1,−1] and p[0..7,−1].
pub(crate) fn pred_chroma(
    mode: u8,
    top: &[i32; 9],
    left: &[i32; 16],
    h: usize,
    a: Avail,
    bd: u32,
    out: &mut [i32; 128],
) -> Result<(), Error> {
    match mode {
        0 => {
            for by in 0..h / 4 {
                for bx in 0..2 {
                    let st: i32 = top[1 + bx * 4..5 + bx * 4].iter().sum();
                    let sl: i32 = left[by * 4..by * 4 + 4].iter().sum();
                    let both = (st + sl + 4) >> 3;
                    let ot = (st + 2) >> 2;
                    let ol = (sl + 2) >> 2;
                    let mid = 1 << (bd - 1);
                    let dc = if (bx == 0 && by == 0) || (bx > 0 && by > 0) {
                        match (a.top, a.left) {
                            (true, true) => both,
                            (true, false) => ot,
                            (false, true) => ol,
                            _ => mid,
                        }
                    } else if bx > 0 {
                        // top row, right block: top first
                        if a.top {
                            ot
                        } else if a.left {
                            ol
                        } else {
                            mid
                        }
                    } else if a.left {
                        ol
                    } else if a.top {
                        ot
                    } else {
                        mid
                    };
                    for y in 0..4 {
                        for x in 0..4 {
                            out[(by * 4 + y) * 8 + bx * 4 + x] = dc;
                        }
                    }
                }
            }
        }
        1 => {
            need(a.left)?;
            for y in 0..h {
                out[y * 8..y * 8 + 8].fill(left[y]);
            }
        }
        2 => {
            need(a.top)?;
            for y in 0..h {
                out[y * 8..y * 8 + 8].copy_from_slice(&top[1..9]);
            }
        }
        3 => {
            need(a.top && a.left && a.top_left)?;
            plane(top, left, 8, h, 0, if h == 16 { 4 } else { 0 }, bd, out);
        }
        _ => return Err(Error::corrupt("invalid chroma prediction mode")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Avail = Avail {
        left: true,
        top: true,
        top_left: true,
        top_right: true,
    };

    #[test]
    fn dc_without_neighbours_is_mid_grey() {
        let mut o = [0; 16];
        pred4x4(2, &[0; 9], &[0; 4], Avail::default(), 10, &mut o).unwrap();
        assert!(o.iter().all(|&v| v == 512));
        let mut o = [0; 256];
        pred16x16(2, &[0; 17], &[0; 16], Avail::default(), 8, &mut o).unwrap();
        assert!(o.iter().all(|&v| v == 128));
    }

    #[test]
    fn directional_modes_need_their_neighbours() {
        let mut o = [0; 16];
        assert!(pred4x4(0, &[0; 9], &[0; 4], Avail::default(), 8, &mut o).is_err());
        assert!(pred4x4(4, &[0; 9], &[0; 4], ALL, 8, &mut o).is_ok());
        assert!(pred4x4(9, &[0; 9], &[0; 4], ALL, 8, &mut o).is_err());
    }

    #[test]
    fn flat_plane_stays_flat() {
        let mut o = [0; 256];
        pred16x16(3, &[100; 17], &[100; 16], ALL, 8, &mut o).unwrap();
        assert!(o.iter().all(|&v| v == 100));
        let mut o = [0; 128];
        pred_chroma(3, &[60; 9], &[60; 16], 16, ALL, 8, &mut o).unwrap();
        assert!(o.iter().all(|&v| v == 60));
    }
}

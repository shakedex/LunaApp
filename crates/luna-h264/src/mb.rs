//! Macroblock layer of intra slices: parsing (CAVLC 7.3.5 / CABAC 9.3) into
//! [`MbData`], then reconstruction (prediction + residual) into a canvas.

use crate::Error;
use crate::bits::BitReader;
use crate::cabac::{Engine, NUM_CTX};
use crate::cavlc::{self, BlockKind};
use crate::intra::{self, Avail};
use crate::tables::{BLK4_AT, BLK4_XY, CBP_INTRA, ZIGZAG4, ZIGZAG8};
use crate::transform;

pub(crate) const KIND_I4: u8 = 0;
pub(crate) const KIND_I8: u8 = 1;
pub(crate) const KIND_I16: u8 = 2;
pub(crate) const KIND_PCM: u8 = 3;

/// coded_block_flag bits kept per macroblock (CABAC neighbour contexts).
const CBF_LUMA_DC: u32 = 16;
const CBF_CDC: u32 = 17; // + iCbCr
const CBF_CAC: u32 = 19; // + 8 * iCbCr + blkIdx
const CBF_ALL: u64 = (1 << 35) - 1;

/// What later macroblocks (and the deblocking filter) need to know.
#[derive(Clone, Copy, Default)]
pub(crate) struct MbInfo {
    /// 1 + index of the slice that decoded it; 0 = not decoded.
    pub slice: u16,
    pub kind: u8,
    pub cbp: u8,
    /// QPY.
    pub qp: i8,
    pub chroma_mode: u8,
    /// Intra4x4/8x8 prediction mode per 4×4 block (2 for other types).
    pub modes: [u8; 16],
    /// TotalCoeff per luma 4×4 block, then Cb and Cr 4×4 blocks (CAVLC nC).
    pub nz: [u8; 32],
    pub cbf: u64,
}

impl MbInfo {
    fn transform8x8(&self) -> bool {
        self.kind == KIND_I8
    }
    fn cbp_luma(&self) -> u8 {
        self.cbp & 15
    }
    fn cbp_chroma(&self) -> u8 {
        self.cbp >> 4
    }
}

/// Picture-wide constants for decoding slices.
pub(crate) struct PicParams {
    pub w_mbs: usize,
    pub h_mbs: usize,
    pub mbh_c: usize,
    pub num_c8x8: usize,
    pub bd_y: u32,
    pub bd_c: u32,
    pub qp_bd_y: i32,
    pub qp_bd_c: i32,
    pub cqp_offset: [i32; 2],
    pub transform8x8_mode: bool,
    /// LevelScale4x4 for Intra Y, Cb, Cr: [list][qP % 6][raster].
    pub ls4: [[[i32; 16]; 6]; 3],
    /// LevelScale8x8 for Intra Y: [qP % 6][raster].
    pub ls8: [[i32; 64]; 6],
}

impl PicParams {
    pub(crate) fn chroma_qp(&self, qpy: i32, c: usize) -> i32 {
        let qpi = (qpy + self.cqp_offset[c]).clamp(-self.qp_bd_c, 51);
        crate::tables::chroma_qp(qpi)
    }
}

/// Decoded samples of one slice's macroblock rows.
pub(crate) struct Canvas {
    pub row0: usize,
    pub stride_y: usize,
    pub stride_c: usize,
    pub mbh_c: usize,
    pub y: Vec<u16>,
    pub c: [Vec<u16>; 2],
}

impl Canvas {
    pub(crate) fn new(p: &PicParams, row0: usize, rows: usize) -> Canvas {
        let sy = p.w_mbs * 16;
        let sc = p.w_mbs * 8;
        Canvas {
            row0,
            stride_y: sy,
            stride_c: sc,
            mbh_c: p.mbh_c,
            y: vec![0; sy * rows * 16],
            c: [vec![0; sc * rows * p.mbh_c], vec![0; sc * rows * p.mbh_c]],
        }
    }

    #[inline]
    fn yi(&self, x: usize, y: usize) -> usize {
        (y - self.row0 * 16) * self.stride_y + x
    }

    #[inline]
    fn ci(&self, x: usize, y: usize) -> usize {
        (y - self.row0 * self.mbh_c) * self.stride_c + x
    }
}

/// One parsed macroblock.
struct MbData {
    kind: u8,
    i16_mode: u8,
    chroma_mode: u8,
    cbp_luma: u8,
    cbp_chroma: u8,
    modes: [u8; 16],
    /// Luma levels by block: 4×4 blocks at [blk*16 + raster], 8×8 at
    /// [blk8*64 + raster8].
    luma: [i32; 256],
    luma_coded: u16,
    dc: [i32; 16],
    cdc: [[i32; 8]; 2],
    cac: [[i32; 128]; 2],
    cac_coded: [u8; 2],
    pcm: Option<Box<[u16; 512]>>,
}

impl MbData {
    fn new() -> MbData {
        MbData {
            kind: 0,
            i16_mode: 0,
            chroma_mode: 0,
            cbp_luma: 0,
            cbp_chroma: 0,
            modes: [2; 16],
            luma: [0; 256],
            luma_coded: 0,
            dc: [0; 16],
            cdc: [[0; 8]; 2],
            cac: [[0; 128]; 2],
            cac_coded: [0; 2],
            pcm: None,
        }
    }
}

/// Entropy decoder state of a slice.
pub(crate) enum Entropy<'a> {
    Cavlc(BitReader<'a>),
    Cabac(Box<CabacState<'a>>),
}

pub(crate) struct CabacState<'a> {
    pub e: Engine<'a>,
    pub ctx: [u8; NUM_CTX],
}

/// Decodes the macroblocks of one slice.
pub(crate) struct SliceDecoder<'a> {
    pub p: &'a PicParams,
    pub first_mb: usize,
    /// Macroblocks first_mb.. (index = addr − first_mb).
    pub mbs: &'a mut [MbInfo],
    pub cv: &'a mut Canvas,
    pub slice_num: u16,
    pub qp: i32,
    prev_dqp_nonzero: bool,
}

#[derive(Clone, Copy)]
struct Nb {
    a: Option<MbInfo>,
    b: Option<MbInfo>,
    c_avail: bool,
    d_avail: bool,
}

impl<'a> SliceDecoder<'a> {
    pub(crate) fn new(
        p: &'a PicParams,
        first_mb: usize,
        mbs: &'a mut [MbInfo],
        cv: &'a mut Canvas,
        slice_num: u16,
        qp: i32,
    ) -> SliceDecoder<'a> {
        SliceDecoder {
            p,
            first_mb,
            mbs,
            cv,
            slice_num,
            qp,
            prev_dqp_nonzero: false,
        }
    }

    fn info(&self, addr: usize) -> Option<MbInfo> {
        if addr < self.first_mb {
            return None;
        }
        self.mbs.get(addr - self.first_mb).copied()
    }

    fn neighbours(&self, addr: usize) -> Nb {
        let w = self.p.w_mbs;
        let x = addr % w;
        let a = if x > 0 { self.info(addr - 1) } else { None };
        let b = if addr >= w { self.info(addr - w) } else { None };
        let c_avail = x + 1 < w && addr >= w && addr - w + 1 >= self.first_mb;
        let d_avail = x > 0 && addr > w && addr - w > self.first_mb;
        Nb {
            a,
            b,
            c_avail,
            d_avail,
        }
    }

    /// Parses and reconstructs the macroblock at `addr`.
    pub(crate) fn macroblock(&mut self, ent: &mut Entropy, addr: usize) -> Result<(), Error> {
        let nb = self.neighbours(addr);
        let mut cur = MbInfo {
            slice: self.slice_num,
            ..MbInfo::default()
        };
        let mut d = MbData::new();
        let dqp = match ent {
            Entropy::Cavlc(r) => self.parse_cavlc(r, &nb, &mut cur, &mut d)?,
            Entropy::Cabac(s) => self.parse_cabac(s, &nb, &mut cur, &mut d)?,
        };
        if let Some(dqp) = dqp {
            let bd = self.p.qp_bd_y;
            self.qp = (self.qp + dqp + 52 + 2 * bd).rem_euclid(52 + bd) - bd;
        }
        self.prev_dqp_nonzero = dqp.is_some_and(|v| v != 0);
        cur.qp = self.qp as i8;
        cur.kind = d.kind;
        cur.cbp = d.cbp_luma | (d.cbp_chroma << 4);
        cur.chroma_mode = d.chroma_mode;
        cur.modes = d.modes;
        if d.kind == KIND_PCM {
            cur.nz = [16; 32];
            cur.cbf = CBF_ALL;
        }
        self.reconstruct(addr, &nb, &mut d)?;
        let i = addr - self.first_mb;
        let slot = self
            .mbs
            .get_mut(i)
            .ok_or_else(|| Error::corrupt("macroblock outside its slice"))?;
        *slot = cur;
        Ok(())
    }

    // ------------------------------------------------------------ prediction modes

    /// predIntraNxNPredMode for the block whose top-left 4×4 is at (bx, by).
    fn predicted_mode(&self, nb: &Nb, cur: &MbInfo, bx: usize, by: usize) -> u8 {
        let mode_of = |m: &Option<MbInfo>, x: usize, y: usize| -> Option<u8> {
            m.map(|m| {
                if m.kind == KIND_I4 || m.kind == KIND_I8 {
                    m.modes[BLK4_AT[y][x] as usize]
                } else {
                    2
                }
            })
        };
        let a = if bx > 0 {
            Some(cur.modes[BLK4_AT[by][bx - 1] as usize])
        } else {
            mode_of(&nb.a, 3, by)
        };
        let b = if by > 0 {
            Some(cur.modes[BLK4_AT[by - 1][bx] as usize])
        } else {
            mode_of(&nb.b, bx, 3)
        };
        match (a, b) {
            (Some(a), Some(b)) => a.min(b),
            _ => 2,
        }
    }

    // ------------------------------------------------------------ CAVLC

    fn nc_luma(&self, nb: &Nb, cur: &MbInfo, blk: usize) -> i32 {
        let (x, y) = BLK4_XY[blk];
        let (bx, by) = (x as usize / 4, y as usize / 4);
        let a = if bx > 0 {
            Some(cur.nz[BLK4_AT[by][bx - 1] as usize])
        } else {
            nb.a.map(|m| m.nz[BLK4_AT[by][3] as usize])
        };
        let b = if by > 0 {
            Some(cur.nz[BLK4_AT[by - 1][bx] as usize])
        } else {
            nb.b.map(|m| m.nz[BLK4_AT[3][bx] as usize])
        };
        nc_of(a, b)
    }

    fn nc_chroma(&self, nb: &Nb, cur: &MbInfo, c: usize, blk: usize) -> i32 {
        let rows = self.p.mbh_c / 4;
        let (bx, by) = (blk % 2, blk / 2);
        let base = 16 + 8 * c;
        let a = if bx > 0 {
            Some(cur.nz[base + by * 2])
        } else {
            nb.a.map(|m| m.nz[base + by * 2 + 1])
        };
        let b = if by > 0 {
            Some(cur.nz[base + (by - 1) * 2 + bx])
        } else {
            nb.b.map(|m| m.nz[base + (rows - 1) * 2 + bx])
        };
        nc_of(a, b)
    }

    fn parse_cavlc(
        &self,
        r: &mut BitReader,
        nb: &Nb,
        cur: &mut MbInfo,
        d: &mut MbData,
    ) -> Result<Option<i32>, Error> {
        let t = cavlc::tables();
        let mb_type = r.ue_max(25, "mb_type")?;
        if mb_type == 25 {
            d.kind = KIND_PCM;
            r.align();
            d.pcm = Some(self.read_pcm(r));
            r.check()?;
            return Ok(None);
        }
        if mb_type == 0 {
            d.kind = if self.p.transform8x8_mode && r.flag() {
                KIND_I8
            } else {
                KIND_I4
            };
            cur.kind = d.kind;
            if d.kind == KIND_I8 {
                for b8 in 0..4 {
                    let (bx, by) = ((b8 % 2) * 2, (b8 / 2) * 2);
                    let pred = self.predicted_mode(nb, cur, bx, by);
                    let mode = if r.flag() {
                        pred
                    } else {
                        let rem = r.u(3) as u8;
                        if rem < pred { rem } else { rem + 1 }
                    };
                    for k in 0..4 {
                        cur.modes[b8 * 4 + k] = mode;
                    }
                }
            } else {
                for blk in 0..16 {
                    let (x, y) = BLK4_XY[blk];
                    let pred = self.predicted_mode(nb, cur, x as usize / 4, y as usize / 4);
                    let mode = if r.flag() {
                        pred
                    } else {
                        let rem = r.u(3) as u8;
                        if rem < pred { rem } else { rem + 1 }
                    };
                    cur.modes[blk] = mode;
                }
            }
            d.modes = cur.modes;
        } else {
            d.kind = KIND_I16;
            let m = mb_type - 1;
            d.i16_mode = (m % 4) as u8;
            d.cbp_chroma = ((m / 4) % 3) as u8;
            d.cbp_luma = if m >= 12 { 15 } else { 0 };
        }
        d.chroma_mode = r.ue_max(3, "intra_chroma_pred_mode")? as u8;
        if d.kind != KIND_I16 {
            let cbp = CBP_INTRA[r.ue_max(47, "coded_block_pattern")? as usize];
            d.cbp_luma = cbp & 15;
            d.cbp_chroma = cbp >> 4;
        }
        if d.cbp_luma == 0 && d.cbp_chroma == 0 && d.kind != KIND_I16 {
            r.check()?;
            return Ok(None);
        }
        let qp_bd = self.p.qp_bd_y;
        let dqp = r.se_range(-(26 + qp_bd / 2), 25 + qp_bd / 2, "mb_qp_delta")?;

        // Luma.
        let mut buf = [0i32; 16];
        if d.kind == KIND_I16 {
            let nc = self.nc_luma(nb, cur, 0);
            buf.fill(0);
            cavlc::residual_block(r, t, BlockKind::Normal(nc), 16, &mut buf)?;
            for k in 0..16 {
                d.dc[ZIGZAG4[k] as usize] = buf[k];
            }
        }
        for b8 in 0..4 {
            if d.cbp_luma & (1 << b8) == 0 {
                continue;
            }
            for k in 0..4 {
                let blk = b8 * 4 + k;
                let nc = self.nc_luma(nb, cur, blk);
                buf.fill(0);
                let n = if d.kind == KIND_I16 {
                    let n = cavlc::residual_block(r, t, BlockKind::Normal(nc), 15, &mut buf)?;
                    for i in 0..15 {
                        d.luma[blk * 16 + ZIGZAG4[i + 1] as usize] = buf[i];
                    }
                    n
                } else if d.kind == KIND_I8 {
                    let n = cavlc::residual_block(r, t, BlockKind::Normal(nc), 16, &mut buf)?;
                    for i in 0..16 {
                        d.luma[b8 * 64 + ZIGZAG8[4 * i + k] as usize] = buf[i];
                    }
                    n
                } else {
                    let n = cavlc::residual_block(r, t, BlockKind::Normal(nc), 16, &mut buf)?;
                    for i in 0..16 {
                        d.luma[blk * 16 + ZIGZAG4[i] as usize] = buf[i];
                    }
                    n
                };
                cur.nz[blk] = n as u8;
                if n > 0 {
                    d.luma_coded |= 1 << blk;
                }
            }
        }
        // Chroma.
        let nblk = 4 * self.p.num_c8x8;
        if d.cbp_chroma & 3 != 0 {
            let kind = if self.p.num_c8x8 == 1 {
                BlockKind::ChromaDc420
            } else {
                BlockKind::ChromaDc422
            };
            for c in 0..2 {
                buf.fill(0);
                cavlc::residual_block(r, t, kind, nblk, &mut buf)?;
                d.cdc[c][..nblk].copy_from_slice(&buf[..nblk]);
            }
        }
        if d.cbp_chroma & 2 != 0 {
            for c in 0..2 {
                for blk in 0..nblk {
                    let nc = self.nc_chroma(nb, cur, c, blk);
                    buf.fill(0);
                    let n = cavlc::residual_block(r, t, BlockKind::Normal(nc), 15, &mut buf)?;
                    for i in 0..15 {
                        d.cac[c][blk * 16 + ZIGZAG4[i + 1] as usize] = buf[i];
                    }
                    cur.nz[16 + 8 * c + blk] = n as u8;
                    if n > 0 {
                        d.cac_coded[c] |= 1 << blk;
                    }
                }
            }
        }
        r.check()?;
        Ok(Some(dqp))
    }

    fn read_pcm(&self, r: &mut BitReader) -> Box<[u16; 512]> {
        let mut s = Box::new([0u16; 512]);
        for v in s.iter_mut().take(256) {
            *v = r.u(self.p.bd_y) as u16;
        }
        let nc = 2 * 8 * self.p.mbh_c;
        for v in s[256..256 + nc].iter_mut() {
            *v = r.u(self.p.bd_c) as u16;
        }
        s
    }

    // ------------------------------------------------------------ CABAC

    fn parse_cabac(
        &mut self,
        s: &mut CabacState,
        nb: &Nb,
        cur: &mut MbInfo,
        d: &mut MbData,
    ) -> Result<Option<i32>, Error> {
        let ctx = &mut s.ctx;
        let e = &mut s.e;
        // mb_type (I slice).
        let inc = nb.a.is_some_and(|m| m.kind != KIND_I4 && m.kind != KIND_I8) as usize
            + nb.b.is_some_and(|m| m.kind != KIND_I4 && m.kind != KIND_I8) as usize;
        if e.decision(&mut ctx[3 + inc]) == 0 {
            d.kind = KIND_I4;
        } else if e.terminate() == 1 {
            d.kind = KIND_PCM;
            let r = e.reader_mut();
            r.align();
            d.pcm = Some(self.read_pcm(r));
            r.check()?;
            e.restart()?;
            return Ok(None);
        } else {
            d.kind = KIND_I16;
            let luma = e.decision(&mut ctx[3 + 3]);
            let chroma = if e.decision(&mut ctx[3 + 4]) == 0 {
                0
            } else if e.decision(&mut ctx[3 + 5]) == 0 {
                1
            } else {
                2
            };
            // The two prediction-mode bins use ctxIdx 9 and 10 either way.
            let b0 = e.decision(&mut ctx[3 + 6]);
            let b1 = e.decision(&mut ctx[3 + 7]);
            d.i16_mode = (b0 * 2 + b1) as u8;
            d.cbp_chroma = chroma;
            d.cbp_luma = if luma == 1 { 15 } else { 0 };
        }
        if d.kind == KIND_I4 {
            if self.p.transform8x8_mode {
                let inc = nb.a.is_some_and(|m| m.transform8x8()) as usize
                    + nb.b.is_some_and(|m| m.transform8x8()) as usize;
                if e.decision(&mut ctx[399 + inc]) == 1 {
                    d.kind = KIND_I8;
                }
            }
            cur.kind = d.kind;
            let n = if d.kind == KIND_I8 { 4 } else { 16 };
            for i in 0..n {
                let (bx, by) = if n == 4 {
                    ((i % 2) * 2, (i / 2) * 2)
                } else {
                    let (x, y) = BLK4_XY[i];
                    (x as usize / 4, y as usize / 4)
                };
                let pred = self.predicted_mode(nb, cur, bx, by);
                let mode = if e.decision(&mut ctx[68]) == 1 {
                    pred
                } else {
                    let mut rem = 0u8;
                    for k in 0..3 {
                        rem |= (e.decision(&mut ctx[69]) as u8) << k;
                    }
                    if rem < pred { rem } else { rem + 1 }
                };
                if n == 4 {
                    for k in 0..4 {
                        cur.modes[i * 4 + k] = mode;
                    }
                } else {
                    cur.modes[i] = mode;
                }
            }
            d.modes = cur.modes;
        }
        // intra_chroma_pred_mode
        let cm = |m: &Option<MbInfo>| m.is_some_and(|m| m.kind != KIND_PCM && m.chroma_mode != 0);
        let inc = cm(&nb.a) as usize + cm(&nb.b) as usize;
        d.chroma_mode = if e.decision(&mut ctx[64 + inc]) == 0 {
            0
        } else if e.decision(&mut ctx[67]) == 0 {
            1
        } else if e.decision(&mut ctx[67]) == 0 {
            2
        } else {
            3
        };
        cur.chroma_mode = d.chroma_mode;
        if d.kind != KIND_I16 {
            // coded_block_pattern: luma prefix, one bin per 8×8 block.
            let mut cbp = 0u8;
            for b8 in 0..4usize {
                let cond_a = if b8 % 2 == 1 {
                    (cbp >> (b8 - 1)) & 1 == 0
                } else {
                    nb.a.is_some_and(|m| m.kind != KIND_PCM && (m.cbp_luma() >> (b8 + 1)) & 1 == 0)
                };
                let cond_b = if b8 >= 2 {
                    (cbp >> (b8 - 2)) & 1 == 0
                } else {
                    nb.b.is_some_and(|m| m.kind != KIND_PCM && (m.cbp_luma() >> (b8 + 2)) & 1 == 0)
                };
                let inc = cond_a as usize + 2 * cond_b as usize;
                cbp |= (e.decision(&mut ctx[73 + inc]) as u8) << b8;
            }
            let c0 =
                |m: &Option<MbInfo>| m.is_some_and(|m| m.kind == KIND_PCM || m.cbp_chroma() != 0);
            let c1 =
                |m: &Option<MbInfo>| m.is_some_and(|m| m.kind == KIND_PCM || m.cbp_chroma() == 2);
            let inc = c0(&nb.a) as usize + 2 * c0(&nb.b) as usize;
            let chroma = if e.decision(&mut ctx[77 + inc]) == 0 {
                0
            } else {
                let inc = c1(&nb.a) as usize + 2 * c1(&nb.b) as usize;
                1 + e.decision(&mut ctx[77 + 4 + inc]) as u8
            };
            d.cbp_luma = cbp;
            d.cbp_chroma = chroma;
        }
        cur.cbp = d.cbp_luma | (d.cbp_chroma << 4);
        if d.cbp_luma == 0 && d.cbp_chroma == 0 && d.kind != KIND_I16 {
            return Ok(None);
        }
        // mb_qp_delta
        let mut dqp = 0i32;
        if e.decision(&mut ctx[60 + self.prev_dqp_nonzero as usize]) == 1 {
            let mut k = 1i32;
            let limit = 2 * (26 + self.p.qp_bd_y / 2) + 2;
            let mut c = 62;
            while e.decision(&mut ctx[c]) == 1 {
                k += 1;
                c = 63;
                if k > limit {
                    return Err(Error::corrupt("mb_qp_delta out of range"));
                }
            }
            dqp = if k & 1 == 1 { (k + 1) / 2 } else { -(k / 2) };
        }
        // Residual.
        if d.kind == KIND_I16 {
            let ca = nb.a.is_none_or(|m| m.cbf >> CBF_LUMA_DC & 1 == 1);
            let cb = nb.b.is_none_or(|m| m.cbf >> CBF_LUMA_DC & 1 == 1);
            let inc = ca as usize + 2 * cb as usize;
            if residual_cabac(e, ctx, 0, 1, Some(inc), &ZIGZAG4, &mut d.dc)? {
                cur.cbf |= 1 << CBF_LUMA_DC;
            }
        }
        for b8 in 0..4 {
            if d.cbp_luma & (1 << b8) == 0 {
                continue;
            }
            if d.kind == KIND_I8 {
                let out = &mut d.luma[b8 * 64..b8 * 64 + 64];
                residual_cabac(e, ctx, 5, 1, None, &ZIGZAG8, out)?;
                cur.cbf |= 0xf << (b8 * 4);
                d.luma_coded |= 0xf << (b8 * 4);
                continue;
            }
            for k in 0..4 {
                let blk = b8 * 4 + k;
                let (x, y) = BLK4_XY[blk];
                let (bx, by) = (x as usize / 4, y as usize / 4);
                let ca = if bx > 0 {
                    cur.cbf >> BLK4_AT[by][bx - 1] & 1 == 1
                } else {
                    nb.a.is_none_or(|m| m.cbf >> BLK4_AT[by][3] & 1 == 1)
                };
                let cb = if by > 0 {
                    cur.cbf >> BLK4_AT[by - 1][bx] & 1 == 1
                } else {
                    nb.b.is_none_or(|m| m.cbf >> BLK4_AT[3][bx] & 1 == 1)
                };
                let inc = ca as usize + 2 * cb as usize;
                let out = &mut d.luma[blk * 16..blk * 16 + 16];
                let coded = if d.kind == KIND_I16 {
                    residual_cabac(e, ctx, 1, 1, Some(inc), &ZIGZAG4[1..], out)?
                } else {
                    residual_cabac(e, ctx, 2, 1, Some(inc), &ZIGZAG4, out)?
                };
                if coded {
                    cur.cbf |= 1 << blk;
                    d.luma_coded |= 1 << blk;
                }
            }
        }
        let nblk = 4 * self.p.num_c8x8;
        if d.cbp_chroma & 3 != 0 {
            for c in 0..2 {
                let bit = CBF_CDC + c as u32;
                let f = |m: &Option<MbInfo>| {
                    m.is_none_or(|m| {
                        m.kind == KIND_PCM || (m.cbp_chroma() != 0 && m.cbf >> bit & 1 == 1)
                    })
                };
                let inc = f(&nb.a) as usize + 2 * f(&nb.b) as usize;
                let scan = &IDENT[..nblk];
                let nc = self.p.num_c8x8;
                if residual_cabac(e, ctx, 3, nc, Some(inc), scan, &mut d.cdc[c])? {
                    cur.cbf |= 1 << bit;
                }
            }
        }
        if d.cbp_chroma & 2 != 0 {
            let rows = self.p.mbh_c / 4;
            for c in 0..2 {
                let base = CBF_CAC + 8 * c as u32;
                for blk in 0..nblk {
                    let (bx, by) = (blk % 2, blk / 2);
                    let nbit = |m: &MbInfo, k: usize| {
                        m.kind == KIND_PCM
                            || (m.cbp_chroma() == 2 && m.cbf >> (base + k as u32) & 1 == 1)
                    };
                    let ca = if bx > 0 {
                        cur.cbf >> (base + (blk - 1) as u32) & 1 == 1
                    } else {
                        nb.a.is_none_or(|m| nbit(&m, by * 2 + 1))
                    };
                    let cb = if by > 0 {
                        cur.cbf >> (base + (blk - 2) as u32) & 1 == 1
                    } else {
                        nb.b.is_none_or(|m| nbit(&m, (rows - 1) * 2 + bx))
                    };
                    let inc = ca as usize + 2 * cb as usize;
                    let out = &mut d.cac[c][blk * 16..blk * 16 + 16];
                    if residual_cabac(e, ctx, 4, 1, Some(inc), &ZIGZAG4[1..], out)? {
                        cur.cbf |= 1 << (base + blk as u32);
                        d.cac_coded[c] |= 1 << blk;
                    }
                }
            }
        }
        if e.overrun() {
            return Err(Error::corrupt("the slice data ends early"));
        }
        Ok(Some(dqp))
    }

    // ------------------------------------------------------------ reconstruction

    fn reconstruct(&mut self, addr: usize, nb: &Nb, d: &mut MbData) -> Result<(), Error> {
        let w = self.p.w_mbs;
        let (mx, my) = ((addr % w) * 16, (addr / w) * 16);
        let mbh_c = self.p.mbh_c;
        let (cx, cy) = ((addr % w) * 8, (addr / w) * mbh_c);
        if let Some(pcm) = &d.pcm {
            for y in 0..16 {
                let i = self.cv.yi(mx, my + y);
                self.cv.y[i..i + 16].copy_from_slice(&pcm[y * 16..y * 16 + 16]);
            }
            for c in 0..2 {
                let off = 256 + c * 8 * mbh_c;
                for y in 0..mbh_c {
                    let i = self.cv.ci(cx, cy + y);
                    self.cv.c[c][i..i + 8].copy_from_slice(&pcm[off + y * 8..off + y * 8 + 8]);
                }
            }
            return Ok(());
        }
        let qp = self.qp + self.p.qp_bd_y;
        let (qd, qm) = (qp / 6, (qp % 6) as usize);
        let _ = qd;
        let bd = self.p.bd_y;
        let maxy = (1i32 << bd) - 1;
        let left = nb.a.is_some();
        let top = nb.b.is_some();
        match d.kind {
            KIND_I16 => {
                let mut tp = [0i32; 17];
                let mut lf = [0i32; 16];
                if top {
                    let i = self.cv.yi(mx, my - 1);
                    for x in 0..16 {
                        tp[1 + x] = self.cv.y[i + x] as i32;
                    }
                }
                if left {
                    for (y, l) in lf.iter_mut().enumerate() {
                        *l = self.cv.y[self.cv.yi(mx - 1, my + y)] as i32;
                    }
                }
                if nb.d_avail {
                    tp[0] = self.cv.y[self.cv.yi(mx - 1, my - 1)] as i32;
                }
                let a = Avail {
                    left,
                    top,
                    top_left: nb.d_avail,
                    top_right: false,
                };
                let mut pred = [0i32; 256];
                intra::pred16x16(d.i16_mode, &tp, &lf, a, bd, &mut pred)?;
                let ls = &self.p.ls4[0][qm];
                let mut dc = d.dc;
                transform::luma_dc(&mut dc, ls[0], qp);
                for blk in 0..16 {
                    let (x, y) = BLK4_XY[blk];
                    let (x, y) = (x as usize, y as usize);
                    let mut c: [i32; 16] = d.luma[blk * 16..blk * 16 + 16]
                        .try_into()
                        .unwrap_or([0; 16]);
                    let dcv = dc[(y / 4) * 4 + x / 4];
                    if d.luma_coded >> blk & 1 == 1 {
                        transform::scale4(&mut c, ls, qp, true);
                        c[0] = dcv;
                        transform::idct4(&mut c);
                    } else {
                        let v = (dcv + 32) >> 6;
                        c = [v; 16];
                    }
                    for yy in 0..4 {
                        let i = self.cv.yi(mx + x, my + y + yy);
                        for xx in 0..4 {
                            let p = pred[(y + yy) * 16 + x + xx] + c[yy * 4 + xx];
                            self.cv.y[i + xx] = p.clamp(0, maxy) as u16;
                        }
                    }
                }
            }
            KIND_I4 => {
                let ls = &self.p.ls4[0][qm];
                for blk in 0..16 {
                    let (x, y) = BLK4_XY[blk];
                    let (x, y) = (x as usize, y as usize);
                    let a = self.avail_sub(nb, x, y, 4, blk);
                    let mut tp = [0i32; 9];
                    let mut lf = [0i32; 4];
                    let (px, py) = (mx + x, my + y);
                    if a.top {
                        let i = self.cv.yi(px, py - 1);
                        for k in 0..4 {
                            tp[1 + k] = self.cv.y[i + k] as i32;
                        }
                        if a.top_right {
                            for k in 4..8 {
                                tp[1 + k] = self.cv.y[i + k] as i32;
                            }
                        } else {
                            for k in 4..8 {
                                tp[1 + k] = tp[4];
                            }
                        }
                    }
                    if a.left {
                        for (k, l) in lf.iter_mut().enumerate() {
                            *l = self.cv.y[self.cv.yi(px - 1, py + k)] as i32;
                        }
                    }
                    if a.top_left {
                        tp[0] = self.cv.y[self.cv.yi(px - 1, py - 1)] as i32;
                    }
                    let mut pred = [0i32; 16];
                    intra::pred4x4(d.modes[blk], &tp, &lf, a, bd, &mut pred)?;
                    if d.luma_coded >> blk & 1 == 1 {
                        let mut c: [i32; 16] = d.luma[blk * 16..blk * 16 + 16]
                            .try_into()
                            .unwrap_or([0; 16]);
                        transform::scale4(&mut c, ls, qp, false);
                        transform::idct4(&mut c);
                        for k in 0..16 {
                            pred[k] += c[k];
                        }
                    }
                    for yy in 0..4 {
                        let i = self.cv.yi(px, py + yy);
                        for xx in 0..4 {
                            self.cv.y[i + xx] = pred[yy * 4 + xx].clamp(0, maxy) as u16;
                        }
                    }
                }
            }
            _ => {
                // Intra 8x8
                let ls = &self.p.ls8[qm];
                for b8 in 0..4 {
                    let (x, y) = ((b8 % 2) * 8, (b8 / 2) * 8);
                    let a = self.avail_sub(nb, x, y, 8, b8);
                    let mut tp = [0i32; 17];
                    let mut lf = [0i32; 8];
                    let (px, py) = (mx + x, my + y);
                    if a.top {
                        let i = self.cv.yi(px, py - 1);
                        for k in 0..8 {
                            tp[1 + k] = self.cv.y[i + k] as i32;
                        }
                        if a.top_right {
                            for k in 8..16 {
                                tp[1 + k] = self.cv.y[i + k] as i32;
                            }
                        } else {
                            for k in 8..16 {
                                tp[1 + k] = tp[8];
                            }
                        }
                    }
                    if a.left {
                        for (k, l) in lf.iter_mut().enumerate() {
                            *l = self.cv.y[self.cv.yi(px - 1, py + k)] as i32;
                        }
                    }
                    if a.top_left {
                        tp[0] = self.cv.y[self.cv.yi(px - 1, py - 1)] as i32;
                    }
                    let mut pred = [0i32; 64];
                    intra::pred8x8(d.modes[b8 * 4], &tp, &lf, a, bd, &mut pred)?;
                    if (d.luma_coded >> (b8 * 4)) & 0xf != 0 {
                        let mut c: [i32; 64] =
                            d.luma[b8 * 64..b8 * 64 + 64].try_into().unwrap_or([0; 64]);
                        transform::scale8(&mut c, ls, qp);
                        transform::idct8(&mut c);
                        for k in 0..64 {
                            pred[k] += c[k];
                        }
                    }
                    for yy in 0..8 {
                        let i = self.cv.yi(px, py + yy);
                        for xx in 0..8 {
                            self.cv.y[i + xx] = pred[yy * 8 + xx].clamp(0, maxy) as u16;
                        }
                    }
                }
            }
        }
        // Chroma.
        let bdc = self.p.bd_c;
        let maxc = (1i32 << bdc) - 1;
        let nblk = 4 * self.p.num_c8x8;
        for c in 0..2 {
            let mut tp = [0i32; 9];
            let mut lf = [0i32; 16];
            if top {
                let i = self.cv.ci(cx, cy - 1);
                for x in 0..8 {
                    tp[1 + x] = self.cv.c[c][i + x] as i32;
                }
            }
            if left {
                for (y, l) in lf.iter_mut().enumerate().take(mbh_c) {
                    *l = self.cv.c[c][self.cv.ci(cx - 1, cy + y)] as i32;
                }
            }
            if nb.d_avail {
                tp[0] = self.cv.c[c][self.cv.ci(cx - 1, cy - 1)] as i32;
            }
            let a = Avail {
                left,
                top,
                top_left: nb.d_avail,
                top_right: false,
            };
            let mut pred = [0i32; 128];
            intra::pred_chroma(d.chroma_mode, &tp, &lf, mbh_c, a, bdc, &mut pred)?;
            if d.cbp_chroma != 0 {
                let qpc = self.p.chroma_qp(self.qp, c) + self.p.qp_bd_c;
                let ls = &self.p.ls4[1 + c][(qpc % 6) as usize];
                let dcs = if self.p.num_c8x8 == 1 {
                    transform::chroma_dc420(&d.cdc[c], ls[0], qpc)
                } else {
                    let qdc = qpc + 3;
                    let ls_dc = self.p.ls4[1 + c][(qdc % 6) as usize][0];
                    transform::chroma_dc422(&d.cdc[c], ls_dc, qdc)
                };
                for blk in 0..nblk {
                    let (x, y) = ((blk % 2) * 4, (blk / 2) * 4);
                    let res: [i32; 16] = if d.cac_coded[c] >> blk & 1 == 1 {
                        let mut co: [i32; 16] = d.cac[c][blk * 16..blk * 16 + 16]
                            .try_into()
                            .unwrap_or([0; 16]);
                        transform::scale4(&mut co, ls, qpc, true);
                        co[0] = dcs[blk];
                        transform::idct4(&mut co);
                        co
                    } else {
                        [(dcs[blk] + 32) >> 6; 16]
                    };
                    for yy in 0..4 {
                        for xx in 0..4 {
                            pred[(y + yy) * 8 + x + xx] += res[yy * 4 + xx];
                        }
                    }
                }
            }
            for y in 0..mbh_c {
                let i = self.cv.ci(cx, cy + y);
                for x in 0..8 {
                    self.cv.c[c][i + x] = pred[y * 8 + x].clamp(0, maxc) as u16;
                }
            }
        }
        Ok(())
    }

    /// Neighbour availability of an N×N luma block at (x, y) in the
    /// macroblock; `idx` is its decoding index (4×4 or 8×8 order).
    fn avail_sub(&self, nb: &Nb, x: usize, y: usize, n: usize, idx: usize) -> Avail {
        let left = x > 0 || nb.a.is_some();
        let top = y > 0 || nb.b.is_some();
        let top_left = match (x > 0, y > 0) {
            (true, true) => true,
            (false, true) => nb.a.is_some(),
            (true, false) => nb.b.is_some(),
            (false, false) => nb.d_avail,
        };
        let xn = x + n;
        let top_right = if y == 0 {
            if xn < 16 { nb.b.is_some() } else { nb.c_avail }
        } else if xn >= 16 {
            false
        } else if n == 4 {
            (BLK4_AT[(y - 1) / 4][xn / 4] as usize) < idx
        } else {
            ((y - 1) / 8) * 2 + xn / 8 < idx
        };
        Avail {
            left,
            top,
            top_left,
            top_right,
        }
    }
}

fn nc_of(a: Option<u8>, b: Option<u8>) -> i32 {
    match (a, b) {
        (Some(a), Some(b)) => (a as i32 + b as i32 + 1) >> 1,
        (Some(a), None) => a as i32,
        (None, Some(b)) => b as i32,
        (None, None) => 0,
    }
}

/// ctxIdxInc of significant_coeff_flag (frame) for 8×8 blocks (Table 9-43).
const SIG8: [u8; 63] = [
    0, 1, 2, 3, 4, 5, 5, 4, 4, 3, 3, 4, 4, 4, 5, 5, 4, 4, 4, 4, 3, 3, 6, 7, 7, 7, 8, 9, 10, 9, 8,
    7, 7, 6, 11, 12, 13, 11, 6, 7, 8, 9, 14, 10, 9, 8, 6, 11, 12, 13, 11, 6, 9, 14, 10, 9, 11, 12,
    13, 11, 14, 10, 12,
];
/// ctxIdxInc of last_significant_coeff_flag for 8×8 blocks (Table 9-43).
const LAST8: [u8; 63] = [
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8,
];
const CBF_OFF: [usize; 5] = [0, 4, 8, 12, 16];
const SIG_OFF: [usize; 5] = [0, 15, 29, 44, 47];
const ABS_OFF: [usize; 5] = [0, 10, 20, 30, 39];

/// Identity scan (chroma DC) and the ctxIdxInc of each coefficient index
/// for blocks whose increment is the index itself.
const IDENT: [u8; 64] = {
    let mut t = [0u8; 64];
    let mut i = 0;
    while i < 64 {
        t[i] = i as u8;
        i += 1;
    }
    t
};
/// Chroma DC ctxIdxInc, Min(i / NumC8x8, 2), for 4:2:0 and 4:2:2.
const CDC_INC: [[u8; 7]; 2] = [[0, 1, 2, 2, 2, 2, 2], [0, 0, 1, 1, 2, 2, 2]];

/// residual_block_cabac (7.3.5.3.3) for ctxBlockCat `cat`. `cbf_inc` is the
/// coded_block_flag ctxIdxInc, `None` when the flag is not coded (cat 5).
/// Coefficient `i` lands at `out[scan[i]]` (the block's coefficient count
/// is `scan.len()`); returns whether the block is coded.
fn residual_cabac(
    e: &mut Engine,
    ctx: &mut [u8; NUM_CTX],
    cat: usize,
    num_c8x8: usize,
    cbf_inc: Option<usize>,
    scan: &[u8],
    out: &mut [i32],
) -> Result<bool, Error> {
    if let Some(inc) = cbf_inc
        && e.decision(&mut ctx[85 + CBF_OFF[cat] + inc]) == 0
    {
        return Ok(false);
    }
    let max = scan.len();
    let (sig_base, last_base, abs_base) = if cat == 5 {
        (402, 417, 426)
    } else {
        (105 + SIG_OFF[cat], 166 + SIG_OFF[cat], 227 + ABS_OFF[cat])
    };
    let (sig_inc, last_inc): (&[u8], &[u8]) = match cat {
        5 => (&SIG8, &LAST8),
        3 => {
            let t = &CDC_INC[num_c8x8 - 1];
            (t, t)
        }
        _ => (&IDENT, &IDENT),
    };
    let (lo, hi) = ctx.split_at_mut(last_base);
    let sig_ctx = &mut lo[sig_base..];
    let (last_ctx, abs_ctx) = hi.split_at_mut(abs_base - last_base);
    let mut sig = [0u8; 64];
    let mut n = 0usize;
    let mut i = 0usize;
    while i + 1 < max {
        if e.decision(&mut sig_ctx[sig_inc[i] as usize]) == 1 {
            sig[n] = i as u8;
            n += 1;
            if e.decision(&mut last_ctx[last_inc[i] as usize]) == 1 {
                break;
            }
        }
        i += 1;
    }
    if i + 1 == max {
        sig[n] = (max - 1) as u8;
        n += 1;
    }
    let mut eq1 = 0usize;
    let mut gt1 = 0usize;
    let gt1_cap = if cat == 3 { 3 } else { 4 };
    for &pos in sig[..n].iter().rev() {
        let inc0 = if gt1 != 0 { 0 } else { (1 + eq1).min(4) };
        let mut abs = 1i32;
        if e.decision(&mut abs_ctx[inc0]) == 1 {
            let c = &mut abs_ctx[5 + gt1.min(gt1_cap)];
            let mut prefix = 1;
            while prefix < 14 && e.decision(c) == 1 {
                prefix += 1;
            }
            abs = prefix + 1;
            if prefix == 14 {
                // UEG0 suffix
                let mut k = 0u32;
                let mut suf = 0i32;
                while e.bypass() == 1 {
                    suf += 1 << k;
                    k += 1;
                    if k > 24 {
                        return Err(Error::corrupt("coefficient level too large"));
                    }
                }
                while k > 0 {
                    k -= 1;
                    suf += (e.bypass() as i32) << k;
                }
                abs += suf;
            }
            gt1 += 1;
        } else {
            eq1 += 1;
        }
        let v = if e.bypass() == 1 { -abs } else { abs };
        out[scan[pos as usize] as usize] = v;
    }
    Ok(true)
}

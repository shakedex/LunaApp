//! Look-up tables from `.cube` files (the Resolve/Adobe format every grading
//! tool writes) and from tables a camera embeds in its clips.
//!
//! 3D tables are applied by tetrahedral interpolation, 1D tables per channel
//! by linear interpolation. Rows are in `.cube` order: red changes fastest.

use std::fmt;

/// A 1D or 3D LUT with its input domain.
#[derive(Debug, Clone, PartialEq)]
pub struct Lut {
    kind: Kind,
    size: usize,
    /// `size³` (3D) or `size` (1D) RGB rows, flattened.
    rows: Vec<f32>,
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    OneD,
    ThreeD,
}

/// Why a table can't be used, as a sentence for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LutError(pub String);

impl fmt::Display for LutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LutError {}

impl Lut {
    /// Parses a `.cube` file's text.
    pub fn parse_cube(text: &str) -> Result<Lut, LutError> {
        let err = |m: String| Err(LutError(m));
        let mut shape = None;
        let (mut min, mut max) = ([0.0; 3], [1.0; 3]);
        let mut rows = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut words = line.split_whitespace();
            let first = words.next().unwrap_or_default();
            let rest: Vec<&str> = words.collect();
            match first {
                "TITLE" => {}
                "LUT_3D_SIZE" | "LUT_1D_SIZE" => {
                    let kind = if first == "LUT_3D_SIZE" {
                        Kind::ThreeD
                    } else {
                        Kind::OneD
                    };
                    let size = rest.first().and_then(|s| s.parse::<usize>().ok());
                    let range = if kind == Kind::ThreeD {
                        2..=129
                    } else {
                        2..=65536
                    };
                    match size {
                        Some(s) if range.contains(&s) => shape = Some((kind, s)),
                        _ => return err(format!("Line {} has an unusable LUT size", n + 1)),
                    }
                }
                "DOMAIN_MIN" | "DOMAIN_MAX" => {
                    let Some(v) = triple(&rest) else {
                        return err(format!("Line {} isn't three numbers", n + 1));
                    };
                    if first == "DOMAIN_MIN" {
                        min = v;
                    } else {
                        max = v;
                    }
                }
                // Other keywords (LUT_1D_INPUT_RANGE and vendor extras) are skipped.
                k if k.starts_with(|c: char| c.is_ascii_alphabetic()) => {}
                _ => {
                    let words: Vec<&str> = std::iter::once(first).chain(rest).collect();
                    match triple(&words) {
                        Some(v) => rows.extend(v),
                        None => return err(format!("Line {} isn't three numbers", n + 1)),
                    }
                }
            }
        }
        let Some((kind, size)) = shape else {
            return err("The file has no LUT_3D_SIZE or LUT_1D_SIZE line".into());
        };
        if (0..3).any(|c| max[c] <= min[c]) {
            return err("DOMAIN_MAX must be above DOMAIN_MIN".into());
        }
        Lut::new(kind, size, rows, min, max)
    }

    /// A 3D table in `.cube` row order with a 0–1 domain.
    pub fn from_table_3d(size: usize, rgb: Vec<f32>) -> Result<Lut, LutError> {
        if size < 2 {
            return Err(LutError(format!("A {size}-point table can't be used")));
        }
        Lut::new(Kind::ThreeD, size, rgb, [0.0; 3], [1.0; 3])
    }

    fn new(
        kind: Kind,
        size: usize,
        rows: Vec<f32>,
        min: [f32; 3],
        max: [f32; 3],
    ) -> Result<Lut, LutError> {
        let (want, shape) = match kind {
            Kind::ThreeD => (
                size * size * size,
                format!("{size}³ ({})", size * size * size),
            ),
            Kind::OneD => (size, size.to_string()),
        };
        if rows.len() != want * 3 {
            return Err(LutError(format!(
                "The file says {shape} rows but has {}",
                rows.len() / 3
            )));
        }
        if rows.iter().any(|v| !v.is_finite()) {
            return Err(LutError(
                "The table holds a value that isn't a number".into(),
            ));
        }
        Ok(Lut {
            kind,
            size,
            rows,
            min,
            max,
        })
    }

    /// The table's output for `rgb`; input outside the domain is clamped.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = (self.size - 1) as f32;
        let pos: [f32; 3] = std::array::from_fn(|c| {
            let v = if rgb[c].is_nan() { 0.0 } else { rgb[c] };
            ((v - self.min[c]) / (self.max[c] - self.min[c])).clamp(0.0, 1.0) * n
        });
        match self.kind {
            Kind::OneD => std::array::from_fn(|c| {
                let i = (pos[c] as usize).min(self.size - 2);
                let f = pos[c] - i as f32;
                let (a, b) = (self.rows[i * 3 + c], self.rows[(i + 1) * 3 + c]);
                a + (b - a) * f
            }),
            Kind::ThreeD => self.tetrahedral(pos),
        }
    }

    fn row(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        let i = ((b * self.size + g) * self.size + r) * 3;
        [self.rows[i], self.rows[i + 1], self.rows[i + 2]]
    }

    fn tetrahedral(&self, pos: [f32; 3]) -> [f32; 3] {
        let base = pos.map(|p| (p as usize).min(self.size - 2));
        let [fr, fg, fb] = std::array::from_fn(|c| pos[c] - base[c] as f32);
        let [r, g, b] = base;
        let c000 = self.row(r, g, b);
        let c111 = self.row(r + 1, g + 1, b + 1);
        // The tetrahedron is chosen by the order of the fractional parts; the
        // result walks from c000 to c111 along the cube edges in that order.
        let (c1, c2, w) = if fr > fg {
            if fg > fb {
                (
                    self.row(r + 1, g, b),
                    self.row(r + 1, g + 1, b),
                    [fr, fg, fb],
                )
            } else if fr > fb {
                (
                    self.row(r + 1, g, b),
                    self.row(r + 1, g, b + 1),
                    [fr, fb, fg],
                )
            } else {
                (
                    self.row(r, g, b + 1),
                    self.row(r + 1, g, b + 1),
                    [fb, fr, fg],
                )
            }
        } else if fb > fg {
            (
                self.row(r, g, b + 1),
                self.row(r, g + 1, b + 1),
                [fb, fg, fr],
            )
        } else if fb > fr {
            (
                self.row(r, g + 1, b),
                self.row(r, g + 1, b + 1),
                [fg, fb, fr],
            )
        } else {
            (
                self.row(r, g + 1, b),
                self.row(r + 1, g + 1, b),
                [fg, fr, fb],
            )
        };
        std::array::from_fn(|c| {
            c000[c] + (c1[c] - c000[c]) * w[0] + (c2[c] - c1[c]) * w[1] + (c111[c] - c2[c]) * w[2]
        })
    }
}

fn triple(words: &[&str]) -> Option<[f32; 3]> {
    let [a, b, c] = words else {
        return None;
    };
    Some([a.parse().ok()?, b.parse().ok()?, c.parse().ok()?])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(size: usize) -> Vec<f32> {
        let n = (size - 1) as f32;
        let mut rows = Vec::with_capacity(size * size * size * 3);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    rows.extend([r as f32 / n, g as f32 / n, b as f32 / n]);
                }
            }
        }
        rows
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn identity_3d_returns_input() {
        let lut = Lut::from_table_3d(17, identity(17)).unwrap();
        let out = lut.apply([0.25, 0.5, 0.75]);
        assert!(close(out, [0.25, 0.5, 0.75]), "{out:?}");
        let out = lut.apply([0.123, 0.987, 0.4567]);
        assert!(close(out, [0.123, 0.987, 0.4567]), "{out:?}");
    }

    #[test]
    fn tetrahedral_hits_corners() {
        let rows: Vec<f32> = (0..24).map(|i| i as f32 / 23.0).collect();
        let lut = Lut::from_table_3d(2, rows.clone()).unwrap();
        for i in 0..8 {
            let input = [(i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32];
            let want = [rows[i * 3], rows[i * 3 + 1], rows[i * 3 + 2]];
            assert!(close(lut.apply(input), want), "corner {i}");
        }
    }

    #[test]
    fn red_axis_is_red_fastest() {
        let mut rows = vec![0.0; 24];
        rows[3..6].copy_from_slice(&[1.0, 0.0, 0.0]);
        let lut = Lut::from_table_3d(2, rows).unwrap();
        assert!(close(lut.apply([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]));
        assert!(close(lut.apply([0.0, 0.0, 1.0]), [0.0, 0.0, 0.0]));
    }

    #[test]
    fn domain_is_honoured() {
        let text = "LUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2\n\
                    0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
        let lut = Lut::parse_cube(text).unwrap();
        assert!(close(lut.apply([1.0, 1.0, 1.0]), [0.5, 0.5, 0.5]));
        assert!(close(lut.apply([3.0, 3.0, 3.0]), [1.0, 1.0, 1.0]));
    }

    #[test]
    fn one_d_applies_per_channel() {
        let lut = Lut::parse_cube("LUT_1D_SIZE 2\n0 0 0\n1 0.5 0.25\n").unwrap();
        assert!(close(lut.apply([1.0, 1.0, 1.0]), [1.0, 0.5, 0.25]));
        assert!(close(lut.apply([0.5, 0.5, 0.5]), [0.5, 0.25, 0.125]));
    }

    #[test]
    fn crlf_comments_title_parse() {
        let text = "# made by hand\r\nTITLE \"Show LUT\"\r\n\r\nLUT_3D_SIZE 2\r\n\
                    0 0 0\r\n1 0 0\r\n0 1 0\r\n1 1 0\r\n# mid-table comment\r\n\
                    0 0 1\r\n1 0 1\r\n0 1 1\r\n1 1 1\r\n";
        let lut = Lut::parse_cube(text).unwrap();
        assert!(close(lut.apply([0.5, 0.25, 1.0]), [0.5, 0.25, 1.0]));
    }

    #[test]
    fn wrong_row_count_is_an_error() {
        let err = Lut::parse_cube("LUT_3D_SIZE 2\n0 0 0\n1 1 1\n").unwrap_err();
        assert!(err.0.contains('8') && err.0.contains('2'), "{err}");
    }

    #[test]
    fn missing_size_is_an_error() {
        assert!(Lut::parse_cube("0 0 0\n1 1 1\n").is_err());
    }

    #[test]
    fn non_numeric_row_names_the_line() {
        let err = Lut::parse_cube("LUT_3D_SIZE 2\n0 0 0\n1 x 0\n").unwrap_err();
        assert!(err.0.contains("Line 3"), "{err}");
    }

    #[test]
    fn embedded_table_of_the_wrong_length_is_an_error() {
        assert!(Lut::from_table_3d(2, vec![0.0; 23]).is_err());
    }
}

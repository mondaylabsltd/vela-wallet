//! The part of a TrueType font a fixed layout needs: character → glyph
//! (`cmap` 4 and 12), advances (`hmtx`), outlines (`glyf`, simple and
//! composite), vertical metrics (`hhea`), and pair kerning (`GPOS` lookup
//! type 2, formats 1 and 2, reached through the `kern` feature).
//!
//! The fonts are ours and embedded, so a malformed table is a build bug: every
//! read is still bounds-checked and answers `None`/nothing rather than panics.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// A glyph outline as closed contours of line and quadratic segments.
#[derive(Default, Debug)]
pub struct Outline {
    pub segments: Vec<Segment>,
}

#[derive(Clone, Copy, Debug)]
pub enum Segment {
    Line(Point, Point),
    Quad(Point, Point, Point),
    /// Not in TrueType outlines; the logo's SVG paths use it.
    Cubic(Point, Point, Point, Point),
}

impl Segment {
    /// The same segment with every point moved by `f`.
    pub fn map(&self, f: impl Fn(Point) -> Point) -> Segment {
        match *self {
            Segment::Line(a, b) => Segment::Line(f(a), f(b)),
            Segment::Quad(a, c, b) => Segment::Quad(f(a), f(c), f(b)),
            Segment::Cubic(a, c1, c2, b) => Segment::Cubic(f(a), f(c1), f(c2), f(b)),
        }
    }

    /// Its points, control points included (a bounding hull).
    pub fn points(&self) -> Vec<Point> {
        match *self {
            Segment::Line(a, b) => vec![a, b],
            Segment::Quad(a, c, b) => vec![a, c, b],
            Segment::Cubic(a, c1, c2, b) => vec![a, c1, c2, b],
        }
    }
}

pub struct Font<'a> {
    pub units_per_em: f32,
    pub ascender: f32,
    pub descender: f32,
    pub line_gap: f32,
    num_glyphs: u16,
    num_h_metrics: u16,
    long_loca: bool,
    cmap: &'a [u8],
    hmtx: &'a [u8],
    loca: &'a [u8],
    glyf: &'a [u8],
    gpos: Option<&'a [u8]>,
    kern_lookups: Vec<u16>,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}
fn i16_at(d: &[u8], at: usize) -> Option<i16> {
    u16_at(d, at).map(|v| v as i16)
}
fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
        *d.get(at + 2)?,
        *d.get(at + 3)?,
    ]))
}
fn sub(d: &[u8], at: usize) -> Option<&[u8]> {
    d.get(at..)
}

impl<'a> Font<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Font<'a>> {
        let num_tables = u16_at(data, 4)? as usize;
        let table = |tag: &[u8; 4]| -> Option<&'a [u8]> {
            (0..num_tables).find_map(|i| {
                let rec = 12 + 16 * i;
                if data.get(rec..rec + 4)? != tag {
                    return None;
                }
                let off = u32_at(data, rec + 8)? as usize;
                let len = u32_at(data, rec + 12)? as usize;
                data.get(off..off.checked_add(len)?)
            })
        };
        let head = table(b"head")?;
        let hhea = table(b"hhea")?;
        let maxp = table(b"maxp")?;
        let mut font = Font {
            units_per_em: f32::from(u16_at(head, 18)?),
            long_loca: i16_at(head, 50)? == 1,
            ascender: f32::from(i16_at(hhea, 4)?),
            descender: f32::from(i16_at(hhea, 6)?),
            line_gap: f32::from(i16_at(hhea, 8)?),
            num_h_metrics: u16_at(hhea, 34)?,
            num_glyphs: u16_at(maxp, 4)?,
            cmap: table(b"cmap")?,
            hmtx: table(b"hmtx")?,
            loca: table(b"loca")?,
            glyf: table(b"glyf")?,
            gpos: table(b"GPOS"),
            kern_lookups: Vec::new(),
        };
        font.kern_lookups = font.find_kern_lookups().unwrap_or_default();
        Some(font)
    }

    // ── cmap ────────────────────────────────────────────────────────────────

    pub fn glyph_id(&self, c: char) -> Option<u16> {
        let cmap = self.cmap;
        let n = u16_at(cmap, 2)? as usize;
        let mut best: Option<&[u8]> = None;
        for i in 0..n {
            let rec = 4 + 8 * i;
            let platform = u16_at(cmap, rec)?;
            let encoding = u16_at(cmap, rec + 2)?;
            let sub_table = sub(cmap, u32_at(cmap, rec + 4)? as usize)?;
            let format = u16_at(sub_table, 0)?;
            let unicode = platform == 0 || (platform == 3 && (encoding == 1 || encoding == 10));
            if unicode && (format == 12 || (format == 4 && best.is_none())) {
                best = Some(sub_table);
            }
        }
        let t = best?;
        let cp = c as u32;
        let gid = match u16_at(t, 0)? {
            4 => {
                if cp > 0xffff {
                    return None;
                }
                let seg_x2 = u16_at(t, 6)? as usize;
                let ends = 14;
                let starts = ends + seg_x2 + 2;
                let deltas = starts + seg_x2;
                let ranges = deltas + seg_x2;
                let mut found = None;
                for s in (0..seg_x2).step_by(2) {
                    let end = u32::from(u16_at(t, ends + s)?);
                    if cp > end {
                        continue;
                    }
                    let start = u32::from(u16_at(t, starts + s)?);
                    if cp < start {
                        break;
                    }
                    let delta = u16_at(t, deltas + s)?;
                    let range = u16_at(t, ranges + s)? as usize;
                    let g = if range == 0 {
                        (cp as u16).wrapping_add(delta)
                    } else {
                        let at = ranges + s + range + 2 * (cp - start) as usize;
                        let g = u16_at(t, at)?;
                        if g == 0 {
                            0
                        } else {
                            g.wrapping_add(delta)
                        }
                    };
                    found = Some(g);
                    break;
                }
                found?
            }
            12 => {
                let groups = u32_at(t, 12)? as usize;
                let mut found = None;
                for g in 0..groups {
                    let rec = 16 + 12 * g;
                    let (start, end) = (u32_at(t, rec)?, u32_at(t, rec + 4)?);
                    if (start..=end).contains(&cp) {
                        found = Some((u32_at(t, rec + 8)? + (cp - start)) as u16);
                        break;
                    }
                }
                found?
            }
            _ => return None,
        };
        (gid != 0 && gid < self.num_glyphs).then_some(gid)
    }

    // ── hmtx ────────────────────────────────────────────────────────────────

    pub fn num_glyphs(&self) -> u16 {
        self.num_glyphs
    }

    /// Left side bearing from `hmtx` (font units).
    pub(crate) fn lsb(&self, gid: u16) -> i16 {
        let n = self.num_h_metrics as usize;
        let i = gid as usize;
        let at = if i < n {
            4 * i + 2
        } else {
            4 * n + 2 * (i - n)
        };
        i16_at(self.hmtx, at).unwrap_or(0)
    }

    pub fn advance(&self, gid: u16) -> f32 {
        let i = gid.min(self.num_h_metrics.saturating_sub(1)) as usize;
        f32::from(u16_at(self.hmtx, 4 * i).unwrap_or(0))
    }

    // ── glyf ────────────────────────────────────────────────────────────────

    /// A glyph's raw `glyf` record (empty for a glyph with no outline).
    pub(crate) fn glyph_data(&self, gid: u16) -> Option<&'a [u8]> {
        let i = gid as usize;
        let (start, end) = if self.long_loca {
            (
                u32_at(self.loca, 4 * i)? as usize,
                u32_at(self.loca, 4 * i + 4)? as usize,
            )
        } else {
            (
                2 * u16_at(self.loca, 2 * i)? as usize,
                2 * u16_at(self.loca, 2 * i + 2)? as usize,
            )
        };
        if end <= start {
            return Some(&[]);
        }
        self.glyf.get(start..end)
    }

    /// The outline in font units, y up.
    pub fn outline(&self, gid: u16) -> Outline {
        let mut out = Outline::default();
        self.append_outline(gid, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0, &mut out);
        out
    }

    fn append_outline(&self, gid: u16, m: [f32; 6], depth: u8, out: &mut Outline) {
        if depth > 8 {
            return;
        }
        let Some(g) = self.glyph_data(gid) else {
            return;
        };
        if g.len() < 10 {
            return;
        }
        let contours = i16_at(g, 0).unwrap_or(0);
        if contours >= 0 {
            if let Some(points) = simple_points(g, contours as usize) {
                for contour in points {
                    let t: Vec<(Point, bool)> = contour
                        .into_iter()
                        .map(|(p, on)| (transform(m, p), on))
                        .collect();
                    contour_segments(&t, &mut out.segments);
                }
            }
        } else {
            let mut at = 10;
            loop {
                let (Some(flags), Some(child)) = (u16_at(g, at), u16_at(g, at + 2)) else {
                    return;
                };
                at += 4;
                let (dx, dy) = if flags & 0x0001 != 0 {
                    let r = (i16_at(g, at), i16_at(g, at + 2));
                    at += 4;
                    match r {
                        (Some(a), Some(b)) => (f32::from(a), f32::from(b)),
                        _ => return,
                    }
                } else {
                    let r = (g.get(at).copied(), g.get(at + 1).copied());
                    at += 2;
                    match r {
                        (Some(a), Some(b)) => (f32::from(a as i8), f32::from(b as i8)),
                        _ => return,
                    }
                };
                // Point-matching placement (args are point indices) is not
                // used by these fonts; such a component is placed unshifted.
                let (dx, dy) = if flags & 0x0002 != 0 {
                    (dx, dy)
                } else {
                    (0.0, 0.0)
                };
                let f2 = |at: usize| i16_at(g, at).map(|v| f32::from(v) / 16384.0);
                let (a, b, c, d) = if flags & 0x0008 != 0 {
                    let s = f2(at).unwrap_or(1.0);
                    at += 2;
                    (s, 0.0, 0.0, s)
                } else if flags & 0x0040 != 0 {
                    let (sx, sy) = (f2(at).unwrap_or(1.0), f2(at + 2).unwrap_or(1.0));
                    at += 4;
                    (sx, 0.0, 0.0, sy)
                } else if flags & 0x0080 != 0 {
                    let v = (f2(at), f2(at + 2), f2(at + 4), f2(at + 6));
                    at += 8;
                    (
                        v.0.unwrap_or(1.0),
                        v.1.unwrap_or(0.0),
                        v.2.unwrap_or(0.0),
                        v.3.unwrap_or(1.0),
                    )
                } else {
                    (1.0, 0.0, 0.0, 1.0)
                };
                // child' = m ∘ (component transform)
                let cm = [
                    m[0] * a + m[2] * b,
                    m[1] * a + m[3] * b,
                    m[0] * c + m[2] * d,
                    m[1] * c + m[3] * d,
                    m[0] * dx + m[2] * dy + m[4],
                    m[1] * dx + m[3] * dy + m[5],
                ];
                self.append_outline(child, cm, depth + 1, out);
                if flags & 0x0020 == 0 {
                    return;
                }
            }
        }
    }

    // ── GPOS kerning ────────────────────────────────────────────────────────

    fn find_kern_lookups(&self) -> Option<Vec<u16>> {
        let gpos = self.gpos?;
        let features = sub(gpos, u16_at(gpos, 6)? as usize)?;
        let mut lookups = Vec::new();
        for i in 0..u16_at(features, 0)? as usize {
            let rec = 2 + 6 * i;
            if features.get(rec..rec + 4)? != b"kern" {
                continue;
            }
            let feature = sub(features, u16_at(features, rec + 4)? as usize)?;
            for j in 0..u16_at(feature, 2)? as usize {
                let l = u16_at(feature, 4 + 2 * j)?;
                if !lookups.contains(&l) {
                    lookups.push(l);
                }
            }
        }
        lookups.sort_unstable();
        Some(lookups)
    }

    /// The x-advance adjustment between two glyphs, in font units: each
    /// `kern` lookup contributes from the first of its subtables that applies.
    pub fn kerning(&self, left: u16, right: u16) -> f32 {
        let mut total = 0.0;
        let Some(gpos) = self.gpos else { return 0.0 };
        let lookup_list = || sub(gpos, u16_at(gpos, 8)? as usize);
        let Some(list) = lookup_list() else {
            return 0.0;
        };
        for &l in &self.kern_lookups {
            let Some(off) = u16_at(list, 2 + 2 * l as usize) else {
                continue;
            };
            let Some(lookup) = sub(list, off as usize) else {
                continue;
            };
            let kind = u16_at(lookup, 0).unwrap_or(0);
            let count = u16_at(lookup, 4).unwrap_or(0) as usize;
            for s in 0..count {
                let Some(st_off) = u16_at(lookup, 6 + 2 * s) else {
                    continue;
                };
                let Some(mut st) = sub(lookup, st_off as usize) else {
                    continue;
                };
                let mut st_kind = kind;
                if kind == 9 {
                    // Extension: the real subtable sits at a 32-bit offset.
                    let (Some(real), Some(ext)) = (u16_at(st, 2), u32_at(st, 4)) else {
                        continue;
                    };
                    st_kind = real;
                    let Some(inner) = sub(st, ext as usize) else {
                        continue;
                    };
                    st = inner;
                }
                if st_kind != 2 {
                    continue;
                }
                if let Some(v) = pair_adjust(st, left, right) {
                    total += v;
                    break;
                }
            }
        }
        total
    }
}

fn transform(m: [f32; 6], p: Point) -> Point {
    Point {
        x: m[0] * p.x + m[2] * p.y + m[4],
        y: m[1] * p.x + m[3] * p.y + m[5],
    }
}

/// A simple glyph's contours as (point, on-curve) lists, in font units.
fn simple_points(g: &[u8], contours: usize) -> Option<Vec<Vec<(Point, bool)>>> {
    let mut ends = Vec::with_capacity(contours);
    for i in 0..contours {
        ends.push(u16_at(g, 10 + 2 * i)? as usize);
    }
    let n = ends.last().map_or(0, |e| e + 1);
    let ins_len = u16_at(g, 10 + 2 * contours)? as usize;
    let mut at = 12 + 2 * contours + ins_len;
    let mut flags = Vec::with_capacity(n);
    while flags.len() < n {
        let f = *g.get(at)?;
        at += 1;
        flags.push(f);
        if f & 0x08 != 0 {
            let repeat = *g.get(at)?;
            at += 1;
            for _ in 0..repeat {
                flags.push(f);
            }
        }
    }
    flags.truncate(n);
    let mut read_coords = |short: u8, same: u8| -> Option<Vec<f32>> {
        let mut v = 0i32;
        let mut out = Vec::with_capacity(n);
        for &f in &flags {
            if f & short != 0 {
                let d = i32::from(*g.get(at)?);
                at += 1;
                v += if f & same != 0 { d } else { -d };
            } else if f & same == 0 {
                v += i32::from(i16_at(g, at)?);
                at += 2;
            }
            out.push(v as f32);
        }
        Some(out)
    };
    let xs = read_coords(0x02, 0x10)?;
    let ys = read_coords(0x04, 0x20)?;
    let mut out = Vec::with_capacity(contours);
    let mut start = 0;
    for &end in &ends {
        if end < start || end >= n {
            return None;
        }
        out.push(
            (start..=end)
                .map(|i| (Point { x: xs[i], y: ys[i] }, flags[i] & 0x01 != 0))
                .collect(),
        );
        start = end + 1;
    }
    Some(out)
}

fn mid(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    }
}

/// TrueType contour → segments, with the implied on-curve midpoints.
fn contour_segments(pts: &[(Point, bool)], out: &mut Vec<Segment>) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let first_on = (0..n).find(|&i| pts[i].1);
    let (start, begin) = match first_on {
        Some(i) => (pts[i].0, i),
        None => (mid(pts[0].0, pts[n - 1].0), 0),
    };
    let mut cur = start;
    let mut ctrl: Option<Point> = None;
    let order: Vec<usize> = if first_on.is_some() {
        (1..=n).map(|k| (begin + k) % n).collect()
    } else {
        (0..n).collect()
    };
    for i in order {
        let (p, on) = pts[i];
        match (on, ctrl) {
            (true, None) => {
                out.push(Segment::Line(cur, p));
                cur = p;
            }
            (true, Some(c)) => {
                out.push(Segment::Quad(cur, c, p));
                cur = p;
                ctrl = None;
            }
            (false, None) => ctrl = Some(p),
            (false, Some(c)) => {
                let m = mid(c, p);
                out.push(Segment::Quad(cur, c, m));
                cur = m;
                ctrl = Some(p);
            }
        }
    }
    match ctrl {
        Some(c) => out.push(Segment::Quad(cur, c, start)),
        None if cur != start => out.push(Segment::Line(cur, start)),
        None => {}
    }
}

fn coverage_index(cov: &[u8], gid: u16) -> Option<usize> {
    match u16_at(cov, 0)? {
        1 => {
            let n = u16_at(cov, 2)? as usize;
            (0..n).find(|&i| u16_at(cov, 4 + 2 * i) == Some(gid))
        }
        2 => {
            let n = u16_at(cov, 2)? as usize;
            for i in 0..n {
                let rec = 4 + 6 * i;
                let (s, e, base) = (
                    u16_at(cov, rec)?,
                    u16_at(cov, rec + 2)?,
                    u16_at(cov, rec + 4)?,
                );
                if (s..=e).contains(&gid) {
                    return Some((base + (gid - s)) as usize);
                }
            }
            None
        }
        _ => None,
    }
}

fn class_of(def: &[u8], gid: u16) -> u16 {
    let lookup = || -> Option<u16> {
        match u16_at(def, 0)? {
            1 => {
                let start = u16_at(def, 2)?;
                let n = u16_at(def, 4)?;
                if gid >= start && gid - start < n {
                    u16_at(def, 6 + 2 * (gid - start) as usize)
                } else {
                    Some(0)
                }
            }
            2 => {
                let n = u16_at(def, 2)? as usize;
                for i in 0..n {
                    let rec = 4 + 6 * i;
                    if (u16_at(def, rec)?..=u16_at(def, rec + 2)?).contains(&gid) {
                        return u16_at(def, rec + 4);
                    }
                }
                Some(0)
            }
            _ => Some(0),
        }
    };
    lookup().unwrap_or(0)
}

/// Bytes of a ValueRecord with this format, and where its XAdvance sits.
fn value_layout(format: u16) -> (usize, Option<usize>) {
    let size = 2 * (format & 0x00ff).count_ones() as usize;
    let x_adv = (format & 0x0004 != 0).then(|| 2 * (format & 0x0003).count_ones() as usize);
    (size, x_adv)
}

fn pair_adjust(st: &[u8], left: u16, right: u16) -> Option<f32> {
    let format = u16_at(st, 0)?;
    let cov = sub(st, u16_at(st, 2)? as usize)?;
    let index = coverage_index(cov, left)?;
    let vf1 = u16_at(st, 4)?;
    let vf2 = u16_at(st, 6)?;
    let (size1, x_adv) = value_layout(vf1);
    let (size2, _) = value_layout(vf2);
    match format {
        1 => {
            let set = sub(st, u16_at(st, 10 + 2 * index)? as usize)?;
            let n = u16_at(set, 0)? as usize;
            let rec_len = 2 + size1 + size2;
            for i in 0..n {
                let rec = 2 + rec_len * i;
                if u16_at(set, rec)? == right {
                    return Some(
                        x_adv
                            .and_then(|x| i16_at(set, rec + 2 + x))
                            .map_or(0.0, f32::from),
                    );
                }
            }
            None
        }
        2 => {
            let c1 = class_of(sub(st, u16_at(st, 8)? as usize)?, left);
            let c2 = class_of(sub(st, u16_at(st, 10)? as usize)?, right);
            let n2 = u16_at(st, 14)? as usize;
            if c1 >= u16_at(st, 12)? || c2 as usize >= n2 {
                return None;
            }
            let rec = 16 + (c1 as usize * n2 + c2 as usize) * (size1 + size2);
            Some(
                x_adv
                    .and_then(|x| i16_at(st, rec + x))
                    .map_or(0.0, f32::from),
            )
        }
        _ => None,
    }
}

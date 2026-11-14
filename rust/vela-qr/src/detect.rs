//! Locating a QR code in a black/white image and sampling its module grid.
//!
//! Ported from zxing-cpp `QRDetector.cpp` (Apache-2.0, see NOTICE), model 2
//! only: find finder patterns row by row, group them in threes, trace the
//! code's edges to place the fourth corner, locate the alignment patterns, and
//! sample the grid piecewise between them.

use crate::bitmatrix::BitMatrix;
use crate::concentric::{
    center_of_ring, find_concentric_pattern_corners, fit_square_to_points, is_pattern,
    locate_concentric_pattern, read_symmetric_pattern, Concentric, FINDER,
};
use crate::cursor::{Cursor, Direction};
use crate::geometry::{p, rectangle, rectangle_xy, Line, Quad, Transform, P};
use crate::version;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FinderSet {
    pub bl: Concentric,
    pub tl: Concentric,
    pub tr: Concentric,
}

// ---------------------------------------------------------------------------
// Finder patterns
// ---------------------------------------------------------------------------

/// Run lengths of one row: white, black, white, …, white (first/last may be 0).
fn runs(row: &[u8], out: &mut Vec<u16>) {
    out.clear();
    let (mut black, mut count) = (false, 0u16);
    for &px in row {
        let b = px != 0;
        if b != black {
            out.push(count);
            count = 0;
            black = b;
        }
        count = count.saturating_add(1);
    }
    out.push(count);
    if black {
        out.push(0);
    }
}

fn window(row: &[u16], at: usize) -> [u16; 5] {
    [row[at], row[at + 1], row[at + 2], row[at + 3], row[at + 4]]
}

/// A quick plausibility test for 1:1:3:1:1, then the real one.
fn is_finder(v: &[u16; 5], space_before: f64) -> bool {
    if v[2] < 3 || v[2] < 2 * v[0].max(v[4]) || v[2] < v[1].max(v[3]) {
        return false;
    }
    is_pattern(v, &FINDER, space_before, 0.1) != 0.0
}

/// The next black-bar index from `start` where a finder pattern begins. Every
/// window up to the last complete one is tried (its trailing white included).
fn find_pattern(row: &[u16], start: usize) -> Option<usize> {
    if start + 5 >= row.len() {
        return None;
    }
    if start == 1 && is_finder(&window(row, 1), f64::MAX) {
        return Some(1);
    }
    let mut at = start;
    while at + 5 < row.len() {
        if is_finder(&window(row, at), f64::from(row[at - 1])) {
            return Some(at);
        }
        at += 2;
    }
    None
}

pub(crate) fn find_finder_patterns(img: &BitMatrix) -> Vec<Concentric> {
    // Every third row: thorough ("try harder") — a phone frame is small enough.
    const SKIP: usize = 3;
    let mut found: Vec<Concentric> = Vec::new();
    let mut row = Vec::new();
    let mut y = SKIP - 1;
    while y < img.height() {
        runs(img.row(y), &mut row);
        let mut start = 1;
        while let Some(at) = find_pattern(&row, start) {
            let v = window(&row, at);
            let in_front: u32 = row[..at].iter().map(|&r| u32::from(r)).sum();
            let center = p(
                f64::from(in_front) + f64::from(v[0]) + f64::from(v[1]) + f64::from(v[2]) / 2.0,
                y as f64 + 0.5,
            );
            if !found
                .iter()
                .any(|old| center.distance(old.p) < f64::from(old.size / 2))
            {
                let width: i32 = v.iter().map(|&x| i32::from(x)).sum();
                // ×3 for very skewed samples.
                if let Some(c) = locate_concentric_pattern(img, &FINDER, center, width * 3) {
                    found.push(c);
                }
            }
            start = at + 4;
        }
        y += SKIP;
    }
    found
}

fn spiral(r: i32) -> impl Iterator<Item = (i32, i32)> {
    let center = (r == 0).then_some((0, 0));
    let top = (-r..r).map(move |x| (x, -r));
    let right = (-r..r).map(move |y| (r, y));
    let bottom = (-r + 1..=r).rev().map(move |x| (x, r));
    let left = (-r + 1..=r).rev().map(move |y| (-r, y));
    center
        .into_iter()
        .chain(top)
        .chain(right)
        .chain(bottom)
        .chain(left)
}

/// Plausible (bottom-left, top-left, top-right) triples, most plausible first.
pub(crate) fn generate_finder_sets(patterns: &mut [Concentric]) -> Vec<FinderSet> {
    // Larger patterns first: less likely to be noise.
    patterns.sort_by_key(|c| core::cmp::Reverse(c.size));
    let n = patterns.len();
    if n < 3 {
        return Vec::new();
    }
    let sq_dist = |a: &Concentric, b: &Concentric| {
        let d = a.p - b.p;
        // ×linear size ratio (not squared) so the cosine is not skewed.
        d.dot(d) * (f64::from(b.size) / f64::from(a.size))
    };
    let cos_upper = (60.0f64).to_radians().cos();
    let cos_lower = (120.0f64).to_radians().cos();

    // Spatial bins, so each pattern only meets its neighbours.
    let (mut min, mut max) = (patterns[0].p, patterns[0].p);
    for c in patterns.iter() {
        min = p(min.x.min(c.p.x), min.y.min(c.p.y));
        max = p(max.x.max(c.p.x), max.y.max(c.p.y));
    }
    let median_size = patterns[n / 2].size;
    let bin = f64::from(32.max(median_size * 3));
    let bins_w = (((max.x - min.x + 1.0) / bin).ceil() as usize).max(1);
    let bins_h = (((max.y - min.y + 1.0) / bin).ceil() as usize).max(1);
    let bin_of = |q: P| {
        (((q.x - min.x) / bin) as usize).min(bins_w - 1)
            + (((q.y - min.y) / bin) as usize).min(bins_h - 1) * bins_w
    };
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); bins_w * bins_h];
    for (i, c) in patterns.iter().enumerate() {
        bins[bin_of(c.p)].push(i);
    }

    const MAX_MODULES: f64 = 177.0 * 1.5;
    const MAX_CANDIDATES: usize = 15;
    let mut sets: Vec<(f64, FinderSet)> = Vec::new();
    let mut candidates = Vec::with_capacity(MAX_CANDIDATES);
    for i in 0..n - 2 {
        let c0 = patterns[i];
        let max_dist = f64::from(c0.size) / 7.0 * MAX_MODULES;
        let home = bin_of(c0.p);
        let (cx, cy) = ((home % bins_w) as i32, (home / bins_w) as i32);
        let radius = (max_dist / bin).ceil() as i32;
        candidates.clear();
        'outer: for r in 0..=radius {
            for (dx, dy) in spiral(r) {
                let (bx, by) = (cx + dx, cy + dy);
                if bx < 0 || by < 0 || bx >= bins_w as i32 || by >= bins_h as i32 {
                    continue;
                }
                for &idx in &bins[by as usize * bins_w + bx as usize] {
                    if idx <= i || c0.size > patterns[idx].size * 2 {
                        continue;
                    }
                    candidates.push(idx);
                    if candidates.len() >= MAX_CANDIDATES {
                        break 'outer;
                    }
                }
            }
        }
        for u in 0..candidates.len().saturating_sub(1) {
            for v in u + 1..candidates.len() {
                let (mut a, mut b, mut c) = (
                    patterns[candidates[v]],
                    patterns[candidates[u]],
                    patterns[i],
                );
                let (mut ab, mut bc, mut ac) = (sq_dist(&a, &b), sq_dist(&b, &c), sq_dist(&a, &c));
                // b becomes the corner opposite the longest side (top-left).
                if bc >= ab && bc >= ac {
                    core::mem::swap(&mut a, &mut b);
                    core::mem::swap(&mut bc, &mut ac);
                } else if ab >= ac && ab >= bc {
                    core::mem::swap(&mut b, &mut c);
                    core::mem::swap(&mut ab, &mut ac);
                }
                // The two legs differ by at most 2× in length…
                if ab > 4.0 * bc || bc > 4.0 * ab {
                    continue;
                }
                let (dab, dbc) = (ab.sqrt(), bc.sqrt());
                // …span a plausible number of modules…
                let modules =
                    (dab + dbc) / (2.0 * f64::from(a.size + b.size + c.size) / 21.0) + 7.0;
                if !(21.0 * 0.9..=177.0 * 1.5).contains(&modules) {
                    continue;
                }
                // …and meet at 60–120°.
                let cos = (ab + bc - ac) / (2.0 * dab * dbc);
                if cos.is_nan() || cos > cos_upper || cos < cos_lower {
                    continue;
                }
                // Clockwise: a is bottom-left, c top-right.
                if (c.p - b.p).cross(a.p - b.p) < 0.0 {
                    core::mem::swap(&mut a, &mut c);
                }
                let score = dab + dbc + (dab - dbc).abs();
                sets.push((
                    score,
                    FinderSet {
                        bl: a,
                        tl: b,
                        tr: c,
                    },
                ));
            }
        }
    }
    sets.sort_by(|x, y| x.0.total_cmp(&y.0));
    sets.into_iter().map(|(_, s)| s).collect()
}

// ---------------------------------------------------------------------------
// Sampling
// ---------------------------------------------------------------------------

fn estimate_module_size(img: &BitMatrix, a: Concentric, b: Concentric) -> f64 {
    let mut cur = Cursor::new(img, a.p, b.p - a.p);
    if !cur.is_black() {
        return -1.0;
    }
    let Some(v) = read_symmetric_pattern(&mut cur, a.size * 2) else {
        return -1.0;
    };
    if is_pattern(&v, &FINDER, f64::MAX, 0.0) == 0.0 {
        return -1.0;
    }
    let sum: f64 = v.iter().map(|&x| f64::from(x)).sum();
    (2.0 * sum - f64::from(v[0]) - f64::from(v[4])) / 12.0 * cur.d.length()
}

struct DimensionEstimate {
    dim: i32,
    ms: f64,
    err: i32,
}

fn estimate_dimension(img: &BitMatrix, a: Concentric, b: Concentric) -> DimensionEstimate {
    let (ma, mb) = (
        estimate_module_size(img, a, b),
        estimate_module_size(img, b, a),
    );
    if ma < 0.0 || mb < 0.0 {
        return DimensionEstimate {
            dim: 0,
            ms: 0.0,
            err: 4,
        };
    }
    let ms = (ma + mb) / 2.0;
    let dim = (a.p.distance(b.p) / ms).round() as i32 + 7;
    // A real dimension is 4n+1.
    let error = 1 - dim.rem_euclid(4);
    DimensionEstimate {
        dim: dim + error,
        ms,
        err: error.abs(),
    }
}

/// The `edge`th edge of the finder at `from`, on the side facing away from
/// `toward`, traced and fitted as a line.
fn trace_line(img: &BitMatrix, from: P, toward: P, edge: i32) -> Line {
    let mut cur = Cursor::new(img, from, toward - from);
    let mut line = Line::default();
    line.set_direction_inward(cur.back());
    // Collect points inside the black line: back up on the 3rd edge.
    cur.step_to_edge(edge, 0, edge == 3);
    if edge == 3 {
        cur.turn_back();
    }
    let mut ci = Cursor::new(
        img,
        p(cur.p.x.trunc(), cur.p.y.trunc()),
        cur.d.main_direction(),
    );
    // Position so that the white→black edge is directly behind.
    let mut guard = 0;
    while ci.is_in() && !ci.edge_at(ci.back()).is_valid() {
        guard += 1;
        if guard > 4 * (img.width() + img.height()) {
            return Line::default();
        }
        if ci.edge_at_dir(Direction::Left).is_valid() {
            ci.turn_right();
        } else if ci.edge_at_dir(Direction::Right).is_valid() {
            ci.turn_left();
        } else {
            ci.step(-1.0);
        }
    }
    for dir in [Direction::Left, Direction::Right] {
        let mut c = Cursor::new(img, ci.p, ci.direction(dir));
        let mut steps = (cur.p - from).max_abs() as i32;
        loop {
            line.add(c.p.centered());
            steps -= 1;
            if !(steps > 0 && c.step_along_edge(dir, true)) {
                break;
            }
        }
    }
    line.evaluate_max_distance(1.0);
    line
}

fn estimate_tilt(fp: &FinderSet) -> f64 {
    let s = [fp.bl.size, fp.tl.size, fp.tr.size];
    f64::from(*s.iter().max().unwrap_or(&1)) / f64::from((*s.iter().min().unwrap_or(&1)).max(1))
}

/// Module coordinates → pixels, from the three finders' centres and a fourth point.
fn mod_to_pix(dimension: i32, br_offset: P, pix: &Quad) -> Option<Transform> {
    let mut quad = rectangle(f64::from(dimension), f64::from(dimension), 3.5);
    quad[2] -= br_offset;
    Transform::between(&quad, pix)
}

fn locate_alignment_pattern(img: &BitMatrix, module: i32, estimate: P) -> Option<P> {
    for d in [
        p(0.0, 0.0),
        p(0.0, -1.0),
        p(0.0, 1.0),
        p(-1.0, 0.0),
        p(1.0, 0.0),
        p(-1.0, -1.0),
        p(1.0, -1.0),
        p(1.0, 1.0),
        p(-1.0, 1.0),
    ] {
        let probe = (estimate + d * (f64::from(module) * 2.25)).floor();
        if !img.is_in(probe) {
            continue;
        }
        // We must land on black, or the concentric finder fails.
        let Some(cor) = center_of_ring(img, probe, module * 3, 1, false) else {
            continue;
        };
        if !img.at(cor) {
            continue;
        }
        if let (Some(c1), Some(c2)) = (
            center_of_ring(img, cor, module, 1, true),
            center_of_ring(img, cor, module * 3, -2, true),
        ) {
            if c1.distance(c2) < f64::from(module / 2) {
                return Some((c1 + c2) / 2.0);
            }
        }
    }
    None
}

fn read_version(img: &BitMatrix, dimension: i32, m2p: &Transform) -> Option<u32> {
    let mut bits = [0i64; 2];
    for (k, mirror) in [false, true].into_iter().enumerate() {
        let mut v: i64 = 0;
        for y in (0..=5).rev() {
            for x in (dimension - 11..=dimension - 9).rev() {
                let m = if mirror {
                    p(f64::from(y), f64::from(x))
                } else {
                    p(f64::from(x), f64::from(y))
                };
                let px = m2p.apply(m.centered());
                if !img.is_in(px) {
                    v = -1;
                } else {
                    v = (v << 1) | i64::from(img.at(px));
                }
            }
        }
        bits[k] = v;
    }
    version::decode_version_info(bits[0], bits[1])
}

struct Roi {
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
    m2p: Transform,
}

/// The module grid, read at each module's centre through its region's transform.
fn sample_grid(img: &BitMatrix, dimension: i32, rois: &[Roi]) -> Option<BitMatrix> {
    if dimension <= 0 {
        return None;
    }
    for r in rois {
        if !r.m2p.is_valid() {
            return None;
        }
    }
    let n = dimension as usize;
    let mut bits = BitMatrix::new(n, n);
    for r in rois {
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let q = r.m2p.apply(p(f64::from(x) + 0.5, f64::from(y) + 0.5));
                if !img.is_in(q) {
                    return None;
                }
                if img.at(q) {
                    bits.set(x as usize, y as usize);
                }
            }
        }
    }
    Some(bits)
}

pub(crate) fn sample_qr(img: &BitMatrix, fp: &FinderSet) -> Option<BitMatrix> {
    let top = estimate_dimension(img, fp.tl, fp.tr);
    let left = estimate_dimension(img, fp.tl, fp.bl);
    if top.dim == 0 && left.dim == 0 {
        return None;
    }
    let (top_dim, left_dim) = (top.dim, left.dim);
    let best = match top.err.cmp(&left.err) {
        core::cmp::Ordering::Less => top,
        core::cmp::Ordering::Equal => {
            if top.dim > left.dim {
                top
            } else {
                left
            }
        }
        core::cmp::Ordering::Greater => left,
    };
    let mut dimension = best.dim;
    let module = (best.ms + 1.0) as i32;

    let mut br = p(-1.0, -1.0);
    let mut br_offset = p(3.0, 3.0);

    // Every version but 1 has an alignment pattern near the bottom-right corner:
    // estimate it by intersecting the extended edges of the other two finders.
    let bl2 = trace_line(img, fp.bl.p, fp.tl.p, 2);
    let bl3 = trace_line(img, fp.bl.p, fp.tl.p, 3);
    let tr2 = trace_line(img, fp.tr.p, fp.tl.p, 2);
    let tr3 = trace_line(img, fp.tr.p, fp.tl.p, 3);
    if bl2.is_valid() && tr2.is_valid() && bl3.is_valid() && tr3.is_valid() {
        let inter = (Line::intersect(&bl2, &tr2)? + Line::intersect(&bl3, &tr3)?) / 2.0;
        if dimension > 21 {
            if let Some(found) = locate_alignment_pattern(img, module, inter) {
                br = found;
            }
        }
        // Tilted, or lines resolved finely enough: their intersection is the best guess.
        if !img.is_in(br)
            && (estimate_tilt(fp) > 1.1
                || (bl2.is_high_res()
                    && bl3.is_high_res()
                    && tr2.is_high_res()
                    && tr3.is_high_res()))
        {
            br = inter;
        }
    }
    // Otherwise the parallelogram guess.
    if !img.is_in(br) || fit_square_to_points(img, fp.bl.p, fp.bl.size, 2, false).is_none() {
        br = fp.tr.p - fp.tl.p + fp.bl.p;
        br_offset = p(0.0, 0.0);
    }
    let corners = [fp.tl.p, fp.tr.p, br, fp.bl.p];
    let mut m2p = mod_to_pix(dimension, br_offset, &corners)?;

    if dimension < version::dimension(7) {
        return sample_grid(
            img,
            dimension,
            &[Roi {
                x0: 0,
                x1: dimension,
                y0: 0,
                y1: dimension,
                m2p,
            }],
        );
    }

    // Version 7+: the version is written in the symbol — trust it over the estimate.
    let v = read_version(img, dimension, &m2p)?;
    let vdim = version::dimension(v);
    if (vdim - top_dim).abs().min((vdim - left_dim).abs()) > 8 {
        return None;
    }
    if vdim != dimension {
        dimension = vdim;
        m2p = mod_to_pix(dimension, br_offset, &corners)?;
    }

    // Alignment pattern centres: found, or guessed from their neighbours.
    let ap_m: Vec<i32> = version::alignment(v)
        .iter()
        .map(|&c| i32::from(c))
        .collect();
    let len = ap_m.len();
    let last = len - 1;
    let mut ap: Vec<Option<P>> = vec![None; len * len];
    let project = |m2p: &Transform, x: usize, y: usize| {
        m2p.apply(p(f64::from(ap_m[x]) + 0.5, f64::from(ap_m[y]) + 0.5))
    };

    // The finders' inner corners stand in for the three missing alignment patterns.
    for (x, y, fpc) in [(0, 0, fp.tl), (0, last, fp.bl), (last, 0, fp.tr)] {
        let pc = project(&m2p, x, y);
        ap[y * len + x] = Some(pc);
        if let Some(quad) = find_concentric_pattern_corners(img, fpc.p, fpc.size, 2) {
            for c in quad {
                if c.distance(pc) < f64::from(fpc.size) / 2.0 {
                    ap[y * len + x] = Some(c);
                }
            }
        }
    }
    for y in 0..len {
        for x in 0..len {
            if ap[y * len + x].is_some() {
                continue;
            }
            let guess = |xx: usize, yy: usize, ap: &[Option<P>]| {
                ap[yy * len + xx].unwrap_or_else(|| project(&m2p, xx, yy))
            };
            let guessed = if x * y == 0 {
                guess(x, y, &ap)
            } else {
                guess(x - 1, y, &ap) + guess(x, y - 1, &ap) - guess(x - 1, y - 1, &ap)
            };
            if let Some(found) = locate_alignment_pattern(img, module, guessed) {
                ap[y * len + x] = Some(found);
            }
        }
    }
    // Still missing: intersect lines through the nearest found neighbours.
    for y in 0..len {
        for x in 0..len {
            if ap[y * len + x].is_some() {
                continue;
            }
            let (mut hori, mut verti) = (Vec::with_capacity(2), Vec::with_capacity(2));
            let mut i = 2usize;
            while i < 2 * last + 2 && hori.len() < 2 {
                let xi = x as isize + (i / 2) as isize * if i % 2 != 0 { 1 } else { -1 };
                if (0..=last as isize).contains(&xi) {
                    if let Some(q) = ap[y * len + xi as usize] {
                        hori.push(q);
                    }
                }
                i += 1;
            }
            let mut i = 2usize;
            while i < 2 * last + 2 && verti.len() < 2 {
                let yi = y as isize + (i / 2) as isize * if i % 2 != 0 { 1 } else { -1 };
                if (0..=last as isize).contains(&yi) {
                    if let Some(q) = ap[yi as usize * len + x] {
                        verti.push(q);
                    }
                }
                i += 1;
            }
            if hori.len() == 2 && verti.len() == 2 {
                let guessed = Line::intersect(
                    &Line::through(hori[0], hori[1]),
                    &Line::through(verti[0], verti[1]),
                )?;
                ap[y * len + x] =
                    Some(locate_alignment_pattern(img, module, guessed).unwrap_or(guessed));
            }
        }
    }
    if let Some(c) = ap[last * len + last] {
        m2p = mod_to_pix(dimension, p(3.0, 3.0), &[fp.tl.p, fp.tr.p, c, fp.bl.p])?;
    }
    for y in 0..len {
        for x in 0..len {
            if ap[y * len + x].is_none() {
                ap[y * len + x] = Some(project(&m2p, x, y));
            }
        }
    }
    let at = |x: usize, y: usize| ap[y * len + x].unwrap_or_default();
    let mut rois = Vec::with_capacity(last * last);
    for y in 0..last {
        for x in 0..last {
            let (x0, x1, y0, y1) = (ap_m[x], ap_m[x + 1], ap_m[y], ap_m[y + 1]);
            let from = rectangle_xy(f64::from(x0), f64::from(x1), f64::from(y0), f64::from(y1));
            let to = [at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y + 1)];
            rois.push(Roi {
                x0: x0 - if x == 0 { 6 } else { 0 },
                x1: x1 + if x == last - 1 { 7 } else { 0 },
                y0: y0 - if y == 0 { 6 } else { 0 },
                y1: y1 + if y == last - 1 { 7 } else { 0 },
                m2p: Transform::between(&from, &to)?,
            });
        }
    }
    sample_grid(img, dimension, &rois)
}

#[cfg(feature = "probe")]
pub(crate) fn probe_row(img: &BitMatrix, y: usize, x0: usize, x1: usize) -> String {
    use core::fmt::Write;
    let mut row = Vec::new();
    runs(img.row(y), &mut row);
    let mut out = String::new();
    let mut px = 0u32;
    for at in 0..row.len() {
        if at % 2 == 1 && at + 5 < row.len() && (px as usize) >= x0 && (px as usize) <= x1 {
            let v = window(&row, at);
            let _ = writeln!(
                out,
                "  at {at} x={px} {:?} prev {} -> finder {} (module {:.2})",
                v,
                row[at - 1],
                is_finder(&v, f64::from(row[at - 1])),
                is_pattern(&v, &FINDER, f64::from(row[at - 1]), 0.1)
            );
        }
        px += u32::from(row[at]);
    }
    out
}

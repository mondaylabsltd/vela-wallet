//! Finding and measuring concentric patterns — the 1:1:3:1:1 finder squares
//! and the 1:1:1 alignment squares.
//!
//! Ported from zxing-cpp `ConcentricFinder.{h,cpp}` and `Pattern.h`
//! (Apache-2.0, see NOTICE).

use crate::bitmatrix::BitMatrix;
use crate::cursor::{Cursor, Direction, EdgeCounter};
use crate::geometry::{blend, p, Line, Quad, P};

/// The finder pattern, end to end in modules: bar, space, bar, space, bar.
pub(crate) const FINDER: [u16; 5] = [1, 1, 3, 1, 1];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Concentric {
    pub p: P,
    /// Width of the whole pattern in pixels (averaged over the probed directions).
    pub size: i32,
}

/// The module size if `v` reads as `pattern` (bar and space widths judged
/// separately, so blur that fattens bars still passes); 0 if it does not.
pub(crate) fn is_pattern(
    v: &[u16; 5],
    pattern: &[u16; 5],
    space_in_pixel: f64,
    min_quiet_zone: f64,
) -> f64 {
    let bars = f64::from(v[0] + v[2] + v[4]);
    let spaces = f64::from(v[1] + v[3]);
    let bar_sum = f64::from(pattern[0] + pattern[2] + pattern[4]);
    let space_sum = f64::from(pattern[1] + pattern[3]);
    let module = [bars / bar_sum, spaces / space_sum];
    let (m, big) = (module[0].min(module[1]), module[0].max(module[1]));
    if big > 4.0 * m {
        return 0.0;
    }
    if min_quiet_zone != 0.0 && space_in_pixel < min_quiet_zone * module[1] {
        return 0.0;
    }
    let thr = [module[0] * 0.75 + 0.5, module[1] / 3.0 + 0.5];
    for x in 0..5 {
        if (f64::from(v[x]) - f64::from(pattern[x]) * module[x & 1]).abs() > thr[x & 1] {
            return 0.0;
        }
    }
    (module[0] + module[1]) / 2.0
}

/// The pattern straddling `cur` along its direction, read outwards both ways.
pub(crate) fn read_symmetric_pattern(cur: &mut Cursor<'_>, range: i32) -> Option<[u16; 5]> {
    let mut res = [0i32; 5];
    let mut range = range;
    let mut back = cur.turned_back();
    for i in 0..=2i32 {
        for (c, idx) in [(&mut *cur, 2 + i), (&mut back, 2 - i)] {
            let v = c.step_to_edge(1, range, false);
            res[idx as usize] += v;
            if range != 0 {
                range -= v;
            }
            if v == 0 {
                return None;
            }
        }
    }
    res[2] -= 1; // the starting pixel was counted twice
    let mut out = [0u16; 5];
    for (o, r) in out.iter_mut().zip(res) {
        *o = u16::try_from(r).ok()?;
    }
    Some(out)
}

/// Total width if the run lengths through `cur` (both ways) form `pattern`,
/// else 0. With `update_position`, recentres `cur` on the middle bar.
fn check_symmetric_pattern(
    cur: &mut Cursor<'_>,
    pattern: &[u16; 5],
    range: i32,
    update_position: bool,
) -> i32 {
    let mut range = range;
    let mut fwd = EdgeCounter::new(cur);
    let mut bwd = EdgeCounter::new(&cur.turned_back());
    let center_fwd = fwd.step_to_next_edge(range);
    if center_fwd == 0 {
        return 0;
    }
    let center_bwd = bwd.step_to_next_edge(range);
    if center_bwd == 0 {
        return 0;
    }
    let mut res = [0i32; 5];
    res[2] = center_fwd + center_bwd - 1; // the starting pixel counted twice
    range -= res[2];
    for i in 1..=2usize {
        let v = fwd.step_to_next_edge(range);
        res[2 + i] = v;
        range -= v;
        if v == 0 {
            return 0;
        }
        let v = bwd.step_to_next_edge(range);
        res[2 - i] = v;
        range -= v;
        if v == 0 {
            return 0;
        }
    }
    let mut v = [0u16; 5];
    for (o, r) in v.iter_mut().zip(res) {
        *o = r as u16;
    }
    if is_pattern(&v, pattern, f64::MAX, 0.0) == 0.0 {
        return 0;
    }
    if update_position {
        cur.step(f64::from(res[2] / 2 - (center_bwd - 1)));
    }
    res.iter().sum()
}

/// Which of the eight neighbour directions `d` points into: `d` scaled to a
/// max-norm of 1, floored (as rxing does — never (0, 0), and every
/// direction of a closed ring gets a bit).
fn octant(d: P) -> (i32, i32) {
    let b = d.bresenham().floor();
    (b.x as i32, b.y as i32)
}

fn ring_bit(d: P) -> u32 {
    let (bx, by) = octant(d);
    1 << (4 + bx + 3 * by)
}

/// Walk the `nth` ring around `center` (inner side for negative `nth`) and
/// return the average of its edge pixels; with `require_circle`, only if the
/// walk came round through all eight directions.
pub(crate) fn center_of_ring(
    img: &BitMatrix,
    center: P,
    range: i32,
    nth: i32,
    require_circle: bool,
) -> Option<P> {
    let center = center.floor();
    let inner = nth < 0;
    let mut cur = Cursor::new(img, center, p(0.0, 1.0));
    if cur.step_to_edge(nth.abs(), range, inner) == 0 {
        return None;
    }
    cur.turn_right();
    let edge_dir = if inner {
        Direction::Left
    } else {
        Direction::Right
    };
    let start = cur.p;
    let (mut mask, mut sum, mut n) = (0u32, P::default(), 0i32);
    loop {
        sum += cur.p.centered();
        n += 1;
        mask |= ring_bit(cur.p - center);
        if !cur.step_along_edge(edge_dir, false) {
            return None;
        }
        if (cur.p - center).max_abs() > f64::from(range) || center == cur.p || n > 4 * 2 * range {
            return None;
        }
        if cur.p == start {
            break;
        }
    }
    if require_circle && mask != 0b1_1110_1111 {
        return None;
    }
    Some(sum / f64::from(n))
}

fn center_of_rings(img: &BitMatrix, center: P, range: i32, num_rings: i32) -> Option<P> {
    let (mut n, mut sum) = (1, center);
    for i in 2..=num_rings {
        match center_of_ring(img, center, range, i, true) {
            None if n == 1 => return None,
            None => return Some(sum / f64::from(n)),
            Some(c) => {
                if c.distance(center) > f64::from(range / num_rings / 2) {
                    return None;
                }
                sum += c;
                n += 1;
            }
        }
    }
    Some(sum / f64::from(n))
}

fn average_edge_pixels(mut cur: Cursor<'_>, range: i32, edges: i32) -> Option<P> {
    let mut sum = P::default();
    for _ in 0..edges {
        if !cur.is_in() {
            return None;
        }
        cur.step_to_edge(1, range, false);
        sum += cur.p.centered() + (cur.p + cur.back()).centered();
    }
    Some(sum / f64::from(2 * edges))
}

fn center_of_double_cross(img: &BitMatrix, center: P, range: i32, edges: i32) -> Option<P> {
    let mut sum = P::default();
    for d in [p(0.0, 1.0), p(1.0, 0.0), p(1.0, 1.0), p(1.0, -1.0)] {
        let a = average_edge_pixels(Cursor::new(img, center, d), range, edges)?;
        let b = average_edge_pixels(Cursor::new(img, center, -d), range, edges)?;
        sum += a + b;
    }
    Some(sum / 8.0)
}

/// The pixels along the `edge_index`th ring, or nothing if it is not a closed ring.
fn collect_ring_points(
    img: &BitMatrix,
    center: P,
    range: i32,
    edge_index: i32,
    backup: bool,
) -> Vec<P> {
    let center = center.floor();
    let mut cur = Cursor::new(img, center, p(0.0, 1.0));
    if cur.step_to_edge(edge_index, range, backup) == 0 {
        return Vec::new();
    }
    cur.turn_right();
    let edge_dir = if backup {
        Direction::Left
    } else {
        Direction::Right
    };
    let start = cur.p;
    let mut mask = 0u32;
    let mut points = Vec::with_capacity(4 * range.max(0) as usize);
    loop {
        points.push(cur.p.centered());
        mask |= ring_bit(cur.p - center);
        if !cur.step_along_edge(edge_dir, false) {
            return Vec::new();
        }
        if (cur.p - center).max_abs() > f64::from(range)
            || center == cur.p
            || points.len() > 4 * 2 * range.max(0) as usize
        {
            return Vec::new();
        }
        if cur.p == start {
            break;
        }
    }
    if mask != 0b1_1110_1111 {
        return Vec::new();
    }
    points
}

fn argmax(points: &[P], from: usize, to: usize, key: impl Fn(P) -> f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, &q) in points.iter().enumerate().take(to).skip(from) {
        let k = key(q);
        if best.is_none_or(|(_, b)| k > b) {
            best = Some((i, k));
        }
    }
    best.map(|(i, _)| i)
}

/// Four lines through the ring's sides, intersected — if every side is straight.
fn fit_quad_to_points(center: P, points: &mut [P]) -> Option<Quad> {
    let n = points.len();
    if n < 8 {
        return None;
    }
    // Start at the point furthest from the centre: a corner.
    let first = argmax(points, 0, n, |q| q.distance(center))?;
    points.rotate_left(first);
    let c0 = 0;
    let c2 = argmax(points, n * 3 / 8, n * 5 / 8, |q| q.distance(center))?;
    let diagonal = Line::through(points[c0], points[c2]);
    let c1 = argmax(points, n / 8, n * 3 / 8, |q| diagonal.distance(q))?;
    let c3 = argmax(points, n * 5 / 8, n * 7 / 8, |q| diagonal.distance(q))?;

    let beg = [c0 + 1, c1 + 1, c2 + 1, c3 + 1];
    let end = [c1, c2, c3, n];
    let mut lines: [Line; 4] = Default::default();
    for i in 0..4 {
        if beg[i] > end[i] {
            return None;
        }
        lines[i] = Line::fit(&points[beg[i]..end[i]]);
        if !lines[i].is_valid() {
            return None;
        }
    }
    for i in 0..4 {
        let len = (end[i] - beg[i]) as f64;
        if len > 3.0 {
            let tolerance = (len / 8.0).clamp(1.0, 8.0);
            if points[beg[i]..end[i]]
                .iter()
                .any(|&q| lines[i].distance(q) > tolerance)
            {
                return None;
            }
        }
    }
    let mut res = [P::default(); 4];
    for i in 0..4 {
        res[i] = Line::intersect(&lines[i], &lines[(i + 1) % 4])?;
    }
    Some(res)
}

fn is_plausible_square(q: &Quad, line_index: i32) -> bool {
    let mut m = q[0].distance(q[3]);
    let mut big = m;
    for i in 1..4 {
        let d = q[i - 1].distance(q[i]);
        m = m.min(d);
        big = big.max(d);
    }
    m >= f64::from(line_index * 2) && m > big / 3.0
}

pub(crate) fn fit_square_to_points(
    img: &BitMatrix,
    center: P,
    range: i32,
    line_index: i32,
    backup: bool,
) -> Option<Quad> {
    let mut points = collect_ring_points(img, center, range, line_index, backup);
    if points.is_empty() {
        return None;
    }
    let res = fit_quad_to_points(center, &mut points)?;
    is_plausible_square(&res, line_index - i32::from(backup)).then_some(res)
}

/// The corners of a finder pattern's `line_index`th ring, from both its sides.
pub(crate) fn find_concentric_pattern_corners(
    img: &BitMatrix,
    center: P,
    range: i32,
    line_index: i32,
) -> Option<Quad> {
    let inner = fit_square_to_points(img, center, range, line_index, false)?;
    let outer = fit_square_to_points(img, center, range, line_index + 1, true)?;
    Some(blend(&inner, &outer))
}

fn finetune_center(img: &BitMatrix, center: P, range: i32, pattern_size: i32) -> Option<P> {
    // At least one white path around the centre…
    let res1 = center_of_ring(img, center, range, 1, true)?;
    if !img.at(res1) {
        return None;
    }
    // …and then either another ring around that,
    if let Some(res2) = center_of_rings(img, res1, range, pattern_size / 2) {
        if img.at(res2) {
            return Some(res2);
        }
    }
    // or a centre a square fits,
    if fit_square_to_points(img, res1, range, 1, false).is_some() {
        return Some(res1);
    }
    // or the centre of a double cross.
    let res2 = center_of_double_cross(img, res1.floor(), range, pattern_size / 2 + 1)?;
    img.at(res2).then_some(res2)
}

/// A concentric `pattern` around `center`: checked vertically, horizontally and
/// on both diagonals, then recentred.
pub(crate) fn locate_concentric_pattern(
    img: &BitMatrix,
    pattern: &[u16; 5],
    center: P,
    range: i32,
) -> Option<Concentric> {
    let mut cur = Cursor::new(img, center.floor(), P::default());
    let (mut min_spread, mut max_spread) = (img.width() as i32, 0i32);
    let mut max_error = 0;
    for (d, r, update) in [
        (p(0.0, 1.0), range, true),
        (p(1.0, 0.0), range, true),
        (p(1.0, 1.0), range * 2, false),
        (p(1.0, -1.0), range * 2, false),
    ] {
        cur.set_direction(d);
        let spread = check_symmetric_pattern(&mut cur, pattern, r, update);
        if spread != 0 {
            min_spread = min_spread.min(spread);
            max_spread = max_spread.max(spread);
        } else {
            max_error -= 1;
            if max_error < 0 {
                return None;
            }
        }
    }
    if max_spread > 5 * min_spread {
        return None;
    }
    let c = finetune_center(img, cur.p, range, pattern.len() as i32)?;
    Some(Concentric {
        p: c,
        size: (max_spread + min_spread) / 2,
    })
}

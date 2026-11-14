//! Points, quadrilaterals, perspective transforms and fitted lines.
//!
//! Ported from zxing-cpp (`Point.h`, `Quadrilateral.h`, `PerspectiveTransform`,
//! `RegressionLine.h`), Apache-2.0 — see NOTICE.

use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct P {
    pub x: f64,
    pub y: f64,
}

pub(crate) const fn p(x: f64, y: f64) -> P {
    P { x, y }
}

impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        p(self.x + o.x, self.y + o.y)
    }
}
impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        p(self.x - o.x, self.y - o.y)
    }
}
impl Neg for P {
    type Output = P;
    fn neg(self) -> P {
        p(-self.x, -self.y)
    }
}
impl Mul<f64> for P {
    type Output = P;
    fn mul(self, s: f64) -> P {
        p(self.x * s, self.y * s)
    }
}
impl Mul<P> for f64 {
    type Output = P;
    fn mul(self, v: P) -> P {
        p(self * v.x, self * v.y)
    }
}
impl Div<f64> for P {
    type Output = P;
    fn div(self, s: f64) -> P {
        p(self.x / s, self.y / s)
    }
}
impl AddAssign for P {
    fn add_assign(&mut self, o: P) {
        *self = *self + o;
    }
}
impl SubAssign for P {
    fn sub_assign(&mut self, o: P) {
        *self = *self - o;
    }
}

impl P {
    pub fn dot(self, o: P) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: P) -> f64 {
        self.x * o.y - o.x * self.y
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn distance(self, o: P) -> f64 {
        (self - o).length()
    }
    pub fn normalized(self) -> P {
        self / self.length()
    }
    pub fn max_abs(self) -> f64 {
        self.x.abs().max(self.y.abs())
    }
    /// The step that moves one pixel along the dominant axis.
    pub fn bresenham(self) -> P {
        self / self.max_abs()
    }
    pub fn main_direction(self) -> P {
        if self.x.abs() > self.y.abs() {
            p(self.x, 0.0)
        } else {
            p(0.0, self.y)
        }
    }
    pub fn floor(self) -> P {
        p(self.x.floor(), self.y.floor())
    }
    /// The centre of the pixel this point lies in.
    pub fn centered(self) -> P {
        p(self.x.floor() + 0.5, self.y.floor() + 0.5)
    }
}

/// Corners in order: top-left, top-right, bottom-right, bottom-left.
pub(crate) type Quad = [P; 4];

pub(crate) fn rectangle(w: f64, h: f64, margin: f64) -> Quad {
    [
        p(margin, margin),
        p(w - margin, margin),
        p(w - margin, h - margin),
        p(margin, h - margin),
    ]
}

pub(crate) fn rectangle_xy(x0: f64, x1: f64, y0: f64, y1: f64) -> Quad {
    let o = 0.5;
    [
        p(x0 + o, y0 + o),
        p(x1 + o, y0 + o),
        p(x1 + o, y1 + o),
        p(x0 + o, y1 + o),
    ]
}

/// Convex — and not so skewed that projecting near a corner turns unstable
/// (zxing-cpp: the M/m ratio stays below 2 for real samples; 14 has broken it).
pub(crate) fn is_convex(q: &Quad) -> bool {
    let mut sign = false;
    let (mut m, mut big) = (f64::INFINITY, 0.0f64);
    for i in 0..4 {
        let d1 = q[(i + 2) % 4] - q[(i + 1) % 4];
        let d2 = q[i] - q[(i + 1) % 4];
        let cp = d1.cross(d2);
        m = m.min(cp.abs());
        big = big.max(cp.abs());
        if i == 0 {
            sign = cp > 0.0;
        } else if sign != (cp > 0.0) {
            return false;
        }
    }
    big / m < 4.0
}

/// The average of two quadrilaterals, matching `b`'s corners to `a`'s first.
pub(crate) fn blend(a: &Quad, b: &Quad) -> Quad {
    let c = a[0];
    let mut offset = 0;
    for i in 1..4 {
        if b[i].distance(c) < b[offset].distance(c) {
            offset = i;
        }
    }
    let mut res = [P::default(); 4];
    for i in 0..4 {
        res[i] = (a[i] + b[(i + offset) % 4]) / 2.0;
    }
    res
}

// ---------------------------------------------------------------------------
// Perspective transform
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub(crate) struct Transform {
    a11: f64,
    a12: f64,
    a13: f64,
    a21: f64,
    a22: f64,
    a23: f64,
    a31: f64,
    a32: f64,
    a33: f64,
}

impl Transform {
    #[allow(clippy::too_many_arguments)]
    const fn new(
        a11: f64,
        a21: f64,
        a31: f64,
        a12: f64,
        a22: f64,
        a32: f64,
        a13: f64,
        a23: f64,
        a33: f64,
    ) -> Self {
        Self {
            a11,
            a12,
            a13,
            a21,
            a22,
            a23,
            a31,
            a32,
            a33,
        }
    }

    /// The projective map taking `from`'s corners onto `to`'s; `None` when
    /// either is not a plausible (convex, not degenerate) quadrilateral.
    pub fn between(from: &Quad, to: &Quad) -> Option<Self> {
        if !is_convex(from) || !is_convex(to) {
            return None;
        }
        Some(Self::square_to_quad(to).times(&Self::square_to_quad(from).adjoint()))
    }

    pub fn apply(&self, q: P) -> P {
        let d = self.a13 * q.x + self.a23 * q.y + self.a33;
        p(
            (self.a11 * q.x + self.a21 * q.y + self.a31) / d,
            (self.a12 * q.x + self.a22 * q.y + self.a32) / d,
        )
    }

    pub fn is_valid(&self) -> bool {
        !self.a33.is_nan()
    }

    fn square_to_quad(q: &Quad) -> Self {
        let [p0, p1, p2, p3] = *q;
        let d3 = p0 - p1 + p2 - p3;
        if d3 == P::default() {
            Self::new(
                p1.x - p0.x,
                p2.x - p1.x,
                p0.x,
                p1.y - p0.y,
                p2.y - p1.y,
                p0.y,
                0.0,
                0.0,
                1.0,
            )
        } else {
            let d1 = p1 - p2;
            let d2 = p3 - p2;
            let den = d1.cross(d2);
            let a13 = d3.cross(d2) / den;
            let a23 = d1.cross(d3) / den;
            Self::new(
                p1.x - p0.x + a13 * p1.x,
                p3.x - p0.x + a23 * p3.x,
                p0.x,
                p1.y - p0.y + a13 * p1.y,
                p3.y - p0.y + a23 * p3.y,
                p0.y,
                a13,
                a23,
                1.0,
            )
        }
    }

    const fn adjoint(&self) -> Self {
        Self::new(
            self.a22 * self.a33 - self.a23 * self.a32,
            self.a23 * self.a31 - self.a21 * self.a33,
            self.a21 * self.a32 - self.a22 * self.a31,
            self.a13 * self.a32 - self.a12 * self.a33,
            self.a11 * self.a33 - self.a13 * self.a31,
            self.a12 * self.a31 - self.a11 * self.a32,
            self.a12 * self.a23 - self.a13 * self.a22,
            self.a13 * self.a21 - self.a11 * self.a23,
            self.a11 * self.a22 - self.a12 * self.a21,
        )
    }

    fn times(&self, r: &Self) -> Self {
        let s = self;
        Self::new(
            s.a11 * r.a11 + s.a21 * r.a12 + s.a31 * r.a13,
            s.a11 * r.a21 + s.a21 * r.a22 + s.a31 * r.a23,
            s.a11 * r.a31 + s.a21 * r.a32 + s.a31 * r.a33,
            s.a12 * r.a11 + s.a22 * r.a12 + s.a32 * r.a13,
            s.a12 * r.a21 + s.a22 * r.a22 + s.a32 * r.a23,
            s.a12 * r.a31 + s.a22 * r.a32 + s.a32 * r.a33,
            s.a13 * r.a11 + s.a23 * r.a12 + s.a33 * r.a13,
            s.a13 * r.a21 + s.a23 * r.a22 + s.a33 * r.a23,
            s.a13 * r.a31 + s.a23 * r.a32 + s.a33 * r.a33,
        )
    }
}

// ---------------------------------------------------------------------------
// Regression line — a least-squares line through traced edge points
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub points: Vec<P>,
    direction_inward: P,
    a: f64,
    b: f64,
    c: f64,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            direction_inward: P::default(),
            a: f64::NAN,
            b: f64::NAN,
            c: f64::NAN,
        }
    }
}

impl Line {
    pub fn through(p1: P, p2: P) -> Self {
        let mut l = Self::default();
        l.evaluate(&[p1, p2]);
        l
    }

    pub fn fit(points: &[P]) -> Self {
        let mut l = Self::default();
        if !points.is_empty() {
            l.evaluate(points);
        }
        l
    }

    pub fn is_valid(&self) -> bool {
        !self.a.is_nan()
    }

    fn normal(&self) -> P {
        if self.is_valid() {
            p(self.a, self.b)
        } else {
            self.direction_inward
        }
    }

    pub fn signed_distance(&self, q: P) -> f64 {
        self.normal().dot(q) - self.c
    }

    pub fn distance(&self, q: P) -> f64 {
        self.signed_distance(q).abs()
    }

    pub fn set_direction_inward(&mut self, d: P) {
        self.direction_inward = d.normalized();
    }

    pub fn add(&mut self, q: P) {
        self.points.push(q);
        if self.points.len() == 1 {
            self.c = self.normal().dot(q);
        }
    }

    fn evaluate(&mut self, points: &[P]) -> bool {
        let n = points.len() as f64;
        let mean = points.iter().fold(P::default(), |s, &q| s + q) / n;
        let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
        for &q in points {
            let d = q - mean;
            xx += d.x * d.x;
            yy += d.y * d.y;
            xy += d.x * d.y;
        }
        if yy >= xx {
            let l = (yy * yy + xy * xy).sqrt();
            self.a = yy / l;
            self.b = -xy / l;
        } else {
            let l = (xx * xx + xy * xy).sqrt();
            self.a = xy / l;
            self.b = -xx / l;
        }
        if self.direction_inward.dot(self.normal()) < 0.0 {
            self.a = -self.a;
            self.b = -self.b;
        }
        self.c = self.normal().dot(mean);
        self.direction_inward.dot(self.normal()) > 0.5
    }

    /// Fit, then drop points further than `max_dist` outside (2× inside) and refit
    /// until stable; keeps the surviving points.
    pub fn evaluate_max_distance(&mut self, max_dist: f64) -> bool {
        if self.points.is_empty() {
            return false;
        }
        let pts = core::mem::take(&mut self.points);
        let mut ret = self.evaluate(&pts);
        let mut pts = pts;
        loop {
            let before = pts.len();
            pts.retain(|&q| {
                let sd = self.signed_distance(q);
                !(sd > max_dist || sd < -2.0 * max_dist)
            });
            if before == pts.len() || pts.is_empty() {
                break;
            }
            ret = self.evaluate(&pts);
        }
        self.points = pts;
        ret
    }

    pub fn is_high_res(&self) -> bool {
        let Some(&first) = self.points.first() else {
            return false;
        };
        let (mut min, mut max) = (first, first);
        for q in &self.points {
            min = p(min.x.min(q.x), min.y.min(q.y));
            max = p(max.x.max(q.x), max.y.max(q.y));
        }
        let diff = max - min;
        let steps = diff.x.abs().min(diff.y.abs());
        steps > 2.0 || diff.max_abs() > 50.0
    }

    pub fn intersect(l1: &Line, l2: &Line) -> Option<P> {
        if !(l1.is_valid() && l2.is_valid()) {
            return None;
        }
        let d = l1.a * l2.b - l1.b * l2.a;
        let x = (l1.c * l2.b - l1.b * l2.c) / d;
        let y = (l1.a * l2.c - l1.c * l2.a) / d;
        (x.is_finite() && y.is_finite()).then_some(p(x, y))
    }
}

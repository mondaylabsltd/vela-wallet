//! Anti-aliased coverage of filled outlines: the signed-area accumulation
//! method (as in font-rs). Each line adds the exact area it covers to the
//! cells it crosses; a running sum along the buffer then gives every pixel's
//! coverage under the non-zero rule. Quadratic curves are flattened first.

use crate::font::{Point, Segment};

pub struct Coverage {
    pub width: usize,
    pub height: usize,
    acc: Vec<f32>,
}

impl Coverage {
    pub fn new(width: usize, height: usize) -> Coverage {
        // Slack after the last row: a line's spill into "the next cell".
        Coverage {
            width,
            height,
            acc: vec![0.0; width * height + width + 4],
        }
    }

    pub fn fill(&mut self, segments: &[Segment]) {
        for s in segments {
            match *s {
                Segment::Line(a, b) => self.line(a, b),
                Segment::Cubic(a, c1, c2, b) => {
                    let dev = |p: Point, q: Point, r: Point| {
                        ((p.x - 2.0 * q.x + r.x).powi(2) + (p.y - 2.0 * q.y + r.y).powi(2)).sqrt()
                    };
                    let d = dev(a, c1, c2).max(dev(c1, c2, b));
                    let n = ((d * 3.0).sqrt().ceil() as usize).clamp(1, 96);
                    let mut prev = a;
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let mt = 1.0 - t;
                        let (w0, w1, w2, w3) =
                            (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
                        let p = Point {
                            x: w0 * a.x + w1 * c1.x + w2 * c2.x + w3 * b.x,
                            y: w0 * a.y + w1 * c1.y + w2 * c2.y + w3 * b.y,
                        };
                        self.line(prev, p);
                        prev = p;
                    }
                }
                Segment::Quad(a, c, b) => {
                    // Subdivide so each piece deviates < 1/8 px from the curve.
                    let dev =
                        ((a.x - 2.0 * c.x + b.x).powi(2) + (a.y - 2.0 * c.y + b.y).powi(2)).sqrt();
                    let n = ((dev * 2.0).sqrt().ceil() as usize).clamp(1, 64);
                    let mut prev = a;
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let mt = 1.0 - t;
                        let p = Point {
                            x: mt * mt * a.x + 2.0 * mt * t * c.x + t * t * b.x,
                            y: mt * mt * a.y + 2.0 * mt * t * c.y + t * t * b.y,
                        };
                        self.line(prev, p);
                        prev = p;
                    }
                }
            }
        }
    }

    fn line(&mut self, p0: Point, p1: Point) {
        if p0.y == p1.y {
            return;
        }
        let (dir, p0, p1) = if p0.y < p1.y {
            (1.0, p0, p1)
        } else {
            (-1.0, p1, p0)
        };
        let w = self.width as f32;
        // Clip to the buffer: the caller sizes it to hold the glyph, so this
        // only ever trims rounding.
        let clamp_x = |x: f32| x.clamp(0.0, w - 1.0);
        let dxdy = (p1.x - p0.x) / (p1.y - p0.y);
        let mut x = p0.x;
        let y_start = p0.y.max(0.0);
        if p0.y < 0.0 {
            x -= p0.y * dxdy;
        }
        let y_end = p1.y.min(self.height as f32);
        let mut y = y_start.floor() as usize;
        while (y as f32) < y_end {
            let line_start = y * self.width;
            let dy = ((y + 1) as f32).min(p1.y) - (y as f32).max(p0.y);
            let x_next = x + dxdy * dy;
            let d = dy * dir;
            let (x0, x1) = if x < x_next {
                (clamp_x(x), clamp_x(x_next))
            } else {
                (clamp_x(x_next), clamp_x(x))
            };
            let x0_floor = x0.floor();
            let x0i = x0_floor as usize;
            let x1_ceil = x1.ceil();
            let x1i = x1_ceil as usize;
            let a = &mut self.acc;
            if x1i <= x0i + 1 {
                let xmf = 0.5 * (x0 + x1) - x0_floor;
                a[line_start + x0i] += d - d * xmf;
                a[line_start + x0i + 1] += d * xmf;
            } else {
                let s = (x1 - x0).recip();
                let x0f = x0 - x0_floor;
                let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
                let x1f = x1 - x1_ceil + 1.0;
                let am = 0.5 * s * x1f * x1f;
                a[line_start + x0i] += d * a0;
                if x1i == x0i + 2 {
                    a[line_start + x0i + 1] += d * (1.0 - a0 - am);
                } else {
                    let a1 = s * (1.5 - x0f);
                    a[line_start + x0i + 1] += d * (a1 - a0);
                    for xi in x0i + 2..x1i - 1 {
                        a[line_start + xi] += d * s;
                    }
                    let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                    a[line_start + x1i - 1] += d * (1.0 - a2 - am);
                }
                a[line_start + x1i] += d * am;
            }
            x = x_next;
            y += 1;
        }
    }

    /// Per-pixel coverage in [0, 1], row-major.
    pub fn finish(self) -> Vec<f32> {
        let mut sum = 0.0f32;
        self.acc[..self.width * self.height]
            .iter()
            .map(|v| {
                sum += v;
                sum.abs().min(1.0)
            })
            .collect()
    }
}

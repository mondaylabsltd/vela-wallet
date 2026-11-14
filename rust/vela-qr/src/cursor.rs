//! A position and a direction on a black/white image, stepping from edge to edge.
//!
//! Ported from zxing-cpp `BitMatrixCursor.h` and `FastEdgeToEdgeCounter.h`
//! (Apache-2.0, see NOTICE).

use crate::bitmatrix::BitMatrix;
use crate::geometry::{p, P};

/// A pixel's colour, or `Invalid` outside the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Invalid,
    White,
    Black,
}

impl Value {
    pub fn is_valid(self) -> bool {
        self != Value::Invalid
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Left = -1,
    Right = 1,
}

impl Direction {
    fn opposite(self) -> Self {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Cursor<'a> {
    pub img: &'a BitMatrix,
    pub p: P,
    pub d: P,
}

impl<'a> Cursor<'a> {
    pub fn new(img: &'a BitMatrix, at: P, d: P) -> Self {
        let mut c = Self {
            img,
            p: at,
            d: P::default(),
        };
        c.set_direction(d);
        c
    }

    pub fn set_direction(&mut self, d: P) {
        self.d = if d == P::default() { d } else { d.bresenham() };
    }

    pub fn test_at(&self, q: P) -> Value {
        if !self.img.is_in(q) {
            Value::Invalid
        } else if self.img.get(q.x as usize, q.y as usize) {
            Value::Black
        } else {
            Value::White
        }
    }

    pub fn is_in(&self) -> bool {
        self.img.is_in(self.p)
    }

    pub fn is_black(&self) -> bool {
        self.test_at(self.p) == Value::Black
    }

    pub fn back(&self) -> P {
        -self.d
    }

    pub fn left(&self) -> P {
        p(self.d.y, -self.d.x)
    }

    pub fn right(&self) -> P {
        p(-self.d.y, self.d.x)
    }

    pub fn direction(&self, dir: Direction) -> P {
        self.right() * f64::from(dir as i8)
    }

    pub fn turn_back(&mut self) {
        self.d = self.back();
    }

    pub fn turn_left(&mut self) {
        self.d = self.left();
    }

    pub fn turn_right(&mut self) {
        self.d = self.right();
    }

    pub fn turn(&mut self, dir: Direction) {
        self.d = self.direction(dir);
    }

    pub fn turned_back(&self) -> Self {
        Self {
            img: self.img,
            p: self.p,
            d: self.back(),
        }
    }

    /// The colour here if the pixel at `+d` differs, else `Invalid`.
    pub fn edge_at(&self, d: P) -> Value {
        let v = self.test_at(self.p);
        if self.test_at(self.p + d) != v {
            v
        } else {
            Value::Invalid
        }
    }

    pub fn edge_at_dir(&self, dir: Direction) -> Value {
        self.edge_at(self.direction(dir))
    }

    pub fn step(&mut self, s: f64) -> bool {
        self.p += self.d * s;
        self.is_in()
    }

    /// Advance to the `nth` edge (one step past it, or onto it with `backup`).
    /// Returns the steps taken, or 0 when the image or `range` ran out first.
    pub fn step_to_edge(&mut self, nth: i32, range: i32, backup: bool) -> i32 {
        let mut nth = nth;
        let mut steps = 0i32;
        let mut lv = self.test_at(self.p);
        // range 0 = unbounded; a spent (negative) range stops at once.
        while nth > 0 && (range == 0 || steps < range) && lv.is_valid() {
            steps += 1;
            let v = self.test_at(self.p + self.d * f64::from(steps));
            if lv != v {
                lv = v;
                nth -= 1;
            }
        }
        if backup {
            steps -= 1;
        }
        self.p += self.d * f64::from(steps);
        steps * i32::from(nth == 0)
    }

    /// Walk along an edge, keeping it on the `dir` side.
    pub fn step_along_edge(&mut self, dir: Direction, skip_corner: bool) -> bool {
        if !self.edge_at_dir(dir).is_valid() {
            self.turn(dir);
        } else if self.edge_at(self.d).is_valid() {
            self.turn(dir.opposite());
            if self.edge_at(self.d).is_valid() {
                self.turn(dir.opposite());
                if self.edge_at(self.d).is_valid() {
                    return false;
                }
            }
        }
        let mut ret = self.step(1.0);
        if ret && skip_corner && !self.edge_at_dir(dir).is_valid() {
            self.turn(dir);
            ret = self.step(1.0);
        }
        ret
    }
}

/// Counts run lengths along an axis-aligned or diagonal line from an integer
/// position — the hot loop of finder-pattern checking. Reaching the image border
/// counts as an edge.
pub(crate) struct EdgeCounter<'a> {
    bits: &'a [u8],
    pos: isize,
    stride: isize,
    steps_to_border: i32,
}

impl<'a> EdgeCounter<'a> {
    pub fn new(cur: &Cursor<'a>) -> Self {
        let img = cur.img;
        let (w, h) = (img.width() as i32, img.height() as i32);
        let (x, y) = (cur.p.x as i32, cur.p.y as i32);
        let (dx, dy) = (cur.d.x as i32, cur.d.y as i32);
        let max_x = if dx == 0 {
            i32::MAX
        } else if dx > 0 {
            w - 1 - x
        } else {
            x
        };
        let max_y = if dy == 0 {
            i32::MAX
        } else if dy > 0 {
            h - 1 - y
        } else {
            y
        };
        Self {
            bits: img.raw(),
            pos: (y as isize) * (w as isize) + x as isize,
            stride: (dy as isize) * (w as isize) + dx as isize,
            steps_to_border: max_x.min(max_y),
        }
    }

    pub fn step_to_next_edge(&mut self, range: i32) -> i32 {
        let max_steps = self.steps_to_border.min(range);
        let mut steps = 0;
        loop {
            steps += 1;
            if steps > max_steps {
                if max_steps == self.steps_to_border {
                    break;
                }
                return 0;
            }
            let here = self.bits[self.pos as usize];
            if self.bits[(self.pos + steps as isize * self.stride) as usize] != here {
                break;
            }
        }
        self.pos += steps as isize * self.stride;
        self.steps_to_border -= steps;
        steps
    }
}

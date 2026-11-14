//! A black/white image, one byte per pixel (1 = black), as zxing-cpp keeps it:
//! the scanning loops read whole rows and compare neighbours, which bytes make
//! cheap and bit-packing would not.

use crate::geometry::P;

#[derive(Clone, Debug)]
pub(crate) struct BitMatrix {
    width: usize,
    height: usize,
    bits: Vec<u8>,
}

impl BitMatrix {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            bits: vec![0; width * height],
        }
    }

    pub fn from_bits(width: usize, height: usize, bits: Vec<u8>) -> Self {
        debug_assert_eq!(bits.len(), width * height);
        Self {
            width,
            height,
            bits,
        }
    }

    /// Black ↔ white: a code shown light-on-dark.
    pub fn invert(&mut self) {
        for b in &mut self.bits {
            *b ^= 1;
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.bits[y * self.width + x] != 0
    }

    pub fn set(&mut self, x: usize, y: usize) {
        self.bits[y * self.width + x] = 1;
    }

    pub fn row(&self, y: usize) -> &[u8] {
        &self.bits[y * self.width..(y + 1) * self.width]
    }

    pub fn raw(&self) -> &[u8] {
        &self.bits
    }

    pub fn set_region(&mut self, left: usize, top: usize, w: usize, h: usize) {
        for y in top..top + h {
            for x in left..left + w {
                self.set(x, y);
            }
        }
    }

    /// Inside the image, in pixel (not module) coordinates.
    pub fn is_in(&self, q: P) -> bool {
        q.x >= 0.0 && q.x < self.width as f64 && q.y >= 0.0 && q.y < self.height as f64
    }

    /// The pixel `q` lies in; white outside the image.
    pub fn at(&self, q: P) -> bool {
        self.is_in(q) && self.get(q.x as usize, q.y as usize)
    }
}

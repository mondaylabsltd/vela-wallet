//! Luminance → black/white.
//!
//! ZXing's `HybridBinarizer` (Apache-2.0, see NOTICE): a threshold per 8×8 block,
//! taken as the average of the 5×5 blocks around it, so uneven light and
//! gradients do not wash a code out. Images too small for blocks fall back to
//! `GlobalHistogramBinarizer`'s single threshold.

use crate::bitmatrix::BitMatrix;

const BLOCK_POWER: usize = 3;
const BLOCK: usize = 1 << BLOCK_POWER;
const BLOCK_MASK: usize = BLOCK - 1;
const MINIMUM_DIMENSION: usize = BLOCK * 5;
const MIN_DYNAMIC_RANGE: u32 = 24;

pub(crate) fn binarize(luma: &[u8], width: usize, height: usize) -> Option<BitMatrix> {
    if width >= MINIMUM_DIMENSION && height >= MINIMUM_DIMENSION {
        Some(hybrid(luma, width, height))
    } else {
        global(luma, width, height)
    }
}

fn hybrid(luma: &[u8], width: usize, height: usize) -> BitMatrix {
    let sub_w = (width >> BLOCK_POWER) + usize::from(width & BLOCK_MASK != 0);
    let sub_h = (height >> BLOCK_POWER) + usize::from(height & BLOCK_MASK != 0);
    let black = black_points(luma, sub_w, sub_h, width, height);

    let mut bits = vec![0u8; width * height];
    let max_y = height - BLOCK;
    let max_x = width - BLOCK;
    for y in 0..sub_h {
        let yoff = (y << BLOCK_POWER).min(max_y);
        let top = y.clamp(2, sub_h - 3);
        for x in 0..sub_w {
            let xoff = (x << BLOCK_POWER).min(max_x);
            let left = x.clamp(2, sub_w - 3);
            let mut sum = 0u32;
            for z in 0..5 {
                let row = &black[(top + z - 2) * sub_w..];
                sum += row[left - 2] + row[left - 1] + row[left] + row[left + 1] + row[left + 2];
            }
            let threshold = sum / 25;
            for yy in 0..BLOCK {
                let o = (yoff + yy) * width + xoff;
                for xx in 0..BLOCK {
                    bits[o + xx] = u8::from(u32::from(luma[o + xx]) <= threshold);
                }
            }
        }
    }
    BitMatrix::from_bits(width, height, bits)
}

/// One black point per block: its mean, or — for a flat block — something
/// below its minimum, borrowed from the neighbours when they say it is dark.
fn black_points(luma: &[u8], sub_w: usize, sub_h: usize, width: usize, height: usize) -> Vec<u32> {
    let max_y = height - BLOCK;
    let max_x = width - BLOCK;
    let mut points = vec![0u32; sub_w * sub_h];
    for y in 0..sub_h {
        let yoff = (y << BLOCK_POWER).min(max_y);
        for x in 0..sub_w {
            let xoff = (x << BLOCK_POWER).min(max_x);
            let (mut sum, mut min, mut max) = (0u32, u8::MAX, u8::MIN);
            let mut yy = 0;
            let mut o = yoff * width + xoff;
            while yy < BLOCK {
                for &px in &luma[o..o + BLOCK] {
                    sum += u32::from(px);
                    min = min.min(px);
                    max = max.max(px);
                }
                if u32::from(max - min) > MIN_DYNAMIC_RANGE {
                    // Contrast established: finish the sum without the min/max work.
                    o += width;
                    yy += 1;
                    while yy < BLOCK {
                        sum += luma[o..o + BLOCK]
                            .iter()
                            .map(|&v| u32::from(v))
                            .sum::<u32>();
                        o += width;
                        yy += 1;
                    }
                    break;
                }
                o += width;
                yy += 1;
            }
            let mut average = sum >> (BLOCK_POWER * 2);
            if u32::from(max - min) <= MIN_DYNAMIC_RANGE {
                // A flat block is white, unless the neighbours say it lies in
                // a dark area — then it takes their black point.
                average = u32::from(min) / 2;
                if y > 0 && x > 0 {
                    let neighbours = (points[(y - 1) * sub_w + x]
                        + 2 * points[y * sub_w + x - 1]
                        + points[(y - 1) * sub_w + x - 1])
                        / 4;
                    if u32::from(min) < neighbours {
                        average = neighbours;
                    }
                }
            }
            points[y * sub_w + x] = average;
        }
    }
    points
}

/// One threshold for the whole image, from a 32-bucket histogram of four rows.
fn global(luma: &[u8], width: usize, height: usize) -> Option<BitMatrix> {
    const SHIFT: usize = 3;
    let mut buckets = [0u32; 32];
    for y in 1..5 {
        let row = &luma[(height * y / 5) * width..];
        for &px in &row[width / 5..(width * 4) / 5] {
            buckets[usize::from(px) >> SHIFT] += 1;
        }
    }
    let black_point = estimate_black_point(&buckets)? << SHIFT;
    let bits = luma
        .iter()
        .map(|&px| u8::from(usize::from(px) < black_point))
        .collect();
    Some(BitMatrix::from_bits(width, height, bits))
}

fn estimate_black_point(buckets: &[u32; 32]) -> Option<usize> {
    let n = buckets.len();
    let (mut first, mut first_size, mut max_count) = (0usize, 0u32, 0u32);
    for (x, &b) in buckets.iter().enumerate() {
        if b > first_size {
            first = x;
            first_size = b;
        }
        max_count = max_count.max(b);
    }
    // The second peak: tall, and far from the first.
    let (mut second, mut second_score) = (0usize, 0u64);
    for (x, &b) in buckets.iter().enumerate() {
        let d = x.abs_diff(first) as u64;
        let score = u64::from(b) * d * d;
        if score > second_score {
            second = x;
            second_score = score;
        }
    }
    let (lo, hi) = if first > second {
        (second, first)
    } else {
        (first, second)
    };
    // Too little contrast for a meaningful black point: say so rather than guess.
    if hi - lo <= n / 16 {
        return None;
    }
    // The valley between them, low and nearer the white peak.
    let (mut best, mut best_score) = (hi - 1, -1i64);
    for x in (lo + 1..hi).rev() {
        let from_first = (x - lo) as i64;
        let score = from_first * from_first * (hi - x) as i64 * i64::from(max_count - buckets[x]);
        if score > best_score {
            best = x;
            best_score = score;
        }
    }
    Some(best)
}

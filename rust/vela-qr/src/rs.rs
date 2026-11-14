//! Reed–Solomon error correction over GF(256), QR's field (x⁸+x⁴+x³+x²+1,
//! generator base 0).
//!
//! The decoder is ZXing's `ReedSolomonDecoder` (Apache-2.0, see NOTICE):
//! syndromes, the Euclidean algorithm for the error locator and evaluator,
//! Chien search, Forney.

const PRIMITIVE: u16 = 0x011D;

struct Field {
    exp: [u8; 512],
    log: [u8; 256],
}

const fn build_field() -> Field {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    let mut x: u16 = 1;
    let mut i = 0;
    while i < 255 {
        exp[i] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if x >= 256 {
            x ^= PRIMITIVE;
        }
        i += 1;
    }
    // Doubled, so a product's log never needs reducing mod 255.
    while i < 512 {
        exp[i] = exp[i - 255];
        i += 1;
    }
    Field { exp, log }
}

static GF: Field = build_field();

fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        0
    } else {
        GF.exp[usize::from(GF.log[usize::from(a)]) + usize::from(GF.log[usize::from(b)])]
    }
}

fn inv(a: u8) -> u8 {
    debug_assert!(a != 0);
    GF.exp[255 - usize::from(GF.log[usize::from(a)])]
}

/// A polynomial, highest degree first, with no leading zeros (zero is `[0]`).
#[derive(Clone, Debug, PartialEq)]
struct Poly(Vec<u8>);

impl Poly {
    fn new(mut c: Vec<u8>) -> Self {
        let lead = c
            .iter()
            .position(|&v| v != 0)
            .unwrap_or(c.len().saturating_sub(1));
        c.drain(..lead);
        if c.is_empty() {
            c.push(0);
        }
        Poly(c)
    }
    fn zero() -> Self {
        Poly(vec![0])
    }
    fn monomial(degree: usize, coeff: u8) -> Self {
        if coeff == 0 {
            return Self::zero();
        }
        let mut c = vec![0u8; degree + 1];
        c[0] = coeff;
        Poly(c)
    }
    fn degree(&self) -> usize {
        self.0.len() - 1
    }
    fn is_zero(&self) -> bool {
        self.0[0] == 0
    }
    fn coeff(&self, degree: usize) -> u8 {
        self.0[self.0.len() - 1 - degree]
    }
    fn eval(&self, a: u8) -> u8 {
        if a == 0 {
            return self.coeff(0);
        }
        self.0.iter().fold(0, |acc, &c| mul(a, acc) ^ c)
    }
    fn add(&self, o: &Poly) -> Poly {
        let (small, large) = if self.0.len() > o.0.len() {
            (&o.0, &self.0)
        } else {
            (&self.0, &o.0)
        };
        let mut sum = large.clone();
        let diff = large.len() - small.len();
        for (i, &v) in small.iter().enumerate() {
            sum[diff + i] ^= v;
        }
        Poly::new(sum)
    }
    fn mul(&self, o: &Poly) -> Poly {
        if self.is_zero() || o.is_zero() {
            return Self::zero();
        }
        let mut prod = vec![0u8; self.0.len() + o.0.len() - 1];
        for (i, &a) in self.0.iter().enumerate() {
            for (j, &b) in o.0.iter().enumerate() {
                prod[i + j] ^= mul(a, b);
            }
        }
        Poly::new(prod)
    }
    fn scale(&self, s: u8) -> Poly {
        Poly::new(self.0.iter().map(|&c| mul(c, s)).collect())
    }
    fn mul_monomial(&self, degree: usize, coeff: u8) -> Poly {
        if coeff == 0 {
            return Self::zero();
        }
        let mut c: Vec<u8> = self.0.iter().map(|&v| mul(v, coeff)).collect();
        c.resize(c.len() + degree, 0);
        Poly::new(c)
    }
}

/// Correct `block` (data then `ec` error-correction codewords) in place.
/// `None` when the errors exceed what the codewords can correct.
pub(crate) fn correct(block: &mut [u8], ec: usize) -> Option<()> {
    let received = Poly::new(block.to_vec());
    let mut syndromes = vec![0u8; ec];
    let mut clean = true;
    for i in 0..ec {
        let s = received.eval(GF.exp[i]);
        syndromes[ec - 1 - i] = s;
        clean &= s == 0;
    }
    if clean {
        return Some(());
    }
    let (sigma, omega) = euclidean(Poly::monomial(ec, 1), Poly::new(syndromes), ec)?;
    let locations = error_locations(&sigma)?;
    let magnitudes = error_magnitudes(&omega, &locations);
    for (&loc, &mag) in locations.iter().zip(&magnitudes) {
        let pos = block
            .len()
            .checked_sub(1 + usize::from(GF.log[usize::from(loc)]))?;
        block[pos] ^= mag;
    }
    Some(())
}

fn euclidean(a: Poly, b: Poly, r: usize) -> Option<(Poly, Poly)> {
    let (mut r_last, mut r_cur) = if a.degree() < b.degree() {
        (b, a)
    } else {
        (a, b)
    };
    let (mut t_last, mut t_cur) = (Poly::zero(), Poly::new(vec![1]));
    while 2 * r_cur.degree() >= r {
        let r_last_last = core::mem::replace(&mut r_last, r_cur.clone());
        let t_last_last = core::mem::replace(&mut t_last, t_cur.clone());
        if r_last.is_zero() {
            return None;
        }
        let mut rem = r_last_last;
        let mut q = Poly::zero();
        let lead_inv = inv(r_last.coeff(r_last.degree()));
        while rem.degree() >= r_last.degree() && !rem.is_zero() {
            let diff = rem.degree() - r_last.degree();
            let scale = mul(rem.coeff(rem.degree()), lead_inv);
            q = q.add(&Poly::monomial(diff, scale));
            rem = rem.add(&r_last.mul_monomial(diff, scale));
        }
        t_cur = q.mul(&t_last).add(&t_last_last);
        if rem.degree() >= r_last.degree() {
            return None;
        }
        r_cur = rem;
    }
    let at_zero = t_cur.coeff(0);
    if at_zero == 0 {
        return None;
    }
    let i = inv(at_zero);
    Some((t_cur.scale(i), r_cur.scale(i)))
}

fn error_locations(locator: &Poly) -> Option<Vec<u8>> {
    let n = locator.degree();
    if n == 1 {
        return Some(vec![locator.coeff(1)]);
    }
    let mut out = Vec::with_capacity(n);
    for i in 1..=255u16 {
        if out.len() == n {
            break;
        }
        if locator.eval(i as u8) == 0 {
            out.push(inv(i as u8));
        }
    }
    (out.len() == n).then_some(out)
}

fn error_magnitudes(evaluator: &Poly, locations: &[u8]) -> Vec<u8> {
    locations
        .iter()
        .enumerate()
        .map(|(i, &loc)| {
            let xi_inv = inv(loc);
            let mut den = 1u8;
            for (j, &other) in locations.iter().enumerate() {
                if i != j {
                    // 1 + X_j / X_i, where + is xor.
                    den = mul(den, mul(other, xi_inv) ^ 1);
                }
            }
            mul(evaluator.eval(xi_inv), inv(den))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 10 EC codewords of version 1-M for "01234567" (ISO 18004 Annex I).
    const DATA: [u8; 16] = [
        0x10, 0x20, 0x0C, 0x56, 0x61, 0x80, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC,
        0x11,
    ];
    const EC: [u8; 10] = [0xA5, 0x24, 0xD4, 0xC1, 0xED, 0x36, 0xC7, 0x87, 0x2C, 0x55];

    fn block() -> Vec<u8> {
        [&DATA[..], &EC[..]].concat()
    }

    #[test]
    fn a_clean_block_is_untouched() {
        let mut b = block();
        assert_eq!(correct(&mut b, 10), Some(()));
        assert_eq!(b, block());
    }

    #[test]
    fn up_to_five_errors_are_corrected() {
        for errors in 1..=5usize {
            let mut b = block();
            for k in 0..errors {
                b[k * 5] ^= 0x5A + k as u8;
            }
            assert_eq!(correct(&mut b, 10), Some(()), "{errors} errors");
            assert_eq!(b, block(), "{errors} errors");
        }
    }

    #[test]
    fn six_errors_are_refused_or_never_silently_wrong() {
        let mut b = block();
        for k in 0..6 {
            b[k * 4] ^= 0x33;
        }
        if correct(&mut b, 10).is_some() {
            // A miscorrection would have to land on a different codeword.
            assert_ne!(b, block());
        }
    }
}

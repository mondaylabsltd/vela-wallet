//! QR model 2 versions: sizes, alignment pattern positions, error-correction
//! block structure, the function-pattern mask, and version-information decoding.
//!
//! Tables from ISO/IEC 18004 as tabulated by ZXing (Apache-2.0, see NOTICE).

use crate::bitmatrix::BitMatrix;

pub(crate) struct VersionInfo {
    #[allow(dead_code)]
    pub number: u8,
    /// Alignment pattern centres, in modules (empty for version 1).
    pub alignment: &'static [u8],
    /// Per level (L, M, Q, H): EC codewords per block, then (count, data codewords) twice.
    pub ec: [(u8, u8, u8, u8, u8); 4],
}

/// Error-correction level, as an index into [`VersionInfo::ec`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EcLevel {
    L = 0,
    M = 1,
    Q = 2,
    H = 3,
}

impl EcLevel {
    /// The two format-information bits: 00 M, 01 L, 10 H, 11 Q.
    pub fn from_bits(bits: u32) -> Self {
        match bits & 3 {
            0 => EcLevel::M,
            1 => EcLevel::L,
            2 => EcLevel::H,
            _ => EcLevel::Q,
        }
    }
}

pub(crate) const fn dimension(version: u32) -> i32 {
    17 + 4 * version as i32
}

pub(crate) fn from_dimension(dim: usize) -> Option<u32> {
    if !(21..=177).contains(&dim) || (dim - 17) % 4 != 0 {
        return None;
    }
    Some(((dim - 17) / 4) as u32)
}

pub(crate) fn info(version: u32) -> &'static VersionInfo {
    &VERSIONS[version as usize - 1]
}

pub(crate) fn alignment(version: u32) -> &'static [u8] {
    info(version).alignment
}

pub(crate) fn total_codewords(version: u32) -> usize {
    let (ec, c1, d1, c2, d2) = info(version).ec[0];
    let (ec, c1, d1, c2, d2) = (
        usize::from(ec),
        usize::from(c1),
        usize::from(d1),
        usize::from(c2),
        usize::from(d2),
    );
    ec * (c1 + c2) + c1 * d1 + c2 * d2
}

/// Every module that is not data: finders, separators, format and version
/// information, timing and alignment patterns.
pub(crate) fn function_pattern(version: u32) -> BitMatrix {
    let d = dimension(version) as usize;
    let mut m = BitMatrix::new(d, d);
    m.set_region(0, 0, 9, 9);
    m.set_region(d - 8, 0, 8, 9);
    m.set_region(0, d - 8, 9, 8);
    let centres = alignment(version);
    let max = centres.len();
    for (x, &cx) in centres.iter().enumerate() {
        for (y, &cy) in centres.iter().enumerate() {
            // None where the finders are.
            if (x != 0 || (y != 0 && y != max - 1)) && (x != max - 1 || y != 0) {
                m.set_region(usize::from(cy) - 2, usize::from(cx) - 2, 5, 5);
            }
        }
    }
    m.set_region(6, 9, 1, d - 17);
    m.set_region(9, 6, d - 17, 1);
    if version > 6 {
        m.set_region(d - 11, 0, 3, 6);
        m.set_region(0, d - 11, 6, 3);
    }
    m
}

/// Versions 7–40 as their 18-bit BCH-coded version information.
const VERSION_DECODE_INFO: [u32; 34] = [
    0x07C94, 0x085BC, 0x09A99, 0x0A4D3, 0x0BBF6, 0x0C762, 0x0D847, 0x0E60D, 0x0F928, 0x10B78,
    0x1145D, 0x12A17, 0x13532, 0x149A6, 0x15683, 0x168C9, 0x177EC, 0x18EC4, 0x191E1, 0x1AFAB,
    0x1B08E, 0x1CC1A, 0x1D33F, 0x1ED75, 0x1F250, 0x209D5, 0x216F0, 0x228BA, 0x2379F, 0x24B0B,
    0x2542E, 0x26A64, 0x27541, 0x28C69,
];

/// The version both copies of the version information agree best on, if
/// within 3 bits (no two codewords are closer than 8).
pub(crate) fn decode_version_info(a: i64, b: i64) -> Option<u32> {
    let (mut best_diff, mut best) = (u32::MAX, 0u32);
    for (i, &target) in VERSION_DECODE_INFO.iter().enumerate() {
        for bits in [a, b] {
            if bits < 0 {
                continue;
            }
            let diff = ((bits as u32) ^ target).count_ones();
            if diff < best_diff {
                best = i as u32 + 7;
                best_diff = diff;
            }
        }
        if best_diff == 0 {
            break;
        }
    }
    (best_diff <= 3).then_some(best)
}

// Generated from ISO/IEC 18004:2015 Table 9 / Annex E, as tabulated by ZXing
// (rxing `version_build_versions_arrays.rs`). Levels in table order: L, M, Q, H.
// Each level: (ec codewords per block, count1, data1, count2, data2).
pub(crate) const VERSIONS: [VersionInfo; 40] = [
    VersionInfo {
        number: 1,
        alignment: &[],
        ec: [
            (7, 1, 19, 0, 0),
            (10, 1, 16, 0, 0),
            (13, 1, 13, 0, 0),
            (17, 1, 9, 0, 0),
        ],
    },
    VersionInfo {
        number: 2,
        alignment: &[6, 18],
        ec: [
            (10, 1, 34, 0, 0),
            (16, 1, 28, 0, 0),
            (22, 1, 22, 0, 0),
            (28, 1, 16, 0, 0),
        ],
    },
    VersionInfo {
        number: 3,
        alignment: &[6, 22],
        ec: [
            (15, 1, 55, 0, 0),
            (26, 1, 44, 0, 0),
            (18, 2, 17, 0, 0),
            (22, 2, 13, 0, 0),
        ],
    },
    VersionInfo {
        number: 4,
        alignment: &[6, 26],
        ec: [
            (20, 1, 80, 0, 0),
            (18, 2, 32, 0, 0),
            (26, 2, 24, 0, 0),
            (16, 4, 9, 0, 0),
        ],
    },
    VersionInfo {
        number: 5,
        alignment: &[6, 30],
        ec: [
            (26, 1, 108, 0, 0),
            (24, 2, 43, 0, 0),
            (18, 2, 15, 2, 16),
            (22, 2, 11, 2, 12),
        ],
    },
    VersionInfo {
        number: 6,
        alignment: &[6, 34],
        ec: [
            (18, 2, 68, 0, 0),
            (16, 4, 27, 0, 0),
            (24, 4, 19, 0, 0),
            (28, 4, 15, 0, 0),
        ],
    },
    VersionInfo {
        number: 7,
        alignment: &[6, 22, 38],
        ec: [
            (20, 2, 78, 0, 0),
            (18, 4, 31, 0, 0),
            (18, 2, 14, 4, 15),
            (26, 4, 13, 1, 14),
        ],
    },
    VersionInfo {
        number: 8,
        alignment: &[6, 24, 42],
        ec: [
            (24, 2, 97, 0, 0),
            (22, 2, 38, 2, 39),
            (22, 4, 18, 2, 19),
            (26, 4, 14, 2, 15),
        ],
    },
    VersionInfo {
        number: 9,
        alignment: &[6, 26, 46],
        ec: [
            (30, 2, 116, 0, 0),
            (22, 3, 36, 2, 37),
            (20, 4, 16, 4, 17),
            (24, 4, 12, 4, 13),
        ],
    },
    VersionInfo {
        number: 10,
        alignment: &[6, 28, 50],
        ec: [
            (18, 2, 68, 2, 69),
            (26, 4, 43, 1, 44),
            (24, 6, 19, 2, 20),
            (28, 6, 15, 2, 16),
        ],
    },
    VersionInfo {
        number: 11,
        alignment: &[6, 30, 54],
        ec: [
            (20, 4, 81, 0, 0),
            (30, 1, 50, 4, 51),
            (28, 4, 22, 4, 23),
            (24, 3, 12, 8, 13),
        ],
    },
    VersionInfo {
        number: 12,
        alignment: &[6, 32, 58],
        ec: [
            (24, 2, 92, 2, 93),
            (22, 6, 36, 2, 37),
            (26, 4, 20, 6, 21),
            (28, 7, 14, 4, 15),
        ],
    },
    VersionInfo {
        number: 13,
        alignment: &[6, 34, 62],
        ec: [
            (26, 4, 107, 0, 0),
            (22, 8, 37, 1, 38),
            (24, 8, 20, 4, 21),
            (22, 12, 11, 4, 12),
        ],
    },
    VersionInfo {
        number: 14,
        alignment: &[6, 26, 46, 66],
        ec: [
            (30, 3, 115, 1, 116),
            (24, 4, 40, 5, 41),
            (20, 11, 16, 5, 17),
            (24, 11, 12, 5, 13),
        ],
    },
    VersionInfo {
        number: 15,
        alignment: &[6, 26, 48, 70],
        ec: [
            (22, 5, 87, 1, 88),
            (24, 5, 41, 5, 42),
            (30, 5, 24, 7, 25),
            (24, 11, 12, 7, 13),
        ],
    },
    VersionInfo {
        number: 16,
        alignment: &[6, 26, 50, 74],
        ec: [
            (24, 5, 98, 1, 99),
            (28, 7, 45, 3, 46),
            (24, 15, 19, 2, 20),
            (30, 3, 15, 13, 16),
        ],
    },
    VersionInfo {
        number: 17,
        alignment: &[6, 30, 54, 78],
        ec: [
            (28, 1, 107, 5, 108),
            (28, 10, 46, 1, 47),
            (28, 1, 22, 15, 23),
            (28, 2, 14, 17, 15),
        ],
    },
    VersionInfo {
        number: 18,
        alignment: &[6, 30, 56, 82],
        ec: [
            (30, 5, 120, 1, 121),
            (26, 9, 43, 4, 44),
            (28, 17, 22, 1, 23),
            (28, 2, 14, 19, 15),
        ],
    },
    VersionInfo {
        number: 19,
        alignment: &[6, 30, 58, 86],
        ec: [
            (28, 3, 113, 4, 114),
            (26, 3, 44, 11, 45),
            (26, 17, 21, 4, 22),
            (26, 9, 13, 16, 14),
        ],
    },
    VersionInfo {
        number: 20,
        alignment: &[6, 34, 62, 90],
        ec: [
            (28, 3, 107, 5, 108),
            (26, 3, 41, 13, 42),
            (30, 15, 24, 5, 25),
            (28, 15, 15, 10, 16),
        ],
    },
    VersionInfo {
        number: 21,
        alignment: &[6, 28, 50, 72, 94],
        ec: [
            (28, 4, 116, 4, 117),
            (26, 17, 42, 0, 0),
            (28, 17, 22, 6, 23),
            (30, 19, 16, 6, 17),
        ],
    },
    VersionInfo {
        number: 22,
        alignment: &[6, 26, 50, 74, 98],
        ec: [
            (28, 2, 111, 7, 112),
            (28, 17, 46, 0, 0),
            (30, 7, 24, 16, 25),
            (24, 34, 13, 0, 0),
        ],
    },
    VersionInfo {
        number: 23,
        alignment: &[6, 30, 54, 78, 102],
        ec: [
            (30, 4, 121, 5, 122),
            (28, 4, 47, 14, 48),
            (30, 11, 24, 14, 25),
            (30, 16, 15, 14, 16),
        ],
    },
    VersionInfo {
        number: 24,
        alignment: &[6, 28, 54, 80, 106],
        ec: [
            (30, 6, 117, 4, 118),
            (28, 6, 45, 14, 46),
            (30, 11, 24, 16, 25),
            (30, 30, 16, 2, 17),
        ],
    },
    VersionInfo {
        number: 25,
        alignment: &[6, 32, 58, 84, 110],
        ec: [
            (26, 8, 106, 4, 107),
            (28, 8, 47, 13, 48),
            (30, 7, 24, 22, 25),
            (30, 22, 15, 13, 16),
        ],
    },
    VersionInfo {
        number: 26,
        alignment: &[6, 30, 58, 86, 114],
        ec: [
            (28, 10, 114, 2, 115),
            (28, 19, 46, 4, 47),
            (28, 28, 22, 6, 23),
            (30, 33, 16, 4, 17),
        ],
    },
    VersionInfo {
        number: 27,
        alignment: &[6, 34, 62, 90, 118],
        ec: [
            (30, 8, 122, 4, 123),
            (28, 22, 45, 3, 46),
            (30, 8, 23, 26, 24),
            (30, 12, 15, 28, 16),
        ],
    },
    VersionInfo {
        number: 28,
        alignment: &[6, 26, 50, 74, 98, 122],
        ec: [
            (30, 3, 117, 10, 118),
            (28, 3, 45, 23, 46),
            (30, 4, 24, 31, 25),
            (30, 11, 15, 31, 16),
        ],
    },
    VersionInfo {
        number: 29,
        alignment: &[6, 30, 54, 78, 102, 126],
        ec: [
            (30, 7, 116, 7, 117),
            (28, 21, 45, 7, 46),
            (30, 1, 23, 37, 24),
            (30, 19, 15, 26, 16),
        ],
    },
    VersionInfo {
        number: 30,
        alignment: &[6, 26, 52, 78, 104, 130],
        ec: [
            (30, 5, 115, 10, 116),
            (28, 19, 47, 10, 48),
            (30, 15, 24, 25, 25),
            (30, 23, 15, 25, 16),
        ],
    },
    VersionInfo {
        number: 31,
        alignment: &[6, 30, 56, 82, 108, 134],
        ec: [
            (30, 13, 115, 3, 116),
            (28, 2, 46, 29, 47),
            (30, 42, 24, 1, 25),
            (30, 23, 15, 28, 16),
        ],
    },
    VersionInfo {
        number: 32,
        alignment: &[6, 34, 60, 86, 112, 138],
        ec: [
            (30, 17, 115, 0, 0),
            (28, 10, 46, 23, 47),
            (30, 10, 24, 35, 25),
            (30, 19, 15, 35, 16),
        ],
    },
    VersionInfo {
        number: 33,
        alignment: &[6, 30, 58, 86, 114, 142],
        ec: [
            (30, 17, 115, 1, 116),
            (28, 14, 46, 21, 47),
            (30, 29, 24, 19, 25),
            (30, 11, 15, 46, 16),
        ],
    },
    VersionInfo {
        number: 34,
        alignment: &[6, 34, 62, 90, 118, 146],
        ec: [
            (30, 13, 115, 6, 116),
            (28, 14, 46, 23, 47),
            (30, 44, 24, 7, 25),
            (30, 59, 16, 1, 17),
        ],
    },
    VersionInfo {
        number: 35,
        alignment: &[6, 30, 54, 78, 102, 126, 150],
        ec: [
            (30, 12, 121, 7, 122),
            (28, 12, 47, 26, 48),
            (30, 39, 24, 14, 25),
            (30, 22, 15, 41, 16),
        ],
    },
    VersionInfo {
        number: 36,
        alignment: &[6, 24, 50, 76, 102, 128, 154],
        ec: [
            (30, 6, 121, 14, 122),
            (28, 6, 47, 34, 48),
            (30, 46, 24, 10, 25),
            (30, 2, 15, 64, 16),
        ],
    },
    VersionInfo {
        number: 37,
        alignment: &[6, 28, 54, 80, 106, 132, 158],
        ec: [
            (30, 17, 122, 4, 123),
            (28, 29, 46, 14, 47),
            (30, 49, 24, 10, 25),
            (30, 24, 15, 46, 16),
        ],
    },
    VersionInfo {
        number: 38,
        alignment: &[6, 32, 58, 84, 110, 136, 162],
        ec: [
            (30, 4, 122, 18, 123),
            (28, 13, 46, 32, 47),
            (30, 48, 24, 14, 25),
            (30, 42, 15, 32, 16),
        ],
    },
    VersionInfo {
        number: 39,
        alignment: &[6, 26, 54, 82, 110, 138, 166],
        ec: [
            (30, 20, 117, 4, 118),
            (28, 40, 47, 7, 48),
            (30, 43, 24, 22, 25),
            (30, 10, 15, 67, 16),
        ],
    },
    VersionInfo {
        number: 40,
        alignment: &[6, 30, 58, 86, 114, 142, 170],
        ec: [
            (30, 19, 118, 6, 119),
            (28, 18, 47, 31, 48),
            (30, 34, 24, 34, 25),
            (30, 20, 15, 61, 16),
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codeword_counts_match_the_standard() {
        assert_eq!(total_codewords(1), 26);
        assert_eq!(total_codewords(7), 196);
        assert_eq!(total_codewords(40), 3706);
        for v in 1..=40 {
            let total = total_codewords(v);
            for &(ec, c1, d1, c2, d2) in &info(v).ec {
                let blocks = usize::from(c1) + usize::from(c2);
                assert_eq!(
                    usize::from(ec) * blocks
                        + usize::from(c1) * usize::from(d1)
                        + usize::from(c2) * usize::from(d2),
                    total,
                    "v{v}"
                );
            }
            // Data modules = total codewords × 8 + remainder bits (0, 3, 4 or 7).
            let d = dimension(v) as usize;
            let fp = function_pattern(v);
            let data = (0..d * d).filter(|&i| !fp.get(i % d, i / d)).count();
            assert!(
                data >= total * 8 && data - total * 8 <= 7,
                "v{v}: {data} data modules for {total} codewords"
            );
        }
    }

    #[test]
    fn version_information_tolerates_three_bit_errors() {
        assert_eq!(decode_version_info(0x07C94, -1), Some(7));
        assert_eq!(decode_version_info(-1, 0x28C69 ^ 0b111), Some(40));
        assert_eq!(
            decode_version_info(0x07C94 ^ 0b1111, 0x07C94 ^ 0b1111),
            None
        );
    }
}

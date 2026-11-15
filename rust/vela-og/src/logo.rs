//! The Vela mark, drawn as vectors.
//!
//! Geometry and colours are verbatim from `app-web/getvela.app/static/favicon.svg`,
//! the canonical icon every platform's icons are rendered from (a 68×68
//! viewBox: a rounded square, two sails, a hull). `the_mark_is_the_favicon`
//! fails if the two ever drift apart.

use crate::font::{Point, Segment};

pub const VIEWBOX: f32 = 68.0;

/// `<rect x y width height rx>` and its fill.
pub const BACKGROUND: ([f32; 5], &str) = ([1.0, 1.0, 66.0, 66.0, 18.0], "#f46d50");

/// `<path d>` and its fill, in paint order.
pub const PATHS: [(&str, &str); 3] = [
    ("M33,12C25,21,20,32,17,42L33,42L33,12Z", "#fff3ec"),
    ("M36,19C45,25,50,34,52,42L36,42L36,19Z", "#ffc6b0"),
    (
        "M13,46L55,46C52,52,47,55,40,55L28,55C21,55,16,52,13,46Z",
        "#5a4037",
    ),
];

/// `#rrggbb` → [r, g, b].
pub fn hex_color(hex: &str) -> Option<[f32; 3]> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(f32::from);
    Some([c(0)?, c(2)?, c(4)?])
}

/// The absolute `M`, `L`, `C`, `Z` subset of SVG path data — what the mark uses.
pub fn parse_path(d: &str) -> Option<Vec<Segment>> {
    let mut out = Vec::new();
    let mut nums: Vec<f32> = Vec::new();
    let mut cmd: Option<char> = None;
    let (mut cur, mut start) = (Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 0.0 });

    let mut flush = |cmd: Option<char>,
                     nums: &mut Vec<f32>,
                     cur: &mut Point,
                     start: &mut Point|
     -> Option<()> {
        let p = |i: usize, nums: &[f32]| Point {
            x: nums[i],
            y: nums[i + 1],
        };
        match cmd {
            None => {}
            Some('M') => {
                if nums.len() != 2 {
                    return None;
                }
                *cur = p(0, nums);
                *start = *cur;
            }
            Some('L') => {
                if nums.is_empty() || nums.len() % 2 != 0 {
                    return None;
                }
                for i in (0..nums.len()).step_by(2) {
                    let q = p(i, nums);
                    out.push(Segment::Line(*cur, q));
                    *cur = q;
                }
            }
            Some('C') => {
                if nums.is_empty() || nums.len() % 6 != 0 {
                    return None;
                }
                for i in (0..nums.len()).step_by(6) {
                    let (c1, c2, q) = (p(i, nums), p(i + 2, nums), p(i + 4, nums));
                    out.push(Segment::Cubic(*cur, c1, c2, q));
                    *cur = q;
                }
            }
            Some('Z') => {
                if !nums.is_empty() {
                    return None;
                }
                if *cur != *start {
                    out.push(Segment::Line(*cur, *start));
                }
                *cur = *start;
            }
            Some(_) => return None,
        }
        nums.clear();
        Some(())
    };

    // A command letter is a token of its own, however the numbers around it are spaced.
    let spaced: String = d
        .chars()
        .flat_map(|c| {
            if c.is_ascii_alphabetic() {
                vec![' ', c, ' ']
            } else {
                vec![c]
            }
        })
        .collect();
    for token in spaced
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
    {
        if token.len() == 1 && token.chars().all(|c| c.is_ascii_alphabetic()) {
            flush(cmd, &mut nums, &mut cur, &mut start)?;
            cmd = token.chars().next();
        } else {
            nums.push(token.parse().ok()?);
        }
    }
    flush(cmd, &mut nums, &mut cur, &mut start)?;
    Some(out)
}

/// A rounded rectangle as lines and circular-arc cubics.
pub fn rounded_rect([x, y, w, h, r]: [f32; 5]) -> Vec<Segment> {
    const K: f32 = 0.552_284_8; // 4/3·(√2 − 1): a quarter circle as one cubic
    let p = |x: f32, y: f32| Point { x, y };
    let (l, t, rt, b) = (x, y, x + w, y + h);
    vec![
        Segment::Line(p(l + r, t), p(rt - r, t)),
        Segment::Cubic(
            p(rt - r, t),
            p(rt - r + K * r, t),
            p(rt, t + r - K * r),
            p(rt, t + r),
        ),
        Segment::Line(p(rt, t + r), p(rt, b - r)),
        Segment::Cubic(
            p(rt, b - r),
            p(rt, b - r + K * r),
            p(rt - r + K * r, b),
            p(rt - r, b),
        ),
        Segment::Line(p(rt - r, b), p(l + r, b)),
        Segment::Cubic(
            p(l + r, b),
            p(l + r - K * r, b),
            p(l, b - r + K * r),
            p(l, b - r),
        ),
        Segment::Line(p(l, b - r), p(l, t + r)),
        Segment::Cubic(
            p(l, t + r),
            p(l, t + r - K * r),
            p(l + r - K * r, t),
            p(l + r, t),
        ),
    ]
}

/// Every shape of the mark in paint order, in viewBox units (y down).
pub fn shapes() -> Vec<([f32; 3], Vec<Segment>)> {
    let mut out = vec![(
        hex_color(BACKGROUND.1).expect("valid colour"),
        rounded_rect(BACKGROUND.0),
    )];
    for (d, fill) in PATHS {
        out.push((
            hex_color(fill).expect("valid colour"),
            parse_path(d).expect("valid path"),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value of `name="…"` in an SVG element's source.
    fn attr<'a>(element: &'a str, name: &str) -> &'a str {
        let key = format!(" {name}=\"");
        let start = element
            .find(&key)
            .unwrap_or_else(|| panic!("no {name} in {element}"))
            + key.len();
        &element[start..start + element[start..].find('"').unwrap()]
    }

    #[test]
    fn the_mark_is_the_favicon() {
        let svg = include_str!("../../../app-web/getvela.app/static/favicon.svg");
        assert!(svg.contains("viewBox=\"0 0 68 68\""));
        let rect = svg
            .split("<rect")
            .nth(1)
            .unwrap()
            .split("/>")
            .next()
            .unwrap();
        let [x, y, w, h, r] = BACKGROUND.0;
        for (name, v) in [
            ("x", x),
            ("y", y),
            ("width", w),
            ("height", h),
            ("rx", r),
            ("ry", r),
        ] {
            assert_eq!(attr(rect, name).parse::<f32>().unwrap(), v, "rect {name}");
        }
        assert_eq!(attr(rect, "fill"), BACKGROUND.1);
        let paths: Vec<(&str, &str)> = svg
            .split("<path")
            .skip(1)
            .map(|p| {
                let el = p.split("/>").next().unwrap();
                (attr(el, "d"), attr(el, "fill"))
            })
            .collect();
        assert_eq!(paths, PATHS.to_vec(), "favicon.svg changed: update PATHS");
    }

    #[test]
    fn parses_the_paths() {
        for (d, _) in PATHS {
            let segs = parse_path(d).unwrap();
            assert!(segs.iter().any(|s| matches!(s, Segment::Cubic(..))));
        }
        assert!(parse_path("M1,2Q3,4,5,6").is_none());
        assert!(parse_path("M1").is_none());
    }
}

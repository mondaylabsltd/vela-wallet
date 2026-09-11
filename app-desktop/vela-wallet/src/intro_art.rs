//! The three intro illustrations — the desktop port of
//! `specs/020-intro-carousel/contracts/intro-illustrations.json`, whose
//! `consumers` line has named "desktop resvg" since spec 020 and which nobody
//! had ported until spec 038.
//!
//! Hand-ported, like the lucide corpus and the passkey marks: the contract is
//! the source, this file is what resvg draws, and the test at the bottom
//! re-reads the JSON and asserts every path still matches. Two colours: the
//! line work in `fg_subtle`, and the ONE accent thing each slide is about in
//! `accent`. `Outline` elements are filled with the page background first, so
//! they sit OVER what is behind them — the compass needle's southern half is
//! the reason.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{Hsla, RenderImage};

use crate::raster::{empty_render_image, render_image_from_pixmap};

pub const VIEW_W: f32 = 160.;
pub const VIEW_H: f32 = 128.;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IntroArt {
    NoSeedPhrase,
    KeysAreYours,
    OneAddress,
}

impl IntroArt {
    pub const ALL: [IntroArt; 3] = [Self::NoSeedPhrase, Self::KeysAreYours, Self::OneAddress];

    pub fn id(self) -> &'static str {
        match self {
            Self::NoSeedPhrase => "no-seed-phrase",
            Self::KeysAreYours => "keys-are-yours",
            Self::OneAddress => "one-address",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Line,
    Accent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Stroke,
    Fill,
    Outline,
}

struct Element {
    role: Role,
    mode: Mode,
    width: f32,
    cap: &'static str,
    opacity: f32,
    d: &'static str,
}

fn elements(art: IntroArt) -> &'static [Element] {
    match art {
        IntroArt::NoSeedPhrase => &[
            Element { role: Role::Line, mode: Mode::Stroke, width: 1.5, cap: "round", opacity: 0.85, d: "M11 3H95.5A10 10 0 0 1 105.5 13V115A10 10 0 0 1 95.5 125H11A10 10 0 0 1 1 115V13A10 10 0 0 1 11 3Z" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 2.5, cap: "round", opacity: 0.7, d: "M17.25 25L48.75 25M59.25 25L84.75 25" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 2.5, cap: "round", opacity: 0.6, d: "M17.25 42L52.75 42M59.25 42L80.75 42" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 2.5, cap: "round", opacity: 0.5, d: "M17.25 59L44.75 59M59.25 59L88.75 59" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 2.5, cap: "round", opacity: 0.4, d: "M17.25 76L50.75 76M59.25 76L82.75 76" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 2.5, cap: "round", opacity: 0.3, d: "M17.25 93L46.75 93M59.25 93L78.75 93" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 2.5, cap: "round", opacity: 0.2, d: "M17.25 110L42.75 110M59.25 110L86.75 110" },
            Element { role: Role::Accent, mode: Mode::Stroke, width: 2., cap: "round", opacity: 1., d: "M121.15 76.91A6.5 8.2 0 1 1 119.87 87.1M115.84 72.03A13 16.4 0 1 1 112.57 81.29M111.71 65.61A19.5 24.6 0 1 1 106.24 79.15M111.34 55.49A26 32.8 0 1 1 108.1 107.38M102.98 99.4A26 32.8 0 0 1 106.18 61.05M105.05 51.14A32.5 41 0 1 1 93.04 80.85" },
            Element { role: Role::Accent, mode: Mode::Fill, width: 0., cap: "round", opacity: 1., d: "M123.9 83A1.6 1.6 0 0 1 127.1 83A1.6 1.6 0 0 1 123.9 83" },
        ],
        IntroArt::KeysAreYours => &[
            Element { role: Role::Line, mode: Mode::Stroke, width: 2., cap: "round", opacity: 1., d: "M48.5 33.5A30.5 30.5 0 0 1 109.5 33.5A30.5 30.5 0 0 1 48.5 33.5" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 1.5, cap: "round", opacity: 0.45, d: "M90.57 55.07A10 10 0 0 1 110.57 55.07A10 10 0 0 1 90.57 55.07M107.64 62.14L133.45 87.95M126.02 80.52L130.27 76.28M130.8 85.3L136.1 79.99M47.43 55.07A10 10 0 0 1 67.43 55.07A10 10 0 0 1 47.43 55.07M50.36 62.14L24.55 87.95M31.98 80.52L36.22 84.77M27.2 85.3L32.51 90.6" },
            Element { role: Role::Accent, mode: Mode::Stroke, width: 3.5, cap: "round", opacity: 1., d: "M79 74L79 126M79 112L87 112M79 121L89 121" },
            Element { role: Role::Accent, mode: Mode::Stroke, width: 4., cap: "round", opacity: 1., d: "M69 64A10 10 0 0 1 89 64A10 10 0 0 1 69 64" },
            Element { role: Role::Accent, mode: Mode::Stroke, width: 3., cap: "round", opacity: 1., d: "M75.5 64A3.5 3.5 0 0 1 82.5 64A3.5 3.5 0 0 1 75.5 64" },
        ],
        IntroArt::OneAddress => &[
            Element { role: Role::Line, mode: Mode::Stroke, width: 1.5, cap: "round", opacity: 0.85, d: "M19.5 64A60.5 60.5 0 0 1 140.5 64A60.5 60.5 0 0 1 19.5 64" },
            Element { role: Role::Line, mode: Mode::Stroke, width: 1.8, cap: "round", opacity: 0.85, d: "M75.5 23.5L75.5 10.5L84.5 23.5L84.5 10.5" },
            Element { role: Role::Line, mode: Mode::Fill, width: 0., cap: "round", opacity: 1., d: "M126.5 64A3 3 0 0 1 132.5 64A3 3 0 0 1 126.5 64M77 113.5A3 3 0 0 1 83 113.5A3 3 0 0 1 77 113.5M27.5 64A3 3 0 0 1 33.5 64A3 3 0 0 1 27.5 64" },
            Element { role: Role::Line, mode: Mode::Fill, width: 0., cap: "round", opacity: 0.55, d: "M120.37 88.75A2.5 2.5 0 0 1 125.37 88.75A2.5 2.5 0 0 1 120.37 88.75M102.25 106.87A2.5 2.5 0 0 1 107.25 106.87A2.5 2.5 0 0 1 102.25 106.87M52.75 106.87A2.5 2.5 0 0 1 57.75 106.87A2.5 2.5 0 0 1 52.75 106.87M34.63 88.75A2.5 2.5 0 0 1 39.63 88.75A2.5 2.5 0 0 1 34.63 88.75M35.13 39.25A2 2 0 0 1 39.13 39.25A2 2 0 0 1 35.13 39.25M53.5 21.13A1.75 1.75 0 0 1 57 21.13A1.75 1.75 0 0 1 53.5 21.13M103 21.13A1.75 1.75 0 0 1 106.5 21.13A1.75 1.75 0 0 1 103 21.13M120.87 39.25A2 2 0 0 1 124.87 39.25A2 2 0 0 1 120.87 39.25" },
            Element { role: Role::Accent, mode: Mode::Fill, width: 0., cap: "round", opacity: 1., d: "M70.86 60.43L97.12 26.04L89.14 68.57Z" },
            Element { role: Role::Line, mode: Mode::Outline, width: 1.2, cap: "round", opacity: 0.8, d: "M70.86 60.43L63.16 102.32L89.14 68.57Z" },
            Element { role: Role::Accent, mode: Mode::Outline, width: 2., cap: "round", opacity: 1., d: "M75.5 64.5A4.5 4.5 0 0 1 84.5 64.5A4.5 4.5 0 0 1 75.5 64.5" },
        ],
    }
}

/// The three colours an illustration is drawn with.
#[derive(Clone, Copy)]
pub struct Palette {
    pub line: Hsla,
    pub accent: Hsla,
    pub paper: Hsla,
}

fn hex(color: Hsla) -> (String, u32) {
    let rgba: gpui::Rgba = color.into();
    let r = (rgba.r * 255.).round() as u32;
    let g = (rgba.g * 255.).round() as u32;
    let b = (rgba.b * 255.).round() as u32;
    (format!("#{r:02x}{g:02x}{b:02x}"), (r << 16) | (g << 8) | b)
}

fn svg_document(art: IntroArt, palette: &Palette) -> String {
    let (line, _) = hex(palette.line);
    let (accent, _) = hex(palette.accent);
    let (paper, _) = hex(palette.paper);
    let mut inner = String::new();
    for el in elements(art) {
        let paint = match el.role {
            Role::Line => &line,
            Role::Accent => &accent,
        };
        let (fill, stroke) = match el.mode {
            Mode::Stroke => ("none".to_owned(), paint.clone()),
            Mode::Fill => (paint.clone(), "none".to_owned()),
            Mode::Outline => (paper.clone(), paint.clone()),
        };
        inner.push_str(&format!(
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" stroke-linecap="{}" stroke-linejoin="round" opacity="{}"/>"#,
            el.d, fill, stroke, el.width, el.cap, el.opacity
        ));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {VIEW_W} {VIEW_H}">{inner}</svg>"##
    )
}

#[derive(Default)]
pub struct IntroArtCache {
    map: HashMap<(IntroArt, u32, u32, u32, u32), Arc<RenderImage>>,
}

impl IntroArtCache {
    /// The illustration at `logical_w` wide (height follows the viewBox),
    /// rasterised at 2× and cached per (art, palette, size).
    pub fn image(&mut self, art: IntroArt, palette: Palette, logical_w: u32) -> Arc<RenderImage> {
        let w = logical_w * crate::icons::RASTER_SCALE;
        let key = (art, hex(palette.line).1, hex(palette.accent).1, hex(palette.paper).1, w);
        if let Some(image) = self.map.get(&key) {
            return Arc::clone(image);
        }
        let h = (w as f32 * VIEW_H / VIEW_W).round() as u32;
        let image = rasterize(&svg_document(art, &palette), w, h).unwrap_or_else(empty_render_image);
        self.map.insert(key, Arc::clone(&image));
        image
    }
}

/// A non-square rasterise — `icons::rasterize` draws squares.
fn rasterize(svg: &str, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let view = tree.size();
    let transform =
        resvg::tiny_skia::Transform::from_scale(w as f32 / view.width(), h as f32 / view.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    render_image_from_pixmap(&pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str =
        include_str!("../../../specs/020-intro-carousel/contracts/intro-illustrations.json");

    /// SC-417: the desktop's paths are the contract's, proven, not eyeballed.
    #[test]
    fn every_illustration_is_the_contract() {
        let json: serde_json::Value = serde_json::from_str(CONTRACT).expect("contract json");
        assert_eq!(json["viewBox"]["width"].as_f64().unwrap() as f32, VIEW_W);
        assert_eq!(json["viewBox"]["height"].as_f64().unwrap() as f32, VIEW_H);
        let specs = json["illustrations"].as_array().unwrap();
        for art in IntroArt::ALL {
            let spec = specs
                .iter()
                .find(|s| s["id"] == art.id())
                .unwrap_or_else(|| panic!("{} missing from the contract", art.id()));
            let want = spec["elements"].as_array().unwrap();
            let have = elements(art);
            assert_eq!(want.len(), have.len(), "{}: element count", art.id());
            for (i, (w, h)) in want.iter().zip(have).enumerate() {
                assert_eq!(w["d"].as_str().unwrap(), h.d, "{}[{i}]: path", art.id());
                assert_eq!(w["opacity"].as_f64().unwrap() as f32, h.opacity, "{}[{i}]: opacity", art.id());
                assert_eq!(w["role"].as_str().unwrap() == "accent", h.role == Role::Accent, "{}[{i}]: role", art.id());
                let mode = w["mode"].as_str().unwrap();
                assert_eq!(mode == "fill", h.mode == Mode::Fill, "{}[{i}]: mode", art.id());
                assert_eq!(mode == "outline", h.mode == Mode::Outline, "{}[{i}]: mode", art.id());
            }
        }
    }

    #[test]
    fn every_illustration_rasterises() {
        let palette = Palette {
            line: gpui::rgb(0x888888).into(),
            accent: gpui::rgb(0xe8532d).into(),
            paper: gpui::rgb(0xffffff).into(),
        };
        for art in IntroArt::ALL {
            assert!(rasterize(&svg_document(art, &palette), 160, 128).is_some(), "{art:?}");
        }
    }
}

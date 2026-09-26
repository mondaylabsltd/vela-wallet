//! Wallet icon corpus — the desktop port of
//! `specs/015-wallet-home-ui/contracts/icons.json` (research.md D2, rev 2).
//!
//! Every glyph is lucide v1.11 (ISC): outline = verbatim stroke defs, nav
//! solid = fills derived from the same geometry (evenodd holes; the users
//! back-person arcs stay stroked). gpui's `svg()` renders monochrome masks
//! through an AssetSource this app doesn't have, so icons are tinted by
//! substituting the color into the SVG template, rasterized with resvg, and
//! cached per (icon, solid, color, size).

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{Hsla, RenderImage, Rgba};

use crate::raster::{empty_render_image, render_image_from_pixmap};

/// Rasterize at 2× the logical size for retina crispness.
pub(crate) const RASTER_SCALE: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    // nav (outline/solid pairs, fill style)
    NavWallet,
    NavContacts,
    NavExplore,
    NavSettings,
    // utility (stroke style)
    ArrowDownLeft,
    ArrowUpRight,
    ScanLine,
    EyeOff,
    Search,
    X,
    Copy,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Link2,
    TriangleAlert,
    RefreshCw,
    Check,
    Inbox,
    WalletOutline,
    // contacts (spec 018 contracts/icons.json, stroke style)
    UserRoundPlus,
    UsersRound,
    FolderPlus,
    Download,
    Upload,
    Pencil,
    Trash2,
    Ellipsis,
    QrCode,
    Plus,
    /// Mobile back chevron. Part of the shared spec-018 glyph contract
    /// (contracts/icons.json) so all four platforms extract the same lucide
    /// source; the desktop shell navigates by sidebar and has no back row.
    #[allow(dead_code, reason = "cross-platform icon contract, mobile-only glyph")]
    ChevronLeft,
    // settings (spec 023, lucide v1.11.0, stroke style)
    Sun,
    Moon,
    Monitor,
    Coins,
    Network,
    Server,
    Zap,
    /// The default transaction speed's Settings page (spec 069) — lucide
    /// `clock`, as the web's row draws it.
    Clock,
    /// Send feedback's nav row (078 S-03) — lucide `message-square-text`,
    /// as the web's.
    MessageSquareText,
    HardDrive,
    Info,
    /// Lucide `circle-alert` — the send form's refusal line (078 F-09).
    CircleAlert,
    LogOut,
    ExternalLink,
    // explore + signing (spec 022; lucide v1.11.0 except `Star`, a computed
    // five-point path — a mis-remembered lucide star draws a shape nobody can
    // name)
    ArrowLeft,
    ArrowRight,
    ArrowDown,
    Star,
    Share2,
    Power,
    Lock,
    /// The group manager's drag handle and its hidden/shown eye. Part of the
    /// shared spec-022 glyph contract so all four platforms extract the same
    /// lucide source; the desktop mocks have no group manager (DE2 manages
    /// favourites by right-click), so nothing here draws them yet.
    #[allow(dead_code, reason = "cross-platform icon contract, phone-only glyphs")]
    GripVertical,
    #[allow(dead_code, reason = "cross-platform icon contract, phone-only glyphs")]
    Eye,
}

/// Inner SVG markup per icon. `{c}` is substituted with the tint. Nav-solid
/// bodies carry explicit per-element paint (mixed fill/stroke, evenodd holes)
/// and are wrapped in a bare `<svg>`; everything else uses the stroke wrapper.
fn body(icon: Icon, solid: bool) -> &'static str {
    match icon {
        Icon::NavWallet => {
            if solid {
                r##"<path fill="{c}" d="M18 3a1 1 0 0 1 1 1v3h1a1 1 0 0 1 1 1v3h-4a2 2 0 0 0 0 4h4v4a1 1 0 0 1-1 1H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h13z"/>"##
            } else {
                r##"<path d="M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1"/><path d="M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4"/>"##
            }
        }
        Icon::NavContacts => {
            if solid {
                r##"<path fill="{c}" d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2z"/><circle fill="{c}" cx="9" cy="7" r="4"/><path fill="none" stroke="{c}" stroke-width="2" stroke-linecap="round" d="M16 3.128a4 4 0 0 1 0 7.744"/><path fill="none" stroke="{c}" stroke-width="2" stroke-linecap="round" d="M22 21v-2a4 4 0 0 0-3-3.87"/>"##
            } else {
                r##"<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><path d="M16 3.128a4 4 0 0 1 0 7.744"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><circle cx="9" cy="7" r="4"/>"##
            }
        }
        Icon::NavExplore => {
            if solid {
                r##"<path fill="{c}" fill-rule="evenodd" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zM16.24 7.76l-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z"/>"##
            } else {
                r##"<circle cx="12" cy="12" r="10"/><path d="m16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z"/>"##
            }
        }
        Icon::NavSettings => {
            if solid {
                r##"<path fill="{c}" fill-rule="evenodd" d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"/>"##
            } else {
                r##"<path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>"##
            }
        }
        Icon::ArrowDownLeft => r##"<path d="M17 7 7 17"/><path d="M17 17H7V7"/>"##,
        Icon::ArrowUpRight => r##"<path d="M7 7h10v10"/><path d="M7 17 17 7"/>"##,
        Icon::ScanLine => {
            r##"<path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><path d="M7 12h10"/>"##
        }
        Icon::EyeOff => {
            r##"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>"##
        }
        Icon::Search => r##"<path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/>"##,
        Icon::X => r##"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"##,
        Icon::Copy => {
            r##"<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"##
        }
        Icon::ChevronRight => r##"<path d="m9 18 6-6-6-6"/>"##,
        Icon::ChevronDown => r##"<path d="m6 9 6 6 6-6"/>"##,
        Icon::ChevronUp => r##"<path d="m18 15-6-6-6 6"/>"##,
        Icon::Link2 => {
            r##"<path d="M9 17H7A5 5 0 0 1 7 7h2"/><path d="M15 7h2a5 5 0 1 1 0 10h-2"/><line x1="8" x2="16" y1="12" y2="12"/>"##
        }
        Icon::TriangleAlert => {
            r##"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>"##
        }
        Icon::RefreshCw => {
            r##"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"##
        }
        Icon::Check => r##"<path d="M20 6 9 17l-5-5"/>"##,
        Icon::Inbox => {
            r##"<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>"##
        }
        Icon::WalletOutline => {
            r##"<path d="M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1"/><path d="M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4"/>"##
        }
        // -- contacts glyphs (specs/018-contacts-ui/contracts/icons.json) ----
        Icon::UserRoundPlus => {
            r##"<path d="M2 21a8 8 0 0 1 13.292-6"/><circle cx="10" cy="8" r="5"/><path d="M19 16v6"/><path d="M22 19h-6"/>"##
        }
        Icon::UsersRound => {
            r##"<path d="M18 21a8 8 0 0 0-16 0"/><circle cx="10" cy="8" r="5"/><path d="M22 20c0-3.37-2-6.5-4-8a5 5 0 0 0-.45-8.3"/>"##
        }
        Icon::FolderPlus => {
            r##"<path d="M12 10v6"/><path d="M9 13h6"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"##
        }
        Icon::Download => {
            r##"<path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/>"##
        }
        Icon::Upload => {
            r##"<path d="M12 3v12"/><path d="m17 8-5-5-5 5"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>"##
        }
        Icon::Pencil => {
            r##"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/>"##
        }
        Icon::Trash2 => {
            r##"<path d="M10 11v6"/><path d="M14 11v6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>"##
        }
        Icon::Ellipsis => {
            r##"<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>"##
        }
        Icon::QrCode => {
            r##"<rect width="5" height="5" x="3" y="3" rx="1"/><rect width="5" height="5" x="16" y="3" rx="1"/><rect width="5" height="5" x="3" y="16" rx="1"/><path d="M21 16h-3a2 2 0 0 0-2 2v3"/><path d="M21 21v.01"/><path d="M12 7v3a2 2 0 0 1-2 2H7"/><path d="M3 12h.01"/><path d="M12 3h.01"/><path d="M12 16v.01"/><path d="M16 12h1"/><path d="M21 12v.01"/><path d="M12 21v-1"/>"##
        }
        Icon::Plus => r##"<path d="M5 12h14"/><path d="M12 5v14"/>"##,
        Icon::ChevronLeft => r##"<path d="m15 18-6-6 6-6"/>"##,
        Icon::Sun => {
            r##"<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>"##
        }
        Icon::Moon => {
            r##"<path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"/>"##
        }
        Icon::Monitor => {
            r##"<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>"##
        }
        Icon::Coins => {
            r##"<path d="M13.744 17.736a6 6 0 1 1-7.48-7.48"/><path d="M15 6h1v4"/><path d="m6.134 14.768.866-.5 2 3.464"/><circle cx="16" cy="8" r="6"/>"##
        }
        Icon::Network => {
            r##"<rect width="6" height="6" x="16" y="16" rx="1"/><rect width="6" height="6" x="2" y="16" rx="1"/><rect width="6" height="6" x="9" y="2" rx="1"/><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"/><path d="M12 12V8"/>"##
        }
        Icon::Server => {
            r##"<rect width="20" height="8" x="2" y="2" rx="2"/><rect width="20" height="8" x="2" y="14" rx="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/>"##
        }
        Icon::Zap => {
            r##"<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>"##
        }
        Icon::Clock => r##"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"##,
        Icon::MessageSquareText => {
            r##"<path d="M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z"/><path d="M7 11h10"/><path d="M7 15h6"/><path d="M7 7h8"/>"##
        }
        Icon::HardDrive => {
            r##"<path d="M10 16h.01"/><path d="M2.212 11.577a2 2 0 0 0-.212.896V18a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-5.527a2 2 0 0 0-.212-.896L18.55 5.11A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><path d="M21.946 12.013H2.054"/><path d="M6 16h.01"/>"##
        }
        Icon::Info => {
            r##"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>"##
        }
        Icon::CircleAlert => {
            r##"<circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/>"##
        }
        Icon::LogOut => {
            r##"<path d="m16 17 5-5-5-5"/><path d="M21 12H9"/><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>"##
        }
        Icon::ExternalLink => {
            r##"<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>"##
        }
        Icon::ArrowLeft => r##"<path d="M19 12H5"/><path d="m12 19-7-7 7-7"/>"##,
        Icon::ArrowRight => r##"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"##,
        Icon::ArrowDown => r##"<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>"##,
        Icon::Star => {
            r##"<path d="M12.00 2.70 L14.35 8.76 L20.84 9.13 L15.80 13.24 L17.47 19.52 L12.00 16.00 L6.53 19.52 L8.20 13.24 L3.16 9.13 L9.65 8.76 Z"/>"##
        }
        Icon::Share2 => {
            r##"<circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><line x1="8.59" x2="15.42" y1="13.51" y2="17.49"/><line x1="15.41" x2="8.59" y1="6.51" y2="10.49"/>"##
        }
        Icon::Power => r##"<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>"##,
        Icon::Lock => {
            r##"<rect width="18" height="11" x="3" y="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"##
        }
        Icon::GripVertical => {
            r##"<circle cx="9" cy="5.5" r="1.2"/><circle cx="9" cy="12" r="1.2"/><circle cx="9" cy="18.5" r="1.2"/><circle cx="16" cy="5.5" r="1.2"/><circle cx="16" cy="12" r="1.2"/><circle cx="16" cy="18.5" r="1.2"/>"##
        }
        Icon::Eye => {
            r##"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"##
        }
    }
}

fn is_nav(icon: Icon) -> bool {
    matches!(
        icon,
        Icon::NavWallet | Icon::NavContacts | Icon::NavExplore | Icon::NavSettings
    )
}

fn svg_document(icon: Icon, solid: bool, color_hex: &str) -> String {
    let inner = body(icon, solid);
    if is_nav(icon) && solid {
        // Nav-solid bodies carry their own per-element paint via `{c}`.
        let inner = inner.replace("{c}", color_hex);
        format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{inner}</svg>"##)
    } else {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{color_hex}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{inner}</svg>"##
        )
    }
}

fn hex_of(color: Hsla) -> (String, u32) {
    let rgba: Rgba = color.into();
    let r = (rgba.r * 255.).round() as u32;
    let g = (rgba.g * 255.).round() as u32;
    let b = (rgba.b * 255.).round() as u32;
    (format!("#{r:02x}{g:02x}{b:02x}"), (r << 16) | (g << 8) | b)
}

#[derive(Default)]
pub struct IconCache {
    map: HashMap<(Icon, bool, u32, u32), Arc<RenderImage>>,
}

impl IconCache {
    /// Tinted icon image at `logical_px` — rasterized once per
    /// (icon, style, color, size) and cached for the app's lifetime.
    pub fn image(
        &mut self,
        icon: Icon,
        solid: bool,
        color: Hsla,
        logical_px: u32,
    ) -> Arc<RenderImage> {
        let (hex, color_key) = hex_of(color);
        let size = logical_px * RASTER_SCALE;
        let key = (icon, solid, color_key, size);
        if let Some(image) = self.map.get(&key) {
            return Arc::clone(image);
        }
        let image =
            rasterize(&svg_document(icon, solid, &hex), size).unwrap_or_else(empty_render_image);
        self.map.insert(key, Arc::clone(&image));
        image
    }
}

pub(crate) fn rasterize(svg: &str, size: u32) -> Option<Arc<RenderImage>> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let view = tree.size();
    let transform = resvg::tiny_skia::Transform::from_scale(
        size as f32 / view.width(),
        size as f32 / view.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    render_image_from_pixmap(&pixmap)
}

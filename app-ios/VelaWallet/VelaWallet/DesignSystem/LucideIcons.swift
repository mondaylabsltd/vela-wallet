//
//  LucideIcons.swift
//  VelaWallet
//
//  The lucide icon corpus (spec 015 contracts/icons.json, rev 2) as SVG
//  documents, rasterized through vela-core's `rasterizeSvgPng` so all four
//  platforms draw identical glyphs (research.md D2 revision: lucide
//  everywhere; SF Symbols retired from the wallet surfaces).
//
//  Outline glyphs are verbatim lucide v1.11 stroke defs (stroke 2, round
//  caps/joins). Nav solid variants are fills derived from the same geometry
//  (evenodd holes; the contacts back-person arcs stay stroked). Paint is
//  always white — the view layer renders the PNG as a template image and
//  tints it with `.foregroundStyle`.
//

import CoreGraphics

/// Every wallet glyph. Raw markup lives here (DesignSystem is the sanctioned
/// home for visual constants); rendering lives in `Components/Wallet/LucideIcon`.
enum LucideGlyph: String {
    case navWalletOutline, navWalletSolid
    case navContactsOutline, navContactsSolid
    case navExploreOutline, navExploreSolid
    case navSettingsOutline, navSettingsSolid
    case arrowDownLeft, arrowUpRight, scanLine
    case eyeOff, search, close, copy
    case chevronRight, chevronDown
    case link2, triangleAlert, refreshCw, check, inbox
    case walletUtility
    // Contacts utility glyphs (spec 018 contracts/icons.json — lucide v1.11.0).
    case userRoundPlus, usersRound, folderPlus
    case download, upload, pencil, trash2
    case ellipsis, qrCode, plus, chevronLeft
    // spec 023 — the settings rows' leading glyphs and their chrome
    case globe, sun, moon, monitor, coins, hash, calendar
    case network, server, hardDrive, info, logOut
    case messageSquareText
    // Wallet-flow glyphs (spec 021 — lucide v1.11.0).
    case clock, userRound, chevronsUpDown, creditCard, fileText, image, zap, rotateCcw
    // Explore + signing glyphs (spec 022 — lucide v1.11.0, except `star`,
    // which is a computed five-point path: a mis-recalled star draws a shape
    // nobody can name). Spec 022's screens were written against these names
    // and the corpus never got them, so `Explore` and `Signing` did not build.
    case arrowLeft, arrowRight, arrowDown
    case eye
    case lock, star, share2, power, externalLink, gripVertical
    /// The same five-point path, filled. A favourite that is ON and one that
    /// is off must differ by more than a tint: colour alone is invisible to
    /// somebody who cannot see it, and the nav bar's outline/solid pairs
    /// already set the convention.
    case starSolid
    // `clock`, `zap` and `externalLink` are declared once, above: specs 021,
    // 022 and 023 each reached for them independently.

    /// Complete SVG document, white paint, 24×24 viewBox.
    var svg: String {
        switch self {
        case .navWalletSolid, .navContactsSolid, .navExploreSolid, .navSettingsSolid, .starSolid:
            return "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\">\(body)</svg>"
        default:
            return "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"#FFFFFF\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\(body)</svg>"
        }
    }

    private var body: String {
        switch self {
        case .navWalletOutline:
            return ##"<path d="M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1"/><path d="M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4"/>"##
        case .navWalletSolid:
            return ##"<path fill="#FFFFFF" d="M18 3a1 1 0 0 1 1 1v3h1a1 1 0 0 1 1 1v3h-4a2 2 0 0 0 0 4h4v4a1 1 0 0 1-1 1H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h13z"/>"##
        case .navContactsOutline:
            return ##"<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><path d="M16 3.128a4 4 0 0 1 0 7.744"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><circle cx="9" cy="7" r="4"/>"##
        case .navContactsSolid:
            return ##"<path fill="#FFFFFF" d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2z"/><circle fill="#FFFFFF" cx="9" cy="7" r="4"/><path fill="none" stroke="#FFFFFF" stroke-width="2" stroke-linecap="round" d="M16 3.128a4 4 0 0 1 0 7.744"/><path fill="none" stroke="#FFFFFF" stroke-width="2" stroke-linecap="round" d="M22 21v-2a4 4 0 0 0-3-3.87"/>"##
        case .navExploreOutline:
            return ##"<circle cx="12" cy="12" r="10"/><path d="m16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z"/>"##
        case .navExploreSolid:
            return ##"<path fill="#FFFFFF" fill-rule="evenodd" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zM16.24 7.76l-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z"/>"##
        case .navSettingsOutline:
            return ##"<path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>"##
        case .navSettingsSolid:
            return ##"<path fill="#FFFFFF" fill-rule="evenodd" d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"/>"##
        case .arrowDownLeft:
            return ##"<path d="M17 7 7 17"/><path d="M17 17H7V7"/>"##
        case .arrowUpRight:
            return ##"<path d="M7 7h10v10"/><path d="M7 17 17 7"/>"##
        case .scanLine:
            return ##"<path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><path d="M7 12h10"/>"##
        case .eyeOff:
            return ##"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>"##
        case .search:
            return ##"<path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/>"##
        case .close:
            return ##"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"##
        case .copy:
            return ##"<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"##
        case .chevronRight:
            return ##"<path d="m9 18 6-6-6-6"/>"##
        case .chevronDown:
            return ##"<path d="m6 9 6 6 6-6"/>"##
        case .link2:
            return ##"<path d="M9 17H7A5 5 0 0 1 7 7h2"/><path d="M15 7h2a5 5 0 1 1 0 10h-2"/><line x1="8" x2="16" y1="12" y2="12"/>"##
        case .triangleAlert:
            return ##"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>"##
        case .refreshCw:
            return ##"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"##
        case .check:
            return ##"<path d="M20 6 9 17l-5-5"/>"##
        case .inbox:
            return ##"<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>"##
        case .walletUtility:
            return ##"<path d="M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1"/><path d="M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4"/>"##
        case .userRoundPlus:
            return ##"<path d="M2 21a8 8 0 0 1 13.292-6"/><circle cx="10" cy="8" r="5"/><path d="M19 16v6"/><path d="M22 19h-6"/>"##
        case .usersRound:
            return ##"<path d="M18 21a8 8 0 0 0-16 0"/><circle cx="10" cy="8" r="5"/><path d="M22 20c0-3.37-2-6.5-4-8a5 5 0 0 0-.45-8.3"/>"##
        case .folderPlus:
            return ##"<path d="M12 10v6"/><path d="M9 13h6"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"##
        case .download:
            return ##"<path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/>"##
        case .upload:
            return ##"<path d="M12 3v12"/><path d="m17 8-5-5-5 5"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>"##
        case .pencil:
            return ##"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/>"##
        case .trash2:
            return ##"<path d="M10 11v6"/><path d="M14 11v6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>"##
        case .ellipsis:
            return ##"<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>"##
        case .qrCode:
            return ##"<rect width="5" height="5" x="3" y="3" rx="1"/><rect width="5" height="5" x="16" y="3" rx="1"/><rect width="5" height="5" x="3" y="16" rx="1"/><path d="M21 16h-3a2 2 0 0 0-2 2v3"/><path d="M21 21v.01"/><path d="M12 7v3a2 2 0 0 1-2 2H7"/><path d="M3 12h.01"/><path d="M12 3h.01"/><path d="M12 16v.01"/><path d="M16 12h1"/><path d="M21 12v.01"/><path d="M12 21v-1"/>"##
        case .plus:
            return ##"<path d="M5 12h14"/><path d="M12 5v14"/>"##
        case .clock:
            return ##"<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>"##
        // The single-person glyph. `usersRound` is the group; SD2's recipient
        // field opens a picker for exactly one person, and two heads there read
        // as "add several".
        case .userRound:
            return ##"<circle cx="12" cy="8" r="5"/><path d="M20 21a8 8 0 0 0-16 0"/>"##
        // SD2's denomination toggle: token or display currency.
        case .chevronsUpDown:
            return ##"<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>"##
        case .creditCard:
            return ##"<rect width="20" height="14" x="2" y="5" rx="2"/><line x1="2" x2="22" y1="10" y2="10"/>"##
        case .fileText:
            return ##"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>"##
        case .image:
            return ##"<rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>"##
        case .zap:
            return ##"<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>"##
        case .rotateCcw:
            return ##"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>"##
        case .eye:
            return ##"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"##
        case .arrowLeft:
            return ##"<path d="M19 12H5"/><path d="m12 19-7-7 7-7"/>"##
        case .arrowRight:
            return ##"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"##
        case .arrowDown:
            return ##"<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>"##
        case .lock:
            return ##"<rect width="18" height="11" x="3" y="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"##
        case .star:
            return ##"<path d="M12.00 2.70 L14.35 8.76 L20.84 9.13 L15.80 13.24 L17.47 19.52 L12.00 16.00 L6.53 19.52 L8.20 13.24 L3.16 9.13 L9.65 8.76 Z"/>"##
        case .starSolid:
            return ##"<path fill="#FFFFFF" d="M12.00 2.70 L14.35 8.76 L20.84 9.13 L15.80 13.24 L17.47 19.52 L12.00 16.00 L6.53 19.52 L8.20 13.24 L3.16 9.13 L9.65 8.76 Z"/>"##
        case .share2:
            return ##"<circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><line x1="8.59" x2="15.42" y1="13.51" y2="17.49"/><line x1="15.41" x2="8.59" y1="6.51" y2="10.49"/>"##
        case .power:
            return ##"<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>"##
        case .externalLink:
            return ##"<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>"##
        case .gripVertical:
            return ##"<circle cx="8.5" cy="5.5" r="1.5" fill="#FFFFFF" stroke="none"/><circle cx="8.5" cy="12" r="1.5" fill="#FFFFFF" stroke="none"/><circle cx="8.5" cy="18.5" r="1.5" fill="#FFFFFF" stroke="none"/><circle cx="15.5" cy="5.5" r="1.5" fill="#FFFFFF" stroke="none"/><circle cx="15.5" cy="12" r="1.5" fill="#FFFFFF" stroke="none"/><circle cx="15.5" cy="18.5" r="1.5" fill="#FFFFFF" stroke="none"/>"##
        case .chevronLeft:
            return ##"<path d="m15 18-6-6 6-6"/>"##
        case .globe:
            return ##"<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>"##
        case .sun:
            return ##"<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>"##
        case .moon:
            return ##"<path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"/>"##
        case .monitor:
            return ##"<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>"##
        case .coins:
            return ##"<path d="M13.744 17.736a6 6 0 1 1-7.48-7.48"/><path d="M15 6h1v4"/><path d="m6.134 14.768.866-.5 2 3.464"/><circle cx="16" cy="8" r="6"/>"##
        case .hash:
            return ##"<line x1="4" x2="20" y1="9" y2="9"/><line x1="4" x2="20" y1="15" y2="15"/><line x1="10" x2="8" y1="3" y2="21"/><line x1="16" x2="14" y1="3" y2="21"/>"##
        case .calendar:
            return ##"<path d="M8 2v4"/><path d="M16 2v4"/><rect width="18" height="18" x="3" y="4" rx="2"/><path d="M3 10h18"/>"##
        case .network:
            return ##"<rect width="6" height="6" x="16" y="16" rx="1"/><rect width="6" height="6" x="2" y="16" rx="1"/><rect width="6" height="6" x="9" y="2" rx="1"/><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"/><path d="M12 12V8"/>"##
        case .server:
            return ##"<rect width="20" height="8" x="2" y="2" rx="2"/><rect width="20" height="8" x="2" y="14" rx="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/>"##
        case .hardDrive:
            return ##"<path d="M10 16h.01"/><path d="M2.212 11.577a2 2 0 0 0-.212.896V18a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-5.527a2 2 0 0 0-.212-.896L18.55 5.11A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><path d="M21.946 12.013H2.054"/><path d="M6 16h.01"/>"##
        case .info:
            return ##"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>"##
        case .logOut:
            return ##"<path d="m16 17 5-5-5-5"/><path d="M21 12H9"/><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>"##
        case .messageSquareText:
            return ##"<path d="M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z"/><path d="M7 11h10"/><path d="M7 15h6"/><path d="M7 7h8"/>"##
        }
    }
}

/// Point sizes for the wallet glyphs — mirrors the retired `WalletIconFont`
/// recipes so swapping renderers didn't move any layout.
enum LucideIconSize {
    static let tab: CGFloat = 22
    static let action: CGFloat = 20
    static let rowGlyph: CGFloat = 15
    static let nameChevron: CGFloat = 12
    static let smallChevron: CGFloat = 11
    static let statusIcon: CGFloat = 13
    static let eye: CGFloat = 17
    static let empty: CGFloat = 28
    static let sheetSearch: CGFloat = 17
    static let checkmark: CGFloat = 15
    // Contacts slots (spec 018).
    static let menuRow: CGFloat = 18
    static let groupTileGlyph: CGFloat = 20
    static let ghostPlus: CGFloat = 16
    static let chipPlus: CGFloat = 12
    static let addressCopy: CGFloat = 18
    // Wallet-flow slots (spec 021).
    static let flowBack: CGFloat = 20
    static let flowRowAction: CGFloat = 18
    static let flowStatus: CGFloat = 26
    static let flowScanTool: CGFloat = 18
    // Explore + signing slots (spec 022). The four names its screens read and
    // the corpus never carried; sized off the same mocks the Android and web
    // clients measured — the browser bar's back/forward/star, the https mark
    // beside an address, a disclosure chevron, the slide-to-confirm arrow and
    // the favourites tile's glyph.
    static let browserBarGlyph: CGFloat = 20
    static let addressLock: CGFloat = 12
    static let disclosure: CGFloat = 14
    static let slideArrow: CGFloat = 20
    static let tileGlyph: CGFloat = 24
}

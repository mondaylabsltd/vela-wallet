//
//  SiteLockMark.swift
//  VelaWallet
//
//  The one mark beside a site's name — in the address bar, the site menu,
//  the connection panel and the sheet a site asks to connect on.
//
//  A padlock says one thing to a person: the connection is encrypted. It was
//  drawn from "may the wallet be used from this page?", which is also true
//  of plain http on this device — so the phones put a padlock beside
//  `127.0.0.1:8137` (device walk 2026-10-11). The mark is the core's
//  `OriginLock` now, and this is the only view that draws one:
//
//  - `secure` (https): the padlock, quiet. The ONLY padlock. It says the
//    line is encrypted, not that the site is honest, so it is decorative to
//    a screen reader (the host is read out).
//  - `local` (http on loopback or the person's own network): a quiet "i" —
//    the mark browsers give `localhost` — named "Local page — not
//    encrypted" for a screen reader. Never a padlock, never the warning.
//  - `insecure` (public http): the open lock in the warning colour, named.
//
//  No word beside any of them, as spec 079 ruled for the lock.
//

import SwiftUI

struct SiteLockMark: View {
    @Environment(\.theme) private var theme

    let lock: OriginLock
    /// `connect.browser.a11yLocal`.
    var localLabel = ""
    /// `connect.browser.a11yInsecure`.
    var insecureLabel = ""
    /// The surface's test hook: "explore", "explore.menu", "explore.connection".
    var idPrefix = "explore"

    var body: some View {
        switch lock {
        case .secure:
            LucideIcon(.lock, size: LucideIconSize.addressLock)
                .foregroundStyle(theme.fgMuted)
                .accessibilityHidden(true)
                .accessibilityIdentifier(Self.id(idPrefix, lock))
        case .local:
            LucideIcon(.info, size: LucideIconSize.addressLock)
                .foregroundStyle(theme.fgMuted)
                .accessibilityLabel(localLabel)
                .accessibilityHidden(localLabel.isEmpty)
                .accessibilityIdentifier(Self.id(idPrefix, lock))
        case .insecure:
            LucideIcon(.lockOpen, size: LucideIconSize.addressLock)
                .foregroundStyle(theme.warningBase)
                .accessibilityLabel(insecureLabel)
                .accessibilityHidden(insecureLabel.isEmpty)
                .accessibilityIdentifier(Self.id(idPrefix, lock))
        }
    }

    /// The glyph each mark draws — one place, so a test can say which.
    static func glyph(_ lock: OriginLock) -> LucideGlyph {
        switch lock {
        case .secure: .lock
        case .local: .info
        case .insecure: .lockOpen
        }
    }

    /// "explore.lock" / "explore.local" / "explore.insecure".
    static func id(_ prefix: String, _ lock: OriginLock) -> String {
        switch lock {
        case .secure: "\(prefix).lock"
        case .local: "\(prefix).local"
        case .insecure: "\(prefix).insecure"
        }
    }
}

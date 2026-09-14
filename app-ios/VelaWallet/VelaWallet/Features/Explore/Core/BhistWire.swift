//
//  BhistWire.swift
//  VelaWallet
//
//  The `browser_history` machine's view model, in Swift.
//
//  ## This machine has no `ready` flag, and that is a trap
//
//  `explore_sites` publishes one; this does not. A visit dispatched before
//  `read_history` has answered is **dropped, silently** — the core's
//  fail-closed choice, because recording into a mirror that is about to be
//  replaced would lose it anyway.
//
//  So the executor signals when the load lands and the controller records
//  nothing before it. Android found this in its phase 0 and it is a property
//  of the core, not of Android.
//

import Foundation

/// One visited site.
struct BhistEntryWire: Decodable, Equatable, Identifiable {
    /// `scheme://host` — the dedupe key.
    let origin: String
    /// The full last-visited URL, verbatim: revisiting reopens where the
    /// person left off, not the site's front door.
    let url: String
    let host: String
    let title: String
    /// `""` when none resolved — an empty string, not an absent field. The TS
    /// shape, ported verbatim, because the other clients read this document.
    let favicon: String
    let lastVisitedMs: Double

    var id: String { origin }
}

struct BhistViewWire: Decodable, Equatable {
    /// Sorted by recency, which can differ from stored order if a shell's
    /// clock ever ran backwards. Recency wins on screen.
    let entries: [BhistEntryWire]

    static let empty = BhistViewWire(entries: [])
}

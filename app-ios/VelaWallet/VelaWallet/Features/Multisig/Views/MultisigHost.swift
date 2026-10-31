//
//  MultisigHost.swift
//  VelaWallet
//
//  The multisig side of the app (the web's `MultisigHost.svelte`): whichever
//  multisig sheet is open — create, add, a link where it stands, the invite,
//  settings, members, a transaction, the review before a proposal — and over
//  it the two sheets a signature may wait on: the high-risk question and an
//  outside member's wallet. ONE sheet at a time from the root; the two
//  overlays are presented by the sheet that is up, so they are never a
//  second sheet racing the first (the nesting bug iOS has with siblings).
//

import SwiftUI

struct MultisigHostModifier: ViewModifier {
    let hub: MultisigHub
    let words: MsWords
    let accounts: [(address: String, name: String)]
    let contacts: [(address: String, name: String)]
    let spaceNetworks: [Int]
    let scheme: ColorScheme
    var onAddNetwork: (Int) -> Void
    var onFund: () -> Void

    func body(content: Content) -> some View {
        content
            .sheet(item: Binding(get: { hub.sheet }, set: { hub.sheet = $0 })) { sheet in
                sheetView(sheet)
                    .modifier(MultisigOverlays(hub: hub, words: words, scheme: scheme))
                    .themed(scheme)
            }
            // With no multisig sheet up, the overlays still have a place.
            .modifier(MultisigOverlays(hub: hub, words: words, scheme: scheme, onlyWithoutSheet: true))
    }

    @ViewBuilder private func sheetView(_ sheet: MultisigSheet) -> some View {
        switch sheet {
        case .create:
            MultisigCreateSheet(
                hub: hub, words: words, accounts: accounts, contacts: contacts, spaceNetworks: spaceNetworks,
                onClose: { hub.sheet = nil },
                onCreated: { _ in hub.sheet = nil },
                onInvite: { address in hub.sheet = .invite(address: address) }
            )
        case .add(let prefill):
            MultisigAddSheet(
                hub: hub, words: words, invite: nil, prefill: prefill,
                onClose: { hub.sheet = nil },
                onAdded: { _ in hub.sheet = nil },
                onAddNetwork: onAddNetwork
            )
        case .link:
            if hub.accountsView.link?.kind == "invite" {
                MultisigAddSheet(
                    hub: hub, words: words, invite: hub.accountsView.link, prefill: nil,
                    onClose: {
                        hub.send(["type": "close_link"])
                        hub.sheet = nil
                    },
                    onAdded: { _ in
                        hub.send(["type": "close_link"])
                        hub.sheet = nil
                    },
                    onAddNetwork: onAddNetwork
                )
            } else {
                MultisigLinkStandSheet(hub: hub, words: words, onClose: { hub.sheet = nil }, onAddNetwork: onAddNetwork)
            }
        case .invite(let address):
            MultisigInviteSheet(hub: hub, words: words, address: address, onClose: { hub.sheet = nil })
        case .settings(let address):
            MultisigSettingsSheet(
                hub: hub, words: words, address: address,
                onClose: { hub.sheet = nil },
                onOpen: { hub.sheet = $0 },
                onRemoved: { hub.sheet = nil }
            )
        case .members(let address):
            MultisigMembersSheet(hub: hub, words: words, address: address, onClose: { hub.sheet = .settings(address: address) })
        case .proposal(let chainId, let hash):
            MultisigProposalSheet(
                hub: hub, chainId: chainId, safeTxHash: hash, words: words,
                onClose: { hub.sheet = nil }, onAddNetwork: onAddNetwork
            )
        case .review(let chainId):
            MultisigDraftReviewSheet(hub: hub, chainId: chainId, words: words)
        }
    }
}

/// The two sheets a signature may wait on, over whatever is up.
struct MultisigOverlays: ViewModifier {
    let hub: MultisigHub
    let words: MsWords
    let scheme: ColorScheme
    var onlyWithoutSheet = false

    func body(content: Content) -> some View {
        let active = !onlyWithoutSheet || hub.sheet == nil
        content
            .sheet(item: Binding(
                get: { active ? hub.riskAsk : nil },
                set: { if $0 == nil { hub.riskAsk?.answer(false) } }
            )) { ask in
                MultisigRiskSheet(ask: ask, words: words).themed(scheme)
            }
            .sheet(item: Binding(
                get: { active && hub.riskAsk == nil ? hub.outsideSession : nil },
                set: { if $0 == nil { hub.outsideSession?.close(); hub.outsideSession = nil } }
            )) { session in
                MultisigOutsideSheet(session: session, words: words, onClose: { hub.outsideSession = nil }).themed(scheme)
            }
    }
}

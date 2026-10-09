//
//  SigningLive.swift
//  VelaWallet
//
//  The signing sheet, from the four machines' views.
//
//  The drawn model keeps only its labels. The dApp is the **host** — guessing
//  a pretty name from a domain is exactly the counterfeit route — the blocks
//  are the core's reading, the fee is the fee policy's, and the confirm opens
//  only when the core's one gate says it may (`signConfirmState`, spec 099
//  R7), with its line under it when it does not.
//
//  Ported from `app-android/.../feature/signing/SigningLive.kt` (spec 044
//  T033), which is the desktop's `signing/live.rs`.
//

import SwiftUI
import VelaCore

enum SigningLive {

    struct Context {
        let loc: Loc
        let chainName: String
        let chainDot: Color
        let nativeSymbol: String
        let walletName: String
        let walletAddress: String
        /// The REQUEST's chain (`RootView.signingContext`), for the fee coins'
        /// marks — never the estimate's, which is absent while a quote is out,
        /// and never 0. `nil` only in hand-built contexts, where the coins
        /// draw their glyph.
        var chainId: Int? = nil
        /// The display currency the fee's "≈" half is written in (issue 201).
        var display: WalletLive.Display = .usd
        /// The page's host, for the sign-in verdict's words.
        var origin: String?
        /// What the chain said this transaction would do (spec 055). `nil`
        /// when nothing has been simulated for this account yet.
        var sim: TrustSimViewWire?
        /// Where the simulation has got to. Three states, three sentences.
        var simulation: SigningController.Simulation = .pending
        /// Whether the fee row's coin list is open (issue #262).
        var feeOpen = false
        /// How the Trusted Signer last ended for this request without signing.
        var trustedSignerNotice: TrustedSignerNotice?
        /// Spec 079: the chain's explorer base, for the landed receipt's link.
        var explorerBase: String?
        /// Spec 079: the tracker's entry for the submitted operation — its
        /// clock and its outcome.
        var track: TrackEntryWire?
        /// Spec 079: the chain's usual inclusion time (the core's table,
        /// `networkTypicalInclusionS`), for the receipt's ring.
        var typicalS: Int?
        /// Spec 079: this account signs on a trusted page — the page's own
        /// slide is the one consent.
        var trustedSignerRoute = false
        /// Spec 102 D4: that page (the account's venue), the place its key
        /// lives, and this phone's integrity line for the page — the hand-off
        /// card. `nil` page: the account signs in Vela.
        var handoffPage: String?
        var handoffPlace: KeyMethod?
        var handoffLine: SignerIntegrityLine?
        /// Spec 082 RF5: the quote could not even start (the account's
        /// deployment could not be read) — the core's failure name for it,
        /// drawn as a failed quote is.
        var feeStartFailure: String?
        /// The clock the landing's pace is read at (spec 099 R6); the screen
        /// counts the seconds itself once a countdown runs.
        var nowMs: Double = Date().timeIntervalSince1970 * 1000
        /// The wallet's networks, the person's own included — what names the
        /// native coin a fee figure is in.
        var networks: WalletNetworks = .builtin

        /// The Trusted Signer's waiting card is up: the account signs on the
        /// page and the signature is under way. The card speaks for the
        /// signature then — "签名中…" beside "签名页没能打开" contradicted it.
        func signerPageOpen(_ sign: SignViewWire) -> Bool {
            // The page is up exactly while the core says the signature is
            // awaited (spec 082 RA9) — not through the network work before.
            trustedSignerRoute && sign.phase == .awaitingSignature
        }
    }

    /// The fee list's id for the chain's own coin (the web's `'native'`).
    static let nativeFeeId = "native"

    private static func s(_ loc: Loc, _ key: String, _ vars: [String: String] = [:]) -> String {
        vars.isEmpty ? loc.t("componentsUi.signing.\(key)")
                     : loc.t("componentsUi.signing.\(key)", vars: vars)
    }

    private static func a(_ loc: Loc, _ key: String, _ vars: [String: String] = [:]) -> String {
        vars.isEmpty ? loc.t("componentsUi.signingApprove.\(key)")
                     : loc.t("componentsUi.signingApprove.\(key)", vars: vars)
    }

    /// The transport of a request the WALLET made of itself (`RootView`).
    /// Nothing listens on it: an answer addressed here never reaches a page.
    static let walletTransport = "wallet"

    /// Where a site's icon conventionally lives, best first. Https only: never
    /// over plain http, where anybody on the path could answer with somebody
    /// else's brand.
    static func siteIconUrls(origin: String) -> [String] {
        guard origin.hasPrefix("https://"),
              let host = URL(string: origin)?.host, !host.isEmpty
        else { return [] }
        let port = URL(string: origin)?.port.map { ":\($0)" } ?? ""
        let base = "https://\(host)\(port)"
        return ["\(base)/apple-touch-icon.png", "\(base)/favicon.ico"]
    }

    /// The Trusted Signer's ending, when it left the request unsigned. Closed is
    /// a person's own decision, told calmly; a refusal or a mismatch is not.
    static func trustedSignerBlocks(_ notice: TrustedSignerNotice?, loc: Loc) -> [SigningBlock] {
        guard let notice else { return [] }
        return [.warning(tone: notice.calm ? .caution : .danger, text: notice.text(loc))]
    }

    /// The core's words in the reader's language. A clear-signing result is
    /// English — a descriptor's intent and labels, the "Unlimited" a threshold
    /// prints — and the core names the ones it recognises (`intentTerm`,
    /// `labelTerm`, `valueTerm`: the key leaf under `componentsUi.signing`).
    /// Each named word is swapped for this locale's; anything unnamed stays as
    /// the descriptor wrote it. Same rule in every shell; runs before
    /// `cappedApproval`. The confirm is left alone: `confirmLabel` switches on
    /// its English intent and falls back to the term.
    ///
    /// The wallet's own key backup is no exception: the core names every word
    /// on it (the intent, Network, Address, Public keys), so nothing here
    /// matches a contract or relabels rows by position.
    static func localizedTerms(_ clear: ClearSigningViewWire, loc: Loc) -> ClearSigningViewWire {
        func word(_ term: String?, _ text: String) -> String {
            guard let term else { return text }
            let key = "componentsUi.signing.\(term)"
            let translated = loc.t(key)
            return translated == key || translated.isEmpty ? text : translated
        }
        func localize(_ result: ClearSignResultWire) -> ClearSignResultWire {
            var result = result
            result.intent = word(result.intentTerm, result.intent)
            for index in result.fields.indices {
                result.fields[index].label = word(result.fields[index].labelTerm, result.fields[index].label)
                result.fields[index].value = word(result.fields[index].valueTerm, result.fields[index].value)
            }
            return result
        }
        var next = clear
        next.result = clear.result.map(localize)
        // 089 S1: every call of a batch, in the words a lone call gets.
        if var batch = clear.batch {
            for index in batch.calls.indices {
                batch.calls[index].result = batch.calls[index].result.map(localize)
            }
            next.batch = batch
        }
        return next
    }

    /// The cap the person chose, where the decode still says "Unlimited".
    ///
    /// The clear-signing result describes the REQUEST; once the guard holds a
    /// finite choice for an unlimited approve (a cap, or revoke), "Unlimited"
    /// in red would describe bytes that are no longer the ones being signed.
    static func cappedApproval(_ clear: ClearSigningViewWire, guard guardView: GuardViewWire) -> ClearSigningViewWire {
        // 089 S1: each call's "Unlimited" is replaced by ITS OWN leg's cap —
        // the guard's legs are the calls, in order.
        if var batch = clear.batch {
            for index in batch.calls.indices {
                guard let result = batch.calls[index].result,
                      let cap = capText(guardView, leg: index) else { continue }
                let shown = result.capped(to: cap)
                batch.calls[index].result = shown
                // Capped, the call is what its decode now says — unless it burns.
                if batch.calls[index].risk == .danger, shown.risk != .danger, !shown.toOwnToken {
                    batch.calls[index].risk = shown.risk
                }
            }
            batch.risk = batch.calls.map(\.risk).max { $0.rank < $1.rank } ?? batch.risk
            var next = clear
            next.batch = batch
            return next
        }
        guard let result = clear.result, let cap = capText(guardView, leg: 0) else { return clear }
        var next = clear
        next.result = result.capped(to: cap)
        return next
    }

    /// The guard's finite choice on an unlimited request, as the cap row prints
    /// it — the single approval's, or batch leg `leg`'s: each call of a batch
    /// carries its own decode, so each "Unlimited" takes its own leg's cap.
    private static func capText(_ guardView: GuardViewWire, leg legIndex: Int) -> String? {
        let detected: GuardDetectedApprovalWire?
        let editor: GuardEditorViewWire?
        let meta: GuardTokenMetaViewWire
        switch guardView.surface {
        case .approvalEditor:
            (detected, editor, meta) = (guardView.detected, guardView.editor, guardView.meta)
        case .batch:
            guard let legs = guardView.batch?.legs, legs.indices.contains(legIndex) else { return nil }
            let leg = legs[legIndex]
            (detected, editor, meta) = (leg.approval, leg.editor, leg.meta)
        default:
            return nil
        }
        guard detected?.isUnbounded == true, let editor, let raw = editor.displayAmountRaw else { return nil }
        switch editor.choice {
        case .amount, .revoke:
            return "\(SendLive.fromBase(raw, decimals: meta.decimals)) \(meta.symbol)"
                .trimmingCharacters(in: .whitespaces)
        default:
            return nil
        }
    }

    static func model(
        fallback: SigningModel,
        request: SigningController.Incoming,
        sign: SignViewWire,
        clear rawClear: ClearSigningViewWire,
        guard guardView: GuardViewWire,
        fee: FeeViewWire?,
        context: Context,
        /// The sheet's speed control (spec 069); `nil` draws the fee alone.
        speed: SendLive.SpeedInputs? = nil,
        /// The core's gate over these views (`SigningController.confirmState`).
        /// `nil` — nobody asked the core — keeps the confirm shut.
        gate: SignConfirmStateWire? = nil
    ) -> SigningModel {
        let loc = context.loc
        let host = BrowserEngine.hostOf(origin: request.origin)
        // The wallet asking itself (the key backup) — the core's word, which
        // the shell said once, where it raised the request; never read off
        // the reading, which a site can submit byte for byte.
        let own = sign.request?.firstParty ?? request.firstParty
        let clear = cappedApproval(localizedTerms(rawClear, loc: loc), guard: guardView)
        let facts = SigningController.firstCall(paramsJson: request.paramsJson, method: request.method)
        // 089 S1: a batch's technical details are the whole batch, never call 1's calldata.
        let wholeBatch = clear.surface == .batch
        let dataBytes = (facts?.data.map { $0.hasPrefix("0x") ? $0.dropFirst(2) : $0[...] }?.count ?? 0) / 2

        // Spec 081: a refused request gets the refusal and nothing else. The
        // decoded body, the balances and the guard all describe a transaction
        // that will never be signed, and reading them invites the question
        // "so why can't I?" — which the sentence above already answers.
        //
        // The Trusted Signer's notice stays either way (071): it says why the
        // last attempt on the page signed nothing, which is the one thing a
        // person does need on a refused card as much as on a live one.
        let refused = sign.blocked != nil
        // Only a TRANSACTION has balances to change. A message moves nothing,
        // and a balance block on a signature would answer a question nobody
        // asked.
        let balances = balanceBlocks(isTransaction: facts != nil, context: context)
        // Issue #314: on the wallet's own request a simulation that moves
        // nothing only confirms what the wallet itself wrote — a technical
        // fact, folded with the others, not a bordered card weighing as much as
        // the outcome. Anything else it has to say (a revert, a node that could
        // not check, a balance that would move) stays on the sheet.
        let quietSim: SigningRow? = {
            guard own, !refused, balances.count == 1,
                  case .balances(_, let rows, let note, _) = balances[0], rows.isEmpty
            else { return nil }
            return SigningRow(label: s(loc, "simResultLabel"), value: note ?? s(loc, "simResultNoChange"))
        }()
        let blocks = refused
            ? statusBlocks(sign: sign, loc: loc)
                + trustedSignerBlocks(context.trustedSignerNotice, loc: loc)
            : statusBlocks(sign: sign, loc: loc, signerPageOpen: context.signerPageOpen(sign))
                + trustedSignerBlocks(context.trustedSignerNotice, loc: loc)
                + self.blocks(clear: clear, to: facts?.to, valueHex: facts?.value,
                              dataBytes: dataBytes, context: context)
                + (quietSim == nil ? balances : [])
                + guardBlocks(guardView, loc: loc)

        // Spec 102 D4: an account whose venue is a page gets the hand-off card,
        // and its Open is shut until this phone's check admitted the page —
        // the core's answer, as the gate's is.
        let handoff: HandoffCardModel? = refused ? nil : context.handoffPage.map { page in
            HandoffCardModel.build(
                page: page,
                keyLabel: context.handoffPlace.map { TrustedSigner.keyLabel($0, loc: loc) },
                line: context.handoffLine ?? SignerPageChecks.checking,
                loc: loc
            )
        }
        var model = SigningModel(
            id: fallback.id,
            // The HOST, twice. A name a page supplies is a claim, and a
            // signing sheet that leads with the claim is a sheet somebody can
            // dress up as a bank.
            dapp: (name: own ? "Vela Wallet" : (host.isEmpty ? request.origin : host),
                   host: own ? "" : (host.isEmpty ? request.origin : host),
                   letter: ExploreLive.site(host: host, name: host, origin: request.origin).letter,
                   tint: ExploreLive.tint(for: host)),
            network: (name: context.chainName, dot: context.chainDot),
            blocks: blocks,
            tech: TechModel(
                title: fallback.tech.title,
                // "· Vela passkey registry" names the wallet's own contract
                // to the wallet's own person: dropped on its own request.
                summary: refused || own ? nil : clear.result?.contractName,
                fn: refused
                    ? nil
                    : clear.result.map { (label: s(loc, "techFunction"), signature: $0.intent) },
                params: [],
                identities: [],
                simResult: quietSim,
                raw: refused
                    ? nil
                    : wholeBatch
                        ? (label: s(loc, "techRawData"), hex: request.paramsJson)
                        : dataBytes > 0
                            ? facts?.data.map { (label: s(loc, "techRawData"), hex: $0) } ?? nil
                            : nil,
                copyLabel: fallback.tech.copyLabel,
                explorerLabel: fallback.tech.explorerLabel
            ),
            techOpen: false,
            fee: refused
                ? nil
                : feeModel(clear: clear, fee: fee, context: context, speedTier: speed?.view.tier),
            signer: (label: s(loc, "signingAccount"),
                     name: context.walletName,
                     seed: context.walletAddress),
            confirm: refused
                ? nil
                : (action: confirmLabel(clear: clear, loc: loc),
                   enabled: (gate ?? .shut).enabled && (handoff?.opens ?? true)),
            panelTitle: s(loc, "signatureRequest")
        )
        // Spec 079: the ✕, and — once approved — the send receipt in place of
        // the form. A refused request never gets that far.
        model.closeLabel = loc.t("onboarding.common.close")
        model.confirmAsButton = !refused && context.trustedSignerRoute
        model.confirmButtonLabel = s(loc, "openSigner")
        if let handoff {
            model.handoff = handoff
            // Minimal context only: where the request stands, how the page
            // last ended, and every warning — never the preview again.
            model.handoffBlocks = blocks.filter {
                if case .warning = $0 { return true }
                return false
            }
        }
        model.receipt = refused ? nil : receipt(sign: sign, blocks: blocks, context: context)
        // Spec 099 R7: a shut confirm says which part of the gate is shut —
        // the core's line for it — never a dead control with no reason.
        if !refused, let gate, !gate.enabled, let key = gate.key {
            model.confirmBlockLine = loc.t(key)
        }
        // The wallet's own request (the key backup) is not a site: no
        // requester header at all — "getvela.app" under a letter read as a
        // stranger, and "Vela Wallet" over the wallet's own sheet said
        // nothing. Its headline and the ✕ take the row (`SigningModel.headline`).
        model.dappOwn = own
        model.dappIconUrls = own ? [] : siteIconUrls(origin: request.origin)
        model.networkLogoUrl = Marks.chainLogoURL(request.chainId)
        if !isOffChain(clear) {
            model.feeSpeed = speed.map {
                SendLive.speedModel($0, view: nil, display: context.display, loc: loc,
                                    networks: context.networks)
            }
        }
        if !refused {
            model.feeRefresh = feeRefresh(clear: clear, fee: fee, loc: loc)
            model.feeChevron = (fee?.options.count ?? 0) > 1
        }
        return model
    }

    // MARK: - What the fee row draws

    // The GATE is the core's (`SignConfirmStateWire`, spec 099 R7): this file
    // no longer decides whether the confirm arms. The two readings below only
    // pick what the fee row DRAWS — the "no network fee" line for a message,
    // and no figure under a speed it was not priced at — and the core's gate
    // applies the same two rules (`sign_confirm::off_chain`,
    // `fee_of_another_tier`), which are not exported on their own.

    /// NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681).
    static func feeOfAnotherTier(_ fee: FeeViewWire?, speedTier: String?) -> Bool {
        guard let estimate = fee?.fee, let speedTier else { return false }
        return SendLive.offered(estimate.tier) != SendLive.offered(speedTier)
    }

    static func isOffChain(_ clear: ClearSigningViewWire) -> Bool {
        clear.result?.signType == .signature
            || clear.surface == .messageSign
            || clear.surface == .ethSign
            || clear.surface == .blindTypedData
    }

    // MARK: - Status

    /// `signerPageOpen`: the Trusted Signer's waiting card speaks for the
    /// signature (spec 079 — "签名中…" above "签名页没能打开" contradicted it).
    static func statusBlocks(sign: SignViewWire, loc: Loc, signerPageOpen: Bool = false) -> [SigningBlock] {
        var blocks: [SigningBlock] = []

        // Spec 081: the core refused this request outright — it would have
        // changed who controls the account. Nothing else on the sheet matters,
        // and `confirmGateOpen` is already false, so say it and stop.
        if let blocked = sign.blocked {
            blocks.append(.intent(text: s(loc, "selfCallBlockedTitle"), tone: .danger))
            let text: String
            if blocked.function == "SafeTx" {
                text = s(loc, "selfCallBlockedSafeTx")
            } else if let leg = blocked.legIndex {
                text = s(loc, "selfCallBlockedLegBody", [
                    "index": String(leg), "function": blocked.function,
                ])
            } else {
                text = s(loc, "selfCallBlockedBody", ["function": blocked.function])
            }
            blocks.append(.warning(tone: .danger, text: text))
            return blocks
        }

        // Founder, 2026-09-22: this surface means THE RELAY HAS NO GAS ON THIS
        // CHAIN. The `componentsUi.funding.*` line it used to show — "your
        // transactions run on a small fee reserve… later transactions top it
        // back up" — describes the retired per-wallet deposit, and was false
        // about whose money this is. The second line is the one the surface
        // never had: it is non-refundable and it goes to the bundler operator,
        // not to Vela.
        if sign.funding != nil {
            blocks.append(.warning(tone: .caution, text: loc.t("componentsUi.treasuryBootstrap.lead")))
            blocks.append(
                .warning(tone: .caution, text: loc.t("componentsUi.treasuryBootstrap.disclaimer"))
            )
        }
        if let error = sign.error {
            let text: String = switch error.kind {
            // `.unlimitedApproval` falls to the plain sentence: since
            // 2026-09-26 it means the approval screen did not show the
            // unlimited approval — a wallet fault, not "unlimited is disabled".
            // The blocked sheet above already says it, in full.
            case .selfCallBlocked: ""
            case .unsupportedChain: loc.t("send.lock.netNotFound")
            // Neither of these is an error a person needs to read: one is
            // their own decision and the other is the wallet's.
            case .userRejected, .walletSwitchedChains: ""
            // Spec 099 R8: the passkey failed — the signer layer's own line.
            case .signerUnavailable, .signerNotDiscoverable, .signerFailed:
                error.kind.signerReasonKey.map { loc.t($0) } ?? loc.t("send.txErrorGeneric")
            // The relay refused it (spec 082 RJ3): nothing was sent, and
            // "try again" would send the same refusal.
            default: sign.failureRefused ? s(loc, "refused") : loc.t("send.txErrorGeneric")
            }
            if !text.isEmpty { blocks.append(.warning(tone: .danger, text: text)) }
        }
        if sign.pendingOpHash != nil {
            blocks.append(.positive(s(loc, "submitted")))
        } else if signerPageOpen {
            // The waiting card says it.
        } else if let words = phaseWords(sign, loc: loc) {
            blocks.append(.sentence(text: words.title, tone: .neutral))
        }
        return blocks
    }

    /// What the sheet says while a request is in flight — from the core's
    /// `phase` (spec 082 RA9, G22), never from `is_signing`: the network work
    /// before the prompt is "preparing", and "awaiting your signature" is
    /// said only while the prompt is up. A message never goes to the network,
    /// so it is never "submitting" (079). `nil` when nothing is in flight.
    static func phaseWords(_ sign: SignViewWire, loc: Loc) -> (title: String, hint: String?)? {
        let onChain = sign.request.map { $0.kind == .transaction || $0.kind == .batch } ?? true
        switch sign.phase {
        case .idle:
            return nil
        case .preparing:
            return (loc.t("send.txPreparing"), nil)
        case .awaitingSignature:
            return (onChain ? loc.t("send.txSigning") : s(loc, "signing"), nil)
        case .submitting:
            return onChain
                ? (loc.t("send.txSubmitting"), loc.t("send.txBackgroundHint"))
                : (s(loc, "signing"), nil)
        }
    }

    // MARK: - After the approval (spec 079)

    /// After the approval the sheet stops being a form — the owner saw nothing
    /// change after the fingerprint ("可信签名器签完后，回到签名提示框，似乎没有
    /// 任何提示"). This is the send receipt's own model and words, so a dApp
    /// transaction lands exactly as a send does: signing → submitting →
    /// submitted with the chain's clock → (the core answers the page, and the
    /// aftercare shows the ending). `nil` while the request is still a request.
    static func receipt(sign: SignViewWire, blocks: [SigningBlock], context: Context) -> SendReceiptModel? {
        let loc = context.loc
        // The Trusted Signer's waiting card is the status while its page holds
        // the signature — "等待生物识别…" or "签名中…" under it would say
        // something else (spec 079; Android draws no receipt under the card).
        if context.signerPageOpen(sign) { return nil }
        let summary = summaryOf(blocks)
        let header = FlowHeaderModel(title: "", backLabel: "")
        let closeBackground = loc.t("send.txCloseBackground")
        // A message never goes to the network: it is signing, then signed —
        // never "submitting" (device-found on the Xiaomi, spec 079).
        let onChain = sign.request.map { $0.kind == .transaction || $0.kind == .batch } ?? true
        if !onChain, let words = phaseWords(sign, loc: loc) {
            return SendReceiptModel(
                header: header, stage: .submitting, title: words.title,
                captions: [summary].compactMap { $0 },
                cta: loc.t("onboarding.common.close"), ctaAccent: false
            )
        }
        // A refusal after the approval (the submission failed): the core's
        // reason, the sheet's own sentence for it. After spec 082 this is a
        // TRUE "not sent": a lost reply never reaches here (RA1). A relay's
        // refusal says so and never "try again" (RJ3, `failure_refused`).
        // Spec 099 R8: a passkey that failed after the approval is the same
        // ending — said in the signer's words, and tried again when the core
        // says a retry can help (`failure_retryable`).
        if let error = sign.error, error.kind != .userRejected,
           sign.pendingOpHash != nil || error.kind == .submitFailed || error.kind.signerReasonKey != nil {
            // Spec 096 F8: the core holds the page's answer until this closes;
            // a failure that sent nothing may be tried again.
            let reason = error.kind.signerReasonKey.map { loc.t($0) }
                ?? (sign.failureRefused ? s(loc, "refused") : loc.t("send.txErrorGeneric"))
            return SendReceiptModel(
                header: header, stage: .failed,
                title: loc.t("componentsTx.receipt.statusFailed"),
                captions: [summary, reason].compactMap { $0 },
                cta: loc.t("componentsTx.receipt.done"), ctaAccent: !sign.failureRetryable,
                retry: sign.failureRetryable ? loc.t("send.txRetryBtn") : nil
            )
        }
        if let op = sign.pendingOpHash {
            // The reply was lost (RA10): it may have been sent, Vela keeps
            // checking, don't send it again — the op hash to look it up by,
            // and no Retry. Once the relay has shown it holds the op, the
            // ordinary words below take over.
            let track = context.track.flatMap {
                $0.userOpHash.caseInsensitiveCompare(op) == .orderedSame ? $0 : nil
            }
            // The tracker has its verdict while the page's answer is still
            // waiting out its window — the relay said twice it never had the
            // op, or the chain's own event was found while the relay stayed
            // mute. The sheet says that verdict, in the core's words for the
            // ending the page is about to get (`signEndingOf` of the op hash,
            // `signEndingState` with the entry): a tick, a revert with its
            // hash, or "not sent" — never "submitted, waiting" with a clock
            // over an op that is settled (082 review).
            if let track, track.outcome == "final",
               let settled = SigningAftercare.of(
                   method: sign.request?.method ?? "eth_sendTransaction",
                   chainId: sign.request?.chainId ?? track.chainId,
                   payload: ["type": "ok", "result": op], submittedUserOp: op
               ) {
                return aftercareReceipt(settled, summary: summary, context: context)
            }
            if sign.pendingOpMaybeSent, track == nil || track?.outcome == "maybe_sent" {
                return maybeSentReceipt(op: op, summary: summary, header: header, loc: loc)
            }
            let still = track?.outcome == "still_confirming"
            let typicalLine = context.typicalS.map {
                loc.t("send.txTypicalTime", vars: ["chainName": context.chainName, "estSecs": String($0)])
            }
            // Spec 099 R6: the core's one countdown, from when the relay put
            // it on the network; before that the relay is sending it, and no
            // chain's clock runs over the relay's own queue.
            let pace = LandingPaceWire.of(
                sentAtMs: track?.relaySentAtMs, typicalS: context.typicalS, nowMs: context.nowMs
            )
            let eta = still ? nil : ReceiptEtaModel.counting(
                sentAtMs: track?.relaySentAtMs, typicalS: context.typicalS, typicalLine: typicalLine,
                loc: loc, nowMs: context.nowMs
            )
            return SendReceiptModel(
                header: header, stage: .submitted,
                title: loc.t("send.txSubmittedTitle"),
                captions: [
                    summary,
                    still ? s(loc, "stillConfirming")
                        : pace.waiting ? loc.t("send.txRelaySending") : loc.t("send.txWaitingConfirm"),
                    eta == nil && !still && !pace.waiting ? typicalLine : nil,
                ].compactMap { $0 },
                cta: closeBackground, ctaAccent: false, eta: eta
            )
        }
        // In flight, in the core's own phases (RA9): preparing, then — only
        // while the prompt is up — awaiting the signature, then submitting.
        guard let words = phaseWords(sign, loc: loc) else { return nil }
        switch sign.phase {
        case .submitting:
            return SendReceiptModel(
                header: header, stage: .submitting, title: words.title,
                captions: [summary, words.hint].compactMap { $0 },
                cta: closeBackground, ctaAccent: false
            )
        default:
            return SendReceiptModel(
                header: header, stage: .submitting, title: words.title,
                captions: [summary].compactMap { $0 },
                cta: loc.t("onboarding.common.close"), ctaAccent: false
            )
        }
    }

    /// "It may have been sent" (spec 082 RA10) — the sheet's and the
    /// aftercare's one drawing: the submitting title, the caption, the op
    /// hash, and a close that leaves it running. Never a Retry.
    static func maybeSentReceipt(
        op: String, summary: String?, header: FlowHeaderModel, loc: Loc
    ) -> SendReceiptModel {
        SendReceiptModel(
            header: header, stage: .submitted,
            title: loc.t("send.txSubmitting"),
            captions: [summary, s(loc, "maybeSent")].compactMap { $0 },
            hash: ReceiptHashModel(
                label: loc.t("componentsTx.receipt.userOpHash"),
                value: "\(op.prefix(10))…\(op.suffix(8))",
                copyLabel: loc.t("componentsUi.identiconViewer.copyAddress"),
                copyValue: op
            ),
            cta: loc.t("send.txCloseBackground"), ctaAccent: false
        )
    }

    /// The ending of a request whose sheet the core has closed — the same
    /// receipt, in the core's words for it once the tracker has had its say
    /// (`signEndingState`, spec 082 RA8): a tick only for a signature or an
    /// op the tracker CONFIRMED; a revert is a failure with its hash and the
    /// explorer (never 已确认, W3); an op the relay never had is "not sent";
    /// anything still on its way says so — never "failed" on time alone.
    static func aftercareReceipt(
        _ aftercare: SigningAftercare, summary: String?, context: Context
    ) -> SendReceiptModel {
        let loc = context.loc
        let header = FlowHeaderModel(title: "", backLabel: "")
        let explorer = !(context.explorerBase ?? "").isEmpty ? loc.t("history.viewOnExplorer") : nil
        func hashRow(_ tx: String) -> ReceiptHashModel {
            ReceiptHashModel(
                label: loc.t("componentsTx.receipt.txHash"),
                value: "\(tx.prefix(10))…\(tx.suffix(8))",
                copyLabel: loc.t("componentsUi.identiconViewer.copyAddress"),
                copyValue: tx
            )
        }
        let track = context.track.flatMap { entry in
            aftercare.userOpHash.flatMap {
                entry.userOpHash.caseInsensitiveCompare($0) == .orderedSame ? entry : nil
            }
        }
        switch aftercare.state(track: track) {
        case .signed:
            return SendReceiptModel(
                header: header, stage: .confirmed,
                // "已签名！" — `signHandoff.signed` reads "已发送" in zh, which
                // a message that went nowhere is not.
                title: loc.t("clearSigning.alertSignedTitle"),
                captions: [summary].compactMap { $0 },
                cta: loc.t("componentsTx.receipt.done"), ctaAccent: true
            )
        case .confirmed(let tx):
            let hash = tx.isEmpty ? nil : tx
            return SendReceiptModel(
                header: header, stage: .confirmed,
                title: loc.t("componentsTx.receipt.statusConfirmed"),
                captions: [summary, context.chainName.isEmpty ? nil : context.chainName].compactMap { $0 },
                hash: hash.map(hashRow),
                viewOnExplorer: hash != nil ? explorer : nil,
                cta: loc.t("componentsTx.receipt.done"), ctaAccent: true
            )
        case .reverted(let tx):
            // It landed and reverted: the money did not move, the fee may
            // have been spent — the corpus's own sentence, and the proof.
            let hash = tx.isEmpty ? nil : tx
            return SendReceiptModel(
                header: header, stage: .failed,
                title: loc.t("componentsTx.receipt.statusFailed"),
                captions: [summary, loc.t("componentsTx.receipt.failedHint")].compactMap { $0 },
                hash: hash.map(hashRow),
                viewOnExplorer: hash != nil ? explorer : nil,
                cta: loc.t("componentsTx.receipt.done"), ctaAccent: true
            )
        case .notSent:
            return SendReceiptModel(
                header: header, stage: .failed,
                title: loc.t("componentsTx.receipt.statusFailed"),
                captions: [summary, loc.t("send.txErrorGeneric")].compactMap { $0 },
                cta: loc.t("componentsTx.receipt.done"), ctaAccent: true
            )
        // The relay refused it (spec 082 RJ3): nothing was sent, and the
        // same op would be refused again — no "try again", no explorer.
        case .refused:
            return SendReceiptModel(
                header: header, stage: .failed,
                title: loc.t("componentsTx.receipt.statusFailed"),
                captions: [summary, s(loc, "refused")].compactMap { $0 },
                cta: loc.t("componentsTx.receipt.done"), ctaAccent: true
            )
        case .following(let op, let outcome, let feeHeld, let relayFunding):
            if outcome == "maybe_sent" {
                return maybeSentReceipt(op: op, summary: summary, header: header, loc: loc)
            }
            let caption: String
            var eta: ReceiptEtaModel?
            if feeHeld {
                caption = loc.t("send.txHeldFees")
            } else if relayFunding && outcome != "unknown" {
                // The relay holds it while it tops up its gas (098 follow-up).
                caption = loc.t("send.txRelayFunding")
            } else {
                switch outcome {
                case "unknown": caption = s(loc, "unknownOutcome")
                case "still_confirming": caption = s(loc, "stillConfirming")
                default:
                    // Spec 099 R6: counted from the relay's send, the core's
                    // one countdown; before it the relay is sending it.
                    let sentAt = track?.relaySentAtMs
                    let pace = LandingPaceWire.of(sentAtMs: sentAt, typicalS: context.typicalS, nowMs: context.nowMs)
                    caption = pace.waiting ? loc.t("send.txRelaySending") : loc.t("send.txWaitingConfirm")
                    eta = ReceiptEtaModel.counting(
                        sentAtMs: sentAt, typicalS: context.typicalS,
                        typicalLine: context.typicalS.map {
                            loc.t("send.txTypicalTime", vars: ["chainName": context.chainName, "estSecs": String($0)])
                        },
                        loc: loc, nowMs: context.nowMs
                    )
                }
            }
            return SendReceiptModel(
                header: header, stage: .submitted,
                title: loc.t("send.txSubmittedTitle"),
                captions: [summary, caption].compactMap { $0 },
                cta: loc.t("send.txCloseBackground"), ctaAccent: false, eta: eta
            )
        }
    }

    /// The request in one line, from the blocks the sheet already drew: what
    /// it is ("发送", "授权") and its figure — so the receipt still says WHAT
    /// is landing once the form has gone.
    static func summaryOf(_ blocks: [SigningBlock]) -> String? {
        let intent = blocks.lazy.compactMap { block -> String? in
            if case .intent(let text, _) = block, !text.isEmpty { return text }
            return nil
        }.first
        let figure = blocks.lazy.compactMap { block -> String? in
            switch block {
            case .amount(let line, _, _):
                return "\(line.sign)\(line.value) \(line.symbol)".trimmingCharacters(in: .whitespaces)
            case .swap(let pay, let receive):
                return "\(pay.value) \(pay.symbol) → \(receive.value) \(receive.symbol)"
            default:
                return nil
            }
        }.first
        let line = [intent, figure].compactMap { $0 }.joined(separator: " · ")
        return line.isEmpty ? nil : line
    }

    // MARK: - What it does

    static func blocks(
        clear: ClearSigningViewWire, to: String?, valueHex: String?, dataBytes: Int,
        context: Context
    ) -> [SigningBlock] {
        // Spec 082 RC1: a plain native transfer — no calldata — is the core's
        // verdict now (`ClearSurface.plainSend`), not a guess drawn here. The
        // interception this replaced read "no result and no bytes" and printed
        // its own figure; the core's card is exact and refuses a value it
        // cannot read, which then falls to the blind rung like any other.
        blocksBySurface(clear: clear, to: to, dataBytes: dataBytes, context: context)
    }

    /// The core's plain send card (RC1–RC5), in the look the wallet's own send
    /// has always had: 发送 · −amount coin · 接收方. A zero value reads
    /// "0 coin" with no minus (RC3): only a simulation may say nothing leaves.
    /// The coin is the fee row's symbol (RC5).
    static func plainSendBlocks(_ plain: ClearPlainSendWire, context: Context) -> [SigningBlock] {
        let loc = context.loc
        return [
            .intent(text: s(loc, "intentSend"), tone: .neutral),
            .amount(
                line: AmountLine(sign: plain.noValue ? "" : "−", value: plain.amount,
                                 symbol: context.nativeSymbol),
                card: true
            ),
            .party(label: s(loc, "recipientLabel"), name: AddressText.short(plain.to), address: plain.to),
        ]
    }

    /// What the simulation found, or that it could not look.
    ///
    /// Three outcomes and they are three different sentences:
    ///
    /// - judgments → the rows, each one the CORE's verdict;
    /// - answered, nothing moved → "checked, nothing moves";
    /// - the core's notice → its sentence in its tone: "expected to fail"
    ///   (danger) or "Vela couldn't check" (caution) — never silence, because
    ///   a wallet that stays quiet when it could not check teaches people
    ///   that silence means safe.
    ///
    /// Only for transactions: a message moves nothing, and a balance block on
    /// a signature would be an answer to a question nobody asked.
    static func balanceBlocks(isTransaction: Bool, context: Context) -> [SigningBlock] {
        let loc = context.loc
        guard isTransaction else { return [] }

        // The core's notice, in the core's tone (spec 082 RG6): a revert is
        // danger, a node that could not check is caution.
        if case .notice(let risk, let key, let reason) = context.simulation {
            let text = reason.map { loc.t(key, vars: ["reason": $0]) } ?? loc.t(key)
            return [.warning(tone: risk == "danger" ? .danger : .caution, text: text)]
        }
        // Still running, or a verdict for a previous request. Silence, because
        // a block that appears and then changes its mind is worse than one that
        // arrives late.
        guard let sim = context.sim, sim.ready, context.simulation == .answered else { return [] }
        guard !sim.judgments.isEmpty else {
            return [.balances(
                title: s(loc, "balanceChangesTitle"),
                rows: [],
                note: s(loc, "balanceNoAssetsMove"),
                noteTone: .neutral
            )]
        }
        // A zero change is not a change: the core writes none (RJ15), and
        // the row is not drawn.
        let rows = sim.judgments.compactMap { balanceRow($0, context: context) }
        // One warning for the whole block, not one per row: the caution is
        // about the same thing each time, and repeating it is how people stop
        // reading it.
        let unverified = sim.judgments.contains {
            if case .erc20Unverified = $0 { return true }
            return false
        }
        return [.balances(
            title: s(loc, "balanceChangesTitle"),
            rows: rows,
            note: unverified ? s(loc, "unverifiedWarning") : nil,
            noteTone: unverified ? .caution : .neutral
        )]
    }

    /// One judgment, as a row — `nil` for a change of zero.
    ///
    /// The figure is the core's (`formatSignedTokenAmount`, spec 082 RJ15):
    /// the shell's own formatter rounded a 1000-wei outflow to "0" and kept
    /// its minus — "xDAI −0" (G49).
    ///
    /// An unverified INFLOW shows its direction and the word "unverified
    /// token" and **no number**: the amount in a simulated log is whatever the
    /// site being signed for chose to emit, and printing it lends this wallet's
    /// credibility to a stranger's arithmetic.
    private static func balanceRow(
        _ judgment: TrustSimJudgmentWire, context: Context
    ) -> BalanceDeltaRow? {
        let loc = context.loc
        let incoming = judgment.incoming
        let tone: SigningTone = incoming ? .success : .neutral
        switch judgment {
        case .native(let delta):
            guard let text = SimDeltas.deltaText(delta, decimals: 18) else { return nil }
            return BalanceDeltaRow(symbol: context.nativeSymbol, delta: text, tone: tone)
        case .erc20Trusted(_, let delta, let symbol, let decimals, _):
            guard let text = SimDeltas.deltaText(delta, decimals: decimals) else { return nil }
            return BalanceDeltaRow(symbol: symbol, delta: text, tone: tone)
        case .erc20Unverified:
            return BalanceDeltaRow(
                symbol: s(loc, "balanceUnverifiedToken"),
                // Direction only. Never the site's own number.
                delta: incoming ? "+" : "\u{2212}",
                tone: .caution
            )
        }
    }

    private static func blocksBySurface(
        clear: ClearSigningViewWire, to: String?, dataBytes: Int, context: Context
    ) -> [SigningBlock] {
        let loc = context.loc
        switch clear.surface {
        case .none:
            return []
        case .loading:
            // Holds the sheet. A blind view must never flash before the clear
            // one.
            return [.sentence(text: s(loc, "loading"), tone: .neutral)]
        case .clearSign:
            return clear.result.map {
                resultBlocks($0, native: clear.nativeValue, context: context)
            } ?? []
        case .ethSign, .messageSign:
            return clear.message.map {
                messageBlocks($0, loc: loc, origin: context.origin, networks: context.networks)
            } ?? []
        case .blindTypedData:
            guard let typed = clear.blindTyped else { return [] }
            var blocks: [SigningBlock] = [
                .intent(text: typed.primaryType ?? s(loc, "signTypedData"), tone: .caution),
                .warning(tone: .caution, text: s(loc, "blindTypedWarning")),
            ]
            if typed.hasDomain {
                blocks.append(.party(
                    label: s(loc, "signingFor"),
                    name: typed.domainName ?? s(loc, "unverifiedLabel"),
                    address: typed.verifyingContract
                ))
            }
            if !typed.fields.isEmpty {
                blocks.append(.rows(typed.fields.map {
                    SigningRow(label: $0.key, value: $0.value, valueTone: .neutral, mono: true)
                }))
            }
            return blocks
        case .plainSend:
            // `plainSend` is present exactly when the surface is; a view
            // without it is drawn as the blind rung rather than as nothing.
            if let plain = clear.plainSend { return plainSendBlocks(plain, context: context) }
            return blindTransactionBlocks(to: to, dataBytes: dataBytes, native: nil, context: context)
        case .blindTransaction:
            return blindTransactionBlocks(
                to: to, dataBytes: dataBytes, native: clear.nativeValue, context: context
            )
        case .batch:
            // 089 S1: every call of a batch, never call 1 alone.
            return clear.batch.map { batchBlocks($0, context: context) } ?? []
        }
    }

    /// 089 S1: a batch as the sheet draws it — "Batch", how many transactions
    /// are signed together, then EVERY call as its own card (the drawn CS26),
    /// the coin the whole batch moves, and every flag any call raised, said
    /// once. The headline is never call 1's: `[1 wei → A, 1 xDAI → B]` read
    /// "Send 0.000…1 xDAI" and signed both. The guard's per-call cap cards and
    /// its unlimited sentence follow (`guardBlocks`).
    static func batchBlocks(_ batch: ClearBatchViewWire, context: Context) -> [SigningBlock] {
        let loc = context.loc
        var blocks: [SigningBlock] = [
            .intent(text: s(loc, "batchIntent"), tone: tone(of: batch.risk)),
            .sentence(text: s(loc, "batchSubtitle", ["count": String(batch.calls.count)]), tone: .accent),
        ]
        blocks += batch.calls.map { batchCallCard($0, context: context) }
        if let total = batch.totalAmount, batch.totalValueWei != "0" {
            blocks.append(.rows([SigningRow(
                label: loc.t("send.splitTotalLabel"), value: "−\(total) \(context.nativeSymbol)"
            )]))
        }
        let results = batch.calls.compactMap(\.result)
        if results.contains(where: \.toOwnToken) {
            blocks.append(.warning(tone: .danger, text: s(loc, "tokenToContractWarning")))
        }
        if results.contains(where: \.bestEffort) {
            blocks.append(.warning(tone: .caution, text: s(loc, "bestEffortWarning")))
        }
        if results.contains(where: \.partial) {
            blocks.append(.warning(tone: .caution, text: s(loc, "partialWarning")))
        }
        if results.contains(where: { $0.provenance == .fetched }) {
            blocks.append(.warning(tone: .caution, text: s(loc, "descriptorFetchedWarning")))
        }
        if results.contains(where: { $0.termsOffChain == true }) {
            blocks.append(.warning(tone: .caution, text: s(loc, "warnOrderTerms")))
        }
        if results.contains(where: { $0.fields.contains(where: \.unverified) }) {
            blocks.append(.warning(tone: .caution, text: s(loc, "unverifiedWarning")))
        }
        if results.contains(where: { $0.fields.contains(where: \.expired) }) {
            blocks.append(.warning(tone: .caution, text: a(loc, "expired")))
        }
        return blocks
    }

    /// 089 S1: one call of a batch, as its own card — the words its call would
    /// get alone. A decoded call is its intent and its fields, then the coin it
    /// moves and whom it calls; a plain send is "Send", the exact amount and
    /// the recipient; a call nobody could read says so in its title, with whom
    /// it calls and what coin it moves.
    private static func batchCallCard(_ call: ClearBatchCallWire, context: Context) -> SigningBlock {
        let loc = context.loc
        func step(_ action: String) -> String {
            s(loc, "batchStep", ["index": String(call.index), "action": action])
        }
        let tone = tone(of: call.risk)
        // What the call moves of the chain's own coin, and whom it calls:
        // inside a batch nothing else on the sheet says it for this call.
        var coin: [SigningRow] = []
        if let amount = call.amount, call.valueWei != "0" {
            coin.append(SigningRow(label: s(loc, "labelAmount"), value: "−\(amount) \(context.nativeSymbol)"))
        }
        // A contract the wallet knows on this chain is named (096 F5); any
        // other is its full address.
        let target = call.to.map { to -> [SigningRow] in
            if let name = call.toName {
                return [SigningRow(label: s(loc, "interactingLabel"), value: name)]
            }
            return [SigningRow(label: s(loc, "interactingLabel"), value: to, mono: true)]
        } ?? []
        if call.surface == .clearSign, let result = call.result {
            return .card(title: step(result.intent),
                         rows: result.fields.filter { !$0.detail }.map { row(of: $0) } + coin + target,
                         tone: tone)
        }
        if call.surface == .plainSend, let plain = call.plainSend {
            return .card(title: step(s(loc, "intentSend")), rows: [
                SigningRow(label: s(loc, "labelAmount"),
                           value: "\(plain.noValue ? "" : "−")\(plain.amount) \(context.nativeSymbol)"),
                SigningRow(label: s(loc, "recipientLabel"), value: plain.to, mono: true),
            ], tone: tone)
        }
        return .card(
            title: step(s(loc, "blindDecodeWarning", ["bytes": String(call.dataBytes)])),
            rows: target + coin,
            tone: tone
        )
    }

    private static func blindTransactionBlocks(
        to: String?, dataBytes: Int, native: ClearNativeValueWire?, context: Context
    ) -> [SigningBlock] {
            let loc = context.loc
            var blocks: [SigningBlock] = [
                .intent(text: s(loc, "intentContractCall"), tone: .caution),
                .warning(tone: .caution,
                         text: s(loc, "blindDecodeWarning", ["bytes": String(dataBytes)])),
            ]
            // Spec 096 F4: nobody could read the call; the coin it sends is known.
            if let native { blocks.append(.rows([coinRow(native, context: context)])) }
            if let to {
                blocks.append(.party(
                    label: s(loc, "interactingLabel"),
                    name: s(loc, "unverifiedLabel"),
                    address: to,
                    badge: PartyBadge(text: s(loc, "unverifiedLabel"), tone: .caution)
                ))
            }
            return blocks
    }

    private static func tone(of risk: ClearRisk) -> SigningTone {
        switch risk {
        case .safe: .success
        case .normal: .neutral
        case .caution: .caution
        case .danger: .danger
        }
    }

    private static func resultBlocks(
        _ result: ClearSignResultWire, native: ClearNativeValueWire?, context: Context
    ) -> [SigningBlock] {
        let loc = context.loc
        var blocks: [SigningBlock] = [.intent(text: result.intent, tone: tone(of: result.risk))]
        blocks += warnings(result, loc: loc)
        // Spec 096 F4: the coin the call sends leads the rows, in a batch
        // call's own words, when the reading does not say it itself.
        let coin = native.map { [coinRow($0, context: context)] } ?? []
        let rows = coin + result.fields.filter { !$0.detail }.map { row(of: $0) }
        if !rows.isEmpty { blocks.append(.rows(rows)) }
        return blocks
    }

    /// "Amount −0.003 BNB" — the core's `nativeValue`, as a batch call's coin
    /// row reads, in the coin symbol the fee row uses (RC5).
    private static func coinRow(_ native: ClearNativeValueWire, context: Context) -> SigningRow {
        SigningRow(label: s(context.loc, "labelAmount"), value: "−\(native.amount) \(context.nativeSymbol)")
    }

    private static func warnings(_ result: ClearSignResultWire, loc: Loc) -> [SigningBlock] {
        var blocks: [SigningBlock] = []
        // Irreversible, and the single most expensive mistake this screen can
        // fail to mention.
        if result.toOwnToken {
            blocks.append(.warning(tone: .danger, text: s(loc, "tokenToContractWarning")))
        }
        if result.bestEffort { blocks.append(.warning(tone: .caution, text: s(loc, "bestEffortWarning"))) }
        if result.partial { blocks.append(.warning(tone: .caution, text: s(loc, "partialWarning"))) }
        // Spec 081 FR-008: the descriptor service answered, over plain HTTP,
        // from a base URL the person can edit, and nothing signed the answer.
        // The other sources say nothing here: built in and pinned are what
        // "verified" means, a token-standard shape is the standard doing its
        // job, the 4-byte database has its own line above, and a deployment
        // claims nothing to doubt.
        if result.provenance == .fetched {
            blocks.append(.warning(tone: .caution, text: s(loc, "descriptorFetchedWarning")))
        }
        // Spec 096 F5: a CoW pre-signature signs an order whose amounts are
        // hashed into its id — not on this sheet, and said so.
        if result.termsOffChain == true {
            blocks.append(.warning(tone: .caution, text: s(loc, "warnOrderTerms")))
        }
        if result.fields.contains(where: \.unverified) {
            blocks.append(.warning(tone: .caution, text: s(loc, "unverifiedWarning")))
        }
        if result.fields.contains(where: \.expired) {
            blocks.append(.warning(tone: .caution, text: a(loc, "expired")))
        }
        return blocks
    }

    private static func row(of field: ClearSignFieldWire) -> SigningRow {
        SigningRow(
            label: field.label,
            value: field.value,
            valueTone: field.warning ? .danger : (field.unverified || field.expired ? .caution : .neutral),
            // An address reads as monospace; a contract the core names
            // ("PancakeSwap Permit2", 096 F5) is a name, in the text face.
            mono: field.address != nil && field.value.hasPrefix("0x")
        )
    }

    private static func messageBlocks(
        _ message: ClearMessageViewWire, loc: Loc, origin: String?, networks: WalletNetworks
    ) -> [SigningBlock] {
        let signingIn = message.siwe != nil
        let danger = message.dangerClass == .ethSign || message.dangerClass == .siwePhish
        var blocks: [SigningBlock] = [
            .intent(text: signingIn ? s(loc, "signInIntent") : s(loc, "signMessage"),
                    tone: danger ? .danger : .neutral),
        ]
        if message.dangerClass == .ethSign {
            blocks.append(.sentence(text: s(loc, "ethSignBody"), tone: .danger))
        }
        if let text = message.decodedText, !text.isEmpty {
            blocks.append(.sentence(text: text, tone: .neutral))
        }
        if let preview = message.binaryPreview {
            blocks.append(.code(lines: [preview]))
        }
        if message.nonPrintable {
            blocks.append(.warning(tone: .caution, text: s(loc, "hexMessageWarning")))
        }
        if let siwe = message.siwe {
            var rows = [SigningRow(label: s(loc, "siweDomain"), value: siwe.domainHost ?? siwe.domain)]
            if let statement = siwe.statement {
                rows.append(SigningRow(label: s(loc, "siweStatement"), value: statement))
            }
            if let uri = siwe.uri {
                rows.append(SigningRow(label: s(loc, "siweOrigin"), value: uri))
            }
            // The chain the MESSAGE names, and its nonce. Both are parsed by
            // the core and were being dropped on the floor here — a sign-in
            // for chain 1 presented on chain 100 looked identical to one that
            // matched, because the number was never on screen.
            //
            // Shown as FACTS, not a verdict: the core binds on the domain and
            // not on the chain, and a shell that decided "this chain is wrong"
            // would be a second, disagreeing adjudicator. Recorded in results.
            if let chainId = siwe.chainId {
                rows.append(SigningRow(
                    label: s(loc, "labelChain"),
                    // From the wallet's list, the person's own included.
                    value: networks.meta(chainId)?.displayName ?? String(chainId)
                ))
            }
            if let nonce = siwe.nonce, !nonce.isEmpty {
                rows.append(SigningRow(label: s(loc, "labelNonce"), value: nonce, mono: true))
            }
            blocks.append(.rows(rows))

            // The string on screen is the string that was adjudicated.
            let domain = siwe.domainHost ?? siwe.domain
            switch message.binding {
            case .ok:
                blocks.append(.positive(s(loc, "siweOk", ["domain": domain])))
            case .mismatch:
                blocks.append(.warning(
                    tone: .danger,
                    text: s(loc, "siweMismatch", ["domain": domain, "origin": origin ?? ""])
                ))
            default:
                break
            }
        }
        if message.dangerClass == .ethSign {
            blocks.append(.warning(tone: .danger, text: s(loc, "ethSignWarning")))
        }
        return blocks
    }

    // MARK: - The guard

    /// The guard's verdict: the spending cap with the chips the core offers,
    /// the custom amount when chosen, the notes, the resulting total for an
    /// increase; an off-chain permit that cannot be capped; a batch's legs.
    static func guardBlocks(_ guardView: GuardViewWire, loc: Loc) -> [SigningBlock] {
        switch guardView.surface {
        case .none:
            return []

        case .permitSign:
            var blocks: [SigningBlock] = []
            if let detected = guardView.detected {
                blocks.append(.party(label: a(loc, "spenderLabel"),
                                     name: AddressText.short(detected.spender),
                                     address: detected.spender))
            }
            // An unlimited permit is said like any unlimited approval (spec
            // 094 S8) — the core's flag, the same sentence.
            if guardView.unlimitedWarning {
                blocks.append(.warning(tone: .danger, text: s(loc, "unlimitedWarning")))
            }
            // The dApp submits its own amount on chain, so rewriting would
            // desync the signature and revert their transaction. Saying so is
            // the only honest move.
            blocks.append(.warning(tone: .danger, text: a(loc, "permitCantCap")))
            return blocks

        case .approvalEditor:
            var blocks: [SigningBlock] = []
            if let editor = guardView.editor {
                blocks.append(allowanceBlock(
                    editor: editor, meta: guardView.meta, increase: guardView.increaseTotal,
                    decimalsUnverified: guardView.decimalsUnverified, expired: guardView.expired,
                    loc: loc
                ))
            }
            if let detected = guardView.detected {
                blocks.append(.party(label: a(loc, "spenderLabel"),
                                     name: AddressText.short(detected.spender),
                                     address: detected.spender))
            }
            // Kept as the site asked (2026-09-26) — allowed, never unsaid. The
            // core decides when (spec 094 S8).
            if guardView.unlimitedWarning {
                blocks.append(.warning(tone: .danger, text: s(loc, "unlimitedWarning")))
            }
            return blocks

        case .batch:
            var blocks: [SigningBlock] = []
            for (index, leg) in (guardView.batch?.legs ?? []).enumerated() {
                if let editor = leg.editor {
                    blocks.append(allowanceBlock(
                        editor: editor, meta: leg.meta, increase: nil,
                        decimalsUnverified: false, expired: false, loc: loc,
                        prefix: "#\(index + 1) ", leg: index
                    ))
                }
                if let approval = leg.approval {
                    blocks.append(.party(label: a(loc, "spenderLabel"),
                                         name: AddressText.short(approval.spender),
                                         address: approval.spender))
                }
            }
            if guardView.unlimitedWarning {
                blocks.append(.warning(tone: .danger, text: s(loc, "unlimitedWarning")))
            }
            return blocks
        }
    }

    private static func allowanceBlock(
        editor: GuardEditorViewWire,
        meta: GuardTokenMetaViewWire,
        increase: GuardIncreaseTotalViewWire?,
        decimalsUnverified: Bool,
        expired: Bool,
        loc: Loc,
        prefix: String = "",
        leg: Int? = nil
    ) -> SigningBlock {
        func chip(_ id: String, _ label: String, _ mode: GuardEditorMode, offered: Bool) -> AllowanceChip {
            AllowanceChip(
                id: id, label: label,
                // **Disabled, not merely unselected.** A balance nobody could
                // read, or a request of zero, has nothing to offer, and a chip
                // that looks available and refuses is worse than one that is
                // plainly out.
                state: !offered ? .disabled : (editor.mode == mode ? .selected : .idle)
            )
        }
        let chips = [
            // An unlimited request opens HERE — the site's own bytes, kept
            // (Permit2 bundles revert when the wallet re-encodes the approve).
            chip("requested", a(loc, "requested"), .requested,
                 offered: editor.requestedFinite || editor.requestedUnlimited),
            chip("balance", a(loc, "balanceCap"), .balance, offered: editor.hasBalanceCap),
            chip("custom", a(loc, "custom"), .custom, offered: true),
            // Not on increaseAllowance: "revoke" would sign an increase of 0.
            chip("revoke", a(loc, "revoke"), .revoke, offered: editor.revokeOffered),
        ]

        let value = editor.displayAmountRaw.map { raw in
            "\(SendLive.fromBase(raw, decimals: meta.decimals)) \(meta.symbol)"
                .trimmingCharacters(in: .whitespaces)
        } ?? a(loc, "unlimitedValue")

        var notes: [String] = []
        if decimalsUnverified { notes.append(a(loc, "decimalsUnverified")) }
        if expired { notes.append(a(loc, "expired")) }

        return .allowance(
            label: prefix + a(loc, "spendingCap"),
            value: value,
            // Only a chosen, finite cap reads as settled; unlimited kept as
            // asked reads as the danger it is.
            valueTone: (editor.choice == nil || editor.choice == .unlimited) ? .danger : .neutral,
            chips: chips,
            note: notes.isEmpty ? nil : notes.joined(separator: "\n"),
            // "increase by 100" must never read as "cap at 100" — and when the
            // read failed it still says the increment ADDS rather than hiding.
            resultingTotal: increase.map { total in
                SigningRow(
                    label: a(loc, "resultingTotal"),
                    value: total.total ?? a(loc, "resultingTotalUnknown", ["amount": total.increment])
                )
            },
            custom: editor.mode == .custom ? AllowanceInput(
                value: editor.customText,
                symbol: meta.symbol,
                placeholder: "0",
                error: editor.error.map { error in
                    switch error {
                    // A typed "cap" of 10^60 is no cap — an amount the field
                    // cannot take. Keeping the site's unlimited ask is the
                    // Requested chip, so "unlimited is disabled" would be false.
                    case .invalidAmount, .unlimitedDisabled: a(loc, "invalidAmount")
                    }
                }
            ) : nil,
            leg: leg
        )
    }

    // MARK: - The fee and the verb

    static func feeModel(
        clear: ClearSigningViewWire, fee: FeeViewWire?, context: Context, speedTier: String? = nil
    ) -> FeeModel {
        if isOffChain(clear) { return .offchain(note: s(context.loc, "noNetworkFee")) }
        let value: String
        // For the moment between a speed being picked and its own figure
        // landing, the fee in hand is the previous speed's: "estimating".
        if let estimate = fee?.fee, !feeOfAnotherTier(fee, speedTier: speedTier) {
            // The send screens' own line, through the send screens' own
            // formatter (issue 201): the coin that is ACTUALLY paying — an
            // in-band ERC-20 fee is its own amount under its own ticker, never
            // the native figure — and what it costs. The design sheet is
            // explicit that these two surfaces must not drift.
            value = "~" + SendLive.feeLine(estimate, view: nil, fee: fee, display: context.display,
                                           networks: context.networks)
        } else if fee?.failed != nil || (fee == nil && context.feeStartFailure != nil) {
            value = context.loc.t("componentsUi.gas.estimateFailed")
        } else {
            value = context.loc.t("componentsUi.gas.estimating")
        }
        // The coins the relay takes the fee in — the Send screen's rows,
        // amounts and "cannot pay" verdict (founder, 2026-09-19: a fee a person
        // can switch when sending and not when signing is two products).
        let options = fee?.options ?? []
        let selector: (title: String, options: [FeeTokenOption])? =
            context.feeOpen && options.count > 1
            ? (title: s(context.loc, "feeTokenTitle"), options: options.map { option in
                FeeTokenOption(
                    id: option.contract ?? nativeFeeId,
                    // The coin's real logo — the send form's fee-coin sheet's
                    // own mark (chain + symbol + contract; the native coin
                    // wears its chain's logo), over the drawn ticker. A letter
                    // on a disc drew USDC and USDT as the same "U".
                    mark: context.chainId.map {
                        TokenMarkModel.of(chainId: $0, symbol: option.symbol,
                                          tokenAddress: option.contract, color: context.chainDot)
                    } ?? TokenMarkModel(ticker: option.symbol, badgeColor: context.chainDot),
                    name: option.symbol,
                    balance: "\(SendLive.trim(SendLive.fromBase(option.balance, decimals: option.decimals))) \(option.symbol)",
                    fee: option.amount.map {
                        "~\(SendLive.feeFromBase($0, decimals: option.decimals)) \(option.symbol)"
                    } ?? "—",
                    selected: option.selected,
                    disabled: option.insufficient,
                    // Issue #408: a refused coin says why — the core's numbers.
                    reason: option.insufficient ? option.short.map {
                        context.loc.t("componentsUi.gas.rowShort", vars: ["need": $0.need, "have": $0.have])
                    } : nil
                )
            })
            : nil
        // Issue #262: the core shut the gate because the selected coin cannot
        // pay this fee — the send form's own sentence (#211), about the same
        // shortfall. A dark confirm with no reason is issue 204.
        var warning: String?
        if let fee, fee.fee != nil, fee.noCoinPays {
            // Issue #408: and not one coin on offer can pay — the core's
            // verdict, said as that rather than naming the coin in force
            // ("Insufficient ETH" over a wallet whose USDT was short too).
            warning = context.loc.t("componentsUi.gas.noCoinPays")
        } else if let fee, fee.fee != nil, !fee.busy, fee.failed == nil, !fee.confirmFeeReady,
           let selected = fee.options.first(where: { $0.selected }), selected.insufficient {
            warning = context.loc.t("send.warnInsufficientGas", vars: ["sym": selected.symbol])
        } else if let fee, !fee.busy, fee.failed == nil,
                  let selected = fee.options.first(where: { $0.selected }), selected.spentByOperation == true {
            // Spec 096 F2: the person chose a coin the transaction itself
            // spends (the PancakeSwap USDC swap, fee in USDC). The core
            // flags it; said under the fee while that coin is the one paying.
            warning = context.loc.t("componentsUi.gas.feeCoinSpent", vars: ["sym": selected.symbol])
        } else if let failed = fee?.failed ?? (fee == nil ? context.feeStartFailure : nil),
                  let key = feeFailureReasonKey(failure: failed) {
            // Spec 079: why there is no fee, and that it will be asked again
            // (the row said "点击重试" with the relay down and stayed so).
            // Which words is the core's (spec 082 RJ13): the relay for a
            // relay's failure, the chain's node — rate-limited, or named
            // unreachable — for a chain read, and none for a failure that is
            // not the network's (G48: a public node's rate limit read "Can't
            // reach Vela").
            warning = context.loc.t(key, vars: ["chain": context.chainName])
        }
        // The same condition `SigningController.feeTapped` acts on, decided
        // once here so the chevron and the handler cannot disagree.
        let tappable = fee?.failed != nil || (fee == nil && context.feeStartFailure != nil)
            || options.count > 1
        return .onchain(label: context.loc.t("componentsUi.gas.networkFee"), value: value,
                        selector: selector, warning: warning, tappable: tappable)
    }

    /// Spec 079: the fee row's refresh — the send form's own control, dimmed
    /// while a measurement is out. None where there is no network fee.
    static func feeRefresh(clear: ClearSigningViewWire, fee: FeeViewWire?, loc: Loc) -> FeeRefreshModel? {
        guard !isOffChain(clear) else { return nil }
        return FeeRefreshModel(label: loc.t("send.feeRefresh"), refreshing: fee?.busy ?? false)
    }

    /// The confirm's label (issue #461: the action alone, no "slide to"
    /// prefix): the core's intent id, **in the corpus's words**.
    ///
    /// Printing the id raw is how Android shipped a button reading 确认send.
    static func confirmLabel(clear: ClearSigningViewWire, loc: Loc) -> String {
        switch clear.confirm {
        case .sign: s(loc, "signLabel")
        case .confirm: s(loc, "confirmLabel")
        case .confirmIntent(let intent, let term):
            switch intent {
            case "send": s(loc, "confirmSend")
            case "swap": s(loc, "confirmSwap")
            case "deposit": s(loc, "confirmDeposit")
            case "withdraw": s(loc, "confirmWithdraw")
            // The core's word for the rest ("Approve" → 授权), else the neutral verb.
            default: term.map { s(loc, $0) } ?? s(loc, "confirmLabel")
            }
        }
    }
}

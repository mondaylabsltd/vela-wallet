//
//  SigningLive.swift
//  VelaWallet
//
//  The signing sheet, from the four machines' views.
//
//  The drawn model keeps only its labels. The dApp is the **host** — guessing
//  a pretty name from a domain is exactly the counterfeit route — the blocks
//  are the core's reading, the fee is the fee policy's, and the slide opens
//  only when all three gating machines say it may.
//
//  Ported from `app-android/.../feature/signing/SigningLive.kt` (spec 044
//  T033), which is the desktop's `signing/live.rs`.
//

import SwiftUI

enum SigningLive {

    struct Context {
        let loc: Loc
        let chainName: String
        let chainDot: Color
        let nativeSymbol: String
        let walletName: String
        let walletAddress: String
        /// The display currency the fee's "≈" half is written in (issue 201).
        var display: WalletLive.Display = .usd
        /// The page's host, for the sign-in verdict's words.
        var origin: String?
        /// What the chain said this transaction would do (spec 055). `nil`
        /// when nothing has been simulated for this account yet.
        var sim: TrustSimViewWire?
        /// Where the simulation has got to. Three states, three sentences.
        var simulation: SigningController.Simulation = .pending
        /// The person's "Sign with" choice for THIS request, and whether its list is open.
        var signMethod = "auto"
        var signWithOpen = false
        /// Whether the fee row's coin list is open (issue #262).
        var feeOpen = false
        /// Every "Sign with" the core offers, in its order (`SignPrefView.offered`).
        var signMethods = ["auto"]
        /// How the Clear Signer last ended for this request without signing.
        var clearSignerNotice: ClearSignerNotice?
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
    static let walletTransport = "wallet"
    /// `registry_backup::REGISTRY` — the one contract the wallet's own backup calls.
    private static let passkeyRegistry = "0x94fd1a891eb6c5f340622baf2f3a0cb70a941ea9"

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

    /// The "Sign with" row: the create flow's own words for where a passkey
    /// is, and the Clear Signer with its one line — for every value the core
    /// offers, in its order. A name this build has no words for is not drawn.
    static func signWith(context: Context) -> SignWithModel {
        let loc = context.loc
        let options = context.signMethods.compactMap { id -> SignWithModel.Option? in
            guard let title = signMethodTitle(id, loc: loc) else { return nil }
            return .init(id: id, title: title, selected: id == context.signMethod,
                         detail: signMethodDetail(id, loc: loc))
        }
        return SignWithModel(
            label: loc.t("componentsUi.signing.signWith"),
            value: options.first(where: \.selected)?.title ?? loc.t("common.automatic"),
            open: context.signWithOpen,
            options: options
        )
    }

    /// One "Sign with" value in words — the signing sheet's and Settings'.
    static func signMethodTitle(_ id: String, loc: Loc) -> String? {
        switch id {
        case "auto": loc.t("common.automatic")
        case "platform": loc.t("onboarding.create.methodPlatformTitle")
        case "hybrid": loc.t("onboarding.create.methodHybridTitle")
        case "security_key": loc.t("onboarding.create.methodSecurityKeyTitle")
        case UserOpSpine.clearSignerMethod: loc.t("componentsUi.signing.clearSignerTitle")
        default: nil
        }
    }

    /// The line under a value: only the Clear Signer needs one — the other
    /// four say where a key is, and this one says what it does instead.
    static func signMethodDetail(_ id: String, loc: Loc) -> String? {
        id == UserOpSpine.clearSignerMethod ? loc.t("componentsUi.signing.clearSignerBody") : nil
    }

    /// The Clear Signer's ending, when it left the request unsigned. Closed is
    /// a person's own decision, told calmly; a refusal or a mismatch is not.
    static func clearSignerBlocks(_ notice: ClearSignerNotice?, loc: Loc) -> [SigningBlock] {
        guard let notice else { return [] }
        return [.warning(tone: notice == .closed ? .caution : .danger, text: loc.t(notice.key))]
    }

    /// The wallet's own key backup, in the person's language. The core's
    /// built-in results are English, like the descriptors beside them ("the
    /// words stay in the shell"); this one is OURS. Matched on the request being
    /// first-party AND the verified registry address — never on the English words.
    static func localizedOwnBackup(_ clear: ClearSigningViewWire, own: Bool, loc: Loc) -> ClearSigningViewWire {
        guard own, let result = clear.result, result.verified,
              result.contractAddress?.lowercased() == passkeyRegistry
        else { return clear }
        let labels = ["settingsModals.backup.registeredAs", "contacts.addressLabel", "settingsModals.backup.publicKeys"].map { loc.t($0) }
        var next = clear
        next.result = result.relabelled(intent: loc.t("settingsModals.backup.intent"), labels: labels)
        return next
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
        speed: SendLive.SpeedInputs? = nil
    ) -> SigningModel {
        let loc = context.loc
        let host = BrowserEngine.hostOf(origin: request.origin)
        let own = request.transportId == walletTransport
        let clear = localizedOwnBackup(rawClear, own: own, loc: loc)
        let facts = SigningController.firstCall(paramsJson: request.paramsJson)
        let dataBytes = (facts?.data.map { $0.hasPrefix("0x") ? $0.dropFirst(2) : $0[...] }?.count ?? 0) / 2

        let blocks = statusBlocks(sign: sign, loc: loc)
            + clearSignerBlocks(context.clearSignerNotice, loc: loc)
            + self.blocks(clear: clear, to: facts?.to, valueHex: facts?.value,
                          dataBytes: dataBytes, context: context)
            // Only a TRANSACTION has balances to change. A message moves
            // nothing, and a balance block on a signature would answer a
            // question nobody asked.
            + balanceBlocks(isTransaction: facts != nil, context: context)
            + guardBlocks(guardView, loc: loc)

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
                summary: clear.result?.contractName,
                fn: clear.result.map { (label: s(loc, "techFunction"), signature: $0.intent) },
                params: [],
                identities: [],
                simResult: nil,
                raw: dataBytes > 0 ? facts?.data.map { (label: s(loc, "techRawData"), hex: $0) } ?? nil : nil,
                copyLabel: fallback.tech.copyLabel,
                explorerLabel: fallback.tech.explorerLabel
            ),
            techOpen: false,
            fee: feeModel(clear: clear, fee: fee, context: context, speedTier: speed?.view.tier),
            signer: (label: s(loc, "signingAccount"),
                     name: context.walletName,
                     seed: context.walletAddress),
            confirm: (hint: s(loc, "slideToConfirm"),
                      action: confirmLabel(clear: clear, loc: loc),
                      enabled: confirmEnabled(sign: sign, guard: guardView, fee: fee, clear: clear,
                                              speedTier: speed?.view.tier)),
            panelTitle: s(loc, "signatureRequest")
        )
        // The wallet's own request (the key backup) is not a site: its own mark
        // and name, and no host — "getvela.app" under a letter read as a stranger.
        model.dappOwn = own
        model.dappIconUrls = own ? [] : siteIconUrls(origin: request.origin)
        model.networkLogoUrl = Marks.chainLogoURL(request.chainId)
        model.signWith = signWith(context: context)
        if !isOffChain(clear) {
            model.feeSpeed = speed.map {
                SendLive.speedModel($0, view: nil, display: context.display, loc: loc)
            }
        }
        return model
    }

    // MARK: - The gate

    /// The slide opens only when the request, the guard and the fee all say it
    /// may — and only then.
    ///
    /// **An off-chain signature has no fee**, so the fee machine has nothing
    /// to be ready about. Requiring its readiness there would make a
    /// `personal_sign` unsignable forever.
    ///
    /// **The fee's say includes its speed** (spec 069): between a tap and that
    /// speed's own figure landing, the core's `confirm_fee_ready` is still true
    /// on the speed just left, and the slide must not sign it.
    static func confirmEnabled(
        sign: SignViewWire, guard guardView: GuardViewWire, fee: FeeViewWire?,
        clear: ClearSigningViewWire, speedTier: String? = nil
    ) -> Bool {
        let feeReady = (fee?.confirmFeeReady ?? false) && !feeOfAnotherTier(fee, speedTier: speedTier)
        return sign.confirmGateOpen
            && guardView.confirmAllowed
            && (isOffChain(clear) || feeReady)
            && !sign.isSigning
            && !sign.isSubmitting
    }

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

    static func statusBlocks(sign: SignViewWire, loc: Loc) -> [SigningBlock] {
        var blocks: [SigningBlock] = []

        if let funding = sign.funding {
            blocks.append(.warning(
                tone: .caution,
                text: loc.t("componentsUi.funding.lead", vars: ["symbol": funding.data.nativeSymbol])
            ))
        }
        if let error = sign.error {
            let text: String = switch error.kind {
            case .unlimitedApproval: a(loc, "unlimitedDisabled")
            case .unsupportedChain: loc.t("send.lock.netNotFound")
            // Neither of these is an error a person needs to read: one is
            // their own decision and the other is the wallet's.
            case .userRejected, .walletSwitchedChains: ""
            default: loc.t("send.txErrorGeneric")
            }
            if !text.isEmpty { blocks.append(.warning(tone: .danger, text: text)) }
        }
        if sign.pendingOpHash != nil {
            blocks.append(.positive(s(loc, "submitted")))
        } else if sign.isSigning || sign.isSubmitting {
            blocks.append(.sentence(text: s(loc, "signing"), tone: .neutral))
        }
        return blocks
    }

    // MARK: - What it does

    static func blocks(
        clear: ClearSigningViewWire, to: String?, valueHex: String?, dataBytes: Int,
        context: Context
    ) -> [SigningBlock] {
        // A plain native transfer — no calldata — is the one transaction the
        // core resolves **without** a descriptor: resolved, and no result.
        // Drawing the blind "cannot decode (0 bytes)" card for it reads a dust
        // send as a contract interaction, which is what Android shipped for
        // one screenshot.
        if clear.resolved, clear.result == nil, clear.message == nil, clear.blindTyped == nil,
           dataBytes == 0, let to {
            return plainTransferBlocks(to: to, valueHex: valueHex, context: context)
        }
        return blocksBySurface(clear: clear, to: to, dataBytes: dataBytes, context: context)
    }

    private static func plainTransferBlocks(
        to: String, valueHex: String?, context: Context
    ) -> [SigningBlock] {
        let loc = context.loc
        let wei = GuardExecutor.decimal(fromWordHex: padded(valueHex ?? "0x0")) ?? "0"
        return [
            .intent(text: s(loc, "intentSend"), tone: .neutral),
            .amount(
                line: AmountLine(sign: "−", value: SendLive.fromBase(wei, decimals: 18),
                                 symbol: context.nativeSymbol),
                card: true
            ),
            .party(label: s(loc, "recipientLabel"), name: AddressText.short(to), address: to),
        ]
    }

    private static func padded(_ hex: String) -> String {
        let digits = hex.hasPrefix("0x") ? String(hex.dropFirst(2)) : hex
        if digits.isEmpty { return "0x00" }
        return digits.count % 2 == 0 ? "0x" + digits : "0x0" + digits
    }

    /// What the simulation found, or that it could not look.
    ///
    /// Three outcomes and they are three different sentences:
    ///
    /// - judgments → the rows, each one the CORE's verdict;
    /// - answered, nothing moved → "checked, nothing moves";
    /// - never answered → the danger-toned "Vela could not tell what this
    ///   does", because a wallet that stays quiet when it could not check
    ///   teaches people that silence means safe.
    ///
    /// Only for transactions: a message moves nothing, and a balance block on
    /// a signature would be an answer to a question nobody asked.
    static func balanceBlocks(isTransaction: Bool, context: Context) -> [SigningBlock] {
        let loc = context.loc
        guard isTransaction else { return [] }

        if context.simulation == .unavailable {
            return [.warning(tone: .danger, text: s(loc, "simUnavailableWarning"))]
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
        let rows = sim.judgments.map { balanceRow($0, context: context) }
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

    /// One judgment, as a row.
    ///
    /// An unverified INFLOW shows its direction and the word "unverified
    /// token" and **no number**: the amount in a simulated log is whatever the
    /// site being signed for chose to emit, and printing it lends this wallet's
    /// credibility to a stranger's arithmetic.
    private static func balanceRow(
        _ judgment: TrustSimJudgmentWire, context: Context
    ) -> BalanceDeltaRow {
        let loc = context.loc
        let incoming = judgment.incoming
        let sign = incoming ? "+" : "−"
        let tone: SigningTone = incoming ? .success : .neutral
        switch judgment {
        case .native(let delta):
            return BalanceDeltaRow(
                symbol: context.nativeSymbol,
                delta: "\(sign)\(SendLive.trim(SendLive.fromBase(magnitude(delta), decimals: 18)))",
                tone: tone
            )
        case .erc20Trusted(_, let delta, let symbol, let decimals):
            return BalanceDeltaRow(
                symbol: symbol,
                delta: "\(sign)\(SendLive.trim(SendLive.fromBase(magnitude(delta), decimals: decimals)))",
                tone: tone
            )
        case .erc20Unverified:
            return BalanceDeltaRow(
                symbol: s(loc, "balanceUnverifiedToken"),
                // Direction only. Never the site's own number.
                delta: sign,
                tone: .caution
            )
        }
    }

    /// The magnitude of a signed decimal string — the sign is already carried
    /// by the row's own glyph.
    private static func magnitude(_ delta: String) -> String {
        delta.hasPrefix("-") ? String(delta.dropFirst()) : delta
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
            return clear.result.map { resultBlocks($0, loc: loc) } ?? []
        case .ethSign, .messageSign:
            return clear.message.map { messageBlocks($0, loc: loc, origin: context.origin) } ?? []
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
        case .blindTransaction:
            var blocks: [SigningBlock] = [
                .intent(text: s(loc, "intentContractCall"), tone: .caution),
                .warning(tone: .caution,
                         text: s(loc, "blindDecodeWarning", ["bytes": String(dataBytes)])),
            ]
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
    }

    private static func tone(of risk: ClearRisk) -> SigningTone {
        switch risk {
        case .safe: .success
        case .normal: .neutral
        case .caution: .caution
        case .danger: .danger
        }
    }

    private static func resultBlocks(_ result: ClearSignResultWire, loc: Loc) -> [SigningBlock] {
        var blocks: [SigningBlock] = [.intent(text: result.intent, tone: tone(of: result.risk))]
        blocks += warnings(result, loc: loc)
        let rows = result.fields.filter { !$0.detail }.map { row(of: $0) }
        if !rows.isEmpty { blocks.append(.rows(rows)) }
        return blocks
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
            mono: field.address != nil
        )
    }

    private static func messageBlocks(
        _ message: ClearMessageViewWire, loc: Loc, origin: String?
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
                    value: ChainCatalog.meta(chainId)?.displayName ?? String(chainId)
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
            if guardView.detected?.isUnbounded == true, guardView.editor?.choice == nil {
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
                        prefix: "#\(index + 1) "
                    ))
                }
                if let approval = leg.approval {
                    blocks.append(.party(label: a(loc, "spenderLabel"),
                                         name: AddressText.short(approval.spender),
                                         address: approval.spender))
                }
            }
            if guardView.batch?.anyUncapped == true {
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
        prefix: String = ""
    ) -> SigningBlock {
        func chip(_ id: String, _ label: String, _ mode: GuardEditorMode, offered: Bool) -> AllowanceChip {
            AllowanceChip(
                id: id, label: label,
                // **Disabled, not merely unselected.** An unlimited request
                // has no finite figure to offer, and a chip that looks
                // available and refuses is worse than one that is plainly out.
                state: !offered ? .disabled : (editor.mode == mode ? .selected : .idle)
            )
        }
        let chips = [
            chip("requested", a(loc, "requested"), .requested, offered: editor.requestedFinite),
            chip("balance", a(loc, "balanceCap"), .balance, offered: editor.hasBalanceCap),
            chip("custom", a(loc, "custom"), .custom, offered: true),
            chip("revoke", a(loc, "revoke"), .revoke, offered: true),
        ]

        let value = editor.displayAmountRaw.map { raw in
            "\(SendLive.fromBase(raw, decimals: meta.decimals)) \(meta.symbol)"
                .trimmingCharacters(in: .whitespaces)
        } ?? a(loc, "unlimitedValue")

        var notes: [String] = []
        if !editor.requestedFinite {
            notes.append(a(loc, "unlimitedDisabled") + "\n" + a(loc, "choosePrompt"))
        }
        if decimalsUnverified { notes.append(a(loc, "decimalsUnverified")) }
        if expired { notes.append(a(loc, "expired")) }

        return .allowance(
            label: prefix + a(loc, "spendingCap"),
            value: value,
            valueTone: editor.choice != nil ? .neutral : .danger,
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
                    case .invalidAmount: a(loc, "invalidAmount")
                    case .unlimitedDisabled: a(loc, "unlimitedDisabled")
                    }
                }
            ) : nil
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
            value = "~" + SendLive.feeLine(estimate, view: nil, fee: fee, display: context.display)
        } else if fee?.failed != nil {
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
                    mark: TokenMark(letter: String(option.symbol.prefix(1)).uppercased(),
                                    tint: context.chainDot),
                    name: option.symbol,
                    balance: "\(SendLive.trim(SendLive.fromBase(option.balance, decimals: option.decimals))) \(option.symbol)",
                    fee: option.amount.map {
                        "~\(SendLive.feeFromBase($0, decimals: option.decimals)) \(option.symbol)"
                    } ?? "—",
                    selected: option.selected,
                    disabled: option.insufficient
                )
            })
            : nil
        // Issue #262: the core shut the gate because the selected coin cannot
        // pay this fee — the send form's own sentence (#211), about the same
        // shortfall. A dark slide with no reason is issue 204.
        var warning: String?
        if let fee, fee.fee != nil, !fee.busy, fee.failed == nil, !fee.confirmFeeReady,
           let selected = fee.options.first(where: { $0.selected }), selected.insufficient {
            warning = context.loc.t("send.warnInsufficientGas", vars: ["sym": selected.symbol])
        }
        return .onchain(label: context.loc.t("componentsUi.gas.networkFee"), value: value,
                        selector: selector, warning: warning)
    }

    /// The slide's verb: the core's intent id, **in the corpus's words**.
    ///
    /// Printing the id raw is how Android shipped a button reading 确认send.
    static func confirmLabel(clear: ClearSigningViewWire, loc: Loc) -> String {
        switch clear.confirm {
        case .sign: s(loc, "signLabel")
        case .confirm: s(loc, "confirmLabel")
        case .confirmIntent(let intent):
            switch intent {
            case "send": s(loc, "confirmSend")
            case "swap": s(loc, "confirmSwap")
            case "deposit": s(loc, "confirmDeposit")
            case "withdraw": s(loc, "confirmWithdraw")
            default: s(loc, "confirmLabel")
            }
        }
    }
}

//
//  UserOpSpine.swift
//  VelaWallet
//
//  The submit spine — ONE implementation for a person's own transfer and a
//  dApp's transaction.
//
//  Ported from `app-android/.../feature/send/core/UserOpSpine.kt` (spec 044
//  T028), which is the desktop's `executor::user_op::submit`. The order below
//  is the file's content and is not to be improvised:
//
//      keys → the Tempo branch → isDeployed → nonce → floors → the quote gate
//      → a placeholder fee leg → draft → estimate → apply → the SETTLED fee
//      leg → the SafeOp hash IS the challenge → the assertion → the envelope
//      → the relay
//
//  The assembly is the core's (`user_op_*` through uniffi); the transports are
//  `RelayClient`'s; the one seam the core cannot have is `UserOpSigner`.
//  Nothing here prices, validates or classifies beyond what the core exports.
//
//  ## Two refusals happen BEFORE any prompt
//
//  A deployed wallet whose nonce could not be read, and a batch carrying a real
//  contract call whose gas could not be estimated. A passkey prompt on an
//  operation the relay must reject is a prompt wasted — and on this platform it
//  is also a Face ID sheet somebody has to dismiss for nothing.
//
//  ## Every signature follows the account's signing plan (spec 102)
//
//  A person says where their passkey is when they create the wallet or sign
//  in, and never again (founder, 2026-09-26). The core reads the stored record
//  (`signingPlan`) and answers three things: the key (credential, place,
//  transports — R5), the account's signing domain, and its VENUE — where its
//  transactions and messages are reviewed and signed (R4). There is no choice
//  at signing time; a record written before the sign-in key was kept signs as
//  it always did, pinned to its first key.
//
//  ## A trusted page is a venue, not a second path (specs 071, 102)
//
//  When the venue is a page, the SAME digest the passkey would have signed
//  goes to that page with the request it covers — the core builds what the
//  page receives (`trustedSignerRequest`, with the key route so the browser
//  asks for that key only) and verifies what comes back — and an accepted
//  answer continues exactly where an assertion does: `userOpSign`,
//  `eip1271Signature`. The page opens only once this phone's check of it was
//  admitted (R6). Every other ending is a `trustedSigner` failure the sheet
//  puts into words — in the corpus's words — and nothing is submitted.
//
//  ## Why `signMessage` lives here and 052 never calls it
//
//  A dApp's `personal_sign` (spec 053) signs the Safe message hash and wraps
//  the assertion in an EIP-1271 envelope. It belongs beside the transfer for
//  the same reason the two share a file on every other client: the sheet that
//  says what will happen and the code that makes it happen must not become two
//  opinions. Splitting it across two cuts is exactly how they would.
//

import Foundation
import VelaCore

extension UserOpSpine.AccountPort {
    /// Nothing known: the trusted page shows the address alone.
    func name(of address: String) async -> String? { nil }
}

@MainActor
final class UserOpSpine {

    /// The displayed fee, signed verbatim.
    struct Quoted {
        let amount: String
        let recipient: String
        /// The speed the displayed fee was priced at, named on the wire beside
        /// it (spec 069) — the core took it from the same estimate as `amount`.
        /// `nil` names nothing: the pre-068 wire.
        var tier: String? = nil
    }

    /// The account store, as the submit path reads it.
    protocol AccountPort {
        /// The wallet's founding keys for `address`, pinned key first; empty
        /// when unknown. **All** of them: the Safe's address is a function of
        /// the whole set, so a signature packed against a subset verifies
        /// against a different account.
        func keys(of address: String) async -> [WalletKeyRecord]
        /// The stored account record, whole, as JSON — what the core's
        /// `signingPlan` reads the account's key, domain and venue from.
        /// `nil` when no record holds `address`.
        func accountJson(of address: String) async -> String?
        /// The pinned key's stored transports and method, for the ceremony's
        /// routing of a record that names no sign-in key.
        func routing(of address: String) async -> (transports: String, method: KeyMethod)
        /// The account's own name, for the trusted page.
        func name(of address: String) async -> String?
    }

    /// What a site asked for, as the Trusted Signer's page is told it (spec
    /// 071): its own method and params, and the origin the browser observed.
    /// `nil` is the wallet's own send, which the core describes from its calls
    /// and the page draws as the wallet itself.
    struct Asked: Equatable {
        let method: String
        let paramsJson: String
        let origin: String
        /// Spec 079: the origin was read from this app's own browser engine
        /// (a page), not claimed — the page then names the site as the browser
        /// saw it instead of "未知站点". The wallet's own requests: `false`.
        var seenByBrowser = false
    }

    enum Failure: Equatable {
        case passkeyCancelled
        /// The passkey ceremony failed, as the passkey classifier read the
        /// platform's error — the same `FailureKind` create and login are
        /// told (spec 099 R8): no passkey can be used here, one sign-in would
        /// never offer, or another failure. Never `cancelled`, which is
        /// `passkeyCancelled`. Nothing was signed.
        case signer(kind: FailureKind, message: String)
        /// The trusted page ended without an answer the wallet accepts — or
        /// was never opened (its check refused it). Nothing was signed; the
        /// sheet says which of its sentences applies.
        case trustedSigner(TrustedSignerNotice)
        /// Spec 102: this account cannot sign here — nothing on this device
        /// can reach its keys (the plan's `blocked`). Nothing was signed, and
        /// trying again would not help: the machines are told `venue_blocked`
        /// and say why in the person's language.
        case venueBlocked(VenueBlockWire)
        case relayerUnavailable
        case bundlerUnderfunded
        case other(String?)
        /// Nothing left the device: the relay was never reached (the core's
        /// `NotSent{None}`, spec 082 RA1). The one failure after a signature
        /// that may honestly say "not sent — try again".
        case notSent
        /// The page that asked is gone (spec 082 RB2): nothing was signed or
        /// sent, and nobody is left to answer.
        case askerGone
        /// The relay refused the op and no earlier attempt can have delivered
        /// it (the core's `NotSent{Some(r)}`, `r` not "relayer unavailable",
        /// spec 082 RJ3): nothing was sent, and trying the same op again will
        /// not help — the page is answered "the network refused this".
        case rejected(String?)
        /// The relay refused it because another operation of this account
        /// holds the nonce (`RelayRejection::NonceHeld` — the relay's
        /// `nonce_in_flight`, or an older relay's `[existingHash:…]` marker;
        /// PR 2 §3). Nothing of this one went out. Send says "waiting for your
        /// last transaction on this network" with Try again; a page is told
        /// `PREVIOUS_PENDING_DETAIL`.
        case previousPending
    }

    /// What a submit came to when it did not fail (spec 082 RA1, RA4).
    struct Submitted: Equatable {
        /// The relay's hash when it answered; the local one otherwise.
        let userOpHash: String
        /// The reply was lost after a POST that may have been acted on: the
        /// op is followed as "may have been sent" to its end, and the local
        /// nonce does not move.
        let maybeSent: Bool
        /// The chain head read before the first POST (ruling 8); `nil` when
        /// the chain could not say.
        let submitBlock: UInt64?
    }

    struct Refused: Error {
        let failure: Failure
    }

    private let relay: RelayClient
    private let accounts: AccountPort
    private let signer: () -> UserOpSigner
    /// `eth_estimateGas({from, to, value, data})` on one chain — the gas hex, or
    /// `nil` when nobody answered. The inner calls' own measurement (spec 062):
    /// the relay estimates `callGasLimit` against the SENDER, and an undeployed
    /// Safe has no code there, so its figure is 21k plus calldata whatever the
    /// call does. Measured from the Safe's address instead, a codeless account
    /// estimates like any other. The rule is the core's (`userOpRaiseCallGas`).
    private let measureCall: @MainActor (_ chainId: Int, _ from: String, _ to: String, _ valueHex: String, _ data: String) async -> String?

    init(
        relay: RelayClient,
        accounts: AccountPort,
        signer: @escaping () -> UserOpSigner,
        measureCall: @escaping @MainActor (Int, String, String, String, String) async -> String? = { _, _, _, _, _ in nil }
    ) {
        self.relay = relay
        self.accounts = accounts
        self.signer = signer
        self.measureCall = measureCall
    }

    /// Every real contract call measured from the Safe's own address; the draft's
    /// `callGasLimit` raised to the core's floor when that is higher. A call nobody
    /// could measure leaves the relay's figure — and its existing guard — alone.
    private func raisedToMeasuredFloor(
        _ draft: UserOpDraft, chainId: Int, account: String, calls: [UserOpCall]
    ) async -> UserOpDraft {
        guard let toMeasure = try? userOpCallsToMeasure(calls: calls), !toMeasure.isEmpty else { return draft }
        var measured: [String] = []
        for index in toMeasure {
            let call = calls[Int(index)]
            let valueHex = call.value.hasPrefix("0x") ? call.value : "0x" + (Self.decimalToHex(call.value) ?? "0")
            guard let hex = await measureCall(chainId, account, call.to, valueHex, call.data),
                  let gas = UInt64(hex.dropFirst(hex.hasPrefix("0x") ? 2 : 0), radix: 16)
            else { return draft }
            measured.append(String(gas))
        }
        return (try? userOpRaiseCallGas(draft: draft, measured: measured, callCount: UInt32(calls.count))) ?? draft
    }

    /// A decimal wei string as bare hex. Values here are small enough for
    /// `UInt64`… except when they are not — then the measurement is skipped.
    static func decimalToHex(_ decimal: String) -> String? {
        decimal.isEmpty ? "0" : UInt64(decimal).map { String($0, radix: 16) }
    }

    private func other(_ message: String) -> Refused { Refused(failure: .other(message)) }

    // MARK: - A page's message (spec 053; ported now, called then)

    /// The Safe message hash under the Safe's own domain is the passkey's
    /// challenge; the assertion becomes the EIP-1271 envelope, which
    /// `isValidSignature` verifies on chain. One ceremony, nothing submitted.
    func signMessage(
        chainId: Int,
        account: String,
        originalHash: Data,
        asked: Asked? = nil,
        signingStarted: () -> Void = {},
        signingDone: () -> Void = {},
        askerLive: () -> Bool = { true }
    ) async throws -> String {
        let keys = await accounts.keys(of: account)
        guard let pinned = keys.first else {
            throw other("No passkey credential for the active account")
        }
        let challenge: Data
        do {
            challenge = try safeMessageHash(
                originalHash: originalHash, chainId: UInt64(chainId), safeAddress: account
            )
        } catch {
            throw other("The message could not be hashed")
        }
        // Never a prompt for a page that is gone (spec 082 RB2).
        guard askerLive() else { throw Refused(failure: .askerGone) }
        signingStarted()
        let signed = try await ceremony(
            account: account, pinned: pinned, keys: keys, challenge: challenge,
            buildRequest: { accountName, offered, keyRoute in
                // A message carries no calls and no operation: the page reads
                // the site's own params.
                guard let asked else { return nil }
                return try trustedSignerRequest(
                    input: Self.trustedSignerInput(
                        asked: asked, chainId: chainId, account: account,
                        accountName: accountName, keys: offered, keyRouteJson: keyRoute, calls: []
                    ),
                    draft: nil
                )
            }
        )
        signingDone()
        do {
            let signature = try eip1271Signature(
                assertion: signed.assertion,
                credentialId: signed.credentialIdHex,
                keys: keys
            )
            return "0x" + signature.map { String(format: "%02x", $0) }.joined()
        } catch {
            throw other("Failed to create signature")
        }
    }

    // MARK: - The submit

    /// Assembles, signs once and submits; answers the accepted user-operation
    /// hash — or the one already pending for this nonce, which is an idempotent
    /// re-submit rather than a second spend — or, when the relay's reply was
    /// lost after a request that may have been acted on, the LOCAL hash marked
    /// `maybeSent` (spec 082 RA1): never "failed — try again" for money that
    /// may be on its way.
    ///
    /// `signingStarted` / `signingDone` bracket the prompt (the sheet's
    /// "awaiting your signature", RA9); `askerLive` is asked right before the
    /// prompt and again right before the relay POST — a page that is gone
    /// gets nothing signed and nothing sent (RB2).
    ///
    /// `writeAhead` is the record before the bytes (spec 082 RJ1): called
    /// once the op is signed, its local hash computed and the head read, it
    /// hands both to the core (`op_signed`), which writes the record "may
    /// have been sent" and hands it to the tracker — and answers `true` only
    /// once the core has cleared the POST (`clear_to_post`). `false` (no
    /// clearance in `userOpWriteAheadWaitMs`) sends nothing: the op is
    /// `notSent`. There is no default: a submit that forgot it would put
    /// money on the wire with no record behind it (G34).
    func submit(
        chainId: Int,
        account: String,
        calls: [UserOpCall],
        gasFeeToken: String?,
        quotedFee: Quoted?,
        asked: Asked? = nil,
        signingStarted: () -> Void = {},
        signingDone: () -> Void = {},
        askerLive: () -> Bool = { true },
        writeAhead: (_ localHash: String, _ submitBlock: UInt64?) async -> Bool
    ) async throws -> Submitted {
        let keys = await accounts.keys(of: account)
        guard let pinned = keys.first else {
            throw other("No passkey credential for the active account")
        }
        // A submit, and the first quote after it, landed or not, measure the
        // chain again (issue 212).
        relay.invalidateFeeSignals(chainId: chainId)
        let tempo = isChainWithoutNativeCoin(chainId: UInt32(chainId))

        guard let deployed = await relay.isDeployed(chainId: chainId, address: account) else {
            throw other("The network could not be reached. Please try again.")
        }
        // Read before the prompt, and fatal: an operation with the wrong nonce
        // is one the relay must reject, and a Face ID sheet raised for it is a
        // sheet somebody dismisses for nothing.
        let nonce: String
        if deployed {
            guard let read = await relay.nonce(chainId: chainId, sender: account) else {
                throw other("The account's nonce could not be read. Please try again.")
            }
            nonce = read
        } else {
            nonce = "0x0"
        }
        // The head, read once before the first POST (ruling 8) — started now,
        // while the chain has just answered, and awaited at the POST.
        let chainReads = relay
        let headRead = Task { await chainReads.headBlock(chainId: chainId) }

        let floors = userOpFloors(
            chainId: UInt32(chainId), deployed: deployed, subCalls: UInt32(calls.count + 1)
        )

        // The displayed-equals-signed gate. `quoted_fee` is present ⇔ in-band,
        // and the core's own predicate decides whether it is still usable.
        guard let quoted = quotedFee, quotedFeeUsable(amount: quoted.amount, recipient: quoted.recipient) else {
            throw other("The fee quote has expired. Please review the updated fee and try again.")
        }

        let feeToken = tempo ? (gasFeeToken ?? Self.tempoDefaultFeeToken) : gasFeeToken
        let settled: UserOpFeeMode
        let placeholder: UserOpFeeMode
        if tempo {
            guard let collector = await relay.accountInfo(chainId: chainId, safe: account)?.feeRecipient else {
                throw other("The Tempo gas relayer is unavailable right now. Please try again.")
            }
            guard quoted.recipient.caseInsensitiveCompare(collector) == .orderedSame else {
                throw other("The gas quote has expired. Please review the updated fee and try again.")
            }
            settled = .tempo(feeToken: feeToken!, collector: collector, reimbursement: quoted.amount)
            placeholder = .tempo(feeToken: feeToken!, collector: collector, reimbursement: "1")
        } else {
            settled = .inBand(gasFeeToken: feeToken, amount: quoted.amount, recipient: quoted.recipient)
            // The placeholder exists so the estimate runs against an operation
            // the same SHAPE as the real one — one more leg, non-zero amounts —
            // without committing to the quoted figure before it is checked.
            placeholder = .inBand(gasFeeToken: feeToken, amount: "1", recipient: account)
        }

        var draft: UserOpDraft
        do {
            draft = try userOpDraft(
                sender: account,
                nonce: nonce,
                deployed: deployed,
                keyHexes: keys.map(\.publicKeyHex),
                calls: calls,
                fee: placeholder,
                floors: floors
            )
        } catch {
            throw other("The operation could not be assembled.")
        }

        let hasContractCall = (try? userOpHasContractCall(calls: calls)) ?? false
        let relayJson: String
        do {
            relayJson = try userOpRelayJson(draft: draft, feeToken: tempo ? feeToken : nil)
        } catch {
            throw other("The operation could not be encoded.")
        }
        switch await relay.estimateUserOpGas(chainId: chainId, opJson: relayJson) {
        case .estimated(let verification, let call, let preVerification, let settlement):
            // The core's one rule for the limits signed: the relay's as
            // returned (an undeployed Safe's floors; the 2M verification floor
            // only for a relay without `settlementGas`), then the inner calls'
            // own measured floor.
            if let applied = try? userOpApplyEstimate(
                draft: draft,
                verificationGasLimit: verification,
                callGasLimit: call,
                preVerificationGas: preVerification,
                settlementGas: settlement,
                floors: floors
            ) {
                draft = await raisedToMeasuredFloor(applied, chainId: chainId, account: account, calls: calls)
            }
        case .refused(let message):
            // What the refusal says, in the core's classes (spec 082 RJ19):
            // a revert the relay simulated, or nothing known. Logged only —
            // this sheet's own simulation already warned about a revert.
            let reading = userOpEstimateFailure(errorJson: CoreJSON.string(["message": message]))
            VelaLog.failure(.relay, kind: "estimate_refused", "chain=\(chainId) class=\(reading.kind)")
            if hasContractCall {
                throw other("Could not estimate gas for this transaction. The network may be busy — please try again.")
            }
        case .unreachable:
            // A plain transfer can ride the floors. A batch carrying a real
            // contract call cannot: its gas is unknowable without the estimate,
            // and submitting anyway spends a prompt on a rejection.
            if hasContractCall {
                throw other("Could not estimate gas for this transaction. The network may be busy — please try again.")
            }
        }

        do {
            draft = try userOpWithCalls(draft: draft, calls: calls, fee: settled)
        } catch {
            throw other("The operation could not be assembled.")
        }

        let challenge: Data
        do {
            challenge = try userOpSafeOpHash(draft: draft, chainId: UInt32(chainId))
        } catch {
            throw other("The operation could not be hashed.")
        }
        guard askerLive() else { throw Refused(failure: .askerGone) }
        signingStarted()
        let assembled = draft
        let answer = try await ceremony(
            account: account, pinned: pinned, keys: keys, challenge: challenge,
            buildRequest: { accountName, offered, keyRoute in
                // The ASSEMBLED operation — the one the digest covers — and
                // the calls before its fee leg, which the core appends last.
                try trustedSignerRequest(
                    input: Self.trustedSignerInput(
                        asked: asked, chainId: chainId, account: account,
                        accountName: accountName, keys: offered, keyRouteJson: keyRoute, calls: calls
                    ),
                    draft: assembled
                )
            }
        )
        signingDone()

        let signed: UserOpDraft
        do {
            signed = try userOpSign(
                draft: draft,
                assertion: answer.assertion,
                credentialId: answer.credentialIdHex,
                keys: keys
            )
        } catch {
            throw other("Failed to create signature")
        }

        let signedJson: String
        do {
            signedJson = try userOpRelayJson(draft: signed, feeToken: tempo ? feeToken : nil)
        } catch {
            throw other("The operation could not be encoded.")
        }

        // The operation's own hash, before anything is sent: what a
        // may-have-been-sent op is followed under (RA6). The relay's wins.
        let localHash: String
        do {
            localHash = try userOpHash(draft: signed, chainId: UInt32(chainId))
        } catch {
            throw other("The operation could not be hashed.")
        }
        // Bounded: a head that has not come back by now is unknown, and the
        // core scans below the head instead. The POST never waits on it.
        let submitBlock = await SignExecutor.within(seconds: 3) { await headRead.value } ?? nil

        // The record before the bytes (spec 082 RJ1): the core writes it "may
        // have been sent" and hands it to the tracker, then clears the POST.
        // No clearance, no POST — a quit, a crash or a closed window from
        // here on leaves a record the tracker resolves on the next launch.
        guard await writeAhead(localHash, submitBlock) else {
            VelaLog.failure(.relay, kind: "not_cleared", "hash=\(VelaLog.short(localHash)) chain=\(chainId)")
            throw Refused(failure: .notSent)
        }
        // A signature for a page that left while the prompt was up is never
        // sent (RB2) — asked after the clearance, right before the POST.
        guard askerLive() else { throw Refused(failure: .askerGone) }

        // The speed the displayed fee was priced at, named beside it (spec 069).
        // No local nonce moves here: this client reads `EntryPoint.getNonce`
        // on every submit (RA5), so a may-have-been-sent op's nonce is the
        // next attempt's too, and the EntryPoint lets at most one land.
        switch await relay.sendUserOp(
            chainId: chainId, opJson: signedJson, localHash: localHash, tier: quoted.tier
        ) {
        case .accepted(let hash):
            return Submitted(userOpHash: hash, maybeSent: false, submitBlock: submitBlock)
        case .maybeSent(let hash):
            return Submitted(userOpHash: hash, maybeSent: true, submitBlock: submitBlock)
        case .notSent(nil):
            throw Refused(failure: .notSent)
        case .notSent(.relayerUnavailable?):
            throw Refused(failure: .relayerUnavailable)
        case .notSent(.bundlerUnderfunded?):
            throw Refused(failure: .bundlerUnderfunded)
        case .notSent(.other(let text)?):
            throw Refused(failure: .rejected(text.isEmpty ? nil : text))
        case .notSent(.nonceHeld?):
            // 083: another operation of the account holds the nonce — its
            // hash is never this request's; nothing of this one went out.
            // PR 2: its own failure, so Send can say so (`previous_pending`).
            throw Refused(failure: .previousPending)
        }
    }

    // MARK: - The one ceremony

    /// The trusted page (specs 071, 102). `nil` where there is no screen to
    /// open a page on — an account whose venue is a page is then refused
    /// before anything is signed.
    var trustedSigner: TrustedSignerPort?

    /// One signature over `challenge`: the account's own passkey, or the
    /// trusted page's answer, which the core has already verified over this
    /// very digest by a key the page was offered. `buildRequest` makes what
    /// the page receives, given the account's name, the keys it is offered
    /// and the key route, and runs only when it is asked.
    private func ceremony(
        account: String,
        pinned: WalletKeyRecord,
        keys: [WalletKeyRecord],
        challenge: Data,
        buildRequest: (_ accountName: String?, _ offered: [WalletKeyRecord], _ keyRouteJson: String?) throws -> String?
    ) async throws -> (assertion: WebAuthnAssertion, credentialIdHex: String) {
        // The plan is the CORE's: the key the account signed in with, where
        // it lives, and where this account reviews and signs (R4, R5).
        let plan = await plan(account: account)
        if let blocked = plan?.blocked {
            // R1: nothing on this device can reach the account's keys. Said
            // by the sign and send machines in the person's language
            // (`venue_blocked`), never retried.
            throw Refused(failure: .venueBlocked(blocked))
        }
        guard let plan, let page = plan.venue.pageUrl else {
            let assertion = try await assert(account: account, pinned: pinned, key: plan?.key, challenge: challenge)
            return (Self.webAuthn(assertion), assertion.credentialIdHex)
        }
        guard let trustedSigner else {
            throw Refused(failure: .trustedSigner(.unavailable))
        }
        // An account with a sign-in key signs there with that key alone: the
        // page is offered it and no other, and an answer by any other
        // founding key is refused like any mismatch — the rule a passkey
        // ceremony pinned to it keeps. A record from before the sign-in key
        // offers every founding key, as it did.
        let offered = plan.key.map { key in
            keys.filter { $0.credentialId.caseInsensitiveCompare(key.credentialId) == .orderedSame }
        } ?? keys
        let accountName = await accounts.name(of: account)
        guard let request = try? buildRequest(accountName, offered, plan.key?.json) else {
            throw Refused(failure: .trustedSigner(.unavailable))
        }
        let ending = await trustedSigner.sign(
            requestJson: request, digest: challenge, keys: offered, page: page,
            keyLabel: plan.keyLabel
        )
        switch ending {
        case .outcome(.accepted(let credentialIdHex, let assertion)):
            return (assertion, credentialIdHex)
        case .outcome(.refused(let refusal)):
            throw Refused(failure: .trustedSigner(TrustedSignerNotice(refusal)))
        case .ceremony:
            // A signature was asked for; a ceremony verdict is a shell bug,
            // and an answer that is not the one asked for is a mismatch.
            throw Refused(failure: .trustedSigner(.mismatch))
        case .timedOut:
            throw Refused(failure: .trustedSigner(.timeout))
        case .unavailable:
            throw Refused(failure: .trustedSigner(.unavailable))
        case .cancelled:
            // Left on the hand-off card: nothing opened, like a cancelled
            // passkey sheet.
            throw Refused(failure: .passkeyCancelled)
        case .notOpened(let line):
            throw Refused(failure: .trustedSigner(.notOpened(line)))
        }
    }

    /// Where this account's signatures go — the core's plan, the one
    /// `ceremony` follows. The signing sheet reads it too, so the sheet that
    /// says "review on your trusted page" and the ceremony that opens it
    /// cannot disagree. `nil` for an account with no readable record.
    func plan(account: String) async -> SigningPlanWire? {
        await accounts.accountJson(of: account).flatMap(SigningPlanWire.of(accountJson:))
    }

    /// What the page is told: a site's request as it asked, or — `asked`
    /// absent — the wallet's own send, which the core builds from `calls`.
    static func trustedSignerInput(
        asked: Asked?,
        chainId: Int,
        account: String,
        accountName: String?,
        keys: [WalletKeyRecord],
        keyRouteJson: String? = nil,
        calls: [UserOpCall]
    ) -> TrustedSignerInput {
        let chain = ChainCatalog.meta(chainId)
        return TrustedSignerInput(
            method: asked?.method ?? "",
            paramsJson: asked?.paramsJson ?? "",
            origin: asked?.origin ?? "",
            originSeenByBrowser: asked?.seenByBrowser ?? false,
            chainId: UInt32(chainId),
            chainName: chain?.displayName,
            nativeSymbol: chain?.nativeSymbol,
            account: account,
            accountName: accountName,
            credentialIdsHex: keys.map(\.credentialId),
            keyRouteJson: keyRouteJson,
            calls: calls
        )
    }

    private func assert(
        account: String,
        pinned: WalletKeyRecord,
        key: KeyRouteWire?,
        challenge: Data
    ) async throws -> Assertion {
        let routing: (credentialId: String, transports: String, method: KeyMethod)
        if let key, !key.credentialId.isEmpty, let place = key.place {
            routing = (key.credentialId, key.transports, place)
        } else {
            let stored = await accounts.routing(of: account)
            routing = (pinned.credentialId, stored.transports, stored.method)
        }
        do {
            return try await signer().sign(
                challenge: challenge,
                credentialIdHex: routing.credentialId,
                transports: routing.transports,
                method: routing.method
            )
        } catch let failure as PasskeyFailure {
            if failure.kind == .cancelled { throw Refused(failure: .passkeyCancelled) }
            // Spec 099 R8: how it failed travels on, so the sheet and the
            // page's record name the passkey, not "couldn't submit".
            throw Refused(failure: .signer(
                kind: failure.kind,
                message: failure.message.isEmpty ? "the passkey ceremony failed" : failure.message
            ))
        } catch is CancellationError {
            throw Refused(failure: .passkeyCancelled)
        }
    }

    private static func webAuthn(_ assertion: Assertion) -> WebAuthnAssertion {
        WebAuthnAssertion(
            authenticatorData: unhex(assertion.authenticatorDataHex),
            clientDataJson: unhex(assertion.clientDataJsonHex),
            signatureDer: unhex(assertion.signatureDerHex)
        )
    }

    /// `fee_policy::TEMPO_DEFAULT_FEE_TOKEN` — pathUSD.
    static let tempoDefaultFeeToken = "0x20c0000000000000000000000000000000000000"

    static func unhex(_ text: String) -> Data {
        var body = Substring(text)
        if body.hasPrefix("0x") || body.hasPrefix("0X") { body = body.dropFirst(2) }
        var out = Data()
        var index = body.startIndex
        while index < body.endIndex {
            let next = body.index(index, offsetBy: 2, limitedBy: body.endIndex) ?? body.endIndex
            guard next > index, let byte = UInt8(body[index..<next], radix: 16) else { break }
            out.append(byte)
            index = next
        }
        return out
    }
}

/// The core's clearance for one POST (spec 082 RJ1), as the executors wait
/// for it: `clear_to_post` opens it for an op hash, `wait` answers whether it
/// opened in time. A gate opened for an op nobody waits on is forgotten by
/// the next `arm` of that hash.
@MainActor
final class WriteAheadGate {
    private var cleared: Set<String> = []

    /// Forget any earlier clearance for `hash` — before `op_signed` goes out,
    /// so a stale one can never let a new POST through.
    func arm(_ hash: String) { cleared.remove(hash.lowercased()) }

    /// The core cleared the POST of `hash`.
    func open(_ hash: String) {
        guard !hash.isEmpty else { return }
        cleared.insert(hash.lowercased())
    }

    /// `true` once `hash` is cleared; `false` at `ms`, or when the waiting
    /// task is cancelled (a cancel before the POST sends nothing).
    ///
    /// `ms` is `nil` under a test's stopped clock (`FeeStore.Timers`): no
    /// time passes, and the wait ends by the clearance or the cancel alone.
    /// The deadline is on the wall clock and the clearance is a few turns of
    /// the main actor away — the record's write, then `clear_to_post`, each
    /// an effect of its own. On a main actor a test run had filled, those
    /// turns took longer than the core's five seconds, and a scripted relay
    /// that answers every call was reported unreachable: a failure the code
    /// never had, measured on a clock (`Waits.swift`).
    func wait(_ hash: String, ms: Double?) async -> Bool {
        let key = hash.lowercased()
        let deadline = ms.map { Date().addingTimeInterval($0 / 1000) }
        while !cleared.contains(key) {
            if Task.isCancelled { return false }
            if let deadline, Date() >= deadline { return false }
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
        cleared.remove(key)
        return true
    }
}

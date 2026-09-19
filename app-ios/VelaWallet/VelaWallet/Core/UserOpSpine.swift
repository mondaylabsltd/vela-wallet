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
    /// Nothing known: the ceremony routes as it always did.
    func keyRoutesJson(of address: String) async -> String { "[]" }
}

@MainActor
final class UserOpSpine {

    /// The displayed fee, signed verbatim.
    struct Quoted {
        let amount: String
        let recipient: String
    }

    /// The account store, as the submit path reads it.
    protocol AccountPort {
        /// The wallet's founding keys for `address`, pinned key first; empty
        /// when unknown. **All** of them: the Safe's address is a function of
        /// the whole set, so a signature packed against a subset verifies
        /// against a different account.
        func keys(of address: String) async -> [WalletKeyRecord]
        /// The pinned key's stored transports and method, for the ceremony's
        /// routing.
        func routing(of address: String) async -> (transports: String, method: KeyMethod)
        /// Every founding key's credential id and stored transports, as JSON for
        /// the core's `signRoute` — `[{credential_id, transports}]`.
        func keyRoutesJson(of address: String) async -> String
    }

    enum Failure: Equatable {
        case passkeyCancelled
        case relayerUnavailable
        case bundlerUnderfunded
        case other(String?)
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
    private let measureCall: (_ chainId: Int, _ from: String, _ to: String, _ valueHex: String, _ data: String) async -> String?

    init(
        relay: RelayClient,
        accounts: AccountPort,
        signer: @escaping () -> UserOpSigner,
        measureCall: @escaping (Int, String, String, String, String) async -> String? = { _, _, _, _, _ in nil }
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
    private static func decimalToHex(_ decimal: String) -> String? {
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
        signingStarted: () -> Void = {}
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
        signingStarted()
        let assertion = try await assert(account: account, pinned: pinned, challenge: challenge)
        do {
            let signature = try eip1271Signature(
                assertion: Self.webAuthn(assertion),
                credentialId: assertion.credentialIdHex,
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
    /// re-submit rather than a second spend.
    func submit(
        chainId: Int,
        account: String,
        calls: [UserOpCall],
        gasFeeToken: String?,
        quotedFee: Quoted?,
        signingStarted: () -> Void = {}
    ) async throws -> String {
        let keys = await accounts.keys(of: account)
        guard let pinned = keys.first else {
            throw other("No passkey credential for the active account")
        }
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
        case .estimated(let verification, let call, let preVerification):
            if let applied = try? userOpApplyEstimate(
                draft: draft,
                verificationGasLimit: verification,
                callGasLimit: call,
                preVerificationGas: preVerification,
                floors: floors
            ) {
                draft = await raisedToMeasuredFloor(applied, chainId: chainId, account: account, calls: calls)
            }
        case .refused, .unreachable:
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
        signingStarted()
        let assertion = try await assert(account: account, pinned: pinned, challenge: challenge)

        let signed: UserOpDraft
        do {
            signed = try userOpSign(
                draft: draft,
                assertion: Self.webAuthn(assertion),
                credentialId: assertion.credentialIdHex,
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

        switch await relay.sendUserOp(chainId: chainId, opJson: signedJson) {
        case .accepted(let hash):
            return hash
        case .unreachable:
            throw other("The gas relayer could not be reached. Please try again.")
        case .rejected(let errorJson):
            let message = relayErrorMessage(errorJson: errorJson)
            // Already pending for this nonce: the same operation, not a second
            // one. Answering its hash lets the tracker follow what is really on
            // the wire instead of submitting a duplicate.
            //
            // **The RAW json is searched first, and that is a deviation from
            // the other clients.** `relay_error_message` is a translator, not a
            // pass-through: a message matching a known rung ("AA25", "invalid
            // account nonce") is REPLACED wholesale by its human sentence, and
            // the relay's `[existingHash:0x…]` marker goes with it. A second
            // submit for the same nonce is exactly an AA25 condition, so the
            // one case where the marker matters most is the one where the
            // translation can eat it. Android and the desktop parse only the
            // translated text; this is reported rather than quietly diverged.
            if let existing = parseExistingUserOpHash(message: errorJson)
                ?? parseExistingUserOpHash(message: message) {
                return existing
            }
            switch classifyRelayRejection(message: message) {
            case .relayerUnavailable: throw Refused(failure: .relayerUnavailable)
            case .bundlerUnderfunded: throw Refused(failure: .bundlerUnderfunded)
            case .other(let text): throw Refused(failure: .other(text.isEmpty ? nil : text))
            }
        }
    }

    // MARK: - The one ceremony

    /// The person's "Sign with" choice for the request in hand — `auto` unless a
    /// signing sheet says otherwise (the spine is shared with Send, which never
    /// sets it). WHICH key that pins, and how it is reached, is the core's.
    var signMethod: () -> String = { "auto" }

    /// The credential a ceremony is pinned to, its transports and method: the
    /// person's choice when they made one, the stored route otherwise.
    private func route(account: String, first: WalletKeyRecord) async -> (credentialId: String, transports: String, method: KeyMethod) {
        let chosen = signMethod()
        if chosen != "auto",
           let json = signRoute(deviceKeysJson: await accounts.keyRoutesJson(of: account), method: chosen),
           let picked = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any],
           let credential = picked["credential_id"] as? String, !credential.isEmpty,
           let method = (picked["method"] as? String).flatMap(KeyMethod.init(rawValue:))
        {
            return (credential, picked["transports"] as? String ?? "", method)
        }
        let stored = await accounts.routing(of: account)
        return (first.credentialId, stored.transports, stored.method)
    }

    private func assert(
        account: String,
        pinned: WalletKeyRecord,
        challenge: Data
    ) async throws -> Assertion {
        let routing = await route(account: account, first: pinned)
        do {
            return try await signer().sign(
                challenge: challenge,
                credentialIdHex: routing.credentialId,
                transports: routing.transports,
                method: routing.method
            )
        } catch let failure as PasskeyFailure {
            if failure.kind == .cancelled { throw Refused(failure: .passkeyCancelled) }
            throw other(failure.message.isEmpty ? "the passkey ceremony failed" : failure.message)
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

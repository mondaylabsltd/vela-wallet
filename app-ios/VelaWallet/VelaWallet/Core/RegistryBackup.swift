//
//  RegistryBackup.swift
//  VelaWallet
//
//  "Is this wallet's record on Ethereum too?" — the iOS transport for
//  `vela_core::registry_backup` (spec 062).
//
//  A wallet is registered on Gnosis when it is created. The registry stores
//  the registration's calldata verbatim and signs over a frozen domain, so the
//  same bytes replay on any deployment of it with no passkey ceremony: the
//  contract re-verifies every signature, and a wrong payload costs a reverted
//  estimate, never funds. WHICH reads prove that, in what order, and what a
//  silent chain means are the core's, shared with the other three shells. What
//  is left here is what only a shell can do: perform the `eth_call`s.
//
//  What the Keys block DRAWS for the answer is the core's too (PR 3): the
//  finished step carries a `row` — its words, its tone and what a tap does —
//  and this file hands it on unread. Each shell used to map the state itself,
//  and iOS drew "could not check" with nothing to tap.
//

import Foundation
import VelaCore

@MainActor
final class RegistryBackup {
    enum State: Equatable {
        /// No registry on the target chain: the row is not drawn.
        case unavailable
        /// The wallet was never registered at home: nothing to copy.
        case notRegistered
        case backedUp
        case notBackedUp
        /// Somebody did not answer. Never drawn as "not backed up"; asking
        /// again may answer it.
        case couldNotCheck
        /// Gnosis answered, and what it holds can never be copied (a wallet
        /// from before registry V13 stored no payload). Asking again gets the
        /// same answer: a calm end, nothing to tap.
        case notCopyable
    }

    /// The Keys block's row for a finished check, as the core words it
    /// (`BackupState::row`): corpus keys, a tone, and what a tap does.
    struct Row: Equatable {
        enum Tone: String { case neutral, positive }
        enum Action: String {
            /// A statement: nothing to tap.
            case none
            /// Open the signing sheet with the check's `call`.
            case copy
            /// Run the walk again.
            case retry
        }

        let titleKey: String
        let subtitleKey: String
        let tone: Tone
        let action: Action

        /// The row for a silence THIS file caused — a step that could not be
        /// read, a walk that ran out of rounds — which the core never saw to
        /// word. Its own "could not check" row, in its own keys.
        static let couldNotCheck = Row(
            titleKey: "settingsModals.backup.title",
            subtitleKey: "settingsModals.backup.couldNotCheck",
            tone: .neutral, action: .retry
        )

        /// A tone or an action this build does not know reads as the quiet
        /// one: a neutral line, nothing to tap.
        init?(json: [String: Any]?) {
            guard let json, let title = json["title_key"] as? String, !title.isEmpty,
                  let subtitle = json["subtitle_key"] as? String, !subtitle.isEmpty
            else { return nil }
            self.init(
                titleKey: title, subtitleKey: subtitle,
                tone: (json["tone"] as? String).flatMap(Tone.init) ?? .neutral,
                action: (json["action"] as? String).flatMap(Action.init) ?? Action.none
            )
        }

        init(titleKey: String, subtitleKey: String, tone: Tone, action: Action) {
            self.titleKey = titleKey
            self.subtitleKey = subtitleKey
            self.tone = tone
            self.action = action
        }
    }

    /// The one transaction that makes the backup — the registry's `register`,
    /// with the payload Gnosis already holds.
    struct Call: Equatable {
        let chainId: Int
        let to: String
        let data: String
    }

    struct Check: Equatable {
        let state: State
        /// Present only with `.notBackedUp`.
        let call: Call?
        /// What the Keys block draws; `nil` where no row is drawn (no
        /// registry on Ethereum, or no record to copy).
        var row: Row?
    }

    /// A check is a handful of rounds; this only stops a contract bug from spinning.
    private static let maxRounds = 16

    /// The RAW `result` hex — a bare `0x` included, which is a chain saying
    /// "no registry here" — or `nil` when nobody answered.
    private let ethCall: @MainActor (_ chainId: Int, _ to: String, _ data: String) async -> String?
    /// `registryBackupStep` from the core; a seam so tests can script it.
    private let step: (_ address: String, _ key: String, _ answersJson: String, _ target: UInt32?) -> String

    init(
        ethCall: @escaping @MainActor (Int, String, String) async -> String?,
        step: @escaping (String, String, String, UInt32?) -> String = {
            registryBackupStep(address: $0, foundingPublicKeyHex: $1, answersJson: $2, targetChain: $3)
        }
    ) {
        self.ethCall = ethCall
        self.step = step
    }

    /// The wallet's FIRST founding key, as the account record keeps it — the
    /// stored order is the founding order. Empty when the record has none (the
    /// dev seed), and then there is nothing to look up.
    static func foundingKeyHex(of address: String, in accounts: UserOpSpine.AccountPort) async -> String {
        await accounts.keys(of: address).first?.publicKeyHex ?? ""
    }

    func check(address: String, foundingKeyHex: String, targetChain: Int? = nil) async -> Check {
        let silent = Check(state: .couldNotCheck, call: nil, row: .couldNotCheck)
        var answers: [[String: Any]] = []
        for _ in 0..<Self.maxRounds {
            guard let transcript = try? JSONSerialization.data(withJSONObject: answers),
                  let next = try? JSONSerialization.jsonObject(with: Data(step(
                      address, foundingKeyHex, String(decoding: transcript, as: UTF8.self),
                      targetChain.map(UInt32.init)
                  ).utf8)) as? [String: Any]
            else { return silent }

            if next["type"] as? String == "ask" {
                let requests = next["requests"] as? [[String: Any]] ?? []
                let round = await withTaskGroup(of: [String: Any].self) { group in
                    for request in requests {
                        group.addTask { [weak self] in await self?.perform(request) ?? [:] }
                    }
                    var collected: [[String: Any]] = []
                    for await answer in group where !answer.isEmpty { collected.append(answer) }
                    return collected
                }
                answers.append(contentsOf: round)
                continue
            }

            let call = (next["call"] as? [String: Any]).flatMap { raw -> Call? in
                guard let chainId = (raw["chain_id"] as? NSNumber)?.intValue,
                      let to = raw["to"] as? String, let data = raw["data"] as? String
                else { return nil }
                return Call(chainId: chainId, to: to, data: data)
            }
            // The core's row for its own verdict, handed on as it came.
            let row = Row(json: next["row"] as? [String: Any])
            switch next["state"] as? String {
            case "unavailable": return Check(state: .unavailable, call: nil)
            case "not_registered": return Check(state: .notRegistered, call: nil)
            case "backed_up": return Check(state: .backedUp, call: nil, row: row)
            // "Not backed up" with nothing to send is not something to show a fee for.
            case "not_backed_up": return call.map { Check(state: .notBackedUp, call: $0, row: row) } ?? silent
            case "could_not_check": return Check(state: .couldNotCheck, call: nil, row: row ?? .couldNotCheck)
            // Its own state, never "could not check": it would retry for ever.
            case "not_copyable": return Check(state: .notCopyable, call: nil, row: row)
            // A state this build has never heard of: not a verdict.
            default: return silent
            }
        }
        return silent
    }

    private func perform(_ request: [String: Any]) async -> [String: Any] {
        let id = request["id"] as? String ?? ""
        guard request["type"] as? String == "eth_call",
              let chainId = (request["chain_id"] as? NSNumber)?.intValue,
              let to = request["to"] as? String, let data = request["data"] as? String,
              let result = await ethCall(chainId, to, data)
        else { return ["id": id, "outcome": "failed", "body": NSNull()] }
        return ["id": id, "outcome": "ok", "body": result]
    }
}

//
//  AgentLinkSession.swift
//  VelaWallet
//
//  Connecting the AI agent with clientmcp.dev (spec 106): the person signs in
//  there, and the wallet comes back with its bridge address and client token.
//
//  The protocol's every decision is the core's (`clientmcpLink*`, the same
//  functions every shell calls). This is the phone's part: the system's
//  sign-in sheet (`ASWebAuthenticationSession`), which catches the return
//  address `app.getvela.wallet:/clientmcp` itself — nothing is registered in
//  Info.plist — and shares Safari's cookies, so a GitHub sign-in there is
//  used again; and the one request that trades the code.
//

import AuthenticationServices
import Foundation
import Security
import UIKit
import VelaCore

/// What clientmcp.dev issued: applied as the person's `configure_agent`.
struct AgentLinked: Equatable {
    let bridgeUrl: String
    let clientToken: String
    let clientId: String
    let login: String
    /// The MCP server address for the person's AI agent (a credential).
    let agentUrl: String?
}

/// Why connecting did not work.
enum AgentLinkProblem: Equatable, Error {
    /// The site said no, in its words.
    case refused(String)
    case unavailable
    case expired
    case notOurs
    case network
    case failed
    /// The person closed the sheet.
    case cancelled

    /// The core's `{kind, detail}`.
    init(core: [String: Any]) {
        switch core["kind"] as? String {
        case "refused": self = .refused(core["detail"] as? String ?? "")
        case "unavailable": self = .unavailable
        case "expired": self = .expired
        case "not_ours": self = .notOurs
        default: self = .failed
        }
    }

    func words(_ loc: Loc) -> String {
        switch self {
        case .refused(let reason):
            return loc.t("settings.agent.linkRefused").replacingOccurrences(of: "{{reason}}", with: reason)
        case .unavailable: return loc.t("settings.agent.linkUnavailable")
        case .expired: return loc.t("settings.agent.linkExpired")
        case .notOurs: return loc.t("settings.agent.linkNotOurs")
        case .network: return loc.t("settings.agent.linkNetwork")
        case .failed, .cancelled: return loc.t("settings.agent.linkFailed")
        }
    }
}

@MainActor
final class AgentLinkSession: NSObject, ASWebAuthenticationPresentationContextProviding {

    /// clientmcp.dev, unless a development build names another
    /// (`VELA_CLIENTMCP_SITE`, Info.plist's `VelaClientMCPSite`).
    static var site: String {
        let named = Bundle.main.object(forInfoDictionaryKey: "VelaClientMCPSite") as? String ?? ""
        return named.hasPrefix("https://") ? named : "https://clientmcp.dev"
    }

    private static let installationKey = "vela.agent.installation"

    /// This installation's ID: made once, kept (not a secret).
    static func installation(_ shelf: VelaStore) -> String {
        if let kept = shelf.readString(installationKey), !kept.isEmpty { return kept }
        let made = clientmcpInstallationId(random: random(16))
        shelf.writeString(installationKey, made)
        return made
    }

    private static func random(_ count: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: count)
        _ = SecRandomCopyBytes(kSecRandomDefault, count, &bytes)
        return Data(bytes)
    }

    private var session: ASWebAuthenticationSession?

    /// Signs in on clientmcp.dev and trades the code. Throws
    /// `AgentLinkProblem`; `.cancelled` when the person closed the sheet.
    func run(shelf: VelaStore) async throws -> (AgentLinked, site: String) {
        let site = Self.site
        let begun = try Self.answer(clientmcpLinkBegin(
            site: site,
            redirectUri: clientmcpAppRedirect(),
            label: UIDevice.current.model,
            installation: Self.installation(shelf),
            random: Self.random(64),
            nowMs: Self.now()
        ))
        guard let pending = begun as? [String: Any],
              let authorize = (pending["authorize_url"] as? String).flatMap(URL.init(string:))
        else { throw AgentLinkProblem.failed }
        let pendingJson = CoreJSON.string(pending)

        let returned = try await signIn(at: authorize)
        let code = try Self.answer(clientmcpLinkReturned(
            pendingJson: pendingJson, url: returned.absoluteString, nowMs: Self.now()
        ))
        guard let code = code as? String,
              let request = try Self.answer(clientmcpLinkExchange(pendingJson: pendingJson, code: code))
                as? [String: Any],
              let url = (request["url"] as? String).flatMap(URL.init(string:)),
              let body = request["body"] as? String
        else { throw AgentLinkProblem.failed }

        var post = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 30)
        post.httpMethod = "POST"
        post.setValue("application/json", forHTTPHeaderField: "content-type")
        post.httpBody = Data(body.utf8)
        post.httpShouldHandleCookies = false
        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await URLSession.shared.data(for: post)
        } catch {
            throw AgentLinkProblem.network
        }
        let status = UInt16((response as? HTTPURLResponse)?.statusCode ?? 0)
        let linked = try Self.answer(clientmcpLinkAccept(
            status: status, body: String(decoding: data, as: UTF8.self)
        ))
        guard let linked = linked as? [String: Any],
              let bridgeUrl = linked["bridge_url"] as? String,
              let clientToken = linked["client_token"] as? String,
              let clientId = linked["client_id"] as? String,
              let login = linked["login"] as? String
        else { throw AgentLinkProblem.failed }
        let agentUrl = linked["agent_url"] as? String
        return (
            AgentLinked(bridgeUrl: bridgeUrl, clientToken: clientToken, clientId: clientId, login: login, agentUrl: agentUrl),
            site
        )
    }

    /// The person closed the app's interest in the sheet (Settings left).
    func cancel() { session?.cancel() }

    private func signIn(at url: URL) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: url,
                callbackURLScheme: "app.getvela.wallet"
            ) { returned, error in
                if let returned {
                    continuation.resume(returning: returned)
                } else if let error = error as? ASWebAuthenticationSessionError,
                          error.code == .canceledLogin {
                    continuation.resume(throwing: AgentLinkProblem.cancelled)
                } else {
                    continuation.resume(throwing: AgentLinkProblem.network)
                }
            }
            session.presentationContextProvider = self
            // Safari's cookies: a GitHub sign-in there is used again.
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: AgentLinkProblem.failed)
            }
        }
    }

    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            let window = UIApplication.shared.connectedScenes
                .compactMap { $0 as? UIWindowScene }
                .first { $0.activationState == .foregroundActive }?
                .windows
                .first { $0.isKeyWindow }
            return window ?? ASPresentationAnchor()
        }
    }

    /// The core's `{"ok": …}`, or its `{"error": …}` thrown.
    private static func answer(_ json: String) throws -> Any {
        let object = (try? CoreJSON.object(json)) ?? [:]
        if let error = object["error"] as? [String: Any] { throw AgentLinkProblem(core: error) }
        guard let ok = object["ok"] else { throw AgentLinkProblem.failed }
        return ok
    }

    private static func now() -> UInt64 { UInt64(Date().timeIntervalSince1970 * 1000) }
}

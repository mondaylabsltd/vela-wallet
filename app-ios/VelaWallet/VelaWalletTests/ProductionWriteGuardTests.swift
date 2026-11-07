//
//  ProductionWriteGuardTests.swift
//  VelaWalletTests
//
//  §28: a test build never writes to production. Proven without reaching a
//  production service: the production host names are answered on this
//  machine by `Loopback`, a URL protocol that stands where the service would
//  be (the names mapped to loopback). The guard comes first in the session;
//  what it lets through reaches `Loopback`, what it refuses never does.
//  The vector `tests/vectors/rules/production-writes.json` is replayed
//  through this shell's door to the core's rule.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// The production names, answered here: records each request and says 200.
nonisolated final class Loopback: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var seen: [String] = []
    private static let lock = NSLock()

    static func take() -> [String] { lock.withLock { defer { seen = [] }; return seen } }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        Self.lock.withLock { Self.seen.append("\(request.httpMethod ?? "GET") \(request.url?.host ?? "") \(request.url?.path ?? "")") }
        let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["content-type": "application/json"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data("{\"ok\":true}".utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

@Suite(.serialized)
struct ProductionWriteGuardTests {

    private static let vectorURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/rules/production-writes.json")

    /// A session as the app makes one (`URLSession.vela`), the guard first,
    /// and the production names answered by `Loopback`.
    private func session() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [Loopback.self]
        ProductionWriteGuard.guarding(configuration)
        return URLSession(configuration: configuration)
    }

    private func post(_ url: String, _ body: String) async throws -> Int {
        var request = URLRequest(url: URL(string: url)!)
        request.httpMethod = "POST"
        request.httpBody = Data(body.utf8)
        let (_, response) = try await session().data(for: request)
        return (response as? HTTPURLResponse)?.statusCode ?? 0
    }

    private func with<T>(_ policy: ProductionWriteGuard.Policy, _ body: () async throws -> T) async rethrows -> T {
        let before = ProductionWriteGuard.policy
        let forwarding = ProductionWriteGuard.forwarding
        ProductionWriteGuard.policy = policy
        // What the guard sends on goes to the loopback stand-in too, never out.
        let loopback = URLSessionConfiguration.ephemeral
        loopback.protocolClasses = [Loopback.self]
        ProductionWriteGuard.forwarding = URLSession(configuration: loopback)
        defer {
            ProductionWriteGuard.policy = before
            ProductionWriteGuard.forwarding = forwarding
        }
        _ = Loopback.take()
        return try await body()
    }

    private static let register = "https://p256-index-v2.getvela.app/api/register"
    private static let testBuild = ProductionWriteGuard.Policy(testBuild: true, allowProductionWrites: false)
    private static let release = ProductionWriteGuard.Policy(testBuild: false, allowProductionWrites: false)

    @Test func aTestBuildRefusesAProductionWriteBeforeItLeaves() async throws {
        try await with(Self.testBuild) {
            do {
                _ = try await post(Self.register, "{\"members\":[]}")
                Issue.record("a test build's write to the key index went out")
            } catch {
                #expect(ProductionWriteRefused.matches(error))
                #expect(error.localizedDescription.contains("This test build won't write to the live service"))
                #expect((error as? URLError)?.code == .cannotConnectToHost)
            }
            #expect(Loopback.take().isEmpty, "nothing reached the (loopback) index")
        }
    }

    @Test func aTestBuildStillReadsAndWritesLocally() async throws {
        try await with(Self.testBuild) {
            let (_, _) = try await session().data(from: URL(string: "https://p256-index-v2.getvela.app/api/health")!)
            #expect(Loopback.take() == ["GET p256-index-v2.getvela.app /api/health"])
            // A JSON-RPC read to the relay is a read: taken by the guard,
            // judged on its body, and sent on.
            let read = #"{"jsonrpc":"2.0","id":1,"method":"eth_estimateUserOperationGas","params":[]}"#
            let status1 = try await post("https://vela-relay-cf.getvela.app/100", read)

            #expect(status1 == 200)
            #expect(Loopback.take() == ["POST vela-relay-cf.getvela.app /100"])
            let send = #"{"jsonrpc":"2.0","id":2,"method":"eth_sendUserOperation","params":[]}"#
            await #expect(throws: (any Error).self) { _ = try await post("https://vela-relay-cf.getvela.app/100", send) }
            #expect(Loopback.take().isEmpty)
            // A local stand-in takes writes.
            let status2 = try await post("http://127.0.0.1:9566/api/register", "{}")

            #expect(status2 == 200)
            #expect(Loopback.take() == ["POST 127.0.0.1 /api/register"])
        }
    }

    @Test func theOptInLetsATestBuildWrite() async throws {
        try await with(ProductionWriteGuard.Policy(testBuild: true, allowProductionWrites: true)) {
            let status3 = try await post(Self.register, "{}")

            #expect(status3 == 200)
            #expect(Loopback.take() == ["POST p256-index-v2.getvela.app /api/register"])
        }
    }

    /// A release build: the guard takes nothing (and is not even compiled
    /// into one — `EXCLUDED_SOURCE_FILE_NAMES`, `check-ios-store-binary.sh`),
    /// and its write leaves as before, here to the loopback stand-in.
    @Test func aReleaseBuildIsUnaffected() async throws {
        #expect(!isTestBuild(devFixtures: false, deviceTest: false, debug: false))
        try await with(Self.release) {
            var request = URLRequest(url: URL(string: Self.register)!)
            request.httpMethod = "POST"
            #expect(!ProductionWriteGuard.canInit(with: request))
            let status4 = try await post(Self.register, "{\"members\":[]}")

            #expect(status4 == 200)
            #expect(Loopback.take() == ["POST p256-index-v2.getvela.app /api/register"])
        }
    }

    /// This (Debug) build is a test build, the device-test build too.
    @Test func thisDebugBuildIsATestBuild() {
        #expect(ProductionWriteGuard.thisBuild.testBuild)
        #expect(isTestBuild(devFixtures: false, deviceTest: true, debug: false))
    }

    /// The key index client says the refusal as what it is: not the network.
    @Test func theKeyIndexClientSaysItHonestly() async throws {
        try await with(Self.testBuild) {
            let guarded = session()
            let client = RegistryClient(baseURL: "https://p256-index-v2.getvela.app", transport: { request in
                try await guarded.data(for: request)
            })
            do {
                _ = try await client.memberChallenge(groupPublicKey: "0x04", publicKey: "0x04", attestation: "", rpId: "getvela.app")
                Issue.record("the challenge went out")
            } catch let failure as RegistryFailure {
                #expect(!failure.network)
                #expect(failure.message.contains("This test build won't write to the live service"))
            }
            #expect(Loopback.take().isEmpty)
        }
    }

    @Test func theDefaultsAreTheCores() throws {
        let data = try Data(contentsOf: Self.vectorURL)
        let vector = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let defaults = try #require(vector["defaults"] as? [String: String])
        let core = CoreServiceDefaults.value
        #expect(defaults["passkeyIndexUrl"] == core.passkeyIndexUrl)
        #expect(defaults["genesisServiceUrl"] == core.genesisServiceUrl)
        #expect(RegistryClient.defaultURL == core.passkeyIndexUrl)
        #expect(MultisigServiceURL.defaultURL == core.genesisServiceUrl)
        #expect(BugReport.endpoint == core.bugReportUrl)
        #expect(NetDefaults.bundlerServiceURL == core.bundlerServiceUrl)
    }

    @Test func everyVectorCaseAnswersAsTheCoreSays() throws {
        let data = try Data(contentsOf: Self.vectorURL)
        let vector = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let cases = try #require(vector["cases"] as? [[String: Any]])
        for c in cases {
            var request = URLRequest(url: URL(string: c["url"] as! String)!)
            request.httpMethod = c["method"] as? String
            let policy = ProductionWriteGuard.Policy(
                testBuild: c["testBuild"] as! Bool,
                allowProductionWrites: c["allowProductionWrites"] as! Bool
            )
            let body = (c["body"] as? String).map { Data($0.utf8) }
            let refused = ProductionWriteGuard.refusal(request, policy: policy) { body }
            #expect(refused == c["host"] as? String, "\(c["about"] ?? "")")
        }
    }
}

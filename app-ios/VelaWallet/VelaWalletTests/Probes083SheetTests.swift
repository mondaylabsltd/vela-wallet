//
//  Probes083SheetTests.swift
//  VelaWalletTests
//
//  Spec 084 CHECK PASS (not a fix): the signing SHEET the person would read,
//  built by the real core (clear-signing, approval guard) and the shipped
//  `SigningLive.model`, for the requests the hand-off names (I-1..I-4, I-W10,
//  I-EXE, I-H4a's typed-data trigger). The verdict text goes to the probe log;
//  nothing is asserted about the product. No money: the sheet is built, read
//  and dropped — nothing is approved, no relay call is scripted, no signer runs.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

private enum SProbe {
    static let path = "/private/tmp/claude-501/-Volumes-data-production-agent-2-vela-wallet/7177859b-46e8-4e29-be2f-0104b6530a2c/scratchpad/probe083-ios.log"
    static func log(_ id: String, _ text: String) {
        let line = "PROBE083|\(id)|\(text)\n"
        print(line, terminator: "")
        if let data = line.data(using: .utf8) {
            if let handle = FileHandle(forWritingAtPath: path) {
                handle.seekToEndOfFile(); handle.write(data); try? handle.close()
            } else {
                FileManager.default.createFile(atPath: path, contents: data)
            }
        }
    }
}

@MainActor
struct Probes083Sheet {

    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// Opens `method`/`params` on a real SigningController (real core), waits
    /// for the clear-signing resolution, and renders the shipped sheet model.
    private func sheet(
        _ id: String, _ what: String, method: String, params: String,
        chain: Int = 100, chainName: String = "Gnosis", symbol: String = "xDAI",
        origin: String = "http://192.168.50.17:8000"
    ) async {
        let suite = "vela.probe083.sheet.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        var answers: [[String: Any]] = []
        let controller = SigningController(
            wallet: (address: golden, credentialId: "cred-1"),
            relay: relay,
            accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store,
            pool: RpcPool(store: store, accounts: AccountStore()),
            ports: SigningController.Ports(
                respond: { _, _, payload, _ in answers.append(payload) },
                nativeSymbol: { _ in symbol },
                knownChains: { [100, 8453, 1] }
            )
        )
        let incoming = SigningController.Incoming(
            id: "probe-\(id)", method: method, paramsJson: params,
            origin: origin, transportId: "tab-1", chainId: chain
        )
        controller.open(incoming)
        // The clear-signing machine is asynchronous; give it its turn.
        let deadline = Date().addingTimeInterval(8)
        while Date() < deadline, !(controller.clear.resolved || controller.sign.blocked != nil) {
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
        try? await Task.sleep(nanoseconds: 300_000_000)

        var context = SigningLive.Context(
            loc: loc, chainName: chainName, chainDot: .green, nativeSymbol: symbol,
            walletName: "MultiTest", walletAddress: golden.lowercased()
        )
        context.typicalS = 5
        let model = SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc),
            request: incoming,
            sign: controller.sign,
            clear: controller.clear,
            guard: controller.guardView,
            fee: controller.fee,
            context: context
        )
        let blocks = model.blocks.map { String(describing: $0) }.joined(separator: "\n      ")
        SProbe.log(id, "\(what)\n   resolved=\(controller.clear.resolved) surface=\(controller.clear.surface) intent=«\(controller.clear.result?.intent ?? "nil")» contract=«\(controller.clear.result?.contractName ?? "nil")»\n   confirm=\(String(describing: model.confirm?.action)) confirm-enabled=\(String(describing: model.confirm?.enabled))\n   BLOCKS:\n      \(blocks)")
        controller.swipeDismissed()   // the ✕: one 4001, nothing signed
    }

    private let founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private let gnosisUSDC = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    @Test func iW10_plainNativeTransfer() async {
        await sheet("I-W10", "Send dust: eth_sendTransaction 0.001 xDAI to founder, Gnosis",
                    method: "eth_sendTransaction",
                    params: #"[{"from":"\#(golden)","to":"\#(founder)","value":"0x38d7ea4c68000"}]"#)
    }

    @Test func i1_strayCallsKey() async {
        await sheet("I-1", "USDC approve(0x1111.., 0) with a stray calls:[{to: founder, value:0x1}]",
                    method: "eth_sendTransaction",
                    params: #"[{"from":"\#(golden)","to":"\#(gnosisUSDC)","value":"0x0","data":"0x095ea7b3\#(String(repeating: "1", count: 40).leftPad(64))\#(String(repeating: "0", count: 64))","calls":[{"to":"\#(founder)","value":"0x1"}]}]"#)
    }

    @Test func i2_multiCallBatchWithPlainFirstLeg() async {
        await sheet("I-2", "wallet_sendCalls [1 wei to dEaD, USDC transfer(founder, 1)]",
                    method: "wallet_sendCalls",
                    params: #"[{"version":"2.0.0","from":"\#(golden)","chainId":"0x64","calls":[{"to":"0x000000000000000000000000000000000000dEaD","value":"0x1"},{"to":"\#(gnosisUSDC)","value":"0x0","data":"0xa9059cbb\#(founder.dropFirst(2).lowercased().leftPad(64))\#(String(format: "%064x", 1))"}]}]"#)
    }

    @Test func i3_inputInsteadOfData() async {
        await sheet("I-3", "eth_sendTransaction with calldata under `input` (router, value 0)",
                    method: "eth_sendTransaction",
                    params: #"[{"from":"\#(golden)","to":"\#(gnosisUSDC)","value":"0x0","input":"0x095ea7b3\#(String(repeating: "1", count: 40).leftPad(64))\#(String(repeating: "0", count: 64))"}]"#)
    }

    @Test func i4_jsonNumberValue() async {
        await sheet("I-4", "eth_sendTransaction value as a JSON number 1000000000000000",
                    method: "eth_sendTransaction",
                    params: #"[{"from":"\#(golden)","to":"\#(founder)","value":1000000000000000}]"#)
    }

    @Test func iEXE_universalRouterExecute() async {
        // Universal Router execute(bytes,bytes[],uint256) with an expired deadline: the
        // selector the hand-off names (0x3593564c).
        func w(_ n: Int) -> String { String(format: "%064x", n) }
        let data = "0x3593564c" + w(0x60) + w(0x80) + w(4102444800) + w(0) + w(0)
        await sheet("I-EXE", "Universal Router execute() on Base from app.uniswap.org",
                    method: "eth_sendTransaction",
                    params: #"[{"from":"\#(golden)","to":"0x6ff5693b99212da76ad316178a184ab56d299b43","value":"0x0","data":"\#(data)"}]"#,
                    chain: 8453, chainName: "Base", symbol: "ETH", origin: "https://app.uniswap.org")
    }

    @Test func iH4a_typedDataThatCannotBeHashed() async {
        await sheet("I-H4a", "eth_signTypedData_v4 with primaryType Missing",
                    method: "eth_signTypedData_v4",
                    params: #"["\#(golden)", "{\"types\":{\"EIP712Domain\":[]},\"primaryType\":\"Missing\",\"domain\":{},\"message\":{}}"]"#)
    }
}

private extension String {
    func leftPad(_ width: Int) -> String {
        String(repeating: "0", count: max(0, width - count)) + self
    }
}

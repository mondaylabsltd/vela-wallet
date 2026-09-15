//
//  PayLinkTests.swift
//  VelaWalletTests
//
//  A deep link, from the URL to the core's verdict on it.
//
//  The tokeniser is the shell's and the VALIDATION is the core's, so these
//  drive the real `payment_request` machine for everything that decides
//  whether a link is a request this wallet can honour.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

struct PayLinkParseTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// The three shapes that carry a payment, and the one that names a page.
    @Test func theFourShapesParse() throws {
        for raw in [
            "velawallet://pay?to=\(me)&chain=100",
            "https://getvela.app/pay?to=\(me)&chain=100",
            "https://wallet.getvela.app/pay?to=\(me)&chain=100",
        ] {
            guard case .pay(let to, let chain, _, _, _, _, _)? = PayLink.parse(raw) else {
                Issue.record("\(raw) did not parse as a payment")
                return
            }
            #expect(to == me)
            #expect(chain == "100")
        }
        #expect(PayLink.parse("velawallet://open?url=https://app.example")
            == .open(url: "https://app.example"))
    }

    /// **`open` takes a WEB page and nothing else.** A link asking this app to
    /// open its own container is one arbitrary URL away from reading its files
    /// out to a page.
    @Test func openRefusesAnythingButHttp() {
        #expect(PayLink.parse("velawallet://open?url=file:///etc/passwd") == nil)
        #expect(PayLink.parse("velawallet://open?url=velawallet://pay") == nil)
        #expect(PayLink.parse("velawallet://open") == nil)
        #expect(PayLink.parse("velawallet://open?url=javascript:alert(1)") == nil)
    }

    /// Somebody else's domain, somebody else's path, and nothing at all.
    @Test func everythingElseIsNotALink() {
        #expect(PayLink.parse("https://getvela.app/download?to=\(me)") == nil)
        #expect(PayLink.parse("https://evil.example/pay?to=\(me)&chain=1") == nil)
        #expect(PayLink.parse("velawallet://send?to=\(me)") == nil)
        #expect(PayLink.parse("") == nil)
        #expect(PayLink.parse(nil) == nil)
    }

    @Test func everyParameterSurvives() throws {
        let link = "velawallet://pay?to=\(me)&chain=100&token=0xdead&amount=1.5&sym=USDC&dec=6&net=Gnosis"
        guard case .pay(let to, let chain, let token, let amount, let sym, let dec, let net)?
            = PayLink.parse(link)
        else {
            Issue.record("the full link did not parse")
            return
        }
        #expect(to == me)
        #expect(chain == "100")
        #expect(token == "0xdead")
        #expect(amount == "1.5")
        #expect(sym == "USDC")
        #expect(dec == "6")
        #expect(net == "Gnosis")
    }
}

@MainActor
struct PayLinkVerdictTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    /// Hand a parsed link to the real machine and read back its verdict.
    private func judged(_ raw: String) throws -> PaymentRequestViewWire {
        let event = try #require(PayLink.parse(raw)?.linkOpened, "\(raw) did not parse")
        let core = PaymentRequestCore()
        let result = try core.dispatch(eventJson: CoreJSON.string(event))
        return try CoreJSON.decode(PaymentRequestViewWire.self, from: try view(from: result))
    }

    /// A good link is accepted, and what comes back is everything a prefilled
    /// send needs — including the `ethereum:` URI, which is how the link and a
    /// scanned QR code of the same request land on exactly the same screen.
    @Test func aGoodLinkBecomesARequest() throws {
        let judged = try judged("velawallet://pay?to=\(me)&chain=100&amount=1.5&sym=xDAI&dec=18")
        #expect(judged.payValid == true)
        let request = try #require(judged.pay)
        #expect(request.recipient.lowercased() == me.lowercased())
        #expect(request.chainId == 100)
        #expect(request.amount == "1.5")
        #expect(request.amountBase == "1500000000000000000", "base units, never a float")
        #expect(request.eip681Uri.hasPrefix("ethereum:"))
    }

    /// An OPEN request — somebody asking to be paid without saying how much —
    /// carries no amount, and the send must not invent one.
    @Test func anOpenRequestHasNoAmount() throws {
        let judged = try judged("velawallet://pay?to=\(me)&chain=100")
        #expect(judged.payValid == true)
        #expect(judged.pay?.amount == nil)
        #expect(judged.pay?.amountBase == nil)
    }

    /// **The core refuses**, and the shell does not second-guess it: a
    /// recipient that is not an address, and a chain that is not a number.
    @Test func theCoreRefusesWhatItCannotHonour() throws {
        #expect(try judged("velawallet://pay?to=notanaddress&chain=100").payValid == false)
        #expect(try judged("velawallet://pay?to=\(me)&chain=mainnet").payValid == false)
        #expect(try judged("velawallet://pay?to=\(me)").payValid == false)
    }

    /// The decimals quirk, ported faithfully on every client: an unparseable
    /// OR ZERO `dec` falls back to 18. A test pins it so nobody "fixes" it into
    /// a divergence.
    @Test func zeroDecimalsFallsBackToEighteen() throws {
        let judged = try judged("velawallet://pay?to=\(me)&chain=100&amount=1&dec=0")
        #expect(judged.pay?.decimals == 18)
        #expect(judged.pay?.amountBase == "1000000000000000000")
    }
}

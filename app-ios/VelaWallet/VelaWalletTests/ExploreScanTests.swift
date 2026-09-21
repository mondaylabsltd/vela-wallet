//
//  ExploreScanTests.swift
//  VelaWalletTests
//
//  Issue 273 + spec 070 US5: the Explore scan button opens a scanned WEB
//  ADDRESS in the browser, sends a payment code to 发送, and names a
//  WalletConnect pairing code as unsupported.
//

import Testing
@testable import VelaWallet

@MainActor
struct ExploreScanTests {

    @Test func anHttpsUrlOpensAsItIs() {
        #expect(ExploreScan.url(from: "https://app.uniswap.org/swap?chain=base")
            == "https://app.uniswap.org/swap?chain=base")
        #expect(ExploreScan.url(from: "  http://localhost:5173/\n") == "http://localhost:5173/")
    }

    @Test func anUppercaseSchemeIsReadAsTheSameAddress() {
        #expect(ExploreScan.url(from: "HTTPS://EXAMPLE.COM/A") == "https://EXAMPLE.COM/A")
    }

    /// A bare domain opens as the search field would open it.
    @Test func aBareDomainGetsHttps() {
        #expect(ExploreScan.url(from: "app.aave.com") == "https://app.aave.com")
        #expect(ExploreScan.url(from: "app.aave.com/markets") == "https://app.aave.com/markets")
    }

    @Test func aWalletConnectCodeIsRefused() {
        #expect(ExploreScan.url(from: "wc:7f6e504bfad60b485450578e05678ed3e8e8c4751d3c6160be17160d63ec90f9@2?relay-protocol=irn&symKey=587d5484ce2a2a6ee3ba1962fdd7e8588e06200c46823bd18fbd67def96ad303") == nil)
    }

    @Test func paymentCodesAndAddressesAreRefused() {
        #expect(ExploreScan.url(from: "ethereum:0x88cCA0EeDbF2C4426110bbFc998F048689266894@100") == nil)
        #expect(ExploreScan.url(from: "0x88cCA0EeDbF2C4426110bbFc998F048689266894") == nil)
    }

    // MARK: - Where each code goes (spec 070 US5)

    @Test func aWebAddressOpensInTheBrowser() {
        #expect(ExploreScan.route("https://app.uniswap.org/swap") == .open("https://app.uniswap.org/swap"))
        #expect(ExploreScan.route("app.aave.com") == .open("https://app.aave.com"))
    }

    /// An account address and an `ethereum:` code are 发送's — through the
    /// core's `scan_resolved`, never opened as a page called `https://0x…`.
    @Test func aPaymentCodeGoesToSend() {
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        #expect(ExploreScan.route(address) == .send(address))
        let code = "ethereum:\(address)@100"
        #expect(ExploreScan.route(code) == .send(code))
        #expect(ExploreScan.route("  \(address)\n") == .send(address))
    }

    @Test func aWalletConnectCodeIsNamedAsUnsupported() {
        #expect(ExploreScan.route("wc:7f6e504bfad60b@2?relay-protocol=irn&symKey=587d") == .walletConnect)
        #expect(ExploreScan.route("WC:abc@2") == .walletConnect)
    }

    @Test func anythingElseIsUnrecognized() {
        #expect(ExploreScan.route("hello world") == .unrecognized)
        #expect(ExploreScan.route("mailto:someone@example.com") == .unrecognized)
        #expect(ExploreScan.route("0x1234") == .unrecognized)
    }

    @Test func textThatIsNotAnAddressIsRefused() {
        #expect(ExploreScan.url(from: "") == nil)
        #expect(ExploreScan.url(from: "hello world") == nil)
        #expect(ExploreScan.url(from: "uniswap") == nil)
        #expect(ExploreScan.url(from: "mailto:someone@example.com") == nil)
        #expect(ExploreScan.url(from: "https://") == nil)
        #expect(ExploreScan.url(from: "user:pass@example.com") == nil)
    }
}

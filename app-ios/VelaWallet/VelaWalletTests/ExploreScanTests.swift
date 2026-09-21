//
//  ExploreScanTests.swift
//  VelaWalletTests
//
//  Issue 273: the Explore scan button opens a scanned WEB ADDRESS in the
//  browser, and refuses everything else — a WalletConnect pairing code is not
//  supported and a payment code is 发送's.
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
        #expect(ExploreScan.url(from: "app.aave.com/markets") == BrowserEngine.coerce("app.aave.com/markets"))
    }

    @Test func aWalletConnectCodeIsRefused() {
        #expect(ExploreScan.url(from: "wc:7f6e504bfad60b485450578e05678ed3e8e8c4751d3c6160be17160d63ec90f9@2?relay-protocol=irn&symKey=587d5484ce2a2a6ee3ba1962fdd7e8588e06200c46823bd18fbd67def96ad303") == nil)
    }

    @Test func paymentCodesAndAddressesAreRefused() {
        #expect(ExploreScan.url(from: "ethereum:0x88cCA0EeDbF2C4426110bbFc998F048689266894@100") == nil)
        #expect(ExploreScan.url(from: "0x88cCA0EeDbF2C4426110bbFc998F048689266894") == nil)
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

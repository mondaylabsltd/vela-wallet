//
//  Eip681Tests.swift
//  VelaWalletTests
//
//  The payment-URI tokeniser, and especially its two refusals — each of which
//  was bought with a real bug on another client.
//

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers
import Testing
@testable import VelaWallet

struct Eip681Tests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let token = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    /// The four shapes this app itself writes onto a receive code.
    @Test func theShapesThisAppWritesReadBack() throws {
        let native = try #require(Eip681.parse("ethereum:\(me)@100"))
        #expect(native.recipient == me)
        #expect(native.chainId == 100)
        #expect(native.tokenAddress == nil)
        #expect(native.amountBaseUnits == nil)

        let withValue = try #require(Eip681.parse("ethereum:\(me)@100?value=1500000000000000000"))
        #expect(withValue.amountBaseUnits == "1500000000000000000")

        let erc20 = try #require(Eip681.parse("ethereum:\(token)@100/transfer?address=\(me)"))
        #expect(erc20.recipient == me)
        #expect(erc20.tokenAddress == token)
        #expect(erc20.amountBaseUnits == nil)

        let priced = try #require(
            Eip681.parse("ethereum:\(token)@100/transfer?address=\(me)&uint256=1000000")
        )
        #expect(priced.amountBaseUnits == "1000000")
    }

    /// **Refusal one.** In a `/transfer` URI, `value` is the ether attached to
    /// the call, not the token argument. Reading it as token base units mixed
    /// two units and two decimals — a URI saying "attach 1 ETH" prefilled a
    /// LOCKED send of 10^12 USDC.
    @Test func aTransfersValueIsNotTheTokenAmount() throws {
        let parsed = try #require(
            Eip681.parse("ethereum:\(token)@100/transfer?address=\(me)&value=1000000000000000000")
        )
        #expect(parsed.tokenAddress == token)
        #expect(parsed.amountBaseUnits == nil, "`value` must not become the token amount")
    }

    /// **Refusal two.** Any function that is not `transfer` is not a payment.
    /// It used to fall through to the native branch, where the CONTRACT became
    /// the recipient: a locked send of the chain's coin to the token contract,
    /// which is a burn dressed as the payment somebody thought they scanned.
    @Test func anyOtherFunctionIsNotAPayment() {
        #expect(Eip681.parse("ethereum:\(token)@1/approve?address=\(me)&uint256=1") == nil)
        #expect(Eip681.parse("ethereum:\(token)@1/withdraw") == nil)
    }

    @Test func theLegacyPayPrefixAndACaseInsensitiveSchemeBothParse() throws {
        #expect(try #require(Eip681.parse("ethereum:pay-\(me)@100")).recipient == me)
        #expect(try #require(Eip681.parse("ETHEREUM:\(me)")).chainId == nil)
        #expect(try #require(Eip681.parse("  ethereum:\(me)@100  ")).chainId == 100)
    }

    @Test func anythingElseIsNotARequest() {
        #expect(Eip681.parse(me) == nil, "a bare address is not a URI")
        #expect(Eip681.parse("ethereum:") == nil)
        #expect(Eip681.parse("ethereum:notanaddress@1") == nil)
        #expect(Eip681.parse("https://getvela.app") == nil)
        // A transfer with no recipient parameter names nobody.
        #expect(Eip681.parse("ethereum:\(token)@1/transfer") == nil)
    }

    /// Unparseable text is not refused — it becomes the raw text the send
    /// screen drops into an editable recipient field.
    @Test func unparseableTextStillReachesTheCore() throws {
        let scan = Eip681.scan(of: "hello")
        #expect(scan["type"] as? String == "text")
        #expect(scan["data"] as? String == "hello")

        let request = Eip681.scan(of: "ethereum:\(me)@100?value=1")
        #expect(request["type"] as? String == "request")
        #expect(request["recipient"] as? String == me)
        #expect((request["chain_id"] as? NSNumber)?.intValue == 100)
        #expect(request["token_address"] is NSNull)
    }

    // MARK: - Amounts

    @Test func amountsAcceptTheFormsAWalletWrites() {
        #expect(Eip681.parseAmount("1000") == "1000")
        #expect(Eip681.parseAmount("1e18") == "1000000000000000000")
        #expect(Eip681.parseAmount("1.5e18") == "1500000000000000000")
        #expect(Eip681.parseAmount("0.000001e6") == "1")
        #expect(Eip681.parseAmount("+42") == "42")
        #expect(Eip681.parseAmount("007") == "7")
    }

    /// Base units are integral, so a fraction of one truncates — and a
    /// NEGATIVE amount is refused rather than floored to zero: minus one coin
    /// is not a request for none of them.
    @Test func amountsTruncateAndRefuse() {
        #expect(Eip681.parseAmount("1.9") == "1")
        #expect(Eip681.parseAmount("0.4") == "0")
        #expect(Eip681.parseAmount("-1") == nil)
        #expect(Eip681.parseAmount("") == nil)
        #expect(Eip681.parseAmount("abc") == nil)
        #expect(Eip681.parseAmount(".") == nil)
        // An exponent nobody could mean, refused rather than expanded.
        #expect(Eip681.parseAmount("1e999") == nil)
    }
}

// MARK: - The decoder's round trip

/// A code this app WROTE, read back by the decoder this app uses.
///
/// The receive screen renders a code from `QrCode.modules`; the scanner reads
/// one with Vision. Nothing else in the app checks that those two agree, and a
/// wallet whose own receive code its own scanner cannot read is a wallet with
/// two QR implementations.
@MainActor
struct QrRoundTripTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// Render the modules as a bitmap the way the share card does: a quiet zone
    /// of four modules, eight pixels each.
    private func image(_ modules: [[Bool]]) -> CGImage? {
        let quiet = 4, scale = 12
        let side = (modules.count + quiet * 2) * scale
        guard side > 0,
              let context = CGContext(
                data: nil, width: side, height: side, bitsPerComponent: 8,
                bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
              )
        else { return nil }
        context.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: side, height: side))
        context.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: 1))
        for (row, line) in modules.enumerated() {
            for (column, on) in line.enumerated() where on {
                // `CGContext`'s origin is bottom-left and the module grid is
                // row-major from the top, so the row index is flipped.
                context.fill(CGRect(
                    x: (column + quiet) * scale,
                    y: (modules.count - 1 - row + quiet) * scale,
                    width: scale, height: scale
                ))
            }
        }
        return context.makeImage()
    }

    @Test func aCodeThisAppWroteReadsBack() throws {
        let payload = "ethereum:\(me)@100"
        let modules = try #require(QrCode.modules(payload), "this app could not render its own code")
        let rendered = try #require(image(modules))
        // Written out so a failure can be LOOKED at rather than guessed about.
        if let destination = CGImageDestinationCreateWithURL(
            URL(fileURLWithPath: "/tmp/vela-qr-roundtrip.png") as CFURL,
            "public.png" as CFString, 1, nil
        ) {
            CGImageDestinationAddImage(destination, rendered, nil)
            CGImageDestinationFinalize(destination)
        }
        #expect(modules.count > 10, "the matrix is \(modules.count) modules wide")
        #expect(QrDecoder.decode(image: rendered) == payload)
    }

    /// The same code, rotated a quarter turn — a person holding a phone does
    /// not hold it square to the paper.
    @Test func aRotatedCodeStillReadsBack() throws {
        let payload = "ethereum:\(me)@100"
        let modules = try #require(QrCode.modules(payload))
        let rendered = try #require(image(modules))
        let side = rendered.width
        guard let context = CGContext(
            data: nil, width: side, height: side, bitsPerComponent: 8,
            bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
        ) else {
            Issue.record("could not make a context to rotate into")
            return
        }
        context.translateBy(x: CGFloat(side), y: 0)
        context.rotate(by: .pi / 2)
        context.draw(rendered, in: CGRect(x: 0, y: 0, width: side, height: side))
        let rotated = try #require(context.makeImage())
        #expect(QrDecoder.decode(image: rotated) == payload)
    }
}

//
//  AddressBreaksTests.swift
//  VelaWalletTests
//
//  Where an address may break across lines (r13: a member's last five digits
//  alone on a line) — the desktop's rule (`ui/address.rs`) and Android's
//  (`AddressBreaksTest`), pinned to the same vectors.
//

import Foundation
import Testing
@testable import VelaWallet

struct AddressBreaksTests {

    @Test func aFullAddressIsSplitOnceEvenlyAndAShortOneNever() {
        let full = "0x777E989C821a7F1b031696f044a2fF114261875a"
        let halves = AddressText.halves(full)
        #expect(halves?.0.count == 22)
        #expect(halves?.1.count == 20)
        #expect((halves?.0 ?? "") + (halves?.1 ?? "") == full)
        #expect(AddressText.halves("0x777E98…61875a") == nil)
        #expect(AddressText.halves("Interleave") == nil)
        #expect(AddressText.isShortForm("0x777E98…61875a"))
        #expect(!AddressText.isShortForm("Interleave · Vela account"))
    }

    @Test func theR13MemberSplitsWhereNobodyIsLeftAlone() {
        let halves = AddressText.halves("0xD400866e00B055B20752a826CD5C89b811de130b")
        #expect(halves?.0 == "0xD400866e00B055B20752")
        #expect(halves?.1 == "a826CD5C89b811de130b")
    }

    @Test func calldataWrapsAsTextAndAHashSplitsLikeAnAddress() {
        #expect(AddressText.halves("0x" + String(repeating: "ab", count: 68)) == nil)
        let hash = AddressText.halves("0x" + String(repeating: "ab", count: 32))
        #expect(hash?.0.count == 34)
        #expect(hash?.1.count == 32)
        #expect(AddressText.halves("0x" + String(repeating: "zz", count: 20)) == nil)
    }
}

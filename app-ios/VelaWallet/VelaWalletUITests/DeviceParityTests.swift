//
//  DeviceParityTests.swift
//  VelaWalletUITests
//
//  Spec 058, on the phone — what a simulator and 581 hermetic tests cannot say.
//
//  Three of this cut's changes are only true on a device: the storage page
//  weighs THIS phone's store, 关于 names the build actually installed, and the
//  language a person picks has to change the screen they are looking at. All
//  three were drawn numbers or dead rows before.
//
//      xcodebuild test \
//        -project app-ios/VelaWallet/VelaWallet.xcodeproj -scheme VelaWallet \
//        -destination 'platform=iOS,id=<device>' \
//        -only-testing:VelaWalletUITests/DeviceParityTests \
//        -resultBundlePath /tmp/parity.xcresult
//      xcrun xcresulttool export attachments \
//        --path /tmp/parity.xcresult --output-path /tmp/parity-images
//
//  **Nothing here needs a wallet**, which is the point: `VELA_PAGE=settings-live`
//  mounts the real settings machine over whatever the device already holds, so
//  these screens can be read without a passkey ceremony. Nothing here spends,
//  and nothing here erases.
//

import XCTest

final class DeviceParityTests: XCTestCase {

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// 存储 · 关于, with this device's own numbers behind them.
    ///
    /// Asserted rather than merely photographed: the fixture's figures are
    /// known (`2.4 MB`, `216 records`, `1.0.0 (6ab8f)`), so "not the fixture's"
    /// is a claim a test can make. What the real numbers ARE is the phone's
    /// business and changes between runs.
    func testStorageAndAboutAreMeasuredOnThisPhone() {
        let storage = launch(page: "settings-live", state: "st13")
        XCTAssertTrue(
            storage.staticTexts["设备存储"].waitForExistence(timeout: 20),
            "the storage page never appeared"
        )
        attach(settled(storage).screenshot(), named: "st13-storage-device")
        XCTAssertFalse(
            storage.staticTexts["2.4"].exists,
            "the storage page is still showing the drawing's 2.4 MB"
        )
        storage.terminate()

        let about = launch(page: "settings-live", state: "st14")
        XCTAssertTrue(
            about.staticTexts["关于"].waitForExistence(timeout: 20),
            "the about page never appeared"
        )
        attach(settled(about).screenshot(), named: "st14-about-device")
        // The fixture's version string, which this build is not.
        XCTAssertFalse(
            about.staticTexts["v1.0.0 (6ab8f)"].exists,
            "关于 is still reading the fixture's version"
        )
        about.terminate()
    }

    /// The stored language decides the app's language at launch.
    ///
    /// Through the ARGUMENT domain (`-vela.language ja`), which `UserDefaults`
    /// lets outrank the persisted value without writing anything — the same
    /// trick the screenshot sweep uses for the parallel space. So this proves
    /// the read path on a real phone and leaves the founder's own choice
    /// exactly as it was.
    func testTheStoredLanguageDecidesWhatThePhoneSays() {
        let japanese = launch(page: "settings-live", language: "ja")
        XCTAssertTrue(
            japanese.staticTexts["設定"].waitForExistence(timeout: 20),
            "a stored `ja` did not put the settings page into Japanese"
        )
        attach(settled(japanese).screenshot(), named: "settings-stored-ja")
        japanese.terminate()

        let german = launch(page: "settings-live", language: "de")
        XCTAssertTrue(
            german.staticTexts["Einstellungen"].waitForExistence(timeout: 20),
            "a stored `de` did not put the settings page into German"
        )
        attach(settled(german).screenshot(), named: "settings-stored-de")
        german.terminate()
    }

    /// What the address book on THIS phone shows — and the gap it proves.
    ///
    /// 058 gave the groups header a working 新建分组. This device's book is
    /// empty, so **the header is not drawn at all** and the action cannot be
    /// reached: the section appears only once a group exists, on iOS and on
    /// Android alike (`ContactsScreen.kt`: `if (model.groups.isNotEmpty())`).
    /// A person with no groups cannot make their first one on either phone.
    ///
    /// Asserted rather than assumed, because "I could not see it" and "it is
    /// not there" are different claims and only one of them is evidence.
    func testAnEmptyBookDrawsNoGroupsHeaderOnEitherPhone() {
        let contacts = launch(page: "contacts-live")
        XCTAssertTrue(
            contacts.staticTexts["通讯录"].waitForExistence(timeout: 20),
            "the contacts page never appeared"
        )
        attach(settled(contacts).screenshot(), named: "contacts-live-device")
        // The empty state, not a list — which is why the next two hold.
        XCTAssertTrue(contacts.staticTexts["还没有联系人"].exists)
        XCTAssertFalse(contacts.staticTexts["分组"].exists,
                       "an empty book drew a groups section")
        XCTAssertFalse(contacts.staticTexts["新建分组"].exists,
                       "058's own claim: the action exists only where the section does")
        contacts.terminate()
    }

    // MARK: - Plumbing

    private func launch(
        page: String,
        state: String? = nil,
        language: String? = nil
    ) -> XCUIApplication {
        let app = XCUIApplication()
        // Never the parallel space, and never inheriting the last run's: an
        // argument-domain value outranks the persisted one without writing.
        app.launchArguments += ["-vela.parallelSpace", "0"]
        if let language {
            app.launchArguments += ["-vela.language", language]
        } else {
            // The env PIN, for the screens whose copy this test reads in
            // Chinese. `Loc` honours it over any stored choice (058).
            app.launchEnvironment["VELA_LANG"] = "zh"
            app.launchArguments += ["-AppleLanguages", "(zh-Hans)"]
        }
        app.launchEnvironment["VELA_PAGE"] = page
        if let state { app.launchEnvironment["VELA_STATE"] = state }
        app.launchEnvironment["VELA_THEME"] = "dark"
        app.launch()
        return app
    }

    /// Wait past the launch animation before photographing.
    ///
    /// The page is composed from the FIRST frame and hidden behind an opaque
    /// overlay (`RootView`, FR-013a), so an element exists in the tree while
    /// the animation is still on screen — `waitForExistence` returns true and
    /// the screenshot catches the boat. Found here: the first Japanese capture
    /// was a picture of the launch screen with a passing assertion behind it.
    private func settled(_ app: XCUIApplication) -> XCUIApplication {
        Thread.sleep(forTimeInterval: 2.5)
        return app
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}

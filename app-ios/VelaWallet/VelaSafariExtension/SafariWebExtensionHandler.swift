//
//  SafariWebExtensionHandler.swift
//  VelaSafariExtension
//
//  The Safari Web Extension's native half — deliberately empty.
//
//  Safari requires a principal class for every web extension, and calls it
//  only for `browser.runtime.sendNativeMessage`, which needs the manifest's
//  `nativeMessaging` permission. The Vela extension does not ask for it: the
//  wallet in Safari is the web wallet's own code and core, packaged
//  (app-web/vela-wallet/extension), and nothing crosses from Safari into the
//  app — no state, no request, and never a key (a key lives in its passkey).
//  So any message that does arrive is refused in one line, and this target
//  links no core, no network stack and no keychain
//  (app-ios/scripts/check-ios-store-binary.sh holds it to that).
//

import Foundation
import SafariServices

final class SafariWebExtensionHandler: NSObject, NSExtensionRequestHandling {

    /// The answer to anything the extension might send: Vela's extension
    /// talks to its own pages and the networks, never to the app.
    static let refusal: [String: String] = ["error": "unsupported"]

    func beginRequest(with context: NSExtensionContext) {
        let response = NSExtensionItem()
        response.userInfo = [SFExtensionMessageKey: Self.refusal]
        context.completeRequest(returningItems: [response], completionHandler: nil)
    }
}

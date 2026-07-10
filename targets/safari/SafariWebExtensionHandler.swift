import SafariServices
import os.log

// Darwin notification name the app side observes (see src/services/dev/app-group-echo.ts
// -> observeDarwin). Keep in sync with that constant.
private let kDarwinExtWrote = "app.getvela.wallet.ext-wrote"

/// Increment 2 echo handler. On `{ type: "echo", payload }` it:
///   1. writes echo-from-ext-<id>.json into the shared App Group container,
///   2. posts a payload-less Darwin notification to poke the app,
///   3. reads the newest echo-from-app-*.json the app wrote,
///   4. returns both in the SFExtensionMessageKey response.
/// That single round-trip proves both directions + Darwin. No signing, no UI.
class SafariWebExtensionHandler: NSObject, NSExtensionRequestHandling {

    func beginRequest(with context: NSExtensionContext) {
        // Guard the casts (the old stub force-unwrapped — a malformed message
        // would crash the handler process and hang the JS promise forever).
        let item = context.inputItems.first as? NSExtensionItem
        let message = item?.userInfo?[SFExtensionMessageKey] as? [String: Any]
        let type = message?["type"] as? String
        let payload = message?["payload"]

        var responseBody: [String: Any]

        if type == "writeSignRequest" {
            // Increment 3: persist the signing request the extension handed off,
            // keyed by rid. The app cold-launches via velawallet://sign?rid=<rid>
            // and polls for sign-req-<rid>.json (the write races the launch).
            let rid = (message?["rid"] as? String) ?? UUID().uuidString
            let request = (message?["request"] as? [String: Any]) ?? [:]
            let filename = "sign-req-\(rid).json"
            let wroteURL = AppGroupStore.writeJSON(request, to: filename)
            responseBody = [
                "type": "sign-req-ack",
                "rid": rid,
                "wrote": wroteURL?.lastPathComponent ?? NSNull(),
                "container": AppGroupStore.containerURL != nil,
            ]
            os_log("Vela R1 sign: wrote %{public}@", type: .default, filename)
        } else if type == "pollSignResult" {
            // Increment 4: the extension returns to Safari and polls for the
            // result the app wrote (sign-result-<rid>.json). Read-only + keyed
            // by exact rid (never `newestFile`, which would collide across
            // concurrent rids). The file is NOT consumed here, so repeat polls
            // are idempotent — single-use is enforced client-side by the rid.
            let rid = (message?["rid"] as? String) ?? ""
            let filename = "sign-result-\(rid).json"
            var found = false
            var responseDict: [String: Any] = [:]
            if let dir = AppGroupStore.containerURL,
               let dict = AppGroupStore.readJSON(url: dir.appendingPathComponent(filename)) {
                found = true
                responseDict = dict
            }
            responseBody = [
                "type": "sign-result",
                "rid": rid,
                "found": found,
                "result": responseDict, // {} when not written yet
            ]
            os_log("Vela R1 poll: %{public}@ found=%{public}@",
                   type: .default, filename, found ? "true" : "false")
        } else if type == "getAccount" {
            // Phase A: the extension answers connect/read/state in-Safari (zero
            // app hop). It reads the account cache the app writes on foreground /
            // account-or-chain change (vela.ext.account.json — see
            // src/services/app-group-account-sync.ts / <AccountFileWriter/>).
            // Read-only; the file carries ONLY public data (address, accounts,
            // chainId, RPC/bundler URLs) — never key material.
            var found = false
            var account: [String: Any] = [:]
            if let dir = AppGroupStore.containerURL,
               let dict = AppGroupStore.readJSON(url: dir.appendingPathComponent("vela.ext.account.json")) {
                found = true
                account = dict
            }
            responseBody = [
                "type": "account",
                "found": found,
                "account": account, // {} when the app hasn't written it yet
            ]
            os_log("Vela account: found=%{public}@", type: .default, found ? "true" : "false")
        } else if type == "echo" {
            let id = UUID().uuidString
            let ts = Date().timeIntervalSince1970

            // 1. Write echo-from-ext-<id>.json into the shared container.
            let record: [String: Any] = [
                "id": id,
                "source": "extension",
                "payload": payload ?? NSNull(),
                "ts": ts,
            ]
            let filename = "echo-from-ext-\(id).json"
            let wroteURL = AppGroupStore.writeJSON(record, to: filename)

            // 2. Poke the app via Darwin (name-only; data already on disk).
            AppGroupStore.postDarwin(kDarwinExtWrote)

            // 3. Read back the newest echo the APP wrote.
            var appEcho: [String: Any] = [:]
            if let appURL = AppGroupStore.newestFile(withPrefix: "echo-from-app-"),
               let dict = AppGroupStore.readJSON(url: appURL) {
                appEcho = dict
            }

            responseBody = [
                "type": "echo-ack",
                "id": id,
                "wrote": wroteURL?.lastPathComponent ?? NSNull(),
                "container": AppGroupStore.containerURL != nil,
                "newestFromApp": appEcho,   // {} if the app hasn't written yet
                "echo": payload ?? NSNull(),
                "ts": ts,
            ]
            os_log("Vela R1 echo: wrote %{public}@, replied with app-echo keys %{public}@",
                   type: .default, filename, "\(Array(appEcho.keys))")
        } else {
            responseBody = ["type": "error", "reason": "unknown message type"]
            os_log("Vela R1 echo: unknown message type %{public}@",
                   type: .default, type ?? "nil")
        }

        let response = NSExtensionItem()
        response.userInfo = [SFExtensionMessageKey: responseBody]
        context.completeRequest(returningItems: [response], completionHandler: nil)
    }
}

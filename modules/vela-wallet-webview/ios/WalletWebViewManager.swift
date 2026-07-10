// WalletWebViewManager (iOS) — legacy RCTViewManager for the WalletWebView.
//
// Registers the native view and exposes imperative commands (respond /
// emitProviderEvent / goBack / goForward / reload) that resolve the view by its
// reactTag off the UIManager queue, mirroring the existing native-module idiom in
// this repo. Props + the ObjC bridge surface are declared in WalletWebViewManager.m.
import Foundation
import React

@objc(WalletWebViewManager)
class WalletWebViewManager: RCTViewManager {

  override func view() -> UIView! {
    return WalletWebView()
  }

  override static func requiresMainQueueSetup() -> Bool {
    return true
  }

  private func withView(_ reactTag: NSNumber, _ body: @escaping (WalletWebView) -> Void) {
    bridge.uiManager.addUIBlock { _, viewRegistry in
      guard let view = viewRegistry?[reactTag] as? WalletWebView else { return }
      body(view)
    }
  }

  @objc func respond(_ reactTag: NSNumber, id rpcId: NSString, resultJson: NSString, errorJson: NSString) {
    withView(reactTag) { $0.deliverResponse(id: rpcId as String, resultJson: resultJson as String, errorJson: errorJson as String) }
  }

  @objc func emitProviderEvent(_ reactTag: NSNumber, event: NSString, dataJson: NSString) {
    withView(reactTag) { $0.deliverEvent(event: event as String, dataJson: dataJson as String) }
  }

  @objc func goBack(_ reactTag: NSNumber) { withView(reactTag) { $0.goBack() } }
  @objc func goForward(_ reactTag: NSNumber) { withView(reactTag) { $0.goForward() } }
  @objc func reload(_ reactTag: NSNumber) { withView(reactTag) { $0.reloadPage() } }
}

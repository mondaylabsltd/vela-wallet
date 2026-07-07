// ReactPackage for the WalletWebView view manager. Registered in MainApplication
// by plugins/with-native-modules.js (registerAndroidPackages), mirroring
// VelaPasskeyPackage.
package com.velawallet.webview

import com.facebook.react.ReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.uimanager.ViewManager

class WalletWebViewPackage : ReactPackage {
    override fun createNativeModules(
        reactContext: ReactApplicationContext,
    ): List<NativeModule> = emptyList()

    override fun createViewManagers(
        reactContext: ReactApplicationContext,
    ): List<ViewManager<*, *>> = listOf(WalletWebViewManager())
}

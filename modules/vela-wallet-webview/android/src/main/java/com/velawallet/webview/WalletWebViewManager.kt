// WalletWebViewManager (Android) — SimpleViewManager for the WalletWebView.
// Exposes props (sourceURL, injectedJavaScript), the two bubbling events, and the
// imperative commands (respond / emitProviderEvent / goBack / goForward / reload).
package com.velawallet.webview

import com.facebook.react.bridge.ReadableArray
import com.facebook.react.common.MapBuilder
import com.facebook.react.uimanager.SimpleViewManager
import com.facebook.react.uimanager.ThemedReactContext
import com.facebook.react.uimanager.annotations.ReactProp

class WalletWebViewManager : SimpleViewManager<WalletWebView>() {

    override fun getName(): String = "WalletWebView"

    override fun createViewInstance(reactContext: ThemedReactContext): WalletWebView =
        WalletWebView(reactContext)

    @ReactProp(name = "sourceURL")
    fun setSourceURL(view: WalletWebView, url: String?) {
        if (!url.isNullOrEmpty()) view.setSource(url)
    }

    @ReactProp(name = "injectedJavaScript")
    fun setInjectedJavaScript(view: WalletWebView, js: String?) {
        if (!js.isNullOrEmpty()) view.setInjectedJavaScriptSource(js)
    }

    @ReactProp(name = "outbox")
    fun setOutbox(view: WalletWebView, json: String?) {
        if (json != null) view.setOutboxSource(json)
    }

    override fun getExportedCustomDirectEventTypeConstants(): Map<String, Any> =
        MapBuilder.of(
            "onProviderRequest", MapBuilder.of("registrationName", "onProviderRequest"),
            "onNavigationChange", MapBuilder.of("registrationName", "onNavigationChange"),
        )

    override fun receiveCommand(view: WalletWebView, commandId: String, args: ReadableArray?) {
        when (commandId) {
            "respond" -> view.deliverResponse(
                args?.getString(0) ?: "",
                if (args != null && args.size() > 1 && !args.isNull(1)) (args.getString(1) ?: "") else "",
                if (args != null && args.size() > 2 && !args.isNull(2)) (args.getString(2) ?: "") else "",
            )
            "emitProviderEvent" -> view.deliverEvent(
                args?.getString(0) ?: "",
                if (args != null && args.size() > 1 && !args.isNull(1)) (args.getString(1) ?: "") else "",
            )
            "goBack" -> if (view.canGoBack()) view.goBack()
            "goForward" -> if (view.canGoForward()) view.goForward()
            "reload" -> view.reload()
        }
    }

    override fun onDropViewInstance(view: WalletWebView) {
        view.destroy()
        super.onDropViewInstance(view)
    }
}

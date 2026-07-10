// ObjC bridge surface for the WalletWebView native view (iOS).
// Props + imperative commands are exported here; the Swift class provides them.
#import <React/RCTViewManager.h>
#import <React/RCTUIManager.h>

@interface RCT_EXTERN_MODULE(WalletWebViewManager, RCTViewManager)

// Props
RCT_EXPORT_VIEW_PROPERTY(sourceURL, NSString)
RCT_EXPORT_VIEW_PROPERTY(injectedJavaScript, NSString)
RCT_EXPORT_VIEW_PROPERTY(outbox, NSString)
RCT_EXPORT_VIEW_PROPERTY(onProviderRequest, RCTDirectEventBlock)
RCT_EXPORT_VIEW_PROPERTY(onNavigationChange, RCTDirectEventBlock)

// Imperative commands (native -> page + navigation control)
RCT_EXTERN_METHOD(respond:(nonnull NSNumber *)reactTag
                  id:(NSString *)rpcId
                  resultJson:(NSString *)resultJson
                  errorJson:(NSString *)errorJson)

RCT_EXTERN_METHOD(emitProviderEvent:(nonnull NSNumber *)reactTag
                  event:(NSString *)event
                  dataJson:(NSString *)dataJson)

RCT_EXTERN_METHOD(goBack:(nonnull NSNumber *)reactTag)
RCT_EXTERN_METHOD(goForward:(nonnull NSNumber *)reactTag)
RCT_EXTERN_METHOD(reload:(nonnull NSNumber *)reactTag)

@end

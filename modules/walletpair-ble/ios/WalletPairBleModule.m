#import <React/RCTBridgeModule.h>
#import <React/RCTEventEmitter.h>

@interface RCT_EXTERN_MODULE(WalletPairBle, RCTEventEmitter)

RCT_EXTERN_METHOD(start:(NSString *)svcUuid writeUuid:(NSString *)writeUuid notifyUuid:(NSString *)notifyUuid name:(NSString *)name resolve:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject)
RCT_EXTERN_METHOD(stop:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject)
RCT_EXTERN_METHOD(sendBatch:(NSArray *)base64Frames resolve:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject)

@end

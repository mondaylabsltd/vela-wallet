/**
 * WalletPair BLE Peripheral native module.
 *
 * Provides a BLE GATT server with WalletPair protocol UUIDs.
 * Used on iOS and Android for direct device-to-device pairing.
 *
 * Uses React Native NativeModules (same pattern as VelaBLE).
 */

import { NativeModules, NativeEventEmitter } from 'react-native';

const { WalletPairBle } = NativeModules;
const emitter = WalletPairBle ? new NativeEventEmitter(WalletPairBle) : null;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface WriteEvent { characteristicUuid: string; value: string }
export interface SubscribeEvent { characteristicUuid: string }
export interface ConnectEvent { address: string }
export interface DisconnectEvent { address?: string; error?: string }
export interface MtuEvent { mtu: number }

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

function assertModule(): void {
  if (!WalletPairBle) {
    throw new Error('WalletPairBle native module is not available. Rebuild the app.');
  }
}

export function start(svc: string, w: string, n: string, name = 'WalletPair'): Promise<void> {
  assertModule();
  return WalletPairBle.start(svc, w, n, name);
}

export function stop(): Promise<void> {
  assertModule();
  return WalletPairBle.stop();
}

export function sendBatch(frames: string[]): Promise<void> {
  assertModule();
  return WalletPairBle.sendBatch(frames);
}

// ---------------------------------------------------------------------------
// Event listeners
// ---------------------------------------------------------------------------

const PREFIX = 'WalletPairBle_';

export function onWrite(h: (e: WriteEvent) => void) {
  if (!emitter) throw new Error('WalletPairBle not available');
  return emitter.addListener(`${PREFIX}onWrite`, h);
}

export function onSubscribe(h: (e: SubscribeEvent) => void) {
  if (!emitter) throw new Error('WalletPairBle not available');
  return emitter.addListener(`${PREFIX}onSubscribe`, h);
}

export function onUnsubscribe(h: (e: SubscribeEvent) => void) {
  if (!emitter) throw new Error('WalletPairBle not available');
  return emitter.addListener(`${PREFIX}onUnsubscribe`, h);
}

export function onConnect(h: (e: ConnectEvent) => void) {
  if (!emitter) throw new Error('WalletPairBle not available');
  return emitter.addListener(`${PREFIX}onConnect`, h);
}

export function onDisconnect(h: (e: DisconnectEvent) => void) {
  if (!emitter) throw new Error('WalletPairBle not available');
  return emitter.addListener(`${PREFIX}onDisconnect`, h);
}

export function onMtuChanged(h: (e: MtuEvent) => void) {
  if (!emitter) throw new Error('WalletPairBle not available');
  return emitter.addListener(`${PREFIX}onMtuChanged`, h);
}

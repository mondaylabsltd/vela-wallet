/**
 * BLE Peripheral transport implementing the WalletPair SDK Transport interface.
 *
 * Uses the walletpair-ble native module for advertising/GATT server
 * and walletpair-sdk/ble framing utilities for message fragmentation.
 *
 * Mobile only — import this file only on native platforms.
 */

import { Platform } from 'react-native';
import {
  BLE_SERVICE_UUID,
  BLE_WRITE_CHAR_UUID,
  BLE_NOTIFY_CHAR_UUID,
  frameMessage,
  Defragmenter,
  DEFAULT_FRAME_PAYLOAD,
} from 'walletpair-sdk/ble';
import type { ProtocolMessage, Transport, TransportState } from 'walletpair-sdk';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function bytesToBase64(bytes: Uint8Array): string {
  const chunks: string[] = [];
  for (let i = 0; i < bytes.length; i += 8192) {
    chunks.push(String.fromCharCode.apply(null, Array.from(bytes.subarray(i, i + 8192))));
  }
  return btoa(chunks.join(''));
}

function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

// ---------------------------------------------------------------------------
// BlePeripheralTransport
// ---------------------------------------------------------------------------

export class BlePeripheralTransport implements Transport {
  state: TransportState = 'disconnected';

  private defragmenter = new Defragmenter();
  private subscribed = false;
  private started = false;
  private subscriptions: { remove(): void }[] = [];
  private mtuPayload = DEFAULT_FRAME_PAYLOAD;

  private messageHandler: ((msg: ProtocolMessage) => void) | null = null;
  private closeHandler: (() => void) | null = null;
  private openHandler: (() => void) | null = null;

  onMessage(handler: (msg: ProtocolMessage) => void): void { this.messageHandler = handler; }
  onClose(handler: () => void): void { this.closeHandler = handler; }
  onOpen(handler: () => void): void { this.openHandler = handler; }

  send(msg: ProtocolMessage): void {
    if (!this.started || !this.subscribed) return;
    const json = JSON.stringify(msg);
    const frames = frameMessage(json, this.mtuPayload);
    const b64Frames = frames.map(f => bytesToBase64(f));

    // Lazy require to avoid loading native module at import time (web safety)
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const Ble: typeof import('../../modules/walletpair-ble') = require('../../modules/walletpair-ble');
    Ble.sendBatch(b64Frames).catch(() => {});
  }

  async connect(): Promise<void> {
    if (Platform.OS === 'web') {
      throw new Error('BLE peripheral mode is not available on web');
    }

    this.state = 'connecting';

    // Lazy require native module
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const Ble: typeof import('../../modules/walletpair-ble') = require('../../modules/walletpair-ble');

    // Clean up any previous session
    try { await Ble.stop(); } catch { /* ok */ }

    this.subscriptions.push(
      Ble.onWrite((event) => {
        const bytes = base64ToBytes(event.value);
        const json = this.defragmenter.push(new Uint8Array(bytes));
        if (json && this.messageHandler) {
          try { this.messageHandler(JSON.parse(json)); } catch { /* bad json */ }
        }
      }),
      Ble.onSubscribe(() => {
        this.subscribed = true;
        this.state = 'connected';
        this.openHandler?.();
      }),
      Ble.onUnsubscribe(() => {
        this.subscribed = false;
        this.state = 'disconnected';
        this.closeHandler?.();
      }),
      Ble.onDisconnect(() => {
        this.subscribed = false;
        this.state = 'disconnected';
        this.closeHandler?.();
      }),
      Ble.onMtuChanged((event) => {
        // MTU = max bytes per notification. Frame header is 3 bytes.
        this.mtuPayload = Math.max(event.mtu - 3, 20);
      }),
    );

    await Ble.start(BLE_SERVICE_UUID, BLE_WRITE_CHAR_UUID, BLE_NOTIFY_CHAR_UUID, 'Vela Wallet');
    this.started = true;

    // BLE advertising started — wait for central to subscribe.
    // The transport transitions to 'connected' when onSubscribe fires.
    // For the SDK's connect() promise, we resolve once advertising is up.
    // The WalletSession handles the protocol-level connection on top.
  }

  disconnect(): void {
    for (const sub of this.subscriptions) sub.remove();
    this.subscriptions = [];
    if (this.started) {
      try {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        const Ble: typeof import('../../modules/walletpair-ble') = require('../../modules/walletpair-ble');
        Ble.stop().catch(() => {});
      } catch { /* best effort */ }
    }
    this.started = false;
    this.subscribed = false;
    this.state = 'disconnected';
    this.mtuPayload = DEFAULT_FRAME_PAYLOAD;
    this.defragmenter = new Defragmenter();
  }
}

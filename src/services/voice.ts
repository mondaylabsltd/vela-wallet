/**
 * Payment voice announcement.
 *
 * Speaks a short English phrase when an incoming payment is detected, so a
 * merchant can confirm a sale without looking at the screen (like the Alipay /
 * WeChat merchant "payment received" speaker). Default ON, user-toggleable.
 *
 * Native: uses `expo-speech` via dynamic import (mirrors how platform.ts loads
 * expo-haptics) so the bundle still compiles if the module isn't installed yet.
 * Install with: `npx expo install expo-speech`.
 * Web: uses the browser SpeechSynthesis API.
 */

import { Platform } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';

const STORAGE_KEY = 'vela.voiceAnnounce.enabled';

let _enabled = false; // default OFF (muted) — opt in from Settings

/** Load the persisted preference. Call once at app start. */
export async function loadVoicePreference(): Promise<void> {
  try {
    const v = await AsyncStorage.getItem(STORAGE_KEY);
    if (v !== null) _enabled = v === '1';
  } catch {
    // keep default
  }
}

export function isVoiceEnabled(): boolean {
  return _enabled;
}

/** Toggle and persist. Returns the new value. */
export async function setVoiceEnabled(enabled: boolean): Promise<boolean> {
  _enabled = enabled;
  try {
    await AsyncStorage.setItem(STORAGE_KEY, enabled ? '1' : '0');
  } catch {
    // best effort
  }
  return _enabled;
}

/**
 * Announce an incoming payment, e.g. announcePayment('120', 'USDT')
 * → "Payment received, 120 USDT". No-op when disabled.
 */
export function announcePayment(amount: string, token: string): void {
  if (!_enabled) return;
  speak(`Payment received, ${amount} ${token}`);
}

/**
 * Speak a short confirmation immediately, ignoring the enabled flag. Call this
 * from the toggle-on tap — being a user gesture, it also "unlocks" the browser
 * SpeechSynthesis engine on web (which blocks timer-triggered speech otherwise).
 */
export function previewVoice(): void {
  speak('Payment voice on');
}

function speak(textToSpeak: string): void {
  if (Platform.OS === 'web') {
    try {
      const synth = (globalThis as any).speechSynthesis;
      if (!synth) return;
      const SpeechUtterance = (globalThis as any).SpeechSynthesisUtterance;
      const u = new SpeechUtterance(textToSpeak);
      u.lang = 'en-US';
      u.rate = 1.0;
      u.pitch = 1.05;
      synth.cancel();
      synth.speak(u);
    } catch {
      // unsupported browser — silent
    }
    return;
  }

  import('expo-speech')
    .then((Speech) => {
      Speech.stop();
      Speech.speak(textToSpeak, { language: 'en-US', rate: 1.0, pitch: 1.05 });
    })
    .catch(() => {
      // expo-speech not installed yet — silent no-op
    });
}

// ---------------------------------------------------------------------------
// Manual test hook (web) — call `velaVoiceTest()` in the browser console.
//   velaVoiceTest()              → "Payment received, 100 USDT"
//   velaVoiceTest(50, 'USDC')    → "Payment received, 50 USDC"
// Bypasses the on/off toggle and logs diagnostics. If nothing is heard, click
// anywhere on the page first (browsers block speech until a user gesture).
// ---------------------------------------------------------------------------
if (Platform.OS === 'web' && typeof globalThis !== 'undefined') {
  (globalThis as any).velaVoiceTest = (amount: string | number = 100, token = 'USDT') => {
    const synth = (globalThis as any).speechSynthesis;
    if (!synth) { console.warn('[velaVoiceTest] speechSynthesis unavailable in this browser'); return; }
    const text = `Payment received, ${amount} ${token}`;
    try {
      const SpeechUtterance = (globalThis as any).SpeechSynthesisUtterance;
      const u = new SpeechUtterance(text);
      u.lang = 'en-US'; u.rate = 1.0; u.pitch = 1.05;
      u.onstart = () => console.log('[velaVoiceTest] 🔊 speaking:', text);
      u.onend = () => console.log('[velaVoiceTest] done');
      u.onerror = (e: any) => console.warn('[velaVoiceTest] error (try clicking the page first):', e?.error ?? e);
      synth.cancel();
      synth.speak(u);
      console.log(`[velaVoiceTest] queued "${text}" · voices loaded: ${synth.getVoices().length}`);
    } catch (e) {
      console.warn('[velaVoiceTest] failed:', e);
    }
    return text;
  };
}

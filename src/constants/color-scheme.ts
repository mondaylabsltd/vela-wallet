/**
 * Color scheme preference system — auto / light / dark.
 *
 * How it works:
 *   1. User sets preference → Appearance.setColorScheme() for native UI
 *      (status bar, keyboard, system dialogs)
 *   2. rebuildColors() mutates color tokens synchronously
 *   3. Stack key={resolved} in _layout.tsx remounts the navigation tree,
 *      so all screens render fresh with correct colors
 */
import { Appearance } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import React, { createContext, useContext, useState, useCallback, useMemo } from 'react';
import { useColorScheme as useSystemColorScheme } from 'react-native';

const STORAGE_KEY = 'vela.colorScheme';

export type ColorSchemePreference = 'auto' | 'light' | 'dark';

// ---------------------------------------------------------------------------
// Synchronous module-level cache
// ---------------------------------------------------------------------------

let _preference: ColorSchemePreference = 'auto';

export function getColorSchemePreference(): ColorSchemePreference {
  return _preference;
}

/** Resolve preference + system scheme into a concrete 'light' | 'dark'. */
export function resolveColorScheme(
  pref: ColorSchemePreference,
  systemScheme: string | null | undefined,
): 'light' | 'dark' {
  if (pref === 'auto') return systemScheme === 'dark' ? 'dark' : 'light';
  return pref;
}

/** Load from storage — call once at app startup before rendering. */
export async function loadColorScheme(): Promise<void> {
  try {
    const stored = await AsyncStorage.getItem(STORAGE_KEY);
    if (stored === 'light' || stored === 'dark' || stored === 'auto') {
      _preference = stored;
    }
  } catch {
    // Use default
  }
}

/** Apply the preference to the native Appearance API (status bar, keyboard, etc.). */
export function applyColorScheme(pref: ColorSchemePreference): void {
  if (typeof Appearance.setColorScheme === 'function') {
    Appearance.setColorScheme(pref === 'auto' ? 'unspecified' : pref);
  }
}

// ---------------------------------------------------------------------------
// React Context
// ---------------------------------------------------------------------------

interface ColorSchemeContextValue {
  preference: ColorSchemePreference;
  resolved: 'light' | 'dark';
  setPreference: (pref: ColorSchemePreference) => void;
}

const ColorSchemeContext = createContext<ColorSchemeContextValue>({
  preference: 'auto',
  resolved: 'light',
  setPreference: () => {},
});

export function useColorSchemePreference() {
  return useContext(ColorSchemeContext);
}

export function ColorSchemeProvider({ children }: { children: React.ReactNode }) {
  const systemScheme = useSystemColorScheme();
  const [preference, setPreferenceState] = useState<ColorSchemePreference>(_preference);

  const resolved = resolveColorScheme(preference, systemScheme);

  // Synchronously rebuild colors during render so tokens are correct
  // before the Stack (with key={resolved}) mounts fresh screens.
  const { rebuildColors } = require('@/constants/theme');
  rebuildColors(resolved === 'dark');

  const setPreference = useCallback((pref: ColorSchemePreference) => {
    _preference = pref;
    // Native UI (status bar, keyboard, system dialogs)
    if (typeof Appearance.setColorScheme === 'function') {
      Appearance.setColorScheme(pref === 'auto' ? 'unspecified' : pref);
    }
    // Rebuild tokens synchronously
    const effectiveScheme = Appearance.getColorScheme();
    const { rebuildColors: rebuild } = require('@/constants/theme');
    rebuild(resolveColorScheme(pref, effectiveScheme) === 'dark');
    // Trigger re-render → resolved changes → Stack key changes → full remount
    setPreferenceState(pref);
    AsyncStorage.setItem(STORAGE_KEY, pref).catch(() => {});
  }, []);

  const value = useMemo(() => ({
    preference,
    resolved,
    setPreference,
  }), [preference, resolved, setPreference]);

  return React.createElement(ColorSchemeContext.Provider, { value }, children);
}

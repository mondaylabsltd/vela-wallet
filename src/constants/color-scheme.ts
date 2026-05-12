/**
 * Color scheme preference system — auto / light / dark.
 *
 * Uses Appearance.setColorScheme() to set the app-wide color scheme natively.
 * This triggers useColorScheme() hooks everywhere — including React Navigation's
 * internal components — causing ALL screens to re-render with correct colors.
 *
 * Pattern: Appearance.setColorScheme → useColorScheme fires → rebuildColors
 *          synchronously → components re-render with new color tokens.
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

/**
 * Apply the preference to the native Appearance API.
 * Call at startup after loading preference, and when user changes preference.
 */
export function applyColorScheme(pref: ColorSchemePreference): void {
  // 'unspecified' tells React Native to follow system preference
  Appearance.setColorScheme(pref === 'auto' ? 'unspecified' : pref);
}

// ---------------------------------------------------------------------------
// React Context
// ---------------------------------------------------------------------------

interface ColorSchemeContextValue {
  version: number;
  preference: ColorSchemePreference;
  resolved: 'light' | 'dark';
  setPreference: (pref: ColorSchemePreference) => void;
}

const ColorSchemeContext = createContext<ColorSchemeContextValue>({
  version: 0,
  preference: 'auto',
  resolved: 'light',
  setPreference: () => {},
});

export function useColorSchemePreference() {
  return useContext(ColorSchemeContext);
}

/**
 * Provider that manages color scheme state.
 *
 * Relies on Appearance.setColorScheme() to propagate changes natively.
 * useColorScheme() fires in ALL components (including React Navigation
 * internals), causing the entire screen tree to re-render.
 *
 * rebuildColors() is called synchronously so color tokens are correct
 * by the time components access them during re-render.
 */
export function ColorSchemeProvider({ children }: { children: React.ReactNode }) {
  // This hook fires whenever Appearance.setColorScheme() is called
  // or when the system scheme changes (for 'auto' mode)
  const systemScheme = useSystemColorScheme();
  const [preference, setPreferenceState] = useState<ColorSchemePreference>(_preference);
  const [version, setVersion] = useState(0);

  const resolved = resolveColorScheme(preference, systemScheme);

  // Synchronously rebuild colors during render.
  // By the time children render, color tokens are already correct.
  const { rebuildColors } = require('@/constants/theme');
  rebuildColors(resolved === 'dark');

  const setPreference = useCallback((pref: ColorSchemePreference) => {
    _preference = pref;
    // 1. Set native color scheme — triggers useColorScheme() everywhere
    Appearance.setColorScheme(pref === 'auto' ? 'unspecified' : pref);
    // 2. Rebuild tokens synchronously (before React processes state updates)
    const effectiveScheme = Appearance.getColorScheme();
    const newResolved = resolveColorScheme(pref, effectiveScheme);
    const { rebuildColors: rebuild } = require('@/constants/theme');
    rebuild(newResolved === 'dark');
    // 3. Update state (React batches with the Appearance-triggered re-renders)
    setPreferenceState(pref);
    setVersion(v => v + 1);
    // 4. Persist in background
    AsyncStorage.setItem(STORAGE_KEY, pref).catch(() => {});
  }, []);

  const value = useMemo(() => ({
    version,
    preference,
    resolved,
    setPreference,
  }), [version, preference, resolved, setPreference]);

  return React.createElement(ColorSchemeContext.Provider, { value }, children);
}
